use core::{fmt::Write, future::Future, ops::ControlFlow};
use embassy_sync::channel::DynamicReceiver;
use embassy_time::{Duration, Instant, TimeoutError, with_deadline};
use heapless::Vec;

use crate::{
    Error,
    at_command::{
        AtParseErr, AtParseLine, AtRequest, CommandGroup, GenericOk, MatchToken, ParseError,
        ParseResult, RequestType, Seq, SimResult,
    },
    log,
    modem::unsolicited,
    pipe::{DynamicReader, DynamicWriter},
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

pub struct CommandRunner<'r, UnsolicitedParser: unsolicited::Parser> {
    modem_reader: ModemReader<'r>,
    tx: DynamicWriter<'r>,
    unsolicited_parser: UnsolicitedParser,
    default_timeout: Option<Duration>,
}

impl<'r, UnsolicitedParser: unsolicited::Parser> CommandRunner<'r, UnsolicitedParser> {
    pub(crate) fn new(
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
        let mut command_buf = Vec::<u8, MAX_COMMAND_LEN>::new();
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
                    Ok(Err(err)) => break ControlFlow::Break(Err(err)),
                    Err(TimeoutError) => break ControlFlow::Break(Err(Error::Timeout)),
                };
            let read_instant = read_instant.unwrap_or(&start_instant);
            if let Some(matched) = DoneT::match_token(line) {
                break ControlFlow::Break(Ok(Ok(matched)));
            }
            if let Some(matched) = ErrorT::match_token(line) {
                break ControlFlow::Break(Ok(Err(matched)));
            }
            match self.unsolicited_parser.parse(line, read_instant).await {
                Ok(Some(urc)) => match self
                    .unsolicited_parser
                    .parse_all(&mut self.modem_reader, urc)
                    .await
                {
                    Ok(()) => {}
                    Err(err) => break ControlFlow::Break(Err(err)),
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
    pub(crate) async fn expect_response<
        DoneT,
        ErrorT,
        DoneToken,
        ErrorToken,
        ResponseT: AtParseLine,
    >(
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
                    Ok(Err(err)) => break ControlFlow::Break(Err(err)),
                    Err(TimeoutError) => break ControlFlow::Break(Err(Error::Timeout)),
                };
            let read_instant = read_instant.unwrap_or(&start_instant);
            if let Some(matched) = DoneT::match_token(line) {
                break ControlFlow::Break(Ok(Ok(matched)));
            }
            if let Some(matched) = ErrorT::match_token(line) {
                break ControlFlow::Break(Ok(Err(matched)));
            }
            match ResponseT::from_line(line, read_instant) {
                Ok(response) => break ControlFlow::Continue(response),
                Err(AtParseErr::Mismatch) => {}
                Err(_err) => {
                    log::info!("Failed to parse response, {:?}", _err);
                    break ControlFlow::Break(Err(Error::UnknownResponse));
                }
            }
            match self.unsolicited_parser.parse(line, read_instant).await {
                Ok(Some(urc)) => match self
                    .unsolicited_parser
                    .parse_all(&mut self.modem_reader, urc)
                    .await
                {
                    Ok(()) => {}
                    Err(err) => break ControlFlow::Break(Err(err)),
                },
                Ok(None) => {}
                Err(err) => {
                    #[cfg(any(feature = "log", feature = "defmt"))]
                    log::warn!("Got unknown response {:?}, {:?}", line, err,);
                }
            }
        }
    }

