use heapless::String;

use super::{
    AtParseErr, AtParseLine, AtRequest, CommandGroup, FwVersion, GenericOk, Imei,
    RequestType, SimError, stub_parser_prefix,
};

/// ATI
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetProductInformation;

impl AtRequest for GetProductInformation {
    type Response = Result<
        GenericOk<(
            ((FwVersion, (Csub, ApRev)), QualityControlNumber),
            ProductInfoImei,
        )>,
        SimError,
    >;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+SIMCOMATI")
    }
}

/// I'm not sure what csub is but it seems to be part of the revision number
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Csub(pub String<7>);

/// Ignored response--the value is just 'FwVersion,Csub'
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ApRev;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct QualityControlNumber(pub String<24>);

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ProductInfoImei(pub Imei);

impl AtParseLine for Csub {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("CSUB:").ok_or(AtParseErr::Mismatch)?;
        String::try_from(line)
            .map_err(|_| AtParseErr::from("Modem csub string is too long"))
            .map(Csub)
    }
}

impl AtParseLine for ApRev {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        stub_parser_prefix(line, "APRev:", ApRev)
    }
}

impl AtParseLine for QualityControlNumber {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("QCN:").ok_or(AtParseErr::Mismatch)?;
        String::try_from(line)
            .map_err(|_| AtParseErr::from("Modem qcn string is too long"))
            .map(QualityControlNumber)
    }
}

impl AtParseLine for ProductInfoImei {
    fn from_line(line: &str, instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("IMEI:").ok_or(AtParseErr::Mismatch)?;
        Imei::from_line(line, instant).map(ProductInfoImei)
    }
}
