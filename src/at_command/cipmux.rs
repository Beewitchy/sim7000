use super::{RequestType, CommandGroup, AtRequest, GenericOk};

/// AT+CIPMUX=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EnableMultiIpConnection(pub bool);

impl AtRequest for EnableMultiIpConnection {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 {
            write!(buf, "+CIPMUX=1")
        } else {
            write!(buf, "+CIPMUX=0")
        }
    }
}
