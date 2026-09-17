use core::{
    fmt::Debug, num::{ParseFloatError, ParseIntError},
};
use embassy_time::{Duration, Instant};

pub mod generic_response;
pub mod plmn;
pub mod unsolicited;

pub use generic_response::{CloseOk, GenericOk, ShutOk, SimError, SimResult, WritePrompt};

pub mod at;
pub mod ate;
pub mod ati;
pub mod atz;
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
pub mod clbs;
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
pub mod sgns;

pub use at::At;
pub use ate::SetEcho;
pub use ati::{ApRev, Csub, GetProductInformation, ProductInfoImei, QualityControlNumber};
pub use cbatchk::EnableVBatCheck;
pub use ccid::{Iccid, ShowIccid};
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

/// This error type is reported by [AtParseLine] (and related
/// parsers) when no value is produced.
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AtParseErr {
    /// The parsed data appears to be a different type than what
    /// this parser is for. This may be not-an-error if the data
    /// type wasn't known.
    Mismatch,
    /// The parser failed to produce a value.
    ///
    /// This will be reported only when the parsed data **did**
    /// appear to match the type for this parser. Otherwise,
    /// [Self::Mismatch] should be reported.
    Parsing(&'static str),
}

impl Default for AtParseErr {
    fn default() -> Self {
        Self::Parsing("")
    }
}

pub(crate) trait AtParseLine: Sized {
    fn from_line(line: &str, _instant: &Instant) -> Result<Self, AtParseErr>;
}

/// Used by [AtRequest] to define the command group for
/// a request type.
///
/// The command group defines the prefix & end of line
/// character that the command encoder will surround
/// the request with
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CommandGroup {
    /// Commands prefixed with "AT".
    ///
    /// Typically these commands are a single letter long,
    /// and have just a single parameter or no parameters.
    ///
    /// When added to a multi-command line, commands will
    /// be appended without any separator.
    ///
    /// Currently this variant is also used for S Parameter
    /// syntax commands.
    Basic,
    /// Commands prefixed with "AT", like basic commands.
    ///
    /// These commands have a "+" at the start of their
    /// name, and may have multiple parameters after an
    /// '=' delimiter, as well as additional status /
    /// info response modes using the '?' suffix.
    ///
    /// When added to multi-command lines, they will be
    /// delimited with a ';'.
    Extended,
    /// Contextual or mode changing commands, with no
    /// prefix but still encoded with an end of line
    /// character.
    Context,
    /// Use for non-command requests to prevent extra
    /// formatting, including the end of line.
    NonCommand,
}

/// Used by [AtRequest] to determine how a request
/// should be encoded.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RequestType {
    Command(CommandGroup),
    /// Use for non-command requests to prevent extra formatting
    NonCommand,
    /// Special group signifying that a request contains
    /// multiple commands that will be encoded to a single
    /// line
    Combined {
        first: CommandGroup,
        last: CommandGroup,
    },
}

impl RequestType {
    pub const fn first_command(&self) -> &CommandGroup {
        match self {
            Self::Command(command_group) => command_group,
            Self::NonCommand => &CommandGroup::NonCommand,
            Self::Combined { first, last: _ } => first,
        }
    }

    pub const fn last_command(&self) -> &CommandGroup {
        match self {
            Self::Command(command_group) => command_group,
            Self::NonCommand => &CommandGroup::NonCommand,
            Self::Combined { first: _, last } => last,
        }
    }
}

/// Defines a request can be encoded and set to the modem,
/// along with the expected response type.
#[cfg(feature = "defmt")]
pub trait AtRequest: Debug + defmt::Format {
    /// The expected response type for this request
    type ResultType;
    /// The request type defines the prefix & end of line
    /// character that the command encoder will surround
    /// the request with
    const TYPE: RequestType;
    /// Encode the request to bytes in the format that the
    /// modem expects, *not* including the AT+ prefix or
    /// end-of-line character.
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result;
    /// Optional per-instance timeout for this request.
    ///
    /// If this type doesn't need a per-instance timeout,
    /// [Self::default_timeout()] can be implemented instead
    /// to provide a timeout for all instances.
    fn timeout(&self) -> Option<Duration> {
        Self::default_timeout()
    }
    /// The default timeout for all instances of this type.
    /// Will be overridden by the value returned by
    /// [Self::timeout()] if that is implemented.
    fn default_timeout() -> Option<Duration> {
        None
    }
}

