use core::str::FromStr as _;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, CnactMode, GenericOk, ResponseCode, Seq};


/// AT+CGACT?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetPdpContextActivation;

impl AtRequest for GetPdpContextActivation {
    type Response = Seq<CGact, 4, GenericOk>;

    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGACT?\r")
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CGact {
    pub cid: u8,
    pub state: CnactMode,
}

impl AtRequest for CGact {
    type Response = (heapless::Vec<CGact, 4>, GenericOk);

    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGACT={},{}\r", self.cid, self.state as u8)
    }
}

impl AtParseLine for CGact {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("+CGACT:").ok_or_else(|| AtParseErr::from("no match"))?;
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

impl AtResponse for CGact {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::PdpContextActivation(v) => Some(v),
            _ => None,
        }
    }
}
