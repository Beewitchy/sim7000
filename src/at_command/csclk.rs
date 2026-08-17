use super::{RequestType, CommandGroup, AtRequest, GenericOk};

/// AT+CSCLK=<1 or 0>
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetSlowClock(pub bool);

impl AtRequest for SetSlowClock {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if self.0 {
            write!(buf, "+CSCLK=1")
        } else {
            write!(buf, "+CSCLK=0")
        }
    }
}
