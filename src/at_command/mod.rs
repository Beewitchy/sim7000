use core::{
    fmt::Debug,
    num::{ParseFloatError, ParseIntError},
};

pub mod generic_response;
pub mod unsolicited;

use cmgr::SmsMessage;
pub use generic_response::{CloseOk, GenericOk, SimError, WritePrompt};

pub mod at;
pub mod ate;
pub mod ati;
pub mod cbatchk;
pub mod ccid;
pub mod cclk;
pub mod cedrxs;
pub mod cereg;
pub mod cfgri;
pub mod cfun;
pub mod cgact;
pub mod cgmr;
pub mod cgnapn;
pub mod cgnscold;
pub mod cgnscpy;
pub mod cgnsinf;
pub mod cgnsmod;
pub mod cgnspwr;
pub mod cgnsurc;
pub mod cgnsxtra;
pub mod cgreg;
pub mod cifsrex;
pub mod ciicr;
pub mod cipclose;
pub mod cipmux;
pub mod cipsend;
pub mod cipshut;
pub mod cipsprt;
pub mod cipstart;
pub mod cmee;
pub mod cmgd;
pub mod cmgf;
pub mod cmgr;
pub mod cmgs;
pub mod cmnb;
pub mod cnact;
pub mod cncfg;
pub mod cnmi;
pub mod cnmp;
pub mod cnsmod;
pub mod cntp;
pub mod cntpcid;
pub mod cops;
pub mod cpin;
pub mod cpowd;
pub mod cpsi;
pub mod creg;
pub mod csclk;
pub mod cscs;
pub mod csms;
pub mod csq;
pub mod cstt;
pub mod gsn;
pub mod httptofs;
pub mod ifc;
pub mod ipr;
pub mod sapbr;

pub use at::At;
pub use ate::SetEcho;
pub use ati::{ApRev, Csub, GetProductInformation, ProductInfoImei, QualityControlNumber};
pub use cbatchk::EnableVBatCheck;
pub use ccid::{Iccid, ShowIccid};
use cclk::CclkTime;
pub use cedrxs::{AcTType, ConfigureEDRX, EDRXSetting};
pub use cfgri::{ConfigureRiPin, RiPinMode};
pub use cgmr::{FwVersion, GetFwVersion};
pub use cgnapn::{GetNetworkApn, NetworkApn};
pub use cgnscold::GnssColdStart;
pub use cgnscpy::CopyXtraFile;
pub use cgnsmod::{GetGnssWorkModeSet, SetGnssWorkModeSet};
pub use cgnspwr::SetGnssPower;
pub use cgnsurc::ConfigureGnssUrc;
pub use cgnsxtra::{GnssXtra, ToggleXtra};
pub use cifsrex::{GetLocalIpExt, IpExt};
pub use ciicr::StartGprs;
pub use cipclose::CloseConnection;
pub use cipmux::EnableMultiIpConnection;
pub use cipsend::IpSend;
pub use cipshut::ShutConnections;
pub use cipsprt::SetCipSendPrompt;
pub use cipstart::{Connect, ConnectMode};
pub use cmee::{CMEErrorMode, ConfigureCMEErrors};
pub use cmgf::{GetSmsMessageFormat, SetSmsMessageFormat, SmsMessageFormat};
pub use cmgs::{MessageReference, SendSms};
pub use cmnb::{NbMode, SetNbMode};
pub use cnact::{CnactMode, SetAppNetwork};
pub use cncfg::PdpConfigure;
pub use cnmp::{NetworkMode, SetNetworkMode};
pub use cnsmod::{SetAutoSystemMode, ShowSystemMode};
pub use cntp::{Execute, SynchronizeNetworkTime};
pub use cntpcid::SetGprsBearerProfileId;
pub use cops::{GetOperatorInfo, OperatorFormat, OperatorInfo, OperatorMode};
pub use cpin::GetPinStatus;
pub use cpowd::PowerDown;
pub use cpsi::{GetSystemInfo, SystemInfo, SystemMode};
pub use csclk::SetSlowClock;
pub use cscs::{CharacterSet, SetTeCharacterSet};
pub use csms::SelectMessageService;
pub use csq::{GetSignalQuality, SignalQuality};
pub use cstt::StartTask;
pub use gsn::{GetImei, Imei};
pub use httptofs::DownloadToFileSystem;
pub use ifc::{FlowControl, SetFlowControl};
pub use ipr::{BaudRate, SetBaudRate};
pub use sapbr::{BearerSettings, CmdType, ConParamType};

