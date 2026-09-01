use crate::{error::Xtra, Error};

use super::{AtParseErr, AtParseLine, AtRequest, CommandGroup, GenericOk, MatchToken, RequestType, SimResult};

/// AT+CGNSCOLD
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssColdStart;

/// AT+CGNSWARM
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssWarmStart;

/// AT+CGNSHOT
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GnssHotStart;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum XtraStatus {
    Success = 0,
    DoesntExist = 1,
    NotEffective = 2,
}

impl XtraStatus {
    pub fn success(&mut self) -> Result<(), Error> {
        match self {
            XtraStatus::Success => Ok(()),
            XtraStatus::DoesntExist => Err(Error::Xtra(Xtra::FileDoesntExist)),
            XtraStatus::NotEffective => Err(Error::Xtra(Xtra::NotEffective)),
        }
    }
}

impl AtRequest for GnssColdStart {
    type ResultType = SimResult<(GenericOk, XtraStatus)>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CGNSCOLD")
    }
}

impl AtRequest for GnssWarmStart {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CGNSWARM")
    }
}

impl AtRequest for GnssHotStart {
    type ResultType = SimResult<GenericOk>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CGNSHOT")
    }
}

impl AtParseLine for XtraStatus {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let (_, line) = Self::match_token(line)
            .ok_or(AtParseErr::Mismatch)?;

        match line.trim() {
            "0" => Ok(XtraStatus::Success),
            "1" => Ok(XtraStatus::DoesntExist),
            "2" => Ok(XtraStatus::NotEffective),
            _ => Err("Invalid response, expected 0, 1 or 2".into()),
        }
    }
}

impl MatchToken for XtraStatus {
    type Token = XtraStatus;

    fn match_token(line: &str) -> Option<(Self::Token, &str)> {
        let line = line
            .strip_prefix("+CGNSXTRA:")?;
        Some((XtraStatus::Success, line))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let str = "+CGNSXTRA: 0";
        let info = XtraStatus::from_line(str, &embassy_time::Instant::now()).expect("Parse XtraStatus");

        let expected = XtraStatus::Success;
        assert_eq!(expected, info);
    }
}
