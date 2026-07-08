#[cfg(feature = "defmt")]
use defmt::bitflags;
#[cfg(not(feature = "defmt"))]
use bitflags::bitflags;
use embassy_time::Duration;

use super::{AtParseErr, AtParseLine, AtRequest, GenericOk, SimError};
use crate::collect_array;

/// Options for NMEA output
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum NmeaOutputPort {
    Off = 0,
    Usb = 1,
    Uart(UartBaudRate) = 2,
}

impl NmeaOutputPort {
    pub const fn id(&self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Usb => 1,
            Self::Uart(_) => 2,
        }
    }
}

/// Baud rate when using UART output port
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum UartBaudRate {
    _9600,
    _19200,
    _38400,
    _57600,
    _115200,
}

impl UartBaudRate {
    pub const fn into_hz(self) -> u32 {
        match self {
            Self::_9600 => 9600,
            Self::_19200 => 19200,
            Self::_38400 => 38400,
            Self::_57600 => 57600,
            Self::_115200 => 115200,
        }
    }
}

bitflags! {
    /// Config flags for NmeaType config variable. Names refer to
    /// NMEA message identifiers.
    #[cfg_attr(not(feature = "defmt"), derive(Debug))]
    #[repr(transparent)]
    pub struct NmeaType: u8 {
        /// GPGSV: GPS satellites in view.
        const GPGSV = 0b0000_0001;
        /// GLGSV: GLONASS satellites in view (exclusive).
        const GLGSV = 0b0000_0010;
        /// GAGSV: Galileo satellites in view.
        const GAGSV = 0b0000_0100;
        /// BDGSV_QZGSV: Beidou/QZSS satellites in view.
        const BDGSV_QZGSV = 0b0000_1000;
        /// GSA: GNSS DOP, active satellites, and 2D/3D mode.
        const GSA = 0b0001_0000;
        /// VTG: nav course and speed over ground.
        const VTG = 0b0010_0000;
        /// RMC: Recommended minimum specific GPS data (time, 2D position, ground speed and course, date).
        const RMC = 0b0100_0000;
        /// GGA: Global Positioning System Fixed Data (time, 3D position, HDOP).
        const GGA = 0b1000_0000;
    }
}

/// Options for the AdssMode (assistance data) config variable
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum AdssMode {
    /// Do not delete any data. Perform hot start if the conditions are permitted after starting GNSS.
    AttemptHotStart = 0,
    /// Delete some related data. Perform warm start if the conditions are permitted after starting GNSS.
    AttemptWarmStart = 1,
    /// Delete all assistance data except almanac data. Enforce cold start after starting GNSS.
    CleanDataAndForceColdStart = 2,
    /// Delete all assistance data except almanac and sv health data. Enforce xtra cold start after starting GNSS.
    ForceColdStart = 3,
    /// Delete all assistance data. Enforce reset start after starting GNSS.
    CleanDataAndForceColdStartWithReset = 4,
}

/// Options for the Mode config variable
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Mode {
    /// Start GPS and GLONASS constellation.
    GLONASS = 0,
    /// Start GPS and Galileo constellation.
    Galileo = 1,
    /// Start GPS and Beidou constellation.
    Beidou = 2,
    /// Start GPS and QZSS constellation.
    QZSS = 3,
}

/// AT+SGNSCFG=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigureGnss {
    /// +SGNSCFG: "NMEAOUTPORT",<port>[,<baudrate>]
    NmeaOutPort(NmeaOutputPort),
    /// +SGNSCFG: "NMEATYPE",<nmeatype>
    NmeaType(NmeaType),
    /// +SGNSCFG: "OUTURC",<mode>
    /// Enable / disable "URC" (unsolicited) output
    UrcOutputOn(bool),
    /// +SGNSCFG: "ADSS",<mode>
    AssistanceData(AdssMode),
    /// +SGNSCFG: "MODE",<mode>
    Mode(Mode),
    /// +SGNSCFG: "THRESHOLD",<threshold> (meters)
    Threshold(u16),
    /// +SGNSCFG: "TIMEOUT",<timeout>
    Timeout(Duration),
    /// +SGNSCFG: "EXTRAINFO",<flag>
    GetExtraInfo(bool),
}

