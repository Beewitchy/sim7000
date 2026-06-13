use core::{
    cmp::min,
    future::Future,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    zerocopy_channel::{ReceiveSlot, Receiver as ZerocopyReceiver, Sender as ZerocopySender},
};
use embassy_time::{Duration, TimeoutError, with_timeout};
use heapless::{String, Vec};

use crate::{
    Error,
    at_command::{AtRequest, AtResponse, Either, ResponseCode, Seq},
    log,
};

/// The default timeout of AT commands
pub const AT_DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct RawAtCommand {
    pub(crate) bytes: Vec<u8, 256>,
    pub(crate) binary: bool,
}

impl core::fmt::Write for RawAtCommand {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.binary = false;
        self.bytes
            .extend_from_slice(s.as_bytes())
            .map_err(|_| core::fmt::Error)
    }
}

impl core::iter::Extend<u8> for RawAtCommand {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        self.binary = true;
        self.bytes.extend(iter)
    }
}

impl<'a> core::iter::Extend<&'a u8> for RawAtCommand {
    fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
        self.binary = true;
        self.bytes.extend(iter.into_iter().cloned())
    }
}

impl From<String<256>> for RawAtCommand {
    fn from(s: String<256>) -> Self {
        RawAtCommand {
            bytes: s.into_bytes(),
            binary: false,
        }
    }
}

impl From<&'_ str> for RawAtCommand {
    fn from(s: &'_ str) -> Self {
        RawAtCommand {
            bytes: s.as_bytes().try_into().unwrap_or_default(),
            binary: false,
        }
    }
}

impl From<&'_ [u8]> for RawAtCommand {
    fn from(s: &'_ [u8]) -> Self {
        RawAtCommand {
            bytes: s.try_into().unwrap_or_default(),
            binary: true,
        }
    }
}

impl RawAtCommand {
    pub const fn new() -> Self {
        Self {
            bytes: Vec::new(),
            binary: false,
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes.as_slice()
    }

    pub fn clear(&mut self) {
        self.binary = false;
        self.bytes.clear()
    }

    pub fn capacity(&self) -> usize {
        self.bytes.capacity()
    }
}

pub struct CommandRunner<'r, M: RawMutex> {
    commands: ZerocopySender<'r, M, RawAtCommand>,
    responses: ZerocopyReceiver<'r, M, ResponseCode>,
    timeout: Option<Duration>,
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

impl<'r, M> CommandRunner<'r, M>
where
    M: RawMutex,
{
    pub fn new(
        commands: ZerocopySender<'r, M, RawAtCommand>,
        responses: ZerocopyReceiver<'r, M, ResponseCode>,
    ) -> Self {
        CommandRunner {
            commands,
            responses,
            timeout: None,
        }
    }

    /// Send a request to the modem, but do not wait for a response.
    pub async fn send_request<R: AtRequest>(&mut self, request: &R) -> Result<(), TimeoutError> {
        let mut command = if let Some(timeout) = self.timeout {
            with_timeout(timeout, self.commands.send()).await?
        } else {
            self.commands.send().await
        };
        command.clear();
        let _ = request.encode(&mut command.bytes);
        command.send_done();
        Ok(())
    }

    /// Wait for the modem to return a specific response.
    pub async fn expect_response<T: AtResponse>(
        &mut self,
    ) -> Result<MappedReceiveSlotRef<'_, M, T, ResponseCode>, Error> {
        loop {
            let response = self.responses.receive();
            let response = if let Some(timeout) = self.timeout {
                with_timeout(timeout, response).await?
            } else {
                response.await
            };
            let response = ReceiveSlotRef::new(response);
            match ReceiveSlotRef::filter_map(response, |response| T::from_generic(response)) {
                Ok(response) => return Ok(response),
                Err(err) => {
                    match &*err {
                        ResponseCode::Error(error) => return Err(Error::Sim(*error)),
                        unexpected_response => {
                            // TODO: we might want to make this a hard error, if/when we feel confident in
                            // how both the driver and the modem behaves
                            log::warn!("Got unexpected ATResponse: {:?}", unexpected_response)
                        }
                    }
                }
            }
        }
    }

