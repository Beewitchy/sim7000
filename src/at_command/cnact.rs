use heapless::String;

use crate::{util::collect_array};

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

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
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CNActPDP {
    pub pdp_index: u8,
    pub mode: CnactMode,
    pub address: Option<String<64>>,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetAppNetworkPDP(pub CNActPDP);

impl AtRequest for SetAppNetworkPDP {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNACT={},{}", self.0.pdp_index, self.0.mode as u8)?;
        if let Some(address) = &self.0.address {
            write!(buf, ",\"{}\"", address.as_str())?;
        }
        write!(buf, "\r")
    }
}

/// AT+CNACT?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetAppNetworkPDP;

impl AtRequest for GetAppNetworkPDP {
    type Response = (heapless::Vec<CNActPDP, 4>, GenericOk);

    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNACT?\r")
    }
}

impl AtParseLine for CNActPDP {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("+CNACT:").ok_or("missing prefix")?;
        let [pdp_index, mode, address] = collect_array(line.splitn(3, ',')).ok_or("missing arguments")?;
        let pdp_index = pdp_index.trim().parse().map_err(|_| "invalid value")?;
        let mode = mode.trim().parse().map_err(|_| "invalid value")?;
        let address = address.trim().strip_prefix('"').ok_or("invalid value")?.strip_suffix('"').ok_or("invalid value")?;
        let address = Some(address.try_into().map_err(|_| "address too long")?);
        Ok(Self {
            pdp_index,
            mode,
            address,
        })
    }
}

impl AtResponse for CNActPDP {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::PdpNetworkActive(v) => Some(v),
            _ => None,
        }
    }
}
