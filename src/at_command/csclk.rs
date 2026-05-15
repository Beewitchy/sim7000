use super::{AtRequest, GenericOk};

/// AT+CSCLK=<1 or 0>
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetSlowClock(pub bool);

impl AtRequest for SetSlowClock {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 {
            write!(buf, "AT+CSCLK=1\r")
        } else {
            write!(buf, "AT+CSCLK=0\r")
        }
    }
}
