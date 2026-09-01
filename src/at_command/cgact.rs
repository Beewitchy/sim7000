use core::str::FromStr as _;

use super::{
    AtParseErr, AtParseLine, AtRequest, CnactMode, CommandGroup, GenericOk,
    RequestType, Seq, SimResult
};

/// AT+CGACT?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetPdpContextActivation;

impl AtRequest for GetPdpContextActivation {
    type ResultType = SimResult<GenericOk<Seq<CGact, 4>>>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CGACT?")
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CGact {
    pub cid: u8,
    pub state: CnactMode,
}

impl AtRequest for CGact {
    type ResultType = SimResult<GenericOk<Seq<CGact, 4>>>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CGACT={},{}", self.cid, self.state as u8)
    }
}

impl AtParseLine for CGact {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("+CGACT:").ok_or(AtParseErr::Mismatch)?;
        let mut cgact = Self {
            cid: 0,
            state: CnactMode::Deactive,
        };
        for (i, part) in line.split(',').enumerate() {
            let part = part.trim();
            match i {
                0 => cgact.cid = u8::from_str(part).map_err(|_| "invalid value")?,
                1 => cgact.state = CnactMode::from_str(part).map_err(|_| "invalid value")?,
                _ => {}
            }
        }
        Ok(cgact)
    }
}