impl AtRequest for ConfigureGnss {
    type Response = Result<GenericOk, SimError>;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        match self {
            Self::NmeaOutPort(NmeaOutputPort::Uart(baudrate)) => write!(
                buf,
                "AT+SGNSCFG=\"NMEAOUTPORT\",{},{}\r",
                NmeaOutputPort::Uart(UartBaudRate::_9600).id(),
                baudrate.into_hz()
            ),
            Self::NmeaOutPort(port) => write!(buf, "AT+SGNSCFG=\"NMEAOUTPORT\",{}\r", port.id()),
            Self::NmeaType(nmea_type) => {
                write!(buf, "AT+SGNSCFG=\"NMEATYPE\",{}\r", nmea_type.bits())
            }
            Self::UrcOutputOn(false) => write!(buf, "AT+SGNSCFG=\"OUTURC\",0\r"),
            Self::UrcOutputOn(true) => write!(buf, "AT+SGNSCFG=\"OUTURC\",1\r"),
            Self::AssistanceData(adss_mode) => {
                write!(buf, "AT+SGNSCFG=\"ADSS\",{}\r", *adss_mode as u8)
            }
            Self::Mode(mode) => write!(buf, "AT+SGNSCFG=\"MODE\",{}\r", *mode as u8),
            Self::Threshold(threshold) => write!(buf, "AT+SGNSCFG=\"THRESHOLD\",{}\r", *threshold),
            Self::Timeout(duration) => write!(
                buf,
                "AT+SGNSCFG=\"TIMEOUT\",{}\r",
                duration.as_millis().clamp(10_000, 180_000)
            ),
            Self::GetExtraInfo(false) => write!(buf, "AT+SGNSCFG=\"EXTRAINFO\",0\r"),
            Self::GetExtraInfo(true) => write!(buf, "AT+SGNSCFG=\"EXTRAINFO\",1\r"),
        }
    }
}

/// Options for the PowerLevel variable
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum PowerLevel {
    /// Use all technologies available to calculate location.
    #[default]
    All = 0,
    /// Use only low power technologies to calculate location.
    LowPower = 1,
    /// Use only low and medium power technologies to calculate location.
    MediumPower = 2,
}

/// Options for the Accuracy variable
#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Accuracy {
    /// Accuracy is not specified, use default.
    #[default]
    Default = 0,
    /// Low Accuracy for location is acceptable.
    Low = 1,
    /// Medium Accuracy for location is acceptable.
    Medium = 2,
    /// Only High Accuracy for location is acceptable.
    High = 3,
}

/// AT+SGNSCMD=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GnssCommand {
    Off,
    Once(PowerLevel),
    Auto {
        min_interval: Duration,
        /// Setting this to 0 will use only the min_interval
        min_distance_meters: u16,
        accuracy: Accuracy,
    },
}

