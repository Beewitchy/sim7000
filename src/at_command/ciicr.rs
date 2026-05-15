use super::{AtRequest, GenericOk};

/// AT+CIICR
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct StartGprs;

impl AtRequest for StartGprs {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CIICR\r")
    }
}