#[cfg(not(feature = "defmt"))]
pub trait AtRequest: Debug {
    type ResultType;
    const TYPE: RequestType;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result;
    fn timeout(&self) -> Option<Duration> {
        Self::default_timeout()
    }
    fn default_timeout() -> Option<Duration> {
        None
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Either<T1, T2> {
    T1(T1),
    T2(T2),
}

impl<T1, T2> From<Result<T1, T2>> for Either<T1, T2> {
    fn from(value: Result<T1, T2>) -> Self {
        match value {
            Ok(val) => Self::T1(val),
            Err(err) => Self::T2(err),
        }
    }
}

impl<T, E> From<Either<T, E>> for Result<T, E> {
    fn from(value: Either<T, E>) -> Self {
        match value {
            Either::T1(val) => Ok(val),
            Either::T2(err) => Err(err),
        }
    }
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Seq<T, const N: usize>(pub heapless::Vec<T, N>);

impl<T, const N: usize> IntoIterator for Seq<T, N> {
    type Item = <heapless::Vec<T, N> as IntoIterator>::Item;
    type IntoIter = <heapless::Vec<T, N> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Timestamped<R> {
    pub response: R,
    pub instant: Instant,
}

impl<R> Timestamped<R> {
    pub fn new_now(response: R) -> Self {
        Self {
            response,
            instant: Instant::now(),
        }
    }

    pub fn new(response: R, instant: Instant) -> Self {
        Self { response, instant }
    }

    pub fn into_response(self) -> R {
        self.response
    }
}

impl<R> AsRef<R> for Timestamped<R> {
    fn as_ref(&self) -> &R {
        &self.response
    }
}

impl<R> AsMut<R> for Timestamped<R> {
    fn as_mut(&mut self) -> &mut R {
        &mut self.response
    }
}

impl<R> From<TimestampedRef<'_, R>> for Timestamped<R>
where
    R: Clone,
{
    fn from(value: TimestampedRef<R>) -> Self {
        Self {
            response: value.response.clone(),
            instant: value.instant,
        }
    }
}

#[derive(PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TimestampedRef<'r, R> {
    pub response: &'r mut R,
    pub instant: Instant,
}

impl<'r, R> TimestampedRef<'r, R> {
    pub fn new_now(response: &'r mut R) -> Self {
        Self {
            response,
            instant: Instant::now(),
        }
    }

    pub fn new(response: &'r mut R, instant: Instant) -> Self {
        Self { response, instant }
    }

    pub fn as_response(&self) -> &R {
        self.response
    }

    pub fn as_mut_response(&mut self) -> &mut R {
        self.response
    }
}

impl<'r, R> AsRef<R> for TimestampedRef<'r, R> {
    fn as_ref(&self) -> &R {
        self.response
    }
}

impl<'r, R> AsMut<R> for TimestampedRef<'r, R> {
    fn as_mut(&mut self) -> &mut R {
        self.response
    }
}

impl<'r, R> core::ops::Deref for TimestampedRef<'r, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.response
    }
}

impl<'r, R> core::ops::DerefMut for TimestampedRef<'r, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.response
    }
}

/// Defines simple matching logic for extracting a single token out of a str slice.
pub trait MatchToken<R = Self> {
    /// The token type to be extracted
    type Token;
    /// Implements the matching behavior.
    fn match_token(line: &str) -> Option<(Self::Token, &str)>;
}

/// Parses a type of error.
///
/// Separated from ParseResult because many error types are reused across
/// different result types.
pub trait ParseError: Sized {
    /// The type of token that signifies the an error occurring.
    type Matcher: MatchToken;
    /// Implements special parsing behavior for when the [Self::Error] token is
    /// encountered.
    ///
    /// The line given will be the remaining text of the line that the [Self::Error]
    /// token was matched on, for example if the line is "MYERROR 3" and
    /// "MYERROR" was matched, the line argument provided will contain " 3".
    fn parse_err(
        error_token: <Self::Matcher as MatchToken>::Token,
        line: &str,
    ) -> Result<Self, AtParseErr>;
}

/// Defines the expected result & response types for a given command, along with any
/// special parsing behavior.
pub trait ParseResult {
    /// The type of the response(s) to be parsed
    type Response;
    /// The successful output type, typically combining the
    /// data from [Self::Response] + [Self::Done].
    type Output;
    /// The type of token that signifies the end of the response(s)
    type Done: MatchToken;
    /// The error type & token
    ///
    /// Unlike the [Self::Output]/[Self::Done] combo, there is only one type for both
    /// the token parser and the final error because all currently supported
    /// error types are standalone rather than being combined with separate
    /// parsed data.
    type Error: ParseError;
    /// Implements special parsing behavior for when the [Self::Error] token is
    /// encountered.
    fn parse_err(
        error_token: <<Self::Error as ParseError>::Matcher as MatchToken>::Token,
        line: &str,
    ) -> Result<Self::Error, AtParseErr> {
        Self::Error::parse_err(error_token, line)
    }
    /// Implements the behavior to 'compose' the collected responses into a success
    /// result.
    fn compose_result(
        response: Self::Response,
        done_token: <Self::Done as MatchToken>::Token,
        line: &str,
    ) -> Result<Self::Output, (Self::Response, AtParseErr)>;
}

impl From<&'static str> for AtParseErr {
    fn from(message: &'static str) -> Self {
        AtParseErr::Parsing(message)
    }
}

impl From<ParseIntError> for AtParseErr {
    fn from(_: ParseIntError) -> Self {
        AtParseErr::Parsing("Failed to parse integer")
    }
}

impl From<ParseFloatError> for AtParseErr {
    fn from(_: ParseFloatError) -> Self {
        AtParseErr::Parsing("Failed to parse float")
    }
}

/// Stub AT response parser that just checks if the line starts with `prefix`
fn stub_parser_prefix<T>(line: &str, prefix: &'static str, t: T) -> Result<T, AtParseErr> {
    line.starts_with(prefix)
        .then(|| t)
        .ok_or(AtParseErr::Mismatch)
}
