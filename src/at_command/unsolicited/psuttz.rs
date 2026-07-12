use crate::at_command::{AtParseErr, AtParseLine, cclk};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Psuttz {
    pub utc_time: cclk::types::UtcDateTime,
    pub tz_offset: Option<cclk::types::LocalTimeOffset>,
}

impl AtParseLine for Psuttz {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("*PSUTTZ:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();
        // I have seen *PSUTTZ responses in cclk format so that is handled too as a fallback
        let (utc_time, tz_offset) =
            cclk::parse_psuttz_time(line).ok_or("couldn't parse datetime arguments")?;
        Ok(Self {
            utc_time,
            tz_offset,
        })
    }
}
