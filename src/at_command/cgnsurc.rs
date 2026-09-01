use super::{AtRequest, CommandGroup, GenericOk, RequestType, SimResult};

/// AT+CGNSURC=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConfigureGnssUrc {
    /// Send URC report every `n` GNSS fix.
    /// Set to 0 to disable.
    pub period: u8,
}

impl AtRequest for ConfigureGnssUrc {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CGNSURC={}", self.period)
    }
}
