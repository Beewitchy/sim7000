use crate::util::collect_array;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum WorkMode {
    Stop = 0,
    Start = 1,
    StartOutsideUs = 2,
}

impl core::str::FromStr for WorkMode {
    type Err = AtParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" => Ok(WorkMode::Stop),
            "1" => Ok(WorkMode::Start),
            "2" => Ok(WorkMode::StartOutsideUs),
            _ => Err("Unknown value".into()),
        }
    }
}

/// AT+CGNSMOD=GLONASS, BEIDOU, GALILIEAN, [qzss] (if supported)
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssWorkModeSet {
    pub glonass: WorkMode,
    pub beidou: WorkMode,
    pub galilean: WorkMode,
    /// Only on supported devices
    pub qzss: Option<WorkMode>,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetGnssWorkModeSet(pub Option<GnssWorkModeSet>);

/// AT+CGNSMOD?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetGnssWorkModeSet;

impl AtRequest for SetGnssWorkModeSet {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if let Some(set) = self.0 {
            write!(
                buf,
                "AT+CGNSMOD=1,{},{},{}",
                set.glonass as u8, set.beidou as u8, set.galilean as u8
            )?;
            if let Some(qzss) = set.qzss {
                write!(buf, ",{}", qzss as u8)?;
            }
            write!(buf, "\r")
        } else {
            write!(buf, "AT+CGNSMOD=0,0,0,0,0\r")
        }
    }
}

impl AtRequest for GetGnssWorkModeSet {
    type Response = (GnssWorkModeSet, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGNSMOD?\r")
    }
}

impl AtParseLine for Option<GnssWorkModeSet> {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CGNSMOD:")
            .ok_or("Missing prefix")?
            .trim_start();
        let [gps, glonass, beidou, galilean, qzss] =
            collect_array(line.splitn(5, ',')).ok_or("missing arguments")?;

        let glonass = glonass.parse()?;
        let beidou = beidou.parse()?;
        let galilean = galilean.parse()?;
        let qzss = qzss.parse().ok();

        if gps == "1" {
            Ok(Some(GnssWorkModeSet {
                glonass,
                beidou,
                galilean,
                qzss,
            }))
        } else {
            Ok(None)
        }
    }
}

impl AtParseLine for GnssWorkModeSet {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        Option::<GnssWorkModeSet>::from_line(line).and_then(|set| set.ok_or("GPS disabled: match against Option<GnssWorkModeSet> instead to collect this result".into()))
    }
}

impl AtResponse for GnssWorkModeSet {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::GnssWorkModeSet(v) => Some(v),
            _ => None,
        }
    }
}
