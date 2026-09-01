use super::{AtRequest, CommandGroup, GenericOk, RequestType, SimResult, unsolicited};

/// AT+CPIN?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetPinStatus;

impl AtRequest for GetPinStatus {
    type ResultType = SimResult<GenericOk<unsolicited::CPin>>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    #[inline]
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CPIN?")
    }
}
