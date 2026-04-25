use core::fmt::Write;
use heapless::String;

use super::{AtRequest, GenericOk};

/// AT+CSCS=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetTeCharacterSet(pub CharacterSet);

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CharacterSet {
    GSM,
    UCS2,
    IRA,
}

impl AtRequest for SetTeCharacterSet {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        let character_set = match self.0 {
            CharacterSet::GSM => "GSM",
            CharacterSet::UCS2 => "USC2",
            CharacterSet::IRA => "IRA",
        };

        write!(buf, "AT+CSCS=\"{}\"\r", character_set)
    }
}
