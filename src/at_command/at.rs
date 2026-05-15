use super::{AtRequest, GenericOk};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct At;

impl AtRequest for At {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT\r")
    }
}
