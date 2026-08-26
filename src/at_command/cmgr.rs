use heapless::String;

use super::{AtParseErr, AtParseLine, AtRequest, CommandGroup, GenericOk, RequestType, SimResult};

/// AT+CMGR=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ReadSms {
    pub index: u8,
}

impl AtRequest for ReadSms {
    type Response = SimResult<GenericOk<(SmsHeader, SmsMessage)>>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CMGR={}", self.index)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SmsHeader {
    pub sender: String<20>,
}

impl AtParseLine for SmsHeader {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let rest = line
            .strip_prefix("+CMGR:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();

        let (_status, rest) = rest.split_once(',').ok_or("Missing ','")?;
        let (sender, _) = rest.split_once(',').ok_or("Missing ','")?;

        Ok(Self {
            sender: sender.trim_matches('\"').try_into().unwrap_or_default(),
        })
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SmsMessage {
    pub message: String<160>,
}

impl AtParseLine for SmsMessage {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        Ok(Self {
            message: line.try_into().map_err(|_| "Message too long")?,
        })
    }
}
