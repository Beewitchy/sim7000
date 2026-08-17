use super::{RequestType, CommandGroup, AtRequest, GenericOk};

/// AT+CBATCHK=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EnableVBatCheck(pub bool);

impl AtRequest for EnableVBatCheck {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 {
            write!(buf, "+CBATCHK=1")
        } else {
            write!(buf, "+CBATCHK=0")
        }
    }
}