use self::{
    cgnscold::XtraStatus, cgnscpy::CopyResponse, cntp::NetworkTime, httptofs::DownloadInfo,
};

#[derive(Clone, Copy, Default, Debug)]
pub struct AtParseErr {
    #[allow(dead_code)]
    message: &'static str,
}

pub(crate) trait AtParseLine: Sized {
    fn from_line(line: &str) -> Result<Self, AtParseErr>;
}

#[cfg(feature = "defmt")]
pub trait AtRequest: Debug + defmt::Format {
    type Response;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result;
}

#[cfg(not(feature = "defmt"))]
pub trait AtRequest: Debug {
    type Response;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result;
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Either<T1: AtResponse + Clone, T2: AtResponse + Clone> {
    T1(T1),
    T2(T2),
}

impl<T1: AtResponse + Clone, T2: AtResponse + Clone> From<Result<T1, T2>> for Either<T1, T2> {
    fn from(value: Result<T1, T2>) -> Self {
        match value {
            Ok(val) => Self::T1(val),
            Err(err) => Self::T2(err),
        }
    }
}

impl<T: AtResponse + Clone, E: AtResponse + Clone> From<Either<T, E>> for Result<T, E> {
    fn from(value: Either<T, E>) -> Self {
        match value {
            Either::T1(val) => Ok(val),
            Either::T2(err) => Err(err),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Seq<T: AtResponse + Clone, const N: usize, DoneT: AtResponse + Clone>(
    pub heapless::Vec<T, N>,
    pub DoneT,
);

// todo: ellie (20.05.2026) - Custom iterator returning the DoneT value after the sequence
impl<T: AtResponse + Clone, const N: usize, DoneT: AtResponse + Clone> IntoIterator for Seq<T, N, DoneT> {
    type Item = <heapless::Vec<T, N> as IntoIterator>::Item;
    type IntoIter = <heapless::Vec<T, N> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub trait AtResponse: Sized {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self>;
}

/// Sim7000 AT-command response code
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ResponseCode {
    Ok(GenericOk),
    Error(SimError),
    WritePrompt(WritePrompt), // "> "
    CloseOk(CloseOk),
    IpExt(IpExt),
    Iccid(Iccid),
    SignalQuality(SignalQuality),
    CPin(unsolicited::CPin),
    SystemInfo(SystemInfo),
    OperatorInfo(OperatorInfo),
    FwVersion(FwVersion),
    Csub(Csub),
    ApRev(ApRev),
    QualityControlNumber(QualityControlNumber),
    ProductInfoImei(ProductInfoImei),
    ConfigureEDRX(ConfigureEDRX),
    CNSMod(cnsmod::CNSMod),
    PdpContextActivation(cgact::CGact),
    PdpNetworkActive(cnact::CNActPDP),
    NetworkApn(NetworkApn),
    NetworkTime(NetworkTime),
    DownloadInfo(DownloadInfo),
    CopyResponse(CopyResponse),
    XtraStatus(XtraStatus),
    XtraInfo(cgnsxtra::GnssXtraInfo),
    GnssWorkModeSet(cgnsmod::GnssWorkModeSet),
    GnssReport(cgnsinf::GnssReport),
    PowerDown(unsolicited::PowerDown),
    Imei(Imei),
    SmsMessageFormat(SmsMessageFormat),
    MessageReference(MessageReference),
    SmsMessage(SmsMessage),
    CclkTime(CclkTime),
}

impl AtParseLine for ResponseCode {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        /// Returns a function that tries to parse the line into a ResponseCode::T
        fn parse<'a, T: AtParseLine>(
            line: &'a str,
            f: impl Fn(T) -> ResponseCode + 'a,
        ) -> impl Fn(AtParseErr) -> Result<ResponseCode, AtParseErr> + 'a {
            move |_| Ok(f(T::from_line(line)?))
        }

        Err(AtParseErr::default())
            .or_else(parse(line, ResponseCode::Ok))
            .or_else(parse(line, ResponseCode::Error))
            .or_else(parse(line, ResponseCode::WritePrompt))
            .or_else(parse(line, ResponseCode::CloseOk))
            .or_else(parse(line, ResponseCode::IpExt))
            .or_else(parse(line, ResponseCode::Iccid))
            .or_else(parse(line, ResponseCode::SignalQuality))
            .or_else(parse(line, ResponseCode::CPin))
            .or_else(parse(line, ResponseCode::SystemInfo))
            .or_else(parse(line, ResponseCode::OperatorInfo))
            .or_else(parse(line, ResponseCode::FwVersion))
            .or_else(parse(line, ResponseCode::Csub))
            .or_else(parse(line, ResponseCode::ApRev))
            .or_else(parse(line, ResponseCode::QualityControlNumber))
            .or_else(parse(line, ResponseCode::ProductInfoImei))
            .or_else(parse(line, ResponseCode::ConfigureEDRX))
            .or_else(parse(line, ResponseCode::CNSMod))
            .or_else(parse(line, ResponseCode::PdpContextActivation))
            .or_else(parse(line, ResponseCode::PdpNetworkActive))
            .or_else(parse(line, ResponseCode::NetworkApn))
            .or_else(parse(line, ResponseCode::NetworkTime))
            .or_else(parse(line, ResponseCode::DownloadInfo))
            .or_else(parse(line, ResponseCode::CopyResponse))
            .or_else(parse(line, ResponseCode::XtraStatus))
            .or_else(parse(line, ResponseCode::XtraInfo))
            .or_else(parse(line, ResponseCode::GnssWorkModeSet))
            .or_else(parse(line, ResponseCode::GnssReport))
            .or_else(parse(line, ResponseCode::CclkTime))
            .or_else(parse(line, ResponseCode::PowerDown))
            // Imei is weird and may not be unambiguously parsed.
            // Take care if trying to implement other, similar, response codes.
            .or_else(parse(line, ResponseCode::Imei))
            .or_else(parse(line, ResponseCode::SmsMessageFormat))
            .or_else(parse(line, ResponseCode::MessageReference))
            // .or_else(parse(line, ResponseCode::SmsInfo))
            // Like the Imei, this one is weird and can't be unambiguously parsed (since it is human input), with the current setup.
            // Anyways, let's have this at the bottom, that way we can catch any other
            // response codes before this one.
            .or_else(parse(line, ResponseCode::SmsMessage))
            .map_err(|_| "Unknown response code".into())
    }
}

impl From<&'static str> for AtParseErr {
    fn from(message: &'static str) -> Self {
        AtParseErr { message }
    }
}

impl From<ParseIntError> for AtParseErr {
    fn from(_: ParseIntError) -> Self {
        AtParseErr {
            message: "Failed to parse integer",
        }
    }
}

impl From<ParseFloatError> for AtParseErr {
    fn from(_: ParseFloatError) -> Self {
        AtParseErr {
            message: "Failed to parse float",
        }
    }
}

/// Stub AT response parser that just checks if the line starts with `prefix`
fn stub_parser_prefix<T>(line: &str, prefix: &'static str, t: T) -> Result<T, AtParseErr> {
    line.starts_with(prefix).then(|| t).ok_or(AtParseErr {
        message: "Stub parser: Missing prefix",
    })
}
