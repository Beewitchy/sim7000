use super::{AtRequest, GenericOk, RequestType, CommandGroup};

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BaudRate {
    Hz0 = 0,
    Hz300 = 300,
    Hz600 = 600,
    Hz1200 = 1200,
    Hz2400 = 2400,
    Hz4800 = 4800,
    Hz9600 = 9600,
    Hz19200 = 19200,
    Hz38400 = 38400,
    Hz57600 = 57600,
    Hz115200 = 115200,
    Hz230400 = 230400,
    Hz921600 = 921600,
    Hz2000000 = 2000000,
    Hz2900000 = 2900000,
    Hz3000000 = 3000000,
    Hz3200000 = 3200000,
    Hz3686400 = 3686400,
    Hz4000000 = 4000000,
}

impl BaudRate {
    #[must_use]
    pub const fn is_auto(&self) -> bool {
        matches!(self, Self::Hz0)
    }

    pub const fn hz(&self) -> u32 {
        *self as u32
    }
}

/// AT+IPR=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetBaudRate(pub BaudRate);

impl AtRequest for SetBaudRate {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+IPR={}", self.0 as u32)
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_secs(30))
    }
}
