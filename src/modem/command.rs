use core::{fmt::Write, future::Future, marker::PhantomData, mem::ManuallyDrop, ops::ControlFlow};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::DynamicReceiver,
    pipe::{DynamicReader, DynamicWriter},
    zerocopy_channel::ReceiveSlot,
};
use embassy_time::{Duration, Instant, TimeoutError, with_deadline};
use heapless::Vec;

use crate::{
    Error,
    at_command::{
        AtParseErr, AtParseLine, AtRequest, CommandGroup, Either, GenericOk, MatchToken,
        ParseResult, RequestType, Seq, SimResult,
    },
    log,
    modem::unsolicited,
    read::{ModemReader, ReadTimestamp},
};

/// The default timeout of AT commands
pub const AT_DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub const MAX_COMMAND_LEN: usize = 599;

#[derive(Clone, Copy, Default, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Timeout {
    #[default]
    None,
    Unarmed(Duration),
    Armed(Instant),
}

impl Timeout {
    pub fn arm(&mut self, default: &Duration, now: Instant) -> &Instant {
        *self = match self {
            Self::None => Self::Armed(now.saturating_add(*default)),
            Self::Unarmed(duration) => Self::Armed(now.saturating_add(*duration)),
            Self::Armed(deadline) => return deadline,
        };
        match self {
            Self::None | Self::Unarmed(_) => unreachable!(),
            Self::Armed(deadline) => deadline,
        }
    }

    pub fn into_timeout(mut self, default: &Duration, now: Instant) -> Instant {
        *self.arm(default, now)
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
struct WithTimeout<Request> {
    request: Request,
    timeout: Duration,
}

impl<Request: AtRequest> AtRequest for WithTimeout<Request> {
    type ResultType = Request::ResultType;
    const TYPE: RequestType = Request::TYPE;
    fn encode(&self, buf: &mut impl Write) -> core::fmt::Result {
        self.request.encode(buf)
    }
    fn timeout(&self) -> Option<Duration> {
        Some(self.timeout)
    }
}

// Impl request for tuples to allow chained requests
impl<R1: AtRequest<ResultType = SimResult<GenericOk<R>>>, R2: AtRequest<ResultType: ParseResult>, R>
    AtRequest for (R1, R2)
{
    type ResultType = SimResult<
        GenericOk<(
            <R1::ResultType as ParseResult>::Response,
            <R2::ResultType as ParseResult>::Response,
        )>,
    >;
    const TYPE: RequestType = RequestType::Combined {
        first: match R1::TYPE {
            RequestType::Command(group) => group,
            RequestType::NonCommand => CommandGroup::NonCommand,
            RequestType::Combined { first, last: _ } => first,
        },
        last: match R2::TYPE {
            RequestType::Command(group) => group,
            RequestType::NonCommand => CommandGroup::NonCommand,
            RequestType::Combined { first: _, last } => last,
        },
    };

    fn encode(&self, buf: &mut impl Write) -> core::fmt::Result {
        self.0.encode(buf)?;
        match R2::TYPE.first_command() {
            CommandGroup::Extended => write!(buf, ";")?,
            CommandGroup::Basic | CommandGroup::NonCommand | CommandGroup::Context => {}
        };
        self.1.encode(buf)
    }

    fn timeout(&self) -> Option<Duration> {
        cfg_select! {
            feature = "nightly" => self.0.timeout().reduce(self.1.timeout(), |t1, t2| {
                t1.checked_add(t2).unwrap_or(Duration::MAX)
            }),
            _ => self.0.timeout().max(self.1.timeout()),
        }
    }
}

pub fn encode_request<R: AtRequest>(request: &R, buf: &mut impl Write) -> core::fmt::Result {
    match R::TYPE {
        RequestType::NonCommand
        | RequestType::Command(CommandGroup::NonCommand)
        | RequestType::Combined {
            first: CommandGroup::NonCommand,
            last: CommandGroup::NonCommand,
        } => request.encode(buf),
        RequestType::Command(CommandGroup::Context) => {
            request.encode(buf)?;
            // Note that this is just \r (<CR>) to mark the end
            // of the command. Linefeed is not used for this
            write!(buf, "\r")
        }
        RequestType::Command(CommandGroup::Basic | CommandGroup::Extended)
        // This matches on any Combined value where *either*
        // end is a command, since only the case where *both*
        // are non-commands is covered above
        | RequestType::Combined { first: _, last: _ } => {
            write!(buf, "AT")?;
            request.encode(buf)?;
            write!(buf, "\r")
        }
    }
}

pub struct ReceiveSlotRef<'r, M: RawMutex, T> {
    slot: ManuallyDrop<ReceiveSlot<'r, M, T>>,
}

pub struct MappedReceiveSlotRef<'r, M: RawMutex, U, T> {
    data: core::ptr::NonNull<U>,
    _orig: ReceiveSlotRef<'r, M, T>,
    _variance: PhantomData<&'r mut T>,
}

