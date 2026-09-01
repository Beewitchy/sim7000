use super::super::{AtParseErr, AtParseLine, MatchToken, ParseResult, SimError};

// stub type
/// Network time zone
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerDown {
    /// Normal power down, triggered by a command or by the power pin.
    Normal,

    /// Chip automatically powered down due to under-voltage
    UnderVoltage,

    /// Chip automatically powered down due to over-voltage
    OverVoltage,
}

impl AtParseLine for PowerDown {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        // example: `NORMAL POWER DOWN`
        let reason = Self::match_token(line)
            .ok_or(AtParseErr::Mismatch)?
            .1;
        Self::parse(Self::Normal, reason)
    }
}

impl PowerDown {
    pub fn parse(_: PowerDown, line: &str) -> Result<Self, AtParseErr> {
        match line.trim_end() {
            "NORMAL" => Ok(PowerDown::Normal),
            "UNDER-VOLTAGE" => Ok(PowerDown::UnderVoltage),
            "OVER-VOLTAGE" => Ok(PowerDown::OverVoltage),
            _ => Err("Invalid power down reason".into()),
        }
    }
}

impl MatchToken for PowerDown {
    type Token = PowerDown;

    fn match_token(line: &str) -> Option<(Self::Token, &str)> {
        let line = line
            .trim_end()
            .strip_suffix("POWER DOWN")?;
        Some((PowerDown::Normal, line))
    }
}

impl ParseResult for Result<PowerDown, SimError> {
    type Response = ();
    type Output = PowerDown;
    type Done = PowerDown;
    type Error = SimError;

    fn parse_err(
        error_token: <Self::Error as MatchToken>::Token,
        line: &str,
    ) -> Result<Self::Error, AtParseErr> {
        Ok(SimError::Command)
    }

    fn compose_result(
        (): Self::Response,
        done_token: <Self::Done as MatchToken>::Token,
        line: &str,
    ) -> Result<Self::Output, (Self::Response, AtParseErr)> {
        PowerDown::parse(done_token, line).map_err(|err| ((), err))
    }
}