use super::{AtRequest, GenericOk};

/// AT+CIPMUX=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EnableMultiIpConnection(pub bool);

impl AtRequest for EnableMultiIpConnection {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 {
            write!(buf, "AT+CIPMUX=1\r")
        } else {
            write!(buf, "AT+CIPMUX=0\r")
        }
    }
}
