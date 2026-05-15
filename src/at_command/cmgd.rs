use super::{AtRequest, GenericOk};

/// AT+CMGD=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DeleteSms(pub DeleteFlag);

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum DeleteFlag {
    Index(u8) = 0,
    Read = 1,
    ReadAndSent = 2,
    ReadAndSentUnsent = 3,
    All = 4,
}

impl DeleteFlag {
    pub fn as_u8(&self) -> u8 {
        match self {
            DeleteFlag::Index(_) => 0,
            DeleteFlag::Read => 1,
            DeleteFlag::ReadAndSent => 2,
            DeleteFlag::ReadAndSentUnsent => 3,
            DeleteFlag::All => 4,
        }
    }
}

impl AtRequest for DeleteSms {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if let DeleteFlag::Index(index) = self.0 {
            write!(buf, "AT+CMGD={}\r", index)
        } else {
            write!(buf, "AT+CMGD=0,{}\r", self.0.as_u8())
        }
    }
}
