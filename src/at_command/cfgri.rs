use super::{RequestType, CommandGroup, AtRequest, GenericOk};

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RiPinMode {
    Off = 0,
    On = 1,
    OnTcpIp = 2,
}

/// AT+CFGRI=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConfigureRiPin(pub RiPinMode);

impl AtRequest for ConfigureRiPin {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CFGRI={}", self.0 as u8)
    }
}
