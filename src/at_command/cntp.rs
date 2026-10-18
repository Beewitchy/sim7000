use embassy_time::Instant;
use heapless::String;

use super::{AtParseErr, AtParseLine, AtRequest, CommandGroup, GenericOk, MatchToken, RequestType, SimResult, cclk};

/// AT+CNTP=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SynchronizeNetworkTime {
    pub ntp_server: String<64>,
    pub timezone_quarter_hours: i8,
    pub cid: u8,
}

/// AT+CNTP
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Execute;

impl AtRequest for SynchronizeNetworkTime {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(
            buf,
            "+CNTP=\"{}\",{},{}",
            self.ntp_server.as_str(),
            self.timezone_quarter_hours,
            self.cid
        )
    }
}

impl AtRequest for Execute {
    type ResultType = SimResult<(GenericOk, NetworkTime)>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CNTP")
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_secs(60))
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
    pub time: Option<cclk::types::LocalDateTime>,
    pub instant: Instant,
    pub code: SyncNtpStatusCode,
}

pub struct NetworkTimeToken;

impl MatchToken for NetworkTime {
    type Token = NetworkTimeToken;

    fn match_token(line: &str) -> Option<(Self::Token, &str)> {
        line
            .strip_prefix("+CNTP:")
            .map(|line| (NetworkTimeToken, line))
    }
}

impl AtParseLine for NetworkTime {
    fn from_line(line: &str, instant: &Instant) -> Result<Self, AtParseErr> {
        let line = Self::match_token(line)
            .ok_or(AtParseErr::Mismatch)?
            .1
            .trim();

        use cclk::FromCclkStr as _;

        let (code, time) = match line.split_once(',') {
            Some((code, time)) => (
                code,
                Some(
                    cclk::types::LocalDateTime::from_cclk_str(
                        time.strip_circumfix('"', '"')
                            .ok_or("no quotes around expected time parameter")?,
                    )
                    .map(|(date, _)| date)?,
                ),
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

        Ok(NetworkTime {
            time,
            instant: *instant,
            code,
        })
    }
}

/// AT+CLTS=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct EnableLocalTimestamp(pub bool);

impl AtRequest for EnableLocalTimestamp {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        let param = match self.0 {
            false => '0',
            true => '1',
        };
        write!(buf, "+CLTS={}", param)
    }
    fn default_timeout() -> Option<embassy_time::Duration> {
        Some(embassy_time::Duration::from_secs(30))
    }
}
