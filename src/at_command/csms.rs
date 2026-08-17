use super::{AtRequest, GenericOk, RequestType, CommandGroup};

/// AT+CSMS=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SelectMessageService;

impl AtRequest for SelectMessageService {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CSMS=0")
    }
}
