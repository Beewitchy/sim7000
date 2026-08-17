use super::{AtRequest, CommandGroup, GenericOk, RequestType};

/// ATE1 / ATE0
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetEcho(pub bool);

impl AtRequest for SetEcho {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Basic);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 { write!(buf, "E1") } else { write!(buf, "E0") }
    }
}
