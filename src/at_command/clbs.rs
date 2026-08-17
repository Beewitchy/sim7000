use embassy_time::Instant;

use crate::util::collect_array;

use super::{
    AtParseErr, AtParseLine, AtRequest, AtResponse, CommandGroup, GenericOk, RequestType,
    ResponseCode,
    cclk::{FromCclkStr as _, types::UtcDateTime},
};

/// AT+CLBS Base-station Location
///
/// Accesses a web service provided by SIMCOM to
/// to determine a location for the receiver based
/// on the connected cell base station info.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BaseStationLocation {
    CellInfo { cid: u8 },
    RetrieveAll { cid: u8 },
}

impl AtRequest for BaseStationLocation {
    type Response = (LocationInfoResult, GenericOk);
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        match self {
            BaseStationLocation::CellInfo { cid } => write!(buf, "+CLBS=1,{}", cid),
            BaseStationLocation::RetrieveAll { cid } => write!(buf, "+CLBS=4,{}", cid),
        }
    }
}

/// Error codes reported in reponse to
/// regular queries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ErrorCode {
    LocationFailed = 1,
    TimeOut = 2,
    NetError = 3,
    DnsError = 4,
    ServiceOverdue = 5,
    AuthenticateFailed = 6,
    OtherError = 7,
}

/// Error codes used when interracting
/// with the paid service.
/// This is for the paid service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ReportErrorCode {
    /// Error in lon/lat reported to server.
    ParameterError = 81,
    /// Failed to report lon/lat to server.
    ServerFailed = 82,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LocationInfo {
    pub latitude: f32,
    pub longitude: f32,
    pub accuracy: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LocationInfoResult {
    Location(LocationInfo),
    Full {
        location: LocationInfo,
        date_time: UtcDateTime,
        instant: Instant,
    },
    ReportOk,
    Error(ErrorCode),
    ReportError(ReportErrorCode),
}

impl AtParseLine for LocationInfoResult {
    fn from_line(line: &str, instant: &Instant) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("+CLBS:").ok_or(AtParseErr::Mismatch)?;
        let (result_code, rest) = line.split_once(',').ok_or("Missing ','")?;

        enum ResultCode {
            Success,
            Error(ErrorCode),
            ReportSuccess,
            ReportError(ReportErrorCode),
        }
        let result_code = match result_code.trim() {
            "0" => ResultCode::Success,
            "1" => ResultCode::Error(ErrorCode::LocationFailed),
            "2" => ResultCode::Error(ErrorCode::TimeOut),
            "3" => ResultCode::Error(ErrorCode::NetError),
            "4" => ResultCode::Error(ErrorCode::DnsError),
            "5" => ResultCode::Error(ErrorCode::ServiceOverdue),
            "6" => ResultCode::Error(ErrorCode::AuthenticateFailed),
            "7" => ResultCode::Error(ErrorCode::OtherError),
            "80" => ResultCode::ReportSuccess,
            "81" => ResultCode::ReportError(ReportErrorCode::ParameterError),
            "82" => ResultCode::ReportError(ReportErrorCode::ServerFailed),
            _ => return Err("Failed to parse result code".into()),
        };

        match result_code {
            ResultCode::Error(error_code) => return Ok(LocationInfoResult::Error(error_code)),
            ResultCode::ReportError(error_code) => {
                return Ok(LocationInfoResult::ReportError(error_code));
            }
            ResultCode::Success => {
                // todo: ellie (17.08.2026) - Check that long/latitude are in the correct order here: the doc says they are <long>,<lat>, but CGNSINF has them in the opposite order to this response so i'm not sure if this is correct
                let [longitude, latitude, accuracy, rest] =
                    collect_array(rest.splitn(4, ',')).ok_or("Missing arguments")?;
                let location = LocationInfo {
                    latitude: latitude.parse()?,
                    longitude: longitude.parse()?,
                    accuracy: accuracy.parse()?,
                };
                if let Some((date_time, rest)) = UtcDateTime::from_cclk_str(rest).ok() {
                    Ok(LocationInfoResult::Full {
                        location,
                        date_time,
                        instant: *instant,
                    })
                } else {
                    Ok(LocationInfoResult::Location(location))
                }
            }
            ResultCode::ReportSuccess => {
                // This isn't included in the docs I have
                Err("The paid service isn't supported by this crate yet.".into())
            }
        }
    }
}

impl AtResponse for LocationInfoResult {
    #[cfg(any(feature = "log", feature = "defmt"))]
    const RESPONSE_KIND: super::ResponseCodeKind = super::ResponseCodeKind::BaseStationLocation;
    #[inline]
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::BaseStationLocation(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn parse_cpsi() {
        let now = embassy_time::Instant::now();
        assert_eq!(
            LocationInfoResult::from_line("+CLBS: 1", &now).ok(),
            Some(LocationInfoResult::Error(ErrorCode::LocationFailed))
        );
        assert_eq!(
            LocationInfoResult::from_line("+CLBS: 0,0.004321,0.0000,000", &now).unwrap(),
            LocationInfoResult::Location(LocationInfo {
                latitude: 0.004321,
                longitude: 0.0,
                accuracy: 0
            })
        );
        assert_eq!(
            LocationInfoResult::from_line("+CLBS: 0,40.004321,90.0000,5,2026/5/5,10:10:10", &now)
                .unwrap(),
            LocationInfoResult::Full {
                location: LocationInfo {
                    latitude: 40.004321,
                    longitude: 90.0,
                    accuracy: 5
                },
                date_time: UtcDateTime::from_cclk_str("2026/5/5,10:10:10")
                    .expect("simple cclk date-time should parse")
                    .0,
                instant: now
            }
        );
    }
}