    pub async fn idle(&mut self) -> Result<(), Error> {
        let start_instant = embassy_time::Instant::now();
        let (line, read_instant) = match with_deadline(
            start_instant.saturating_add(AT_DEFAULT_TIMEOUT),
            self.modem_reader.read_line(),
        )
        .await
        {
            Ok(Ok(read)) => read,
            Ok(Err(err)) => return Err(err),
            Err(TimeoutError) => return Ok(()),
        };
        let read_instant = read_instant.unwrap_or(&start_instant);
        match self.unsolicited_parser.parse(line, read_instant).await {
            Ok(Some(urc)) => {
                self.unsolicited_parser
                    .parse_all(&mut self.modem_reader, urc)
                    .await
            }
            Ok(None) => Ok(()),
            Err(err) => {
                #[cfg(any(feature = "log", feature = "defmt"))]
                log::warn!("Got unknown response {:?}, {:?}", line, err,);
                Err(Error::UnknownResponse)
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
        ResultParser: ParseResult<Response: ExpectResponse<ResultParser>, Output = T, Error = E>,
        E: ParseError,
    {
        log::debug!("Running AT command: {:?}", command);
        let timeout = self.send_request(&command).await?;
        log::trace!("Waiting for response for AT command: {:?}", command);
        let result = expect_result::<ResultParser, _, _, _>(self, timeout).await;
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
        ResultParser: ParseResult<Response: ExpectResponse<ResultParser>, Output = T, Error = E>,
        E: ParseError,
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

async fn expect_result<
    'runner,
    P: ParseResult<Output = T, Error = E>,
    UnsolicitedParser: unsolicited::Parser,
    T,
    E: ParseError,
>(
    runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
    timeout: Timeout,
) -> Result<Result<T, E>, Error>
where
    P::Response: ExpectResponse<P>,
{
    let mut timeout = timeout;
    timeout.arm(&AT_DEFAULT_TIMEOUT, Instant::now());
    let mut response = match P::Response::expect(runner, timeout).await {
        ControlFlow::Continue(response) => response,
        ControlFlow::Break(Ok(Ok(_))) => {
            log::error!("Got done result before expected response.");
            return Err(Error::UnknownResponse);
        }
        ControlFlow::Break(Ok(Err((error_token, line)))) => match P::parse_err(error_token, line) {
            Err(_err) => {
                log::error!("Failed to parse 'error' result response, {:?}.", _err);
                return Err(Error::UnknownResponse);
            }
            Ok(error) => return Ok(Err(error)),
        },
        ControlFlow::Break(Err(err)) => return Err(err),
    };
    loop {
        match response.expect_more(runner, timeout).await {
            ControlFlow::Continue(()) => {}
            ControlFlow::Break(Ok(Ok((done_token, line)))) => {
                match P::compose_result(response, done_token, line) {
                    Err((_response, _err)) => {
                        log::error!("Failed to parse 'done' result response, {:?}.", _err);
                        break Err(Error::UnknownResponse);
                    }
                    Ok(output) => break Ok(Ok(output)),
                }
            }
            ControlFlow::Break(Ok(Err((error_token, line)))) => {
                match P::parse_err(error_token, line) {
                    Err(_err) => {
                        log::error!("Failed to parse 'error' result response, {:?}.", _err);
                        break Err(Error::UnknownResponse);
                    }
                    Ok(error) => break Ok(Err(error)),
                }
            }
            ControlFlow::Break(Err(err)) => break Err(err),
        }
    }
}

/// Implemented for (tuples of) AtResponse.
///
/// In order to support AtRequest::Response being a tuple of arbitrary size, we
/// implement the ExpectResponse trait for tuples with as many member as we need.
pub trait ExpectResponse<P: ParseResult>: Sized {
    /// Try to parse the next response for this type.
    fn expect<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> impl Future<
        Output = ControlFlow<
            Result<
                Result<
                    (
                        <<P as ParseResult>::Done as MatchToken>::Token,
                        &'runner str,
                    ),
                    (
                        <<<P as ParseResult>::Error as ParseError>::Matcher as MatchToken>::Token,
                        &'runner str,
                    ),
                >,
                Error,
            >,
            Self,
        >,
    >;
    fn expect_more<'runner, UnsolicitedParser: unsolicited::Parser>(
        &mut self,
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> impl Future<
        Output = ControlFlow<
            Result<
                Result<
                    (
                        <<P as ParseResult>::Done as MatchToken>::Token,
                        &'runner str,
                    ),
                    (
                        <<<P as ParseResult>::Error as ParseError>::Matcher as MatchToken>::Token,
                        &'runner str,
                    ),
                >,
                Error,
            >,
        >,
    > {
        async move {
            if let Some(done_token) = P::Done::AUTO_MATCH {
                ControlFlow::Break(Ok(Ok((done_token, ""))))
            } else {
                runner
                    .expect_break::<P::Done, <P::Error as ParseError>::Matcher, _, _>(timeout)
                    .await
            }
        }
    }
}

// No response
impl<P: ParseResult> ExpectResponse<P> for () {
    async fn expect<'runner, UnsolicitedParser: unsolicited::Parser>(
        _runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        _timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (
                    <<P as ParseResult>::Done as MatchToken>::Token,
                    &'runner str,
                ),
                (
                    <<<P as ParseResult>::Error as ParseError>::Matcher as MatchToken>::Token,
                    &'runner str,
                ),
            >,
            Error,
        >,
        Self,
    > {
        ControlFlow::Continue(())
    }
}

impl<T: AtParseLine, P: ParseResult> ExpectResponse<P> for T {
    async fn expect<'runner, UnsolicitedParser: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (
                    <<P as ParseResult>::Done as MatchToken>::Token,
                    &'runner str,
                ),
                (
                    <<<P as ParseResult>::Error as ParseError>::Matcher as MatchToken>::Token,
                    &'runner str,
                ),
            >,
            Error,
        >,
        Self,
    > {
        runner
            .expect_response::<P::Done, <P::Error as ParseError>::Matcher, _, _, T>(timeout)
            .await
    }
}

impl<R1: ExpectResponse<P>, R2: ExpectResponse<P>, P: ParseResult> ExpectResponse<P> for (R1, R2) {
    async fn expect<'runner, Up: unsolicited::Parser>(
        runner: &'runner mut CommandRunner<'_, Up>,
        timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (
                    <<P as ParseResult>::Done as MatchToken>::Token,
                    &'runner str,
                ),
                (
                    <<<P as ParseResult>::Error as ParseError>::Matcher as MatchToken>::Token,
                    &'runner str,
                ),
            >,
            Error,
        >,
        Self,
    > {
        let r1 = match <R1 as ExpectResponse<P>>::expect(runner, timeout).await {
            ControlFlow::Break(res) => return ControlFlow::Break(res),
            ControlFlow::Continue(r1) => r1,
        };
        let r2 = match <R2 as ExpectResponse<P>>::expect(runner, timeout).await {
            ControlFlow::Break(res) => return ControlFlow::Break(res),
            ControlFlow::Continue(r2) => r2,
        };
        ControlFlow::Continue((r1, r2))
    }
}

impl<T: ExpectResponse<P>, P, const N: usize> ExpectResponse<P> for Seq<T, N>
where
    P: ParseResult,
{
    async fn expect<'runner, UnsolicitedParser: unsolicited::Parser>(
        _runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        _timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (
                    <<P as ParseResult>::Done as MatchToken>::Token,
                    &'runner str,
                ),
                (
                    <<<P as ParseResult>::Error as ParseError>::Matcher as MatchToken>::Token,
                    &'runner str,
                ),
            >,
            Error,
        >,
        Self,
    > {
        ControlFlow::Continue(Seq(heapless::Vec::new()))
    }
    async fn expect_more<'runner, UnsolicitedParser: unsolicited::Parser>(
        &mut self,
        runner: &'runner mut CommandRunner<'_, UnsolicitedParser>,
        timeout: Timeout,
    ) -> ControlFlow<
        Result<
            Result<
                (
                    <<P as ParseResult>::Done as MatchToken>::Token,
                    &'runner str,
                ),
                (
                    <<<P as ParseResult>::Error as ParseError>::Matcher as MatchToken>::Token,
                    &'runner str,
                ),
            >,
            Error,
        >,
    > {
        match T::expect(runner, timeout).await {
            ControlFlow::Continue(item) => match self.0.push(item) {
                Ok(()) => ControlFlow::Continue(()),
                Err(_err) => ControlFlow::Break(Err(Error::BufferOverflow)),
            },
            ControlFlow::Break(Ok(res)) => ControlFlow::Break(Ok(res)),
            ControlFlow::Break(Err(err)) => ControlFlow::Break(Err(err)),
        }
    }
}
