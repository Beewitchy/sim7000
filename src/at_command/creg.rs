use super::{AtRequest, GenericOk};

/// AT+CREG=...
///
/// Configure network registration URC
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigureRegistrationUrc {
    /// Disable URC
    Disable = 0,

    /// Network registration URC
    EnableReg = 1,

    /// Network registration and location information URC
    EnableRegLocation = 2,
    //
    // EnableGprsTimeAndRau = 4,
}

/// AT+CREG?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetRegistrationStatus;

impl AtRequest for ConfigureRegistrationUrc {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CREG={}\r", *self as u8)
    }
}

impl AtRequest for GetRegistrationStatus {
    // The actual response is generated as an URC
    type Response = GenericOk;

    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CREG?\r")
    }
}
