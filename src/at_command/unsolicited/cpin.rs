use crate::at_command::{AtParseErr, AtParseLine, AtResponse, ResponseCode};

/// Indicates SIM status
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CPin {
    NotReady,
    NotInserted,
    Ready
}

impl AtParseLine for CPin {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        if let Some(line) = line.strip_prefix("+CPIN:") {
            let line = line.trim();
            match line {
                "NOT READY" => Ok(Self::NotReady),
                "NOT INSERTED" => Ok(Self::NotInserted),
                "READY" => Ok(Self::Ready),
                _ => Err("unkown CPIN value".into()),
            }
        } else {
            Err(AtParseErr::Mismatch)
        }
    }
}

impl AtResponse for CPin {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::CPin(v) => Some(v),
            _ => None,
        }
    }
}
