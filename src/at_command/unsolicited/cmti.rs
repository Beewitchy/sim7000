use heapless::String;

use crate::at_command::{AtParseErr, AtParseLine};

// stub type
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NewSmsIndex {
    pub memory: String<2>,
    pub index: u8,
}

impl AtParseLine for NewSmsIndex {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let rest = line.strip_prefix("+CMTI:").ok_or(AtParseErr::Mismatch)?;

        let (memory, index) = rest.split_once(',').ok_or("Missing ','")?;

        Ok(Self {
            memory: memory.trim().trim_matches('\"').try_into().unwrap_or_default(),
            index: index.trim().parse()?,
        })
    }
}
