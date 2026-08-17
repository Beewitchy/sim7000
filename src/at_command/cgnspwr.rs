use super::{AtRequest, CommandGroup, GenericOk, RequestType};

/// AT+CGNSPWR=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetGnssPower(pub bool);

impl AtRequest for SetGnssPower {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 {
            write!(buf, "+CGNSPWR=1")
        } else {
            write!(buf, "+CGNSPWR=0")
        }
    }
}
