use super::{RequestType, CommandGroup, AtRequest, CloseOk};

/// AT+CIPCLOSE=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CloseConnection {
    pub connection: usize,
}

impl AtRequest for CloseConnection {
    type Response = CloseOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CIPCLOSE={}", self.connection)
    }
}
