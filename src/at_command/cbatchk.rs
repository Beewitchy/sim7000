use heapless::String;

use super::{AtRequest, GenericOk};

/// AT+CBATCHK=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EnableVBatCheck(pub bool);

impl AtRequest for EnableVBatCheck {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 {
            write!(buf, "AT+CBATCHK=1\r")
        } else {
            write!(buf, "AT+CBATCHK=0\r")
        }
    }
}
