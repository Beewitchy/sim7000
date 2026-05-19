use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode, stub_parser_prefix};

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ToggleXtra {
    Disable = 0,
    Enable = 1,
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

/// AT+CGNSXTRA=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ValidateGnssXtra;

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssXtraInfo;

impl AtRequest for ValidateGnssXtra {
    type Response = (GnssXtraInfo, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGNSXTRA\r")
    }
}

impl AtParseLine for GnssXtraInfo {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        stub_parser_prefix(line, "+CGNSXTRA:", GnssXtraInfo)
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
