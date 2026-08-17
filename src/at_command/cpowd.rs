use embassy_time::Duration;

use super::{AtRequest, GenericOk, RequestType, CommandGroup};

/// AT+CPOWD=0
///
/// This is implemented as a separate type to [NormalPowerDown] because
/// the "urgent" mode has no response.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct UrgentPowerDown;

/// AT+CPOWD=1
///
/// This is implemented as a separate type to [UrgentPowerDown] because
/// the "urgent" mode has no response.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NormalPowerDown;

impl AtRequest for UrgentPowerDown {
    type Response = ();
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CPOWD=0")
    }
}

impl AtRequest for NormalPowerDown {
    // Check the status with [crate::modem::Modem::ready()]
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CPOWD=1")
    }
    fn default_timeout() -> Option<Duration> {
        // There is no specified max response for this command, but
        // for my modem 30 is often too short, while 60 has been
        // enough
        Some(Duration::from_secs(60))
    }
}