// Derived from core::mem::DropGuard--takes the value in the
//  ManuallyDrop cell and calls receive_done before it drops
impl<'r, M: RawMutex, T> Drop for ReceiveSlotRef<'r, M, T> {
    fn drop(&mut self) {
        // SAFETY: `ReceiveSlotRef` is in the process of being dropped.
        let inner = unsafe { ManuallyDrop::take(&mut self.slot) };
        inner.receive_done();
    }
}

impl<'r, M: RawMutex, T> ReceiveSlotRef<'r, M, T> {
    fn new(slot: ReceiveSlot<'r, M, T>) -> Self {
        Self {
            slot: ManuallyDrop::new(slot),
        }
    }

    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedReceiveSlotRef<'r, M, U, T>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: Sized,
    {
        let mut orig = orig;
        match f(&mut orig.slot) {
            Some(data) => {
                let data = core::ptr::NonNull::from(data);
                Ok(MappedReceiveSlotRef {
                    data,
                    _orig: orig,
                    _variance: PhantomData,
                })
            }
            None => Err(orig),
        }
    }
}

impl<'r, M: RawMutex, T> core::ops::Deref for ReceiveSlotRef<'r, M, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.slot.deref()
    }
}

impl<'r, M: RawMutex, T> core::ops::DerefMut for ReceiveSlotRef<'r, M, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.slot.deref_mut()
    }
}

impl<'r, M: RawMutex, U, T> core::ops::Deref for MappedReceiveSlotRef<'r, M, U, T> {
    type Target = U;

    fn deref(&self) -> &Self::Target {
        // SAFETY: data is created from the original slot ref,
        // so it's always a referenceable ptr
        unsafe { self.data.as_ref() }
    }
}

impl<'r, M: RawMutex, U, T> core::ops::DerefMut for MappedReceiveSlotRef<'r, M, U, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: data is created from the original slot ref,
        // so it's always a referenceable ptr
        unsafe { self.data.as_mut() }
    }
}

impl<'r, M: RawMutex, U, T> AsRef<U> for MappedReceiveSlotRef<'r, M, U, T> {
    fn as_ref(&self) -> &U {
        // SAFETY: data is created from the original slot ref,
        // so it's always a referenceable ptr
        unsafe { self.data.as_ref() }
    }
}

pub struct CommandRunner<'r, UnsolicitedParser: unsolicited::Parser> {
    modem_reader: ModemReader<'r>,
    tx: DynamicWriter<'r>,
    unsolicited_parser: UnsolicitedParser,
    default_timeout: Option<Duration>,
}

impl<'r, UnsolicitedParser: unsolicited::Parser> CommandRunner<'r, UnsolicitedParser> {
    pub fn new(
        rx: DynamicReader<'r>,
        rx_timestamps: DynamicReceiver<'r, ReadTimestamp>,
        tx: DynamicWriter<'r>,
        unsolicited_parser: UnsolicitedParser,
    ) -> Self {
        CommandRunner {
            modem_reader: ModemReader::new(rx, rx_timestamps),
            tx,
            unsolicited_parser,
            default_timeout: None,
        }
    }

    pub async fn write_all(&mut self, buf: &[u8]) {
        let mut buf = buf;
        while !buf.is_empty() {
            let n = self.tx.write(buf).await;
            buf = &buf[n..];
        }
    }

    /// Send a request to the modem, but do not wait for a response.
    pub async fn send_request<R: AtRequest>(&mut self, request: &R) -> Result<Timeout, Error> {
        let mut command_buf = heapless::Vec::<u8, MAX_COMMAND_LEN>::new();
        encode_request(request, &mut command_buf).map_err(|_| Error::BufferOverflow)?;
        self.write_all(&*command_buf).await;
        if let Some(duration) = request.timeout().or(self.default_timeout) {
            Ok(Timeout::Unarmed(duration))
        } else {
            Ok(Timeout::None)
        }
    }

