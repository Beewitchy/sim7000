use core::cell::RefMut;
use core::future::Future;
use core::{cmp::min, mem, cell::RefCell};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    zerocopy_channel::{Channel, Receiver, Sender},
    mutex::{Mutex, MutexGuard},
};
use embassy_time::{with_timeout, Duration, TimeoutError};
use heapless::{String, Vec};

use crate::at_command::{AtRequest, AtResponse, ResponseCode};
use crate::log;
use crate::modem::ModemContext;
use crate::Error;

/// The default timeout of AT commands
pub const AT_DEFAULT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct RawAtCommand {
    bytes: Vec<u8, 256>,
    binary: bool,
}

impl core::fmt::Write for RawAtCommand {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.binary = false;
        self.bytes.extend_from_slice(s.as_bytes()).map_err(|_| core::fmt::Error)
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
        self.bytes.extend(iter)
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
            bytes: s.try_into().unwrap_or_default(),
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
        Self { bytes: Vec::new(), binary: false }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes.as_bytes()
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
    command_lock: &'a Mutex<M, ()>,
    commands: RefCell<Sender<'a, M, RawAtCommand>>,
    responses: RefCell<Receiver<'a, M, Option<ResponseCode>>>,
}

impl<'a, M> CommandRunner<'a, M> where M: RawMutex + 'static {
    pub fn create(ctx: &'a ModemContext<M>) -> Self {
        let (sender, _) = ctx.commands.get_or_init(|| Channel::new(&mut [RawAtCommand::new(), RawAtCommand::new(), RawAtCommand::new(), RawAtCommand::new()])).split();
        let (_, receiver) = ctx.generic_response.get_or_init(|| Channel::new(&mut [None])).split();
        CommandRunner {
            command_lock: &ctx.command_lock,
            commands: core::cell::RefCell::new(sender),
            responses: core::cell::RefCell::new(receiver),
        }
    }
}

pub struct CommandRunnerGuard<'a, M: RawMutex> {
    _commands_guard: MutexGuard<'a, M, ()>,
    runner: &'a CommandRunner<'a, M>,
    timeout: Option<Duration>,
}

impl<'a, M> CommandRunner<'a, M> where M: RawMutex {
    pub async fn lock(&'a self) -> CommandRunnerGuard<'a, M> {
        CommandRunnerGuard {
            _commands_guard: self.command_lock.lock().await,
            runner: self,
            timeout: Some(AT_DEFAULT_TIMEOUT),
        }
    }
}

impl<'a, M> CommandRunnerGuard<'a, M> where M: RawMutex {
    /// Run a future with the timeout configured for self
    async fn timeout<T, F: Future<Output = T>>(&self, future: F) -> Result<T, TimeoutError> {
        Ok(match self.timeout {
            Some(timeout) => with_timeout(timeout, future).await?,
            None => future.await,
        })
    }

    /// Send a request to the modem, but do not wait for a response.
    pub async fn send_request<R: AtRequest>(&self, request: &R) -> Result<(), TimeoutError> {
        self.timeout(async {
            let mut commands = self.runner.commands.borrow_mut();
            let command = commands.send().await;
            let _ = request.encode(command);
            commands.send_done();
        })
        .await
    }

    /// Wait for the modem to return a specific response.
    pub async fn expect_response<'r, T: AtResponse + 'r>(&mut self) ->  Result<ResponseGuard<'r, '_, T, M>, Error> where 'a: 'r {
        let mut responses = self.runner.responses.borrow_mut();
        self.timeout(async {
            loop {
                let code = responses.receive().await;
                if let Some(code) = code {
                    match T::from_generic(code) {
                        Ok(received) => { return Ok(ResponseGuard { responses, response: received } ) },
                        Err(ResponseCode::Error(error)) => return Err(Error::Sim(*error)),
                        Err(unknown_response) => {
                            // TODO: we might want to make this a hard error, if/when we feel confident in
                            // how both the driver and the modem behaves
                            log::warn!("Got unexpected ATResponse: {:?}", unknown_response)
                        }
                    }
                }
                responses.receive_done();
            }
        })
        .await?
    }

    /// Send raw bytes to the modem, use with care.
    pub async fn send_bytes(&self, mut bytes: &[u8]) {
        let commands = self.runner.commands.borrow_mut();
        while !bytes.is_empty() {
            let chunk = commands.send().await;
            chunk.clear();
            let n = min(chunk.capacity(), bytes.len());
            let _ = chunk.extend(&bytes[..n]);
            bytes = &bytes[n..];
            commands.send_done();
        }
    }

    /// Send a request to the modem, and wait for the modem to respond.
    pub async fn run<Request, Response>(&self, command: Request) -> Result<Response, Error>
    where
        Request: AtRequest<Response = Response>,
        Response: ExpectResponse,
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
        Response: ExpectResponse,
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

pub struct ResponseGuard<'a, 'r, T: AtResponse, M: RawMutex> {
    responses: RefMut<'r, Receiver<'a, M, Option<ResponseCode>>>,
    response: &'r mut T,
}

/// Implemented for (tuples of) AtResponse.
///
/// In order to support AtRequest::Response being a tuple of arbitrary size, we
/// implement the ExpectResponse trait for tuples with as many member as we need.
pub trait ExpectResponse: Sized {
    fn expect<'a>(runner: &'a CommandRunnerGuard<'a>) -> impl Future<Output = Result<Self, Error>>;
}

impl<T: AtResponse> ExpectResponse for T {
    async fn expect<'a>(runner: &'a CommandRunnerGuard<'a>) -> Result<Self, Error> {
        runner.expect_response().await
    }
}

impl<T: AtResponse, Y: AtResponse> ExpectResponse for (T, Y) {
    async fn expect<'a>(runner: &'a CommandRunnerGuard<'a>) -> Result<Self, Error> {
        let r1 = runner.expect_response().await?;
        let r2 = runner.expect_response().await?;
        Ok((r1, r2))
    }
}

impl<T: AtResponse, Y: AtResponse, Z: AtResponse> ExpectResponse for (T, Y, Z) {
    async fn expect<'a>(runner: &'a CommandRunnerGuard<'a>) -> Result<Self, Error> {
        let r1 = runner.expect_response().await?;
        let r2 = runner.expect_response().await?;
        let r3 = runner.expect_response().await?;
        Ok((r1, r2, r3))
    }
}
