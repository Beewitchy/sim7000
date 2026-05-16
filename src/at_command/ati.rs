use heapless::String;

use super::{
    AtParseErr, AtParseLine, AtRequest, AtResponse, FwVersion, GenericOk, Imei, ResponseCode,
    stub_parser_prefix,
};

/// ATI
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetProductInformation;

impl AtRequest for GetProductInformation {
    type Response = (
        FwVersion,
        Csub,
        ApRev,
        QualityControlNumber,
        ProductInfoImei,
        GenericOk,
    );
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+SIMCOMATI\r")
    }
}

/// I'm not sure what csub is but it seems to be part of the revision number
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Csub(pub String<7>);

impl AtResponse for Csub {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::Csub(v) => Some(v),
            _ => None,
        }
    }
}

/// Ignored response--the value is just 'FwVersion,Csub'
#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ApRev;

impl AtResponse for ApRev {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::ApRev(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct QualityControlNumber(pub String<24>);

impl AtResponse for QualityControlNumber {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::QualityControlNumber(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ProductInfoImei(pub Imei);

impl AtResponse for ProductInfoImei {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::ProductInfoImei(v) => Some(v),
            _ => None,
        }
    }
}

impl AtParseLine for Csub {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("CSUB:")
            .ok_or_else(|| AtParseErr::from("No csub string."))?;
        String::try_from(line)
            .map_err(|_| AtParseErr::from("Modem csub string is too long"))
            .map(Csub)
    }
}

impl AtParseLine for ApRev {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        stub_parser_prefix(line, "APRev:", ApRev)
    }
}

impl AtParseLine for QualityControlNumber {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("QCN:")
            .ok_or_else(|| AtParseErr::from("No qcn string."))?;
        String::try_from(line)
            .map_err(|_| AtParseErr::from("Modem qcn string is too long"))
            .map(QualityControlNumber)
    }
}

impl AtParseLine for ProductInfoImei {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("IMEI:")
            .ok_or_else(|| AtParseErr::from("No IMEI string."))?;
        Imei::from_line(line).map(ProductInfoImei)
    }
}
