use heapless::String;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+CMGR=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ReadSms {
    pub index: u8,
}

impl AtRequest for ReadSms {
    type Response = (SmsMessage, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CMGR={}\r", self.index)
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SmsMessage {
    pub sender: String<20>,
    pub message: String<160>,
}

impl AtParseLine for SmsMessage {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let rest = line
            .strip_prefix("+CMGR:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();

        let (_status, rest) = rest.split_once(',').ok_or("Missing ','")?;
        let (sender, _) = rest.split_once(',').ok_or("Missing ','")?;

        Ok(Self {
            sender: sender.trim_matches('\"').try_into().unwrap_or_default(),
            message: "".try_into().unwrap_or_default(),
        })
    }
}

impl AtResponse for SmsMessage {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::SmsMessage(sms) => Some(sms),
            _ => None,
        }
    }
}
