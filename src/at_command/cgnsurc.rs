use core::fmt::Write;
use heapless::String;

use super::{AtRequest, GenericOk};

/// AT+CGNSURC=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConfigureGnssUrc {
    /// Send URC report every `n` GNSS fix.
    /// Set to 0 to disable.
    pub period: u8,
}

impl AtRequest for ConfigureGnssUrc {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGNSURC={}\r", self.period)
    }
}
