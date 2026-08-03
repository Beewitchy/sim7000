use crate::at_command::{AtParseErr, AtParseLine, cclk};

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Psuttz {
    pub utc_time: cclk::types::UtcDateTime,
    pub tz_offset: Option<cclk::types::LocalTimeOffset>,
    pub instant: embassy_time::Instant,
}

impl Default for Psuttz {
    fn default() -> Self {
        Self {
            utc_time: Default::default(),
            tz_offset: None,
            instant: embassy_time::Instant::MIN,
        }
    }
}

impl AtParseLine for Psuttz {
    fn from_line(line: &str, instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("*PSUTTZ:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();
        let (utc_time, tz_offset) = cclk::parse_psuttz_time(line)?;
        Ok(Self {
            utc_time,
            tz_offset,
            instant: *instant,
        })
    }
}
