use super::{AtRequest, GenericOk};

/// AT+CGREG=...
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

/// AT+CGREG?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetRegistrationStatus;

impl AtRequest for ConfigureRegistrationUrc {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGREG={}\r", *self as u8)
    }
}

impl AtRequest for GetRegistrationStatus {
    // The actual response is generated as an URC
    type Response = GenericOk;

    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CGREG?\r")
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Functionality {
    Minimal = 0,
    Full = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SetFunctionalityOption {
    NoReset = 0,
    Reset = 1,
}

/// AT+CFUN?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetFunctionality(pub Functionality, pub Option<SetFunctionalityOption>);

impl AtRequest for SetFunctionality {
    // The actual response is generated as an URC
    type Response = GenericOk;

    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        let fun = self.0 as u8;
        let rst = self.1;
        if let Some(rst) = rst {
            let rst = rst as u8;
            write!(buf, "AT+CFUN={fun},{rst}\r")
        } else {
            write!(buf, "AT+CFUN={fun}\r")
        }
    }
}