use heapless::String;
use embassy_time::Instant;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode, cclk};

/// AT+CNTP=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SynchronizeNetworkTime {
    pub ntp_server: String<64>,
    pub timezone: u16,
    pub cid: u8,
}

/// AT+CNTP
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Execute;

impl AtRequest for SynchronizeNetworkTime {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(
            buf,
            "AT+CNTP=\"{}\",{},{}\r",
            self.ntp_server.as_str(),
            self.timezone,
            self.cid
        )
    }
}

impl AtRequest for Execute {
    type Response = (GenericOk, NetworkTime);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNTP\r")
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SyncNtpStatusCode {
    Success = 1,
    NetworkError = 61,
    DnsResolutionError = 62,
    ConnectionError = 63,
    ServiceResponseError = 64,
    ServiceResponseTimeout = 65,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NetworkTime {
    #[cfg(feature = "chrono")]
    pub time: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg(not(feature = "chrono"))]
    pub time: Option<super::unsolicited::DateTime>,
    pub instant: Instant,
    pub code: SyncNtpStatusCode,
}

impl AtParseLine for NetworkTime {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        Self::from_line_timestamped(line, Instant::now())
    }

    fn from_line_timestamped(line: &str, instant: Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CNTP:")
            .ok_or("Missing '+CNTP:'")?
            .trim();

        use cclk::FromCclkStr as _;

        let (code, time) = match line.split_once(',') {
            Some((code, time)) => (
                code,
                chrono::DateTime::<chrono::Utc>::from_cclk_str(
                    time.strip_circumfix('"', '"')
                        .ok_or("no quotes around expected time parameter")?,
                )
                .map(|(date, _)| date),
            ),
            None => (line, None),
        };

        let code = match code {
            "1" => SyncNtpStatusCode::Success,
            "61" => SyncNtpStatusCode::NetworkError,
            "62" => SyncNtpStatusCode::DnsResolutionError,
            "63" => SyncNtpStatusCode::ConnectionError,
            "64" => SyncNtpStatusCode::ServiceResponseError,
            "65" => SyncNtpStatusCode::ServiceResponseTimeout,
            _ => return Err("Unexpected response".into()),
        };

        Ok(NetworkTime { time, instant, code })
    }
}

impl AtResponse for NetworkTime {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::NetworkTime(v) => Some(v),
            _ => None,
        }
    }
}

/// AT+CLTS=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EnableLocalTimestamp(pub bool);

impl AtRequest for EnableLocalTimestamp {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        let param = match self.0 {
            false => '0',
            true => '1',
        };
        write!(buf, "AT+CLTS={}\r", param)
    }
}