impl AtRequest for GnssCommand {
    type Response = Result<GenericOk, Error>;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        match self {
            Self::Off => write!(buf, "AT+SGNSCMD=0\r"),
            Self::Once(power_level) => write!(buf, "AT+SGNSCMD=1,{}\r", *power_level as u8),
            Self::Auto {
                min_interval,
                min_distance_meters,
                accuracy,
            } => {
                write!(
                    buf,
                    "AT+SGNSCMD=2,{},{},{}\r",
                    min_interval.as_millis().clamp(1_000, 60_000),
                    (*min_distance_meters).min(1_000),
                    *accuracy as u8
                )
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GnssCommandMode {
    Off = 0,
    Once = 1,
    Auto = 2,
}

/// +SGNSCMD:<mode>[,<date>],<time>[,<total_satellites>],<latitude>,<longitude>,<accuracy>,
/// <altitude>,<altitude_mean_sea_level>,<speed>,<bearing>,<timestamp>,<flags>.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssResult {
    pub mode: GnssCommandMode,
    pub date_time: super::cclk::UtcDateTime,
    pub total_satellites: Option<u32>,
    pub latitude: f32,
    pub longitude: f32,
    pub accuracy: f32,
    pub altitude: f32,
    pub altitude_mean_sea_level: f32,
    pub speed_over_ground: f32,
    pub course_over_ground: f32,
    pub timestamp_millis: u64,
}

impl AtParseLine for GnssResult {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+SGNSCMD:")
            .ok_or("missing prefix")?
            .trim();

        let num_parameters = line.matches(',').count();
        if num_parameters != 11 && num_parameters != 13 {
            return Err("invalid length".into());
        }

        let (mode, date, time, total_satellites, rest) = if num_parameters == 13 {
            let [mode, date, time, total_satellites, rest] =
                collect_array(line.splitn(5, ',')).ok_or("malformed parameters")?;
            let total_satellites = total_satellites.parse()?;
            (mode, Some(date), time, Some(total_satellites), rest)
        } else {
            let [mode, time, rest] =
                collect_array(line.splitn(3, ',')).ok_or("malformed parameters")?;
            (mode, None, time, None, rest)
        };

        let [
            latitude,
            longitude,
            accuracy,
            altitude,
            altitude_mean_sea_level,
            speed,
            bearing,
            timestamp,
            _flags,
        ] = collect_array(rest.split(',')).ok_or("malformed parameters")?;

        let mode = match mode {
            "0" => GnssCommandMode::Off,
            "1" => GnssCommandMode::Once,
            "2" => GnssCommandMode::Auto,
            _ => {
                return Err("invalid mode argument".into());
            }
        };

        let date_time = super::cclk::parse_sgnscmd_time(date, time, timestamp)
            .ok_or("invalid date-time argument")?;


        let latitude = latitude.parse()?;
        let longitude = longitude.parse()?;
        let accuracy = accuracy.parse()?;
        let altitude = altitude.parse()?;
        let altitude_mean_sea_level = altitude_mean_sea_level.parse()?;
        let speed_over_ground = speed.parse()?;
        let course_over_ground = bearing.parse()?;
        let timestamp_millis = if let Some(timestamp) = timestamp.strip_prefix("0x") {
            i64::from_str_radix(timestamp, 16)?
        } else {
            timestamp.parse()?
        }.max(0).cast_unsigned();

        Ok(Self {
            mode,
            date_time,
            total_satellites,
            latitude,
            longitude,
            accuracy,
            altitude,
            altitude_mean_sea_level,
            speed_over_ground,
            course_over_ground,
            timestamp_millis,
        })
    }
}

/// Options for the Accuracy variable
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum Error {
    /// Success.
    Success = 0,
    /// General failure.
    GeneralFailure = 1,
    /// Callback is missing.
    CallbackIsMissing = 2,
    /// Invalid parameter.
    InvalidParameter = 3,
    /// ID already exists.
    IdAlreadyExists = 4,
    /// ID is unknown.
    UnknownId = 5,
    /// Already started.
    AlreadyStarted = 6,
    /// Not initialized.
    NotInitialized = 7,
    /// Maximum number of geofences reached.
    MaxGeofencesExceeded = 8,
    /// Not supported.
    NotSupported = 9,
    /// Timeout when asking single shot.
    SingleShotTimeout = 10,
    /// GNSS engine could not get loaded.
    UnableToLoad = 11,
    /// Location module license is disabled.
    Unlicensed = 12,
    /// Best available position is invalid.
    NoValidPosition = 13,
    UnknownError,
}

impl AtParseLine for Error {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+SGNSERR:")
            .ok_or("missing prefix")?
            .trim();
        let code: u8 = line.parse().map_err(|_| "invalid parameter")?;
        Ok(match code {
            0 => Self::Success,
            1 => Self::GeneralFailure,
            2 => Self::CallbackIsMissing,
            3 => Self::InvalidParameter,
            4 => Self::IdAlreadyExists,
            5 => Self::UnknownId,
            6 => Self::AlreadyStarted,
            7 => Self::NotInitialized,
            8 => Self::MaxGeofencesExceeded,
            9 => Self::NotSupported,
            10 => Self::SingleShotTimeout,
            11 => Self::UnableToLoad,
            12 => Self::Unlicensed,
            13 => Self::NoValidPosition,
            _ => Self::UnknownError,
        })
    }
}
