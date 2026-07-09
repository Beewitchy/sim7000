use embassy_time::TimeoutError;

use crate::at_command::{httptofs::StatusCode, SimError};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum Error {
    InvalidUtf8,
    BufferOverflow,
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

    /// Context isn't fully initialized
    InvalidContext
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Xtra {
    FileDoesntExist,
    NotEffective,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SIM7000 error {self:?}")
    }
}

impl core::error::Error for Error {}

impl embedded_io_async::Error for Error {
    fn kind(&self) -> embedded_io_async::ErrorKind {
        match self {
            Self::InvalidUtf8 => embedded_io_async::ErrorKind::InvalidData,
            Self::BufferOverflow => embedded_io_async::ErrorKind::OutOfMemory,
            Self::Sim(_) => embedded_io_async::ErrorKind::NotConnected,
            Self::SimUnavailable => embedded_io_async::ErrorKind::NotConnected,
            Self::Timeout => embedded_io_async::ErrorKind::TimedOut,
            Self::Serial => embedded_io_async::ErrorKind::BrokenPipe,
            Self::UnknownResponse => embedded_io_async::ErrorKind::Other,
            Self::IncompatibleMapping => embedded_io_async::ErrorKind::Other,
            Self::Transmit => embedded_io_async::ErrorKind::Interrupted,
            Self::NoApn => embedded_io_async::ErrorKind::NotConnected,
            Self::Httptofs(_) => embedded_io_async::ErrorKind::Interrupted,
            Self::Xtra(_) => embedded_io_async::ErrorKind::Interrupted,
            Self::InvalidContext => embedded_io_async::ErrorKind::InvalidInput,
        }
    }
}

impl From<TimeoutError> for Error {
    fn from(_: TimeoutError) -> Self {
        Error::Timeout
    }
}
