use crate::at_command::{AtParseErr, AtParseLine, cclk};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Psuttz(pub cclk::UtcDateTime);

impl AtParseLine for Psuttz {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("*PSUTTZ:")
            .ok_or("missing prefix")?
            .trim();
        // I have seen *PSUTTZ responses in cclk format so that is handled too as a fallback
        Ok(Self(
            cclk::parse_psuttz_time(line)
                .or_else(|| cclk::FromCclkStr::from_cclk_str(line).map(|(time, _rem)| time))
                .ok_or("couldn't parse datetime arguments")?,
        ))
    }
}
