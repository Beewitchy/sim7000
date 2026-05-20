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
            Error::InvalidUtf8 => embedded_io_async::ErrorKind::InvalidData,
            Error::BufferOverflow => embedded_io_async::ErrorKind::OutOfMemory,
            Error::Sim(_) => embedded_io_async::ErrorKind::NotConnected,
            Error::SimUnavailable => embedded_io_async::ErrorKind::NotConnected,
            Error::Timeout => embedded_io_async::ErrorKind::TimedOut,
            Error::Serial => embedded_io_async::ErrorKind::BrokenPipe,
            Error::UnknownResponse => embedded_io_async::ErrorKind::Other,
            Error::Transmit => embedded_io_async::ErrorKind::Interrupted,
            Error::NoApn => embedded_io_async::ErrorKind::NotConnected,
            Error::Httptofs(_) => embedded_io_async::ErrorKind::Interrupted,
            Error::Xtra(_) => embedded_io_async::ErrorKind::Interrupted,
            Error::InvalidContext => embedded_io_async::ErrorKind::InvalidInput,
        }
    }
}

impl From<TimeoutError> for Error {
    fn from(_: TimeoutError) -> Self {
        Error::Timeout
    }
}
