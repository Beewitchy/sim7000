use crate::util::collect_array;

use super::{
    AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, MetaResponse, ResponseCode, cclk,
    cgnscold::XtraStatus,
};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ToggleXtra {
    Disable = 0,
    Enable = 1,
}

impl TryFrom<XtraStatus> for ToggleXtra {
    type Error = AtParseErr;

    fn try_from(value: XtraStatus) -> Result<Self, Self::Error> {
        match value {
            XtraStatus::Success => Ok(Self::Disable),
            XtraStatus::DoesntExist => Ok(Self::Enable),
            XtraStatus::NotEffective => Err(AtParseErr::from("not a toggle value")),
        }
    }
}

/// AT+CGNSXTRA=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssXtra(pub ToggleXtra);

impl AtRequest for GnssXtra {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGNSXTRA={}\r", self.0 as u8)
    }
}

/// AT+CGNSXTRA?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetGnssXtra;

/// AT+CGNSXTRA
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ValidateGnssXtra;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssXtraInfo {
    pub valid_diff_hours: Option<u8>,
    pub valid_duration_hours: u32,
    pub download_time: cclk::UtcDateTime,
}

impl AtRequest for GetGnssXtra {
    type Response = (MetaResponse<XtraStatus, ToggleXtra>, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGNSXTRA?\r")
    }
}

impl AtRequest for ValidateGnssXtra {
    type Response = (Result<GnssXtraInfo, XtraStatus>, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGNSXTRA\r")
    }
}

impl AtParseLine for GnssXtraInfo {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        // Prefix is optional
        let line = line.strip_prefix("+CGNSXTRA:").unwrap_or(line);
        let [valid_diff_hours, valid_duration_hours, download_time] =
            collect_array(line.splitn(3, ',')).ok_or("missing arguments")?;
        let valid_diff_hours: i16 = valid_diff_hours.parse().map_err(|_| "invalid data")?;
        let valid_diff_hours = if valid_diff_hours >= 0 {
            Some(valid_diff_hours as u8)
        } else {
            None
        };
        let valid_duration_hours = valid_duration_hours.parse().map_err(|_| "invalid data")?;
        let (download_time, _remain) =
            cclk::FromCclkStr::from_cclk_str(download_time).ok_or("missing time")?;
        Ok(GnssXtraInfo {
            valid_diff_hours,
            valid_duration_hours,
            download_time,
        })
    }
}

impl AtResponse for GnssXtraInfo {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::XtraInfo(v) => Some(v),
            _ => None,
        }
    }
}

impl AtParseLine for ToggleXtra {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CGNSXTRA:")
            .ok_or("Missing '+CGNSXTRA: '")?
            .trim();

        match line {
            "0" => Ok(ToggleXtra::Disable),
            "1" => Ok(ToggleXtra::Enable),
            _ => Err("Invalid response, expected 0, 1".into()),
        }
    }
}
