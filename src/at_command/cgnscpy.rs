use crate::{error::Xtra, Error};

use super::{AtParseErr, AtParseLine, AtRequest, CommandGroup, GenericOk, RequestType, SimResult};

/// AT+CGNSCPY=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CopyXtraFile;

impl AtRequest for CopyXtraFile {
    type ResultType = SimResult<GenericOk<CopyResponse>>;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CGNSCPY")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CopyResponse {
    Success = 0,
    FileDoesntExist = 1,
}

impl CopyResponse {
    pub fn success(&mut self) -> Result<(), Error> {
        match self {
            CopyResponse::Success => Ok(()),
            CopyResponse::FileDoesntExist => Err(Error::Xtra(Xtra::FileDoesntExist)),
        }
    }
}

impl AtParseLine for CopyResponse {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CGNSCPY:")
            .ok_or(AtParseErr::Mismatch)?;

        match line.trim() {
            "0" => Ok(CopyResponse::Success),
            "1" => Ok(CopyResponse::FileDoesntExist),
            _ => Err("Invalid response, expected 0 or 1".into()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse() {
        let str = "+CGNSCPY: 0";
        let info = CopyResponse::from_line(str, &embassy_time::Instant::now()).expect("Parse CopyResponse");

        let expected = CopyResponse::Success;
        assert_eq!(expected, info);
    }
}
