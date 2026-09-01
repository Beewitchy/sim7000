use core::marker::PhantomData;

use super::{AtRequest, CommandGroup, GenericOk, SimResult, RequestType};

/// AT
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct At;

impl AtRequest for At {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Basic);
    fn encode(&self, _: &mut impl core::fmt::Write) -> core::fmt::Result {
        Ok(())
    }
}

/// A/ -- Repeat last command
#[derive(Debug, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rep<R>(PhantomData<R>);

impl<R: AtRequest> AtRequest for Rep<R> {
    type ResultType = R::ResultType;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Context);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "A/")
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_millis(120000))
    }
}

pub struct RepeatRequestBuilder<R, Repeats> {
    request: R,
    _repeats: PhantomData<Repeats>,
}

impl<R, Repeats: Default> RepeatRequestBuilder<R, Repeats> {
    pub fn repeat(self) -> RepeatRequestBuilder<R, (Repeats, Rep<R>)> {
        RepeatRequestBuilder {
            request: self.request,
            _repeats: PhantomData,
        }
    }
    pub fn build(self) -> (R, Repeats) {
        (self.request, Repeats::default())
    }
}

/// Repeat the given request
///
/// ```
/// use crate::{at_command::{At, at::{Rep, Repeat}}, modem::CommandRunner};
/// let r = At.repeat().repeat().build();
/// //command_runner.run(r).await;
/// ```
pub trait Repeat<R = Self> {
    fn repeat(self) -> RepeatRequestBuilder<R, Rep<R>>;
}

impl<T: AtRequest> Repeat for T {
    fn repeat(self) -> RepeatRequestBuilder<Self, Rep<Self>> {
        RepeatRequestBuilder { request: self, _repeats: PhantomData }
    }
}

/// +++ Pause a data mode and switch back to command mode
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PauseDataMode;

impl AtRequest for PauseDataMode {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Context);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+++")
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_secs(1))
    }
}

/// ATO[n] Switch back to a paused data mode from command mode
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SwitchToDataMode(pub Option<u8>);

impl AtRequest for SwitchToDataMode {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Context);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if let Some(n) = self.0 { write!(buf, "O{n}") } else { write!(buf, "O") }
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_secs(1))
    }
}