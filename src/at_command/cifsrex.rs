use crate::util::collect_array;

use super::{AtParseErr, AtParseLine, AtRequest, CommandGroup, GenericOk, RequestType, SimResult};

/// AT+CIFSREX
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetLocalIpExt;

impl AtRequest for GetLocalIpExt {
    type ResultType = SimResult<GenericOk<IpExt>>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CIFSREX")
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IpExt {
    pub addr: [u8; 4],
}

impl AtParseLine for IpExt {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let addr = line
            .strip_prefix("+CIFSREX:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();
        let addr = collect_array(addr.splitn(4, '.').filter_map(|seg| seg.parse().ok()))
            .ok_or("Failed to parse IP segment")?;

        Ok(IpExt { addr })
    }
}
