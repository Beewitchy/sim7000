use crate::at_command::{AtParseErr, AtParseLine};

/// Daylight savings time
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Dst {
    pub dst_quater_hours: u8
}

impl AtParseLine for Dst {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("DST:").ok_or(AtParseErr::Mismatch)?;
        let dst_hours: u8 = line.trim().parse().map_err(|_| "Invalid character")?;
        let dst_quater_hours = dst_hours * 4;
        Ok(Self {
            dst_quater_hours
        })
    }
}
