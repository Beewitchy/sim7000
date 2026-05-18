use crate::{
    at_command::{AtParseErr, AtParseLine},
    collect_array,
};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Psuttz(
    #[cfg(feature = "chrono")] pub chrono::DateTime<chrono::Utc>,
    #[cfg(not(feature = "chrono"))] pub heapless::String<37>,
);

impl AtParseLine for Psuttz {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("*PSUTTZ:")
            .ok_or("missing prefix")?
            .trim();
        #[cfg(feature = "chrono")]
        {
            //year, month, day, hour, min, sec, "time_zone", dst
            use chrono::format::{Item, Numeric, Pad};
            let mut parsed = Default::default();
            chrono::format::parse(
                &mut parsed,
                line,
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
            .map_err(|_| "unable to parse date-time elements")?;
            parsed.set_offset(0); // ignore local-time offset, only care about utc data
            Ok(Self(
                parsed
                    .to_datetime_with_timezone(&chrono::Utc)
                    .map_err(|_| "unable to convert parsed datetime to utc")?,
            ))
        }
        #[cfg(not(feature = "chrono"))]
        {
            Ok(Self(line.try_into().unwrap_or_default()))
        }
    }
}
