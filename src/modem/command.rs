use core::{cmp::min, fmt::Write, future::Future, marker::PhantomData, mem::ManuallyDrop};
use embassy_sync::{
    blocking_mutex::raw::RawMutex, pubsub::{SendDynPublisher, SendDynSubscriber}, zerocopy_channel::{ReceiveSlot, Receiver as ZerocopyReceiver, Sender as ZerocopySender},
};
use embassy_time::{Duration, Instant, with_deadline};
use heapless::{String, Vec, string::StringView};

use crate::{
    Error, PacketProvider, at_command::{
        AtRequest, CommandGroup, Either, GenericOk, MatchToken, MetaResponse, ParseResult, RequestType, Seq,
    }, log, PacketStorage,
};

/// The default timeout of AT commands
pub const AT_DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

pub const MAX_COMMAND_LEN: usize = 599;

#[derive(Clone)]
pub struct CommandPacket<S: PacketStorage> {
    storage: S,
    len: usize,
}

impl<'r, S: PacketStorage> Write for CommandPacket<'r, S> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let mut data = self.storage.borrow_data_mut();
        let data: &mut [u8] = data.as_mut();
        if let Some(len) = self.len.checked_add(s.len()) && len < data.len() {
            let start = self.len;
            self.len = len;
            data[start..len].copy_from_slice(s.as_bytes());
            Ok(())
        } else {
            Err(core::fmt::Error)
        }
    }
}

