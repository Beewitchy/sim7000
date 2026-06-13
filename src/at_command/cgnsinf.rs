use crate::util::collect_array;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode, cclk};

use embassy_time::Instant;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GnssReport {
    NotEnabled,
    NoFix { satellites_in_view: Option<u8> },
    Fix { nav_info: NavInfo, instant: Instant },
}

/// AT+CGNSINF response
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
    pub speed_over_ground: Option<f32>,
    pub course_over_ground: Option<f32>,
    pub fix_mode: u32,
    pub hdop: Option<f32>,
    pub pdop: Option<f32>,
    pub vdop: Option<f32>,
    pub hpa: Option<f32>,
    pub vpa: Option<f32>,
    pub satellites_in_view: u8,
}

/// AT+CGNSINF
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
        Self::from_line_timestamped(line, Instant::now())
    }
    fn from_line_timestamped(line: &str, instant: Instant) -> Result<Self, AtParseErr> {
        // <GNSS run status>,<Fix status>,<UTC date & Time>,
        // <Latitude>,<Longitude>,<MSL Altitude>,
        // <Speed Over Ground>,<Course Over Ground>,<Fix Mode>,
        // <Reserved1>,<HDOP>,<PDOP>,<VDOP>,<Reserved2>,
        // <GNSS Satellites in View>,<Reserved3>,<HPA>,<VPA>
        // eg:
        // 1,1,20191024051848.000,31.221946,121.355565,3.417,
        // 0.00,,0,,1.4,1.7,0.9,,6,,12.4,12.0
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
            speed_over_ground,
            course_over_ground,
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
        ] = collect_array(line.splitn(18, ',')).ok_or("missing arguments")?;

        if running.trim() == "1" {
            if has_fix.trim() == "1" {
                let utc = cclk::parse_18char_str(utc);
                let latitude = latitude.parse().ok();
                let longitude = longitude.parse().ok();
                let altitude = altitude.parse()?;
                let speed_over_ground = speed_over_ground.parse().ok();
                let course_over_ground = course_over_ground.parse().ok();
                let fix_mode = fix_mode.parse()?;
                let hdop = hdop.parse().ok();
                let pdop = pdop.parse().ok();
                let vdop = vdop.parse().ok();
                let hpa = hpa.parse().ok();
                let vpa = vpa.parse().ok();
                let satellites_in_view = satellites_in_view.parse().unwrap_or_default();
                Ok(Self::Fix {
                    instant,
                    nav_info: NavInfo {
                        utc,
                        latitude,
                        longitude,
                        altitude,
                        speed_over_ground,
                        course_over_ground,
                        fix_mode,
                        hdop,
                        pdop,
                        vdop,
                        hpa,
                        vpa,
                        satellites_in_view,
                    },
                })
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
