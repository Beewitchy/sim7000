use crate::at_command::{AtParseErr, AtParseLine};

/// The modem is receiving data on a connection. It will transmit `length` bytes right after this header.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ReceiveHeader {
    pub connection: usize,
    pub length: usize,
}

impl AtParseLine for ReceiveHeader {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let (_, rest) = line
            .strip_prefix("+RECEIVE")
            .ok_or(AtParseErr::Mismatch)?
            .split_once(',')
            .ok_or(AtParseErr::Mismatch)?;

        let (connection, length) = rest
            .trim_end_matches(':')
            .split_once(',')
            .ok_or("Missing second ','")?;

        Ok(ReceiveHeader {
            connection: connection.parse()?,
            length: length.parse()?,
        })
    }
}
