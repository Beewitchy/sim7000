use super::{AtRequest, CommandGroup, GenericOk, RequestType};

/// AT
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct At;

impl AtRequest for At {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Basic);
    fn encode(&self, _: &mut impl core::fmt::Write) -> core::fmt::Result {
        Ok(())
    }
}

/// A/ -- Repeat last command
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Rep;

impl AtRequest for Rep {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Context);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "A/")
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_millis(120000))
    }
}

/// +++ Pause a data mode and switch back to command mode
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PauseDataMode;

impl AtRequest for PauseDataMode {
    type Response = GenericOk;
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
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Context);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if let Some(n) = self.0 { write!(buf, "O{n}") } else { write!(buf, "O") }
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_secs(1))
    }
}