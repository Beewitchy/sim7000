use super::{AtRequest, CloseOk};

/// AT+CIPCLOSE=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CloseConnection {
    pub connection: usize,
}

impl AtRequest for CloseConnection {
    type Response = CloseOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CIPCLOSE={}\r", self.connection)
    }
}
