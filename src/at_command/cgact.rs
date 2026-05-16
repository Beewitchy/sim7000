use super::{AtRequest, GenericOk};


/// AT+CGACT?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetPdpContextActivation;

impl AtRequest for GetPdpContextActivation {
    // The actual response is generated as an URC
    type Response = GenericOk;

    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGACT?\r")
    }
}
