use super::{RequestType, CommandGroup, AtRequest, WritePrompt};

/// AT+CIPSEND
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct IpSend {
    pub connection: usize,
    pub data_length: usize,
}

impl AtRequest for IpSend {
    type Response = WritePrompt;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CIPSEND={},{}", self.connection, self.data_length)
    }
}
