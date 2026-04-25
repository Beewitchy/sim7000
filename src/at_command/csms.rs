use core::fmt::Write;
use heapless::String;

use super::{AtRequest, GenericOk};

/// AT+CSMS=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SelectMessageService;

impl AtRequest for SelectMessageService {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CSMS=0\r")
    }
}
