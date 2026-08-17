use super::{RequestType, CommandGroup, AtRequest, GenericOk};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum NbMode {
    CatM = 1,
    NbIot = 2,
    Both = 3,
}

/// AT+CMNB=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetNbMode(pub NbMode);

impl AtRequest for SetNbMode {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CMNB={}", self.0 as u8)
    }
}
