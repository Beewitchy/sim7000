use crate::at_command::{AtParseErr, AtParseLine};

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GprsDisconnected;

impl AtParseLine for GprsDisconnected {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        line.eq("+PDP: DEACT")
            .then(|| GprsDisconnected)
            .ok_or(AtParseErr::Mismatch)
    }
}
