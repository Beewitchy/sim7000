use crate::at_command::{AtParseErr, AtParseLine, cclk};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Psuttz {
    pub utc_time: cclk::types::UtcDateTime,
    pub tz_offset: Option<cclk::types::LocalTimeOffset>,
}

impl AtParseLine for Psuttz {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("*PSUTTZ:")
            .ok_or("missing prefix")?
            .trim();
        // I have seen *PSUTTZ responses in cclk format so that is handled too as a fallback
        let (utc_time, tz_offset) = cclk::parse_psuttz_time(line)
            .or_else(|| {
                <cclk::types::LocalDateTime as cclk::FromCclkStr>::from_cclk_str(line).map(
                    |(time, _rem)| {
                        let utc_time = time.to_utc();
                        let tz_off = time.timezone();
                        (utc_time, Some(tz_off))
                    },
                )
            })
            .ok_or("couldn't parse datetime arguments")?;
        Ok(Self {
            utc_time,
            tz_offset,
        })
    }
}
