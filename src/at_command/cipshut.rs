use super::{AtRequest, GenericOk};

/// AT+CIPSHUT
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ShutConnections;

impl AtRequest for ShutConnections {
    type Response = GenericOk; // TODO: should have its own type
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CIPSHUT\r")
    }
}
