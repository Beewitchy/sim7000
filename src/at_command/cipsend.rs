use core::fmt::Write;
use heapless::String;

use super::{AtRequest, WritePrompt};

/// AT+CIPSEND
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IpSend {
    pub connection: usize,
    pub data_length: usize,
}

impl AtRequest for IpSend {
    type Response = WritePrompt;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CIPSEND={},{}\r", self.connection, self.data_length)
    }
}
