use super::{AtRequest, CommandGroup, GenericOk, RequestType, SimResult};

/// ATZ
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ResetConfigurationToDefaults;

impl AtRequest for ResetConfigurationToDefaults {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Basic);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "Z")
    }
}