    /// Wait for the modem to return a specific response.
    pub async fn expect_break<DoneT, ErrorT, DoneToken, ErrorToken>(
        &mut self,
        timeout: Timeout,
    ) -> ControlFlow<Result<Result<(DoneToken, &str), (ErrorToken, &str)>, Error>>
    where
        DoneT: MatchToken<Token = DoneToken>,
        ErrorT: MatchToken<Token = ErrorToken>,
    {
        let start_instant = Instant::now();
        let timeout_at = timeout.into_timeout(&AT_DEFAULT_TIMEOUT, start_instant);
        loop {
            let (line, read_instant) =
                match with_deadline(timeout_at, self.modem_reader.read_line()).await {
                    Ok(Ok(read)) => read,
                    Ok(Err(err)) => return ControlFlow::Break(Err(err)),
                    Err(TimeoutError) => return ControlFlow::Break(Err(Error::Timeout)),
                };
            let read_instant = read_instant.unwrap_or(&start_instant);
            if let Some(matched) = DoneT::match_token(line) {
                return ControlFlow::Break(Ok(Ok(matched)));
            }
            if let Some(matched) = ErrorT::match_token(line) {
                return ControlFlow::Break(Ok(Err(matched)));
            }
            match self.unsolicited_parser.parse(line, read_instant).await {
                Ok(Some(urc)) => match self
                    .unsolicited_parser
                    .parse_all(&mut self.modem_reader, urc)
                    .await
                {
                    Ok(()) => {}
                    Err(err) => return ControlFlow::Break(Err(err)),
                },
                Ok(None) => {}
                Err(err) => {
                    #[cfg(any(feature = "log", feature = "defmt"))]
                    log::warn!("Got unexpected response {:?}, {:?}", line, err,);
                }
            }
        }
    }

    /// Wait for the modem to return a specific response.
    pub async fn expect_response<DoneT, ErrorT, DoneToken, ErrorToken, ResponseT: AtParseLine>(
        &mut self,
        timeout: Timeout,
    ) -> ControlFlow<Result<Result<(DoneToken, &str), (ErrorToken, &str)>, Error>, ResponseT>
    where
        DoneT: MatchToken<Token = DoneToken>,
        ErrorT: MatchToken<Token = ErrorToken>,
    {
        let start_instant = Instant::now();
        let timeout_at = timeout.into_timeout(&AT_DEFAULT_TIMEOUT, start_instant);
        loop {
            let (line, read_instant) =
                match with_deadline(timeout_at, self.modem_reader.read_line()).await {
                    Ok(Ok(read)) => read,
                    Ok(Err(err)) => return ControlFlow::Break(Err(err)),
                    Err(TimeoutError) => return ControlFlow::Break(Err(Error::Timeout)),
                };
            let read_instant = read_instant.unwrap_or(&start_instant);
            if let Some(matched) = DoneT::match_token(line) {
                return ControlFlow::Break(Ok(Ok(matched)));
            }
            if let Some(matched) = ErrorT::match_token(line) {
                return ControlFlow::Break(Ok(Err(matched)));
            }
            match ResponseT::from_line(line, read_instant) {
                Ok(response) => return ControlFlow::Continue(response),
                Err(AtParseErr::Mismatch) => {}
                Err(_err) => {
                    log::info!("Failed to parse response, {:?}", _err);
                    return ControlFlow::Break(Err(Error::UnknownResponse));
                }
            }
            match self.unsolicited_parser.parse(line, read_instant).await {
                Ok(Some(urc)) => match self
                    .unsolicited_parser
                    .parse_all(&mut self.modem_reader, urc)
                    .await
                {
                    Ok(()) => {}
                    Err(err) => return ControlFlow::Break(Err(err)),
                },
                Ok(None) => {}
                Err(err) => {
                    #[cfg(any(feature = "log", feature = "defmt"))]
                    log::warn!("Got unexpected response {:?}, {:?}", line, err,);
                }
            }
        }
    }

