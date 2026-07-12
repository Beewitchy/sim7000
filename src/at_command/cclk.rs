use embassy_time::Instant;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+CCLK
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetTime;

impl GetTime {
    pub const fn new() -> Self {
        Self
    }
}

impl AtRequest for GetTime {
    type Response = (CclkTime, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CCLK?\r")
    }
}

/// Time returned by +CCLK
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CclkTime {
    pub time: types::LocalDateTime,
    pub instant: Instant,
}

impl AtParseLine for CclkTime {
    fn from_line(line: &str, instant: &Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CCLK:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();
        let line = line
            .strip_circumfix('"', '"')
            .ok_or("Missing string argument")?;
        let (time, _) = types::LocalDateTime::from_cclk_str(line).ok_or("couldn't parse time")?;
        Ok(CclkTime { time, instant: *instant })
    }
}

impl AtResponse for CclkTime {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::CclkTime(time) => Some(time),
            _ => None,
        }
    }
}

/// Parse a 80/01/06,00:37:28+00 type local date-time
/// (as returned from CCLK)
#[cfg(feature = "defmt")]
pub trait FromCclkStr: core::fmt::Debug + defmt::Format + Sized {
    /// Try to parse a cclk time from the given string
    /// Should return the paresed result & the remaining unparsed string
    fn from_cclk_str(s: &str) -> Option<(Self, &str)>;
}
/// Parse a 80/01/06,00:37:28+00 type local date-time
/// (as returned from CCLK)
#[cfg(not(feature = "defmt"))]
pub trait FromCclkStr: core::fmt::Debug + Sized {
    /// Try to parse a cclk time from the given string
    /// Should return the paresed result & the remaining unparsed string
    fn from_cclk_str(s: &str) -> Option<(Self, &str)>;
}

#[cfg(feature = "chrono")]
pub mod types {
    /// Common type alias for UTC times parsed from responses
    pub type UtcDateTime = chrono::DateTime<chrono::Utc>;
    /// Common type alias for local tz times parsed from responses
    pub type LocalDateTime = chrono::DateTime<chrono::FixedOffset>;
    /// Common type alias for timezone offset parsed from responses
    pub type LocalTimeOffset = chrono::FixedOffset;
}
#[cfg(not(feature = "chrono"))]
pub mod types {
    /// Common type alias for UTC times parsed from responses
    pub type UtcDateTime = super::unsolicited::DateTime;
    /// Common type alias for local tz times parsed from responses
    pub type LocalDateTime = super::unsolicited::DateTime;
    /// Common type alias for timezone offset parsed from responses
    pub type LocalTimeOffset = i8;
}

impl FromCclkStr for super::unsolicited::DateTime {
    fn from_cclk_str(s: &str) -> Option<(Self, &str)> {
        let (year, s) = s.split_once('/')?;
        let year = year.parse().ok()?;
        let (month, s) = s.split_once('/')?;
        let month = month.parse().ok()?;
        let (day, s) = s.split_once(',')?;
        let day = day.parse().ok()?;
        let (hour, s) = s.split_once(':')?;
        let hour = hour.parse().ok()?;
        let (minute, s) = s.split_once(':')?;
        let minute = minute.parse().ok()?;
        let (second, s) = s.split_once(|c: char| !c.is_digit(10))?;
        let second = second.parse().ok()?;
        let (tz_off, s) = s.split_once(|c| match c {
            '+' => true,
            '-' => true,
            _ => c.is_digit(10)
        })?;
        let tz_off = tz_off.parse().unwrap_or_default();
        Some((
            Self {
                year,
                month,
                day,
                hour,
                minute,
                second,
                tz_off,
            },
            s,
        ))
    }
}

#[cfg(feature = "chrono")]
impl FromCclkStr for chrono::DateTime<chrono::FixedOffset> {
    fn from_cclk_str(s: &str) -> Option<(Self, &str)> {
        use chrono::format::{Item, Numeric, Pad};
        let mut parsed = Default::default();
        let remain = chrono::format::parse_and_remainder(
            &mut parsed,
            s,
            [
                Item::Numeric(Numeric::Year, Pad::None),
                Item::Literal("/"),
                Item::Numeric(Numeric::Month, Pad::None),
                Item::Literal("/"),
                Item::Numeric(Numeric::Day, Pad::None),
                Item::Literal(","),
                Item::Numeric(Numeric::Hour, Pad::None),
                Item::Literal(":"),
                Item::Numeric(Numeric::Minute, Pad::None),
                Item::Literal(":"),
                Item::Numeric(Numeric::Second, Pad::None),
            ]
            .into_iter(),
        )
        .ok()?;
        if remain.starts_with(&['+', '-']) {
            if let Some((tzoff, remain)) = remain.split_once(|c: char| {
                !(match c {
                    '+' => true,
                    '-' => true,
                    c => c.is_digit(10),
                })
            }) {
                if let Ok(tzoff_quater_hours) = tzoff.parse() {
                    let tzoff_seconds = (15i64 * 60).saturating_mul(tzoff_quater_hours);
                    let _ = parsed.set_offset(tzoff_seconds);
                    let dt_local = parsed.to_datetime().ok()?;
                    return Some((dt_local, remain));
                }
            }
        }
        // tz parsing failed: just return the naive dt
        let dt = parsed.to_datetime().ok()?;
        Some((dt, remain))
    }
}

