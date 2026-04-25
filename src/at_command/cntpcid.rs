use core::fmt::Write;
use heapless::String;

use super::{AtRequest, GenericOk};

/// AT+CNTPCID=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetGprsBearerProfileId(pub u8);

impl AtRequest for SetGprsBearerProfileId {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNTPCID={}\r", self.0)
    }
}
