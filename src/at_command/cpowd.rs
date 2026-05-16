use super::{AtRequest, unsolicited};

/// AT+CPOWD=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PowerDown(pub Mode);

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Mode {
    Urgent,
    Normal,
}

impl AtRequest for PowerDown {
    type Response = unsolicited::PowerDown;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        let arg = match self.0 {
            Mode::Urgent => '0',
            Mode::Normal => '1',
        };
        write!(buf, "AT+CPOWD={arg}\r")
    }
}