/// Parse a yyyymmddhhmmss.sss format date-time like returned from
/// gnss CGNSINF or UGNSINF requests. Assumed to be utc
pub fn parse_18char_str(s: &str) -> Option<types::UtcDateTime> {
    #[cfg(feature = "chrono")]
    {
        use chrono::format::{Fixed, Item, Numeric, Pad};
        let mut parsed = Default::default();
        let _ = chrono::format::parse(
            &mut parsed,
            s,
            [
                Item::Numeric(Numeric::Year, Pad::None),
                Item::Numeric(Numeric::Month, Pad::None),
                Item::Numeric(Numeric::Day, Pad::None),
                Item::Numeric(Numeric::Hour, Pad::None),
                Item::Numeric(Numeric::Minute, Pad::None),
                Item::Numeric(Numeric::Second, Pad::None),
                Item::Fixed(Fixed::Nanosecond3),
            ]
            .into_iter(),
        );
        parsed.to_datetime_with_timezone(&chrono::Utc).ok()
    }
    #[cfg(not(feature = "chrono"))]
    super::unsolicited::DateTime::new(s)
}

/// Parse utc date-time from *PSUTTZ message data, like
/// 'year,mo,da,hr,mn,sc,"timezone",to'
pub fn parse_psuttz_time(s: &str) -> Option<(types::UtcDateTime, Option<types::LocalTimeOffset>)> {
    #[cfg(feature = "chrono")]
    {
        //year, month, day, hour, min, sec, "time_zone", dst
        use chrono::format::{Item, Numeric, Pad};
        let mut parsed = Default::default();
        let remainder = if !s.contains("/") {
            chrono::format::parse_and_remainder(
                &mut parsed,
                s,
                [
                    Item::Numeric(Numeric::Year, Pad::None),
                    Item::Literal(","),
                    Item::Space(" "),
                    Item::Numeric(Numeric::Month, Pad::None),
                    Item::Literal(","),
                    Item::Space(" "),
                    Item::Numeric(Numeric::Day, Pad::None),
                    Item::Literal(","),
                    Item::Space(" "),
                    Item::Numeric(Numeric::Hour, Pad::None),
                    Item::Literal(","),
                    Item::Space(" "),
                    Item::Numeric(Numeric::Minute, Pad::None),
                    Item::Literal(","),
                    Item::Space(" "),
                    Item::Numeric(Numeric::Second, Pad::None),
                ]
                .into_iter(),
            )
            .ok()?
        } else {
            chrono::format::parse_and_remainder(
                &mut parsed,
                s,
                [
                    Item::Numeric(Numeric::Year, Pad::None),
                    Item::Literal("/"),
                    Item::Numeric(Numeric::Month, Pad::None),
                    Item::Literal("/"),
                    Item::Numeric(Numeric::Day, Pad::None),
                    Item::Literal(","),
                    Item::Space(" "),
                    Item::Numeric(Numeric::Hour, Pad::None),
                    Item::Literal(":"),
                    Item::Numeric(Numeric::Minute, Pad::None),
                    Item::Literal(":"),
                    Item::Numeric(Numeric::Second, Pad::None),
                ]
                .into_iter(),
            )
            .ok()?
        };
        let _ = parsed.set_offset(0).ok()?; // PSUTTZ times are utc
        let dt = parsed.to_datetime_with_timezone(&chrono::Utc).ok()?;
        let (timezone_args, _remainder) = remainder.split_once(',')?;
        let tz_offset = parse_timezone(timezone_args);
        Some((dt, tz_offset))
    }
    #[cfg(not(feature = "chrono"))]
    {
        let (date_delim, time_delim) = if s.contains("/") {
            ('/', ':')
        } else {
            (',', ',')
        };
        let (year, s) = s.split_once(date_delim)?;
        let year = year.parse().ok()?;
        let (month, s) = s.split_once(date_delim)?;
        let month = month.parse().ok()?;
        let (day, s) = s.split_once(',')?;
        let day = day.parse().ok()?;
        let (hour, s) = s.split_once(time_delim)?;
        let hour = hour.parse().ok()?;
        let (minute, s) = s.split_once(time_delim)?;
        let minute = minute.parse().ok()?;
        let (second, s) = s.split_once(&',')?;
        let second = second.parse().ok()?;
        Some((
            super::unsolicited::DateTime {
                year,
                month,
                day,
                hour,
                minute,
                second,
            },
            parse_timezone(s)
        ))
    }
}

