use heapless::String;

use crate::util::collect_array;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+CCLK
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetTime;

impl AtRequest for GetTime {
    type Response = (CclkTime, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CCLK?\r")
    }
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CclkTime {
    #[cfg(feature = "chrono")]
    pub time: chrono::DateTime<chrono::Utc>,
    #[cfg(not(feature = "chrono"))]
    pub time: heapless::String<32>,
}

impl AtParseLine for CclkTime {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("+CCLK: ").ok_or("Missing '+CCLK: '")?;
        let [ndt, tzoff] = collect_array(line.split_inclusive(&['+', '-'][..])).unwrap_or_else(|| [line, "+00"]);

        Ok(CclkTime {
            #[cfg(feature = "chrono")]
            time: {
                // e.g. 80/01/06,00:37:28+00
                use chrono::format::{Fixed, Item, Numeric, Pad};
                let mut parsed = Default::default();
                chrono::format::parse(
                    &mut parsed,
                    line,
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
                );
                if let Ok(tzoff) = tzoff.parse() {
                    parsed.set_offset(tzoff);
                }
                parsed.to_datetime_with_timezone(&chrono::Utc).unwrap_or(chrono::DateTime::<chrono::Utc>::MIN_UTC)
            },
            #[cfg(not(feature = "chrono"))]
            time: line.try_into().unwrap_or_default(),
        })
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