    /// Wait for the modem to return either of two response types.
    pub async fn expect_either_response<T1: AtResponse, T2: AtResponse>(
        &mut self,
    ) -> Result<
        embassy_futures::select::Either<
            MappedReceiveSlotRef<'_, M, T1, ResponseCode>,
            MappedReceiveSlotRef<'_, M, T2, ResponseCode>,
        >,
        Error,
    > {
        use embassy_futures::select::Either;
        loop {
            let response = self.responses.receive();
            let response = if let Some(timeout) = self.timeout {
                with_timeout(timeout, response).await?
            } else {
                response.await
            };
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
                            log::warn!("Got unexpected ATResponse: {:?}", unexpected_response)
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
    pub async fn run<Request, Response>(&mut self, command: Request) -> Result<Response, Error>
    where
        Request: AtRequest<Response = Response>,
        Response: ExpectResponse<M>,
    {
        log::trace!("Running AT command: {:?}", command);
        self.send_request(&command).await?;
        log::trace!("Waiting for response for AT command: {:?}", command);
        let result = Response::expect(self).await;
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
        mut timeout: Option<Duration>,
        command: Request,
    ) -> Result<Response, Error>
    where
        Request: AtRequest<Response = Response>,
        Response: ExpectResponse<M>,
    {
        mem::swap(&mut self.timeout, &mut timeout);
        let result = self.run(command).await;
        mem::swap(&mut self.timeout, &mut timeout);
        result
    }

    /// Set the timeout of subsequent commands
    ///
    /// Note that the timeout defaults to [AT_DEFAULT_TIMEOUT].
    pub fn with_timeout(self, timeout: Option<Duration>) -> Self {
        Self { timeout, ..self }
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
pub trait ExpectResponse<M: RawMutex>: Sized {
    fn expect(runner: &mut CommandRunner<'_, M>) -> impl Future<Output = Result<Self, Error>>;
}

impl<T: AtResponse + Clone, M: RawMutex> ExpectResponse<M> for T {
    async fn expect(runner: &mut CommandRunner<'_, M>) -> Result<Self, Error> {
        runner
            .expect_response::<T>()
            .await
            .map(|response| response.clone())
    }
}

impl<T: ExpectResponse<M>, Y: AtResponse + Clone, M: RawMutex> ExpectResponse<M> for (T, Y) {
    async fn expect(runner: &mut CommandRunner<'_, M>) -> Result<Self, Error> {
        let r1 = <T as ExpectResponse<M>>::expect(runner).await?;
        let r2 = <Y as ExpectResponse<M>>::expect(runner).await?;
        Ok((r1, r2))
    }
}

impl<T: AtResponse + Clone, Y: AtResponse + Clone, Z: AtResponse + Clone, M: RawMutex>
    ExpectResponse<M> for (T, Y, Z)
{
    async fn expect(runner: &mut CommandRunner<'_, M>) -> Result<Self, Error> {
        let r1 = <T as ExpectResponse<M>>::expect(runner).await?;
        let r2 = <Y as ExpectResponse<M>>::expect(runner).await?;
        let r3 = <Z as ExpectResponse<M>>::expect(runner).await?;
        Ok((r1, r2, r3))
    }
}

impl<
    T1: AtResponse + Clone,
    T2: AtResponse + Clone,
    T3: AtResponse + Clone,
    T4: AtResponse + Clone,
    T5: AtResponse + Clone,
    T6: AtResponse + Clone,
    M: RawMutex,
> ExpectResponse<M> for (T1, T2, T3, T4, T5, T6)
{
    async fn expect(runner: &mut CommandRunner<'_, M>) -> Result<Self, Error> {
        let r1 = <T1 as ExpectResponse<M>>::expect(runner).await?;
        let r2 = <T2 as ExpectResponse<M>>::expect(runner).await?;
        let r3 = <T3 as ExpectResponse<M>>::expect(runner).await?;
        let r4 = <T4 as ExpectResponse<M>>::expect(runner).await?;
        let r5 = <T5 as ExpectResponse<M>>::expect(runner).await?;
        let r6 = <T6 as ExpectResponse<M>>::expect(runner).await?;
        Ok((r1, r2, r3, r4, r5, r6))
    }
}

impl<T: AtResponse + Clone, DoneT: AtResponse + Clone, M: RawMutex, const N: usize>
    ExpectResponse<M> for Seq<T, N, DoneT>
{
    async fn expect(runner: &mut CommandRunner<'_, M>) -> Result<Self, Error> {
        let mut response_vec = heapless::Vec::new();
        let done_response = loop {
            match runner.expect_either_response::<T, DoneT>().await {
                Ok(embassy_futures::select::Either::First(item)) => response_vec
                    .push(item.clone())
                    .map_err(|_| Error::BufferOverflow)?,
                Ok(embassy_futures::select::Either::Second(done)) => break done.clone(),
                Err(err) => return Err(err),
            }
        };
        Ok(Seq(response_vec, done_response))
    }
}

impl<T: AtResponse + Clone, E: AtResponse + Clone, M: RawMutex> ExpectResponse<M> for Result<T, E> {
    async fn expect(runner: &mut CommandRunner<'_, M>) -> Result<Self, Error> {
        match runner.expect_either_response::<T, E>().await {
            Ok(embassy_futures::select::Either::First(item)) => Ok(Ok(item.clone())),
            Ok(embassy_futures::select::Either::Second(item)) => Ok(Err(item.clone())),
            Err(err) => Err(err),
        }
    }
}

impl<T1: AtResponse + Clone, T2: AtResponse + Clone, M: RawMutex> ExpectResponse<M>
    for Either<T1, T2>
{
    async fn expect(runner: &mut CommandRunner<'_, M>) -> Result<Self, Error> {
        match runner.expect_either_response::<T1, T2>().await {
            Ok(embassy_futures::select::Either::First(item)) => Ok(Either::T1(item.clone())),
            Ok(embassy_futures::select::Either::Second(item)) => Ok(Either::T2(item.clone())),
            Err(err) => Err(err),
        }
    }
}