/// Parse the quoted "timezone" argument format used by *PSUTTZ and +CTZV message data.
/// Will also try to parse a dst value if there is any remaining arguments in the string
pub fn parse_timezone(s: &str) -> Option<types::LocalTimeOffset> {
    #[cfg(feature = "chrono")]
    {
        let (timezone, remain) = if let Some((timezone, remain)) = s.split_once(',') {
            (timezone, remain)
        } else {
            (s, "")
        };
        let timezone = timezone.strip_circumfix('"', '"').unwrap_or(timezone);
        let (tzoff, _) = timezone.split_once(|c: char| {
            !(match c {
                '+' => true,
                '-' => true,
                c => c.is_digit(10),
            })
        })?;
        let dst: i32 = remain.parse().unwrap_or(0);
        let dst_quater_hours = dst.checked_mul(4)?;
        let tzoff_quater_hours: i32 = tzoff.parse().ok()?;
        let tzoff_seconds = (15i32 * 60).checked_mul(tzoff_quater_hours.checked_add(dst_quater_hours)?)?;
        chrono::FixedOffset::east_opt(tzoff_seconds)
    }
    #[cfg(not(feature = "chrono"))]
    {
        None
    }
}

/// Parse utc date and time arguments from +SGNSCMD message data, like
/// 'yyyy-mm-dd,hh:mm:ss'.
pub fn parse_sgnscmd_time(date: Option<&str>, time: &str, timestamp: &str) -> Option<types::UtcDateTime> {
    #[cfg(feature = "chrono")]
    {
        use chrono::format::{Item, Numeric, Pad};
        let mut parsed = Default::default();
        if let Some(date) = date {
            let _ = chrono::format::parse_and_remainder(
                &mut parsed,
                date,
                [
                    Item::Numeric(Numeric::Year, Pad::Zero),
                    Item::Literal("-"),
                    Item::Numeric(Numeric::Month, Pad::Zero),
                    Item::Literal("-"),
                    Item::Numeric(Numeric::Day, Pad::Zero),
                    Item::Literal(","),
                ]
                .into_iter(),
            )
            .ok()?;
        }
        let _ = chrono::format::parse_and_remainder(
            &mut parsed,
            time,
            [
                Item::Numeric(Numeric::Hour, Pad::Zero),
                Item::Literal(":"),
                Item::Numeric(Numeric::Minute, Pad::Zero),
                Item::Literal(":"),
                Item::Numeric(Numeric::Second, Pad::Zero),
            ]
            .into_iter(),
        )
        .ok()?;
        let timestamp_millis = if let Some(timestamp) = timestamp.strip_prefix("0x") {
            i64::from_str_radix(timestamp, 16).ok()?
        } else {
            timestamp.parse().ok()?
        };
        let timestamp = timestamp_millis / 1000;
        let nanosecond = (timestamp_millis % 1000) * 1_000_000;
        let _ = parsed.set_nanosecond(nanosecond).ok()?;
        let _ = parsed.set_timestamp(timestamp).ok()?;
        let dt = parsed.to_datetime_with_timezone(&chrono::Utc).ok()?;
        Some(dt)
    }
    #[cfg(not(feature = "chrono"))]
    {
        let (year, month, day) = if let Some(date) = date {
            let (year, s) = s.split_once('-')?;
            let year = year.parse().ok()?;
            let (month, s) = s.split_once('-')?;
            let month = month.parse().ok()?;
            let (day, s) = s.split_once(',')?;
            let day = day.parse().ok()?;
            (year, month, day)
        } else {
            (1970, 1, 1)
        };
        let (hour, s) = s.split_once(':')?;
        let hour = hour.parse().ok()?;
        let (minute, s) = s.split_once(':')?;
        let minute = minute.parse().ok()?;
        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }
}
