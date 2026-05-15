use crate::util::collect_array;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+CIFSREX
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetLocalIpExt;

impl AtRequest for GetLocalIpExt {
    type Response = (IpExt, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CIFSREX\r")
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IpExt {
    pub addr: [u8; 4],
}

impl AtParseLine for IpExt {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let addr = line
            .strip_prefix("+CIFSREX: ")
            .ok_or("Missing '+CIFSREX: '")?;
        let addr = collect_array(addr.splitn(4, '.').filter_map(|seg| seg.parse().ok()))
            .ok_or("Failed to parse IP segment")?;

        Ok(IpExt { addr })
    }
}

impl AtResponse for IpExt {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::IpExt(ip_ext) => Some(ip_ext),
            _ => None,
        }
    }
}