impl<'r, S: PacketStorage> CommandPacket<'r, S> {
    pub const fn new(storage: &'r S) -> Self {
        Self {
            storage,
            len: 0,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.storage.borrow_data().as_ref()
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
}

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
    type Response = Request::Response;
    const TYPE: RequestType = Request::TYPE;
    fn encode(&self, buf: &mut impl Write) -> core::fmt::Result {
        self.request.encode(buf)
    }
    fn timeout(&self) -> Option<Duration> {
        Some(self.timeout)
    }
}

// Impl request for tuples to allow chained requests
impl<R1: AtRequest, R2: AtRequest> AtRequest for (R1, R2) {
    type Response = (R1::Response, R2::Response);
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

pub fn encode_request<R: AtRequest>(
    request: &R,
    buf: &mut impl Write,
) -> core::fmt::Result {
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

pub struct CommandRunner<'r, P: PacketProvider> {
    commands: SendDynPublisher<'r, CommandPacket<'r, P::Packet>>,
    responses: SendDynSubscriber<'r, P::Packet>,
    packet_provider: &'r mut P,
    default_timeout: Option<Duration>,
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

impl<'r, P: PacketProvider<Packet: Clone>> CommandRunner<'r, P> {
    pub fn new(
        commands: SendDynPublisher<'r, P::Packet>,
        responses: SendDynSubscriber<'r, P::Packet>,
        packet_provider: &'r mut P,
    ) -> Self {
        CommandRunner {
            commands,
            responses,
            packet_provider,
            default_timeout: None,
        }
    }

    /// Send a request to the modem, but do not wait for a response.
    pub async fn send_request<R: AtRequest>(&mut self, request: &R) -> Result<Timeout, Error> {
        let mut command = self.packet_provider.allocate().ok_or_else(|| Error::OutOfPackets)?;
        encode_request(request, &mut command.borrow_mut()).map_err(|_| Error::BufferOverflow)?;
        command.send_done();
        if let Some(duration) = request.timeout().or(self.default_timeout) {
            Ok(Timeout::Unarmed(duration))
        } else {
            Ok(Timeout::None)
        }
    }

    /// Wait for the modem to return a specific response.
    pub async fn expect_response<P: ParseResult, T>(
        &mut self,
        timeout: Timeout,
    ) -> Result<MappedReceiveSlotRef<'_, M, T, D>, Error> {
        let default_timeout = T::default_timeout().unwrap_or(AT_DEFAULT_TIMEOUT);
        let timeout_at = timeout.into_timeout(&default_timeout, Instant::now());
        loop {
            let response = with_deadline(timeout_at, self.responses.receive()).await?;
            let response = ReceiveSlotRef::new(response);
            match ReceiveSlotRef::filter_map(response, |response| T::from_generic(response)) {
                Ok(response) => return Ok(response),
                Err(err) => {
                    match &*err {
                        ResponseCode::Error(error) => return Err(Error::Sim(*error)),
                        unexpected_response => {
                            // TODO: we might want to make this a hard error, if/when we feel confident in
                            // how both the driver and the modem behaves
                            #[cfg(any(feature = "log", feature = "defmt"))]
                            log::warn!(
                                "Got unexpected response {:?} while waiting for {:?}",
                                unexpected_response,
                                T::RESPONSE_KIND
                            );
                        }
                    }
                }
            }
        }
    }

    /// Wait for the modem to return either of two response types.
    pub async fn expect_either_response<T1: AtResponse, T2: AtResponse>(
        &mut self,
        timeout: Timeout,
    ) -> Result<
        embassy_futures::select::Either<
            MappedReceiveSlotRef<'_, M, T1>,
            MappedReceiveSlotRef<'_, M, T2>,
        >,
        Error,
    > {
        use embassy_futures::select::Either;
        let default_timeout = cfg_select! {
            feature = "nightly" => T1::default_timeout()
                .reduce(T2::default_timeout(), |l, r| {
                    l.checked_add(r).unwrap_or(Duration::MAX)
                }),
            _ => T1::default_timeout().max(T2::default_timeout()),
        }
        .unwrap_or(AT_DEFAULT_TIMEOUT);
        let timeout_at = timeout.into_timeout(&default_timeout, Instant::now());
        loop {
            let response = with_deadline(timeout_at, self.responses.receive()).await?;
            let response = ReceiveSlotRef::new(response);
            match ReceiveSlotRef::filter_map(response, |response| T1::from_generic(response)) {
                Ok(response) => return Ok(Either::First(response)),
                Err(response) => match ReceiveSlotRef::filter_map(response, |response| {
                    T2::from_generic(response)
                }) {
                    Ok(response) => return Ok(Either::Second(response)),
                    Err(err) => match &*err {
                        ResponseCode::Error(error) => return Err(Error::Sim(*error)),
                        unexpected_response => {
                            // TODO: we might want to make this a hard error, if/when we feel confident in
                            // how both the driver and the modem behaves
                            #[cfg(any(feature = "log", feature = "defmt"))]
                            log::warn!(
                                "Got unexpected response {:?} while waiting for {:?} or {:?}",
                                unexpected_response,
                                T1::RESPONSE_KIND,
                                T2::RESPONSE_KIND
                            );
                        }
                    },
                },
            };
        }
    }

    /// Send raw bytes to the modem, use with care.
    pub async fn send_bytes(&mut self, bytes: &[u8]) {
        let mut bytes = bytes;
        while !bytes.is_empty() {
            let mut chunk = self.commands.send().await;
            let n = min(chunk.capacity(), bytes.len());
            let _ = chunk.extend(&bytes[..n]);
            bytes = &bytes[n..];
        }
    }

    /// Send a request to the modem, and wait for the modem to respond.
    pub async fn run<ResultParser, Request, Output>(&mut self, command: Request) -> Result<Output, Error>
    where
        ResultParser: ParseResult<Output = Output> + ExpectResponse<ResultParser>,
        Request: AtRequest<Response = ResultParser>,
    {
        log::debug!("Running AT command: {:?}", command);
        let timeout = self.send_request(&command).await?;
        log::trace!("Waiting for response for AT command: {:?}", command);
        let result = ResultParser::expect(self, timeout).await;
        log::trace!("Completed AT command: {:?}", command);

        if let Err(e) = &result {
            log::debug!("AT command {:?} error: {:?}", command, e);
        }

        result
    }

    /// Send a request to the modem and wait for the modem to respond.
    ///
    /// Use the provided timeout value instead of the configured one.
    pub async fn run_with_timeout<Request, Response>(
        &mut self,
        timeout: Duration,
        request: Request,
    ) -> Result<Response, Error>
    where
        Request: AtRequest<Response = Response>,
        Response: ExpectResponse<M>,
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

/// Implemented for (tuples of) AtResponse.
///
/// In order to support AtRequest::Response being a tuple of arbitrary size, we
/// implement the ExpectResponse trait for tuples with as many member as we need.
///
/// Also to support variable length response sequences, there is a specialization
/// tuples matching (heapless::Vec<T1, N>, T2) which will parse 0..N T1 responses
/// until a T2 response is encountered.
pub trait ExpectResponse<P: ParseResult>: Sized {
    type Output;
    fn expect<M: RawMutex>(
        runner: &mut CommandRunner<'_, M>,
        timeout: Timeout,
    ) -> impl Future<Output = Result<Self::Output, Error>>;
}

// No response
impl<P: ParseResult> ExpectResponse<P> for () {
    type Output = ();
    async fn expect<M: RawMutex>(_: &mut CommandRunner<'_, M>, _: Timeout) -> Result<Self::Output, Error> {
        Ok(())
    }
}

impl<T: AtResponse + Clone, P: ParseResult> ExpectResponse<P> for T {
    type Output = T;
    async fn expect<M: RawMutex>(runner: &mut CommandRunner<'_, M>, timeout: Timeout) -> Result<Self, Error> {
        runner
            .expect_response::<P, T>(timeout)
            .await
            .map(|response| response.clone())
    }
}

impl<T: ExpectResponse<P>, Y: AtResponse + Clone, P: ParseResult> ExpectResponse<P> for (T, Y) {
    type Output = (T::Output, Y);
    async fn expect<M: RawMutex>(runner: &mut CommandRunner<'_, M>, timeout: Timeout) -> Result<Self::Output, Error> {
        let r1 = <T as ExpectResponse<P>>::expect(runner, timeout).await?;
        let r2 = <Y as ExpectResponse<P>>::expect(runner, timeout).await?;
        Ok((r1, r2))
    }
}

impl<T: AtResponse + Clone, P: ParseResult, const N: usize>
    ExpectResponse<P> for Seq<T, N>
{
    async fn expect<M: RawMutex>(runner: &mut CommandRunner<'_, M>, timeout: Timeout) -> Result<Self, Error> {
        let mut response_vec = heapless::Vec::new();
        loop {
            match runner.expect_response::<P, T>(timeout).await {
                Ok(Some(item)) => response_vec
                    .push(item.clone())
                    .map_err(|_| Error::BufferOverflow)?,
                Ok(None) => break,
                Err(err) => return Err(err),
            }
        };
        Ok(Seq(response_vec, done_response))
    }
}

impl<T: AtResponse + Clone, E: AtResponse + Clone, M: RawMutex> ExpectResponse<M> for Result<T, E> {
    async fn expect(runner: &mut CommandRunner<'_, M>, timeout: Timeout) -> Result<Self, Error> {
        match runner.expect_either_response::<T, E>(timeout).await {
            Ok(embassy_futures::select::Either::First(item)) => Ok(Ok(item.clone())),
            Ok(embassy_futures::select::Either::Second(item)) => Ok(Err(item.clone())),
            Err(err) => Err(err),
        }
    }
}

impl<T1: AtResponse + Clone, T2: AtResponse + Clone, M: RawMutex> ExpectResponse<M>
    for Either<T1, T2>
{
    async fn expect(runner: &mut CommandRunner<'_, M>, timeout: Timeout) -> Result<Self, Error> {
        match runner.expect_either_response::<T1, T2>(timeout).await {
            Ok(embassy_futures::select::Either::First(item)) => Ok(Either::T1(item.clone())),
            Ok(embassy_futures::select::Either::Second(item)) => Ok(Either::T2(item.clone())),
            Err(err) => Err(err),
        }
    }
}

impl<T: AtResponse + Clone, O: TryFrom<T>, M: RawMutex> ExpectResponse<M> for MetaResponse<T, O> {
    async fn expect(runner: &mut CommandRunner<'_, M>, timeout: Timeout) -> Result<Self, Error> {
        let o = <T as ExpectResponse<M>>::expect(runner, timeout)
            .await?
            .try_into()
            .map_err(|_| Error::IncompatibleMapping)?;
        Ok(Self::new(o))
    }
}
