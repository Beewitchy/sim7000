use embassy_time::TimeoutError;

use crate::at_command::{SimError, httptofs::StatusCode, AtParseErr};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Error {
    InvalidUtf8,
    BufferOverflow,
    OutOfPackets,
    Sim(SimError),
    Timeout,
    Serial,
    UnknownResponse,
    /// MetaResponse type conversion failed
    IncompatibleMapping,

    Transmit,

    SimUnavailable,
    /// No default APN was set, and the network did not provide one.
    NoApn,
    Httptofs(StatusCode),
    Xtra(Xtra),

    /// Context isn't fully initialized, or incorrectly set up
    InvalidContext,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Xtra {
    FileDoesntExist,
    NotEffective,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Modem driver error {self:?}")
    }
}

impl core::error::Error for Error {}

impl embedded_io_async::Error for Error {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        match self {
            Self::InvalidUtf8 => embedded_io_async::ErrorKind::InvalidData,
            Self::BufferOverflow | Self::OutOfPackets => embedded_io_async::ErrorKind::OutOfMemory,
            Self::Sim(_) | Self::SimUnavailable | Self::NoApn => {
                embedded_io_async::ErrorKind::NotConnected
            }
            Self::Timeout => embedded_io_async::ErrorKind::TimedOut,
            Self::Serial => embedded_io_async::ErrorKind::BrokenPipe,
            Self::UnknownResponse | Self::IncompatibleMapping => {
                embedded_io_async::ErrorKind::Other
            }
            Self::Transmit | Self::Httptofs(_) | Self::Xtra(_) => {
                embedded_io_async::ErrorKind::Interrupted
            }
            Self::InvalidContext => embedded_io_async::ErrorKind::InvalidInput,
        }
    }
}

impl From<SimError> for Error {
    fn from(v: SimError) -> Self {
        Self::Sim(v)
    }
}

impl From<TimeoutError> for Error {
    fn from(_: TimeoutError) -> Self {
        Error::Timeout
    }
}

impl From<crate::BuildIoError> for Error {
    fn from(_: crate::BuildIoError) -> Self {
        Self::InvalidContext
    }
}

impl From<AtParseErr> for Error {
    fn from(_: AtParseErr) -> Self {
        Error::UnknownResponse
    }
}