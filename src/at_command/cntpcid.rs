use super::{AtRequest, CommandGroup, GenericOk, RequestType, SimResult};

/// AT+CNTPCID=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetGprsBearerProfileId(pub u8);

impl AtRequest for SetGprsBearerProfileId {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CNTPCID={}", self.0)
    }
}
