use core::cell::RefMut;
use core::future::Future;
use core::{cell::RefCell, cmp::min, mem};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    mutex::{Mutex, MutexGuard},
    zerocopy_channel::{Channel, Receiver, Sender},
};
use embassy_time::{Duration, TimeoutError, with_timeout};
use heapless::{String, Vec};

use crate::Error;
use crate::at_command::{AtRequest, AtResponse, ResponseCode};
use crate::log;
use crate::modem::ModemContext;

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

pub struct CommandRunner<'a, M: RawMutex> {
    commands: Sender<'a, M, RawAtCommand>,
    responses: Receiver<'a, M, Option<ResponseCode>>,
}

impl<'a, M> CommandRunner<'a, M>
where
    M: RawMutex + 'static,
{
    pub fn create(ctx: &'a mut ModemContext<M>) -> Self {
        CommandRunner {
            commands: sender,
            responses: receiver,
        }
    }
}

pub struct CommandRunnerGuard<'a, M: RawMutex> {
    runner: MutexGuard<'a, M, CommandRunner<'a, M>>,
    timeout: Option<Duration>,
}

impl<'a, M> CommandRunner<'a, M>
where
    M: RawMutex,
{
    pub async fn lock(mutex: &'a embassy_sync::mutex::Mutex<M, Self>) -> CommandRunnerGuard<'a, M> {
        let runner = mutex.lock().await;
        CommandRunnerGuard {
            runner,
            timeout: Some(AT_DEFAULT_TIMEOUT),
        }
    }
}

impl<'a, M> CommandRunnerGuard<'a, M>
where
    M: RawMutex,
{
    /// Send a request to the modem, but do not wait for a response.
    pub async fn send_request<R: AtRequest>(&mut self, request: &R) -> Result<(), TimeoutError> {
        let command = self.runner.commands.send();
        let command = if let Some(timeout) = self.timeout {
            with_timeout(timeout, command).await?
        } else {
            command.await
        };
        let _ = request.encode(command);
        self.runner.commands.send_done();
        Ok(())
    }

    /// Wait for the modem to return a specific response.
    pub async fn expect_response<T: AtResponse + 'a>(&mut self) -> Result<&mut T, Error> {
        loop {
            let response = self.runner.responses.receive();
            let response = if let Some(timeout) = self.timeout {
                with_timeout(timeout, response).await?
            } else {
                response.await
            };
            if let Some(response) = response {
                match T::from_generic(response) {
                    Ok(response) => return Ok(response),
                    Err(ResponseCode::Error(error)) => return Err(Error::Sim(*error)),
                    Err(unknown_response) => {
                        // TODO: we might want to make this a hard error, if/when we feel confident in
                        // how both the driver and the modem behaves
                        log::warn!("Got unexpected ATResponse: {:?}", unknown_response)
                    }
                }
            }
            self.runner.responses.receive_done();
        }
    }

    /// Send raw bytes to the modem, use with care.
    pub async fn send_bytes(&mut self, mut bytes: &[u8]) {
        while !bytes.is_empty() {
            let chunk = self.runner.commands.send().await;
            chunk.clear();
            let n = min(chunk.capacity(), bytes.len());
            let _ = chunk.extend(&bytes[..n]);
            bytes = &bytes[n..];
            self.runner.commands.send_done();
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
            log::error!("AT command {:?} error: {:?}", command, e);
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

pub struct ResponseGuard<'a, 'r, M: RawMutex> {
    responses: RefMut<'r, Receiver<'a, M, Option<ResponseCode>>>,
}

/// Implemented for (tuples of) AtResponse.
///
/// In order to support AtRequest::Response being a tuple of arbitrary size, we
/// implement the ExpectResponse trait for tuples with as many member as we need.
pub trait ExpectResponse<M: RawMutex>: Sized {
    fn expect(runner: &mut CommandRunnerGuard<'_, M>) -> impl Future<Output = Result<Self, Error>>;
}

impl<T: AtResponse + Clone + 'static, M: RawMutex> ExpectResponse<M> for T {
    async fn expect(runner: &mut CommandRunnerGuard<'_, M>) -> Result<Self, Error> {
        runner.expect_response().await.cloned()
    }
}

impl<T: AtResponse + Clone + 'static, Y: AtResponse + Clone + 'static, M: RawMutex>
    ExpectResponse<M> for (T, Y)
{
    async fn expect(runner: &mut CommandRunnerGuard<'_, M>) -> Result<Self, Error> {
        let r1 = runner.expect_response().await.cloned()?;
        let r2 = runner.expect_response().await.cloned()?;
        Ok((r1, r2))
    }
}

impl<
    T: AtResponse + Clone + 'static,
    Y: AtResponse + Clone + 'static,
    Z: AtResponse + Clone + 'static,
    M: RawMutex,
> ExpectResponse<M> for (T, Y, Z)
{
    async fn expect(runner: &mut CommandRunnerGuard<'_, M>) -> Result<Self, Error> {
        let r1 = runner.expect_response().await.cloned()?;
        let r2 = runner.expect_response().await.cloned()?;
        let r3 = runner.expect_response().await.cloned()?;
        Ok((r1, r2, r3))
    }
}
