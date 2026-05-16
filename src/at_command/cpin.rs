use super::{AtRequest, GenericOk, unsolicited};

/// AT+CPIN?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetPinStatus;

impl AtRequest for GetPinStatus {
    type Response = (unsolicited::CPin, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CPIN?\r")
    }
}
