use crate::at_command::{AtParseErr, AtParseLine};

// stub type
/// Indicates phone functionality
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CFun(pub crate::at_command::cfun::Functionality);

impl AtParseLine for CFun {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("+CFUN:").ok_or("missing prefix")?;
        Ok(Self(line.trim().parse()?))
    }
}
