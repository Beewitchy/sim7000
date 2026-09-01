use heapless::String;

use super::{AtParseErr, AtParseLine, AtRequest, CommandGroup, GenericOk, RequestType, SimResult, WritePrompt};

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
    type ResultType = SimResult<WritePrompt>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CMGS=\"{}\"", self.destination)
    }
}

impl AtRequest for SendSmsMessage {
    type ResultType = SimResult<GenericOk<MessageReference>>;
    const TYPE: RequestType = RequestType::NonCommand;
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
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let rest = line
            .strip_prefix("+CMGS:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();

        Ok(Self {
            value: rest.parse().map_err(|_| "Invalid message reference")?,
        })
    }
}
