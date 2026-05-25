use super::{AtRequest, GenericOk};

/// AT+CFUN arguments for both the set command and responses
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Functionality {
    Minimal = 0,
    Full = 1,
    DisablePhoneRadioCircuits = 4,
    FactoryTest = 5,
    Reset = 6,
    Offline = 7,
}

impl core::str::FromStr for Functionality {
    type Err = super::AtParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "0" => Ok(Self::Minimal),
            "1" => Ok(Self::Full),
            _ => Err("invalid data".into())
        }
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SetFunctionalityOption {
    NoReset = 0,
    Reset = 1,
}

/// AT+CFUN=
///
/// - AT+CFUN=1,1 can be used to reset module purposely at minimum/full functionality mode.
/// - Response string "OK" will be returned after module resets if baud rate is set to fixed baud rate.
/// - AT+CFUN=6 must be used after setting AT+CFUN=7. If module in offline mode,must execute
///  AT+CFUN=6 or restart module to online mode.
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