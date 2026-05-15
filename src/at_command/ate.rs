use super::{AtRequest, GenericOk};

/// ATE1 / ATE0
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetEcho(pub bool);

impl AtRequest for SetEcho {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 { write!(buf, "ATE1\r") } else { write!(buf, "ATE0\r") }
    }
}
