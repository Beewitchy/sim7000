use core::str::FromStr as _;

use crate::util::collect_array;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GnssReport {
    NotEnabled,
    NoFix { satellites_in_view: Option<u8> },
    Fix(NavInfo),
}

/// AT+CGNSINF=<GNSS run status>,<Fix status>,<UTC date &
/// Time>,<Latitude>,<Longitude>,<MSL Altitude>,<Speed Over
/// Ground>,<Course Over Ground>,<Fix
/// Mode>,<Reserved1>,<HDOP>,<PDOP>,<VDOP>,<Reserved2>,<G
/// NSS Satellites in View>,<Reserved3>,<HPA>,<VPA>
/// eg:
/// 1,1,20191024051848.000,31.221946,121.355565,3.417,0.00,,0,,1.4,1.7,0.9,,6,,12.4,12.0
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NavInfo {
    #[cfg(feature = "chrono")]
    pub utc: Option<chrono::DateTime<chrono::Utc>>,
    #[cfg(not(feature = "chrono"))]
    pub utc: Option<super::unsolicited::DateTime>,
    pub latitude: Option<f32>,
    pub longitude: Option<f32>,
    pub altitude: f32,
    pub speed: Option<f32>,
    pub course: Option<f32>,
    pub fix_mode: u32,
    pub hdop: Option<f32>,
    pub pdop: Option<f32>,
    pub vdop: Option<f32>,
    pub hpa: Option<f32>,
    pub vpa: Option<f32>,
    pub satellites_in_view: u8,
}

/// AT+CGNSINF?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetGnssReport;

impl AtRequest for GetGnssReport {
    type Response = (GnssReport, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGNSINF\r")
    }
}

impl AtParseLine for GnssReport {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CGNSINF:")
            .ok_or("Missing prefix")?
            .trim_start();
        let [
            running,
            has_fix,
            utc,
            latitude,
            longitude,
            altitude,
            speed,
            course,
            fix_mode,
            _reserved1,
            hdop,
            pdop,
            vdop,
            _reserved2,
            satellites_in_view,
            _reserved3,
            hpa,
            vpa,
        ] = collect_array(line.splitn(5, ',')).ok_or("missing arguments")?;

        if running.trim() == "1" {
            if has_fix.trim() == "1" {
                #[cfg(feature = "chrono")]
                let utc = {
                    use chrono::format::{Fixed, Item, Numeric, Pad};
                    let mut parsed = Default::default();
                    let _ = chrono::format::parse(
                        &mut parsed,
                        utc,
                        [
                            Item::Numeric(Numeric::Year, Pad::None),
                            Item::Numeric(Numeric::Month, Pad::None),
                            Item::Numeric(Numeric::Day, Pad::None),
                            Item::Numeric(Numeric::Hour, Pad::None),
                            Item::Numeric(Numeric::Minute, Pad::None),
                            Item::Numeric(Numeric::Second, Pad::None),
                            Item::Fixed(Fixed::Nanosecond),
                        ]
                        .into_iter(),
                    );
                    let _ = parsed.set_offset(0);
                    parsed.to_datetime_with_timezone(&chrono::Utc).ok()
                };
                #[cfg(not(feature = "chrono"))]
                let utc = super::unsolicited::DateTime::new(utc);
                let latitude = latitude.parse().ok();
                let longitude = longitude.parse().ok();
                let altitude = altitude.parse()?;
                let speed = speed.parse().ok();
                let course = course.parse().ok();
                let fix_mode = fix_mode.parse()?;
                let hdop = hdop.parse().ok();
                let pdop = pdop.parse().ok();
                let vdop = vdop.parse().ok();
                let hpa = hpa.parse().ok();
                let vpa = vpa.parse().ok();
                let satellites_in_view = satellites_in_view.parse().unwrap_or_default();
                Ok(Self::Fix(NavInfo {
                    utc,
                    latitude,
                    longitude,
                    altitude,
                    speed,
                    course,
                    fix_mode,
                    hdop,
                    pdop,
                    vdop,
                    hpa,
                    vpa,
                    satellites_in_view,
                }))
            } else {
                Ok(Self::NoFix {
                    satellites_in_view: satellites_in_view.parse().ok(),
                })
            }
        } else {
            Ok(Self::NotEnabled)
        }
    }
}

impl AtResponse for GnssReport {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::GnssReport(v) => Some(v),
            _ => None,
        }
    }
}
