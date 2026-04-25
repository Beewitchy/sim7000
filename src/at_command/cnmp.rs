use core::fmt::Write;
use heapless::String;

use super::{AtRequest, GenericOk};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum NetworkMode {
    Automatic = 2,
    Gsm = 13,
    Lte = 38,
    GsmAndLts = 51,
}

/// AT+CNMP=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetNetworkMode(pub NetworkMode);

impl AtRequest for SetNetworkMode {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNMP={}\r", self.0 as u8)
    }
}
