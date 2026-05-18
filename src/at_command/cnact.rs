use heapless::String;

use super::{AtRequest, GenericOk, AtParseErr};

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CnactMode {
    Deactive = 0,
    Active = 1,
    AutoActive = 2,
}

impl core::str::FromStr for CnactMode {
    type Err = AtParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = u8::from_str(s)?;
        match val {
            0 => Ok(Self::Deactive),
            1 => Ok(Self::Active),
            2 => Ok(Self::AutoActive),
            _ => Err("unhandled CnactMode value".into())
        }
    }
}

/// AT+CNACT=... for SIM7000
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetAppNetwork {
    pub mode: CnactMode,
    pub apn: String<63>,
}

impl AtRequest for SetAppNetwork {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNACT={},\"{}\"\r", self.mode as u8, self.apn)
    }
}

/// AT+CNACT=... for PDP
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetAppNetworkPDP {
    pub pdp_index: u8,
    pub mode: CnactMode,
    pub address: Option<String<64>>,
}

impl AtRequest for SetAppNetworkPDP {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNACT={},{}", self.pdp_index, self.mode as u8)?;
        if let Some(address) = &self.address {
            write!(buf, ",\"{}\"", address.as_str())?;
        }
        write!(buf, "\r")
    }
}