    /// Wait for the modem to return either of two response types.
    pub async fn expect_either_response<
        DoneT,
        ErrorT,
        DoneToken,
        ErrorToken,
        T1: AtParseLine,
        T2: AtParseLine,
    >(
        &mut self,
        timeout: Timeout,
    ) -> ControlFlow<Result<Result<(DoneToken, &str), (ErrorToken, &str)>, Error>, Either<T1, T2>>
    where
        DoneT: MatchToken<Token = DoneToken>,
        ErrorT: MatchToken<Token = ErrorToken>,
    {
        let start_instant = Instant::now();
        let timeout_at = timeout.into_timeout(&AT_DEFAULT_TIMEOUT, start_instant);
        loop {
            let (line, read_instant) =
                match with_deadline(timeout_at, self.modem_reader.read_line()).await {
                    Ok(Ok(read)) => read,
                    Ok(Err(err)) => return ControlFlow::Break(Err(err)),
                    Err(TimeoutError) => return ControlFlow::Break(Err(Error::Timeout)),
                };
            let read_instant = read_instant.unwrap_or(&start_instant);
            if let Some(matched) = DoneT::match_token(line) {
                return ControlFlow::Break(Ok(Ok(matched)));
            }
            if let Some(matched) = ErrorT::match_token(line) {
                return ControlFlow::Break(Ok(Err(matched)));
            }
            match T1::from_line(line, read_instant) {
                Ok(response) => return ControlFlow::Continue(Either::T1(response)),
                Err(AtParseErr::Mismatch) => {}
                Err(_err) => {
                    log::info!("Failed to parse response, {:?}", _err);
                    return ControlFlow::Break(Err(Error::UnknownResponse));
                }
            }
            match T2::from_line(line, read_instant) {
                Ok(response) => return ControlFlow::Continue(Either::T2(response)),
                Err(AtParseErr::Mismatch) => {}
                Err(_err) => {
                    log::info!("Failed to parse response, {:?}", _err);
                    return ControlFlow::Break(Err(Error::UnknownResponse));
                }
            }
            match self.unsolicited_parser.parse(line, read_instant).await {
                Ok(Some(urc)) => match self
                    .unsolicited_parser
                    .parse_all(&mut self.modem_reader, urc)
                    .await
                {
                    Ok(()) => {}
                    Err(err) => return ControlFlow::Break(Err(err)),
                },
                Ok(None) => {}
                Err(err) => {
                    #[cfg(any(feature = "log", feature = "defmt"))]
                    log::warn!("Got unexpected response {:?}, {:?}", line, err,);
                }
            }
        }
    }

    /// Send raw bytes to the modem, use with care.
    pub async fn send_bytes(&mut self, bytes: &[u8]) {
        let mut bytes = bytes;
        while !bytes.is_empty() {
            let n = self.tx.write(bytes).await;
            bytes = &bytes[n..];
        }
    }

    /// Send a request to the modem, and wait for the modem to respond.
    pub async fn run<ResultParser, Request, T, E>(
        &mut self,
        command: Request,
    ) -> Result<Result<T, E>, Error>
    where
        Request: AtRequest<ResultType = ResultParser>,
        ResultParser: ParseResult<Response: ExpectResult<ResultParser>, Output = T, Error = E>,
    {
        log::debug!("Running AT command: {:?}", command);
        let timeout = self.send_request(&command).await?;
        log::trace!("Waiting for response for AT command: {:?}", command);
        let result = ResultParser::Response::finalise(self, timeout).await;
        log::trace!("Completed AT command: {:?}", command);

        if let Err(e) = &result {
            log::debug!("AT command {:?} error: {:?}", command, e);
        }

        result
    }

    /// Send a request to the modem and wait for the modem to respond.
    ///
    /// Use the provided timeout value instead of the configured one.
    pub async fn run_with_timeout<ResultParser, Request, T, E>(
        &mut self,
        timeout: Duration,
        request: Request,
    ) -> Result<Result<T, E>, Error>
    where
        Request: AtRequest<ResultType = ResultParser>,
        ResultParser: ParseResult<Response: ExpectResult<ResultParser>, Output = T, Error = E>,
    {
        let runnable = WithTimeout { request, timeout };
        let result = self.run(runnable).await;
        result
    }

