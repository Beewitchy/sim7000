use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SystemMode {
    NoService,
    Gsm,
    Gprs,
    LteCatM1,
    LteNbIot,
}

/// AT+CNSMOD?
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CNSMod {
    pub enabled: bool,
    pub system_mode: SystemMode,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ShowSystemMode;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SetAutoSystemMode(pub CNSMod);

impl AtRequest for ShowSystemMode {
    type Response = (CNSMod, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CNSMOD?\r")
    }
}

impl AtRequest for SetAutoSystemMode {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(
            buf,
            "AT+CNSMOD={},{}\r",
            if self.0.enabled { '1' } else { '0' },
            self.0.system_mode as u8
        )
    }
}

impl AtParseLine for CNSMod {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CNSMOD:")
            .ok_or(AtParseErr::Mismatch)?
            .trim_start();
        let (enabled, system_mode) = line.split_once(',').ok_or("Missing delimiter")?;

        let enabled = match enabled {
            "0" => false,
            "1" => true,
            _ => return Err("Failed to parse enabled".into()),
        };

        let system_mode = match system_mode.trim() {
            "0" => SystemMode::NoService,
            "1" => SystemMode::Gsm,
            "3" => SystemMode::Gprs,
            "7" => SystemMode::LteCatM1,
            "9" => SystemMode::LteNbIot,
            _ => return Err("Failed to parse System Mode".into()),
        };

        Ok(Self {
            enabled,
            system_mode,
        })
    }
}

impl AtResponse for CNSMod {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::CNSMod(v) => Some(v),
            _ => None,
        }
    }
}
