use super::{AtRequest, GenericOk};

/// ATZ
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ResetConfigurationToDefaults;

impl AtRequest for ResetConfigurationToDefaults {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "ATZ\r")
    }
}
