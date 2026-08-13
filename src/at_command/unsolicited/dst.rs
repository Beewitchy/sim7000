use crate::at_command::{AtParseErr, AtParseLine, cclk::OffsetUnit};

/// Daylight savings time
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Dst {
    pub dst_offset: OffsetUnit
}

impl AtParseLine for Dst {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("DST:").ok_or(AtParseErr::Mismatch)?;
        let dst_hours: u8 = line.trim().parse().map_err(|_| "Invalid character")?;
        let dst_offset = OffsetUnit::checked_from_hours_unsigned(dst_hours).ok_or("DST hours value is out of range")?;
        Ok(Self {
            dst_offset
        })
    }
}
