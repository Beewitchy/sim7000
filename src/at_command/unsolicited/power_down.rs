use crate::at_command::{AtParseErr, AtParseLine, AtResponse, ResponseCode};

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

impl AtResponse for PowerDown {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::PowerDown(v) => Some(v),
            _ => None,
        }
    }
}

impl AtParseLine for PowerDown {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        // example: `NORMAL POWER DOWN`
        let reason = line
            .trim_end()
            .strip_suffix("POWER DOWN")
            .ok_or(AtParseErr::Mismatch)?
            .trim_end();

        match reason {
            "NORMAL" => Ok(PowerDown::Normal),
            "UNDER-VOLTAGE" => Ok(PowerDown::UnderVoltage),
            "OVER-VOLTAGE" => Ok(PowerDown::OverVoltage),
            _ => Err("Invalid power down reason".into()),
        }
    }
}
