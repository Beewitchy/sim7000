use crate::at_command::{AtParseErr, AtParseLine, cclk};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Psuttz(pub cclk::UtcTime);

impl AtParseLine for Psuttz {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("*PSUTTZ:")
            .ok_or("missing prefix")?
            .trim();
        Ok(Self(cclk::parse_psuttz_time(line).ok_or("couldn't parse datetime arguments")?))
    }
}
