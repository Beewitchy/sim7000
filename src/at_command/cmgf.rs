use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+CMGF=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetSmsMessageFormat(pub SmsMessageFormat);

impl AtRequest for SetSmsMessageFormat {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CMGF={}\r", self.0 as u8)
    }
}

/// AT+CMGF?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetSmsMessageFormat;

impl AtRequest for GetSmsMessageFormat {
    type Response = (SmsMessageFormat, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CMGF?\r")
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SmsMessageFormat {
    Pdu = 0,
    Text = 1,
}

impl AtParseLine for SmsMessageFormat {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let (message, rest) = line.split_once(": ").ok_or("Missing ': '")?;

        if message != "+CMGF" {
            return Err("Missing +CMGF prefix".into());
        }

        match rest {
            "0" => Ok(SmsMessageFormat::Pdu),
            "1" => Ok(SmsMessageFormat::Text),
            _ => Err("Invalid SMS message format".into()),
        }
    }
}

impl AtResponse for SmsMessageFormat {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::SmsMessageFormat(format) => Some(format),
            _ => None,
        }
    }
}
