use core::fmt::Write;
use heapless::String;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode, WritePrompt};

/// AT+CMGS=...
///
/// This has to be sent before sending the message [SendSmsMessage]. Likewise, the [SendSmsMessage] has to be sent directly after this.
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SendSms {
    pub destination: String<20>,
}

/// *IMPORTANT*: This has to be sent directly after [SendSms]
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SendSmsMessage(pub String<160>);

impl AtRequest for SendSms {
    type Response = WritePrompt;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CMGS=\"{}\"\r", self.destination)
    }
}

impl AtRequest for SendSmsMessage {
    type Response = (MessageReference, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "{}\x1A", self.0)
    }
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct MessageReference {
    pub value: u32,
}

impl AtParseLine for MessageReference {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let (message, rest) = line.split_once(": ").ok_or("Missing ': '")?;

        if message != "+CMGS" {
            return Err("Missing +CMGS prefix".into());
        }

        Ok(Self {
            value: rest.parse().map_err(|_| "Invalid message reference")?,
        })
    }
}

impl AtResponse for MessageReference {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::MessageReference(format) => Some(format),
            _ => None,
        }
    }
}
