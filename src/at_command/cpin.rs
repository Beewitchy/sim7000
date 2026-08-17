use super::{AtRequest, RequestType, CommandGroup, GenericOk, unsolicited};

/// AT+CPIN?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetPinStatus;

impl AtRequest for GetPinStatus {
    type Response = (unsolicited::CPin, GenericOk);
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    #[inline]
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CPIN?")
    }
}