    /// Set the timeout of subsequent commands
    ///
    /// Note that the timeout defaults to [AT_DEFAULT_TIMEOUT]
    /// when this is seto to None.
    pub fn with_default_timeout(self, default_timeout: Option<Duration>) -> Self {
        Self {
            default_timeout,
            ..self
        }
    }
}

/// Defines a line-by-line parser for a given result type.
///
/// This is intended to effectively present a dyn compatible interface to a
/// [ParseResult] implementation.
pub trait ResultParser {
    fn parse_next(&mut self, line: &str) -> ControlFlow<Result<(), Error>>;
}

/// Implemented for (tuples of) AtResponse.
///
/// In order to support AtRequest::Response being a tuple of arbitrary size, we
/// implement the ExpectResponse trait for tuples with as many member as we need.
///
/// Also to support variable length response sequences, there is a specialization
/// tuples matching (heapless::Vec<T1, N>, T2) which will parse 0..N T1 responses
/// until a T2 response is encountered.
pub trait ExpectResponse<P: ParseResult>: Sized {
    /// Return the default response for this type, if there is one.
    fn default_response() -> Option<Self>;
    /// Try to parse the next response for this type.
    fn expect<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> impl Future<
        Output = ControlFlow<
            Result<
                Result<
                    (<P::Done as MatchToken>::Token, &'runner str),
                    (<P::Error as MatchToken>::Token, &'runner str),
                >,
                Error,
            >,
            Self,
        >,
    >;
}

/// Attempts to parse a complete structured result from responses implementing
/// [ExpectResponse].
pub trait ExpectResult<P: ParseResult>: Sized {
    /// Return the default response for this type, if there is one.
    fn default_response() -> Option<Self>;
    /// Call [ExpectResponse] recursively to produce the composed result or error
    /// with the given parser.
    fn finalise<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> impl Future<Output = Result<Result<P::Output, P::Error>, Error>>;
}

// No response
impl<P: ParseResult> ExpectResponse<P> for () {
    fn default_response() -> Option<Self> {
        Some(())
    }
    async fn expect<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (<P::Done as MatchToken>::Token, &'runner str),
                (<P::Error as MatchToken>::Token, &'runner str),
            >,
            Error,
        >,
        Self,
    > {
        runner
            .expect_break::<P::Done, P::Error, _, _>(timeout)
            .await
    }
}

impl<T: AtParseLine, P: ParseResult> ExpectResponse<P> for T {
    fn default_response() -> Option<Self> {
        None
    }
    async fn expect<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (<P::Done as MatchToken>::Token, &'runner str),
                (<P::Error as MatchToken>::Token, &'runner str),
            >,
            Error,
        >,
        Self,
    > {
        runner
            .expect_response::<P::Done, P::Error, _, _, T>(timeout)
            .await
    }
}

impl<R1: ExpectResponse<P>, R2: ExpectResponse<P>, P: ParseResult> ExpectResponse<P> for (R1, R2) {
    fn default_response() -> Option<Self> {
        None
    }
    async fn expect<'runner, Up: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, Up>,
        timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (<P::Done as MatchToken>::Token, &'runner str),
                (<P::Error as MatchToken>::Token, &'runner str),
            >,
            Error,
        >,
        Self,
    > {
        let r1 = match <R1 as ExpectResponse<P>>::expect(runner, timeout).await {
            ControlFlow::Break(done) => return ControlFlow::Break(done),
            ControlFlow::Continue(r1) => r1,
        };
        let r2 = match <R2 as ExpectResponse<P>>::expect(runner, timeout).await {
            ControlFlow::Break(done) => return ControlFlow::Break(done),
            ControlFlow::Continue(r2) => r2,
        };
        ControlFlow::Continue((r1, r2))
    }
}

impl<T: ExpectResponse<P>, P: ParseResult> ExpectResult<P> for T
where
    P: ParseResult<Response = Self>,
{
    fn default_response() -> Option<Self> {
        None
    }
    async fn finalise<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> Result<Result<<P as ParseResult>::Output, <P as ParseResult>::Error>, Error> {
        let mut maybe_response = P::Response::default_response();
        loop {
            maybe_response = match (maybe_response, P::Response::expect(runner, timeout).await) {
                (None, ControlFlow::Continue(response)) => Some(response),
                (Some(response), ControlFlow::Continue(_)) => {
                    log::warn!("Got unexpected duplicated response.");
                    Some(response)
                }
                (None, ControlFlow::Break(Ok(Ok(_)))) => {
                    log::error!("Got done result without expected response.");
                    break Err(Error::UnknownResponse);
                }
                (Some(response), ControlFlow::Break(Ok(Ok((done_token, line))))) => {
                    match P::compose_result(response, done_token, line) {
                        Err((_response, _err)) => {
                            log::error!(
                                "Failed to parse 'done' result response, {:?}.",
                                _err
                            );
                            break Err(Error::UnknownResponse);
                        }
                        Ok(output) => break Ok(Ok(output)),
                    }
                }
                (_, ControlFlow::Break(Ok(Err((error_token, line))))) => {
                    match P::parse_err(error_token, line) {
                        Err(_err) => {
                            log::error!(
                                "Failed to parse 'error' result response, {:?}.",
                                _err
                            );
                            break Err(Error::UnknownResponse);
                        }
                        Ok(error) => break Ok(Err(error)),
                    }
                }
                (_, ControlFlow::Break(Err(err))) => break Err(err),
            }
        }
    }
}

impl<T: ExpectResponse<P>, P, const N: usize> ExpectResult<P> for Seq<T, N>
where
    P: ParseResult<Response = Self>,
{
    fn default_response() -> Option<Self> {
        None
    }
    async fn finalise<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> Result<Result<<P as ParseResult>::Output, <P as ParseResult>::Error>, Error> {
        let mut response_vec = heapless::Vec::new();
        loop {
            match T::expect(runner, timeout).await {
                ControlFlow::Break(Ok(Ok((done, line)))) => {
                    break Ok(Ok(
                        P::compose_result(Seq(response_vec), done, line).map_err(|(_, err)| err)?
                    ));
                }
                ControlFlow::Break(Ok(Err((error, line)))) => {
                    break Ok(Err(P::parse_err(error, line)?));
                }
                ControlFlow::Break(Err(err)) => break Err(err),
                ControlFlow::Continue(response) => {
                    response_vec.push(response);
                }
            }
        }
    }
}

// impl<T: AtParseLine, E: AtParseLine, M: RawMutex> ExpectResponse<M> for Result<T,
// E> {     async fn expect<M: RawMutex>(runner: &mut CommandRunner<'_, P>, timeout:
// Timeout) -> Result<Self, Error> {         match runner.expect_either_response::<T,
// E>(timeout).await {             Ok(embassy_futures::select::Either::First(item))
// => Ok(Ok(item.clone())),
// Ok(embassy_futures::select::Either::Second(item)) => Ok(Err(item.clone())),
//             Err(err) => Err(err),
//         }
//     }
// }

// impl<T1: AtParseLine, T2: AtParseLine, P: ParseResult> ExpectResponse<P> for
// Either<T1, T2> {     type Output = Self;
//     async fn expect<Up: unsolicited::Parser>(
//         runner: &mut CommandRunner<'_, Up>,
//         timeout: Timeout,
//     ) -> Result<Self::Output, Error> {
//         match runner.expect_either_response::<P::Done, P::Error, _, _, T1,
// T2>(timeout).await {             ControlFlow::Continue(Either::T1(item)) =>
// Ok(Either::T1(item)),             ControlFlow::Continue(Either::T2(item)) =>
// Ok(Either::T2(item)),             ControlFlow::Break(Ok(Ok((token, line)))) => {
//                 Ok(Ok(P::compose_result()))
//             }
//             ControlFlow::Break(Ok(Err((token, line)))) => {
//                 Ok(Err(P::parse_err(token, line)))
//             }
//             ControlFlow::Break(Err(err)) => Err(err),
//         }
//     }
// }

// impl<Parsed: AtParseLine, Output: TryFrom<Parsed>, P: ParseResult>
// ExpectResponse<P>     for MetaResponse<Parsed, Output>
// {
//     type Output = Output;
//     async fn expect<Up: unsolicited::Parser>(
//         runner: &mut CommandRunner<'_, Up>,
//         timeout: Timeout,
//     ) -> Result<Self::Output, Error> {
//         let o = <Parsed as ExpectResponse<P>>::expect(runner, timeout)
//             .await?
//             .try_into()
//             .map_err(|_| Error::IncompatibleMapping)?;
//         Ok(o)
//     }
// }
