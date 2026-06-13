use embassy_time::Instant;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+CCLK
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetTime<Time = UtcTime>(core::marker::PhantomData<Time>);

impl<Time> GetTime<Time> {
    pub const fn new() -> Self {
        Self(core::marker::PhantomData)
    }
}

impl<Time> AtRequest for GetTime<Time>
where
    Time: FromCclkStr,
{
    type Response = (CclkTime<Time>, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CCLK?\r")
    }
}

/// Time returned by +CCLK
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CclkTime<Time = UtcTime> {
    pub time: Time,
    pub instant: Instant,
}

impl<Time> AtParseLine for CclkTime<Time>
where
    Time: FromCclkStr,
{
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        Self::from_line_timestamped(line, Instant::now())
    }

    fn from_line_timestamped(line: &str, instant: Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CCLK:")
            .ok_or("Missing '+CCLK:'")?
            .trim();
        let line = line
            .strip_prefix('"')
            .ok_or("Missing string argument")?
            .strip_suffix('"')
            .ok_or("Missing string argument")?;
        let (time, _) = Time::from_cclk_str(line).ok_or("couldn't parse time")?;
        Ok(CclkTime { time, instant })
    }
}

impl AtResponse for CclkTime<UtcTime> {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::CclkTime(time) => Some(time),
            _ => None,
        }
    }
}

// todo: ellie (18.05.2026) - Stateful request/response handling?
// impl<Time> AtParseResponse for GetCclkTime<Time> {
//     fn parse_response(code: &str) -> Option<Self::Response> {
//         //...
//         None
//     }
// }

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
/// Common type alias for UTC times parsed from responses
pub type UtcTime = chrono::DateTime<chrono::Utc>;
#[cfg(not(feature = "chrono"))]
/// Common type alias for UTC times parsed from responses
pub type UtcTime = super::unsolicited::DateTime;

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
        let (second, s) = s.split_once(&['+', '-'])?;
        let second = second.parse().ok()?;
        Some((
            Self {
                year,
                month,
                day,
                hour,
                minute,
                second,
            },
            s,
        ))
    }
}

#[cfg(feature = "chrono")]
impl FromCclkStr for chrono::DateTime<chrono::Utc> {
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
                    c => c.is_numeric(),
                })
            }) {
                if let Ok(tzoff_quater_hours) = tzoff.parse() {
                    let tzoff_seconds = (15i64 * 60).saturating_mul(tzoff_quater_hours);
                    let _ = parsed.set_offset(tzoff_seconds);
                    let dt = parsed.to_datetime().ok()?.to_utc();
                    return Some((dt, remain));
                }
            }
        }
        // tz parsing failed: just return the naive dt & ignore the rest of the str
        let dt = parsed.to_datetime_with_timezone(&chrono::Utc).ok()?;
        Some((dt, remain))
    }
}

/// Parse a yyyymmddhhmmss.sss format date-time like returned from
/// gnss CGNSINF or UGNSINF requests. Assumed to be utc
pub fn parse_18char_str(s: &str) -> Option<UtcTime> {
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
///
/// The last two parameters (timezone & offset) are ignored
///  since they indicate the local timezone, so they aren't
///  relevant when just the UTC time itself is wanted
pub fn parse_psuttz_time(s: &str) -> Option<UtcTime> {
    #[cfg(feature = "chrono")]
    {
        //year, month, day, hour, min, sec, "time_zone", dst
        use chrono::format::{Item, Numeric, Pad};
        let mut parsed = Default::default();
        chrono::format::parse(
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
                Item::Literal(","),
                Item::Space(" "),
            ]
            .into_iter(),
        )
        .ok()?;
        let _ = parsed.set_offset(0).ok()?; // PSUTTZ times are utc
        parsed.to_datetime_with_timezone(&chrono::Utc).ok()
    }
    #[cfg(not(feature = "chrono"))]
    {
        let (year, s) = s.split_once(',')?;
        let year = year.parse().ok()?;
        let (month, s) = s.split_once(',')?;
        let month = month.parse().ok()?;
        let (day, s) = s.split_once(',')?;
        let day = day.parse().ok()?;
        let (hour, s) = s.split_once(',')?;
        let hour = hour.parse().ok()?;
        let (minute, s) = s.split_once(',')?;
        let minute = minute.parse().ok()?;
        let (second, s) = s.split_once(&',')?;
        let second = second.parse().ok()?;
        super::unsolicited::DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }
}
