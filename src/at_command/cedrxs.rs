use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum EDRXSetting {
    Disable = 0,
    Enable = 1,
    EnableWithAutoReport = 2,
}

#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AcTType {
    CatM = 4,
    NbIot = 5,
}

/// AT+CEDRX=...
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ConfigureEDRX {
    pub n: EDRXSetting,
    pub act_type: AcTType,
    pub requested_edrx_value: EdrxCycleLength,
}

/// The EDRX cycle length, in seconds.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum EdrxCycleLength {
    _5 = 0x0,
    _10 = 0x1,
    _20 = 0x2,
    _40 = 0x3,
    _61 = 0x4,
    _81 = 0x5,
    _102 = 0x6,
    _122 = 0x7,
    _143 = 0x8,
    _163 = 0x9,
    _327 = 0xA,
    _655 = 0xB,
    _1310 = 0xC,
    _2621 = 0xD,
    _5242 = 0xE,
    _10485 = 0xF,
}

impl TryFrom<u8> for EdrxCycleLength {
    type Error = &'static str;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == const { EdrxCycleLength::_5 as u8 } => Ok(EdrxCycleLength::_5),
            x if x == const { EdrxCycleLength::_10 as u8 } => Ok(EdrxCycleLength::_10),
            x if x == const { EdrxCycleLength::_20 as u8 } => Ok(EdrxCycleLength::_20),
            x if x == const { EdrxCycleLength::_40 as u8 } => Ok(EdrxCycleLength::_40),
            x if x == const { EdrxCycleLength::_61 as u8 } => Ok(EdrxCycleLength::_61),
            x if x == const { EdrxCycleLength::_81 as u8 } => Ok(EdrxCycleLength::_81),
            x if x == const { EdrxCycleLength::_102 as u8 } => Ok(EdrxCycleLength::_102),
            x if x == const { EdrxCycleLength::_122 as u8 } => Ok(EdrxCycleLength::_122),
            x if x == const { EdrxCycleLength::_143 as u8 } => Ok(EdrxCycleLength::_143),
            x if x == const { EdrxCycleLength::_163 as u8 } => Ok(EdrxCycleLength::_163),
            x if x == const { EdrxCycleLength::_327 as u8 } => Ok(EdrxCycleLength::_327),
            x if x == const { EdrxCycleLength::_655 as u8 } => Ok(EdrxCycleLength::_655),
            x if x == const { EdrxCycleLength::_1310 as u8 } => Ok(EdrxCycleLength::_1310),
            x if x == const { EdrxCycleLength::_2621 as u8 } => Ok(EdrxCycleLength::_2621),
            x if x == const { EdrxCycleLength::_5242 as u8 } => Ok(EdrxCycleLength::_5242),
            x if x == const { EdrxCycleLength::_10485 as u8 } => Ok(EdrxCycleLength::_10485),
            _ => Err("Not a EdrxCycleLength"),
        }
    }
}

impl AtRequest for ConfigureEDRX {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(
            buf,
            "AT+CEDRXS={},{},\"{:04b}\"\r",
            self.n as u8, self.act_type as u8, self.requested_edrx_value as u8,
        )
    }
}

/// AT+CEDRX?
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetEDRXSetting;

impl AtRequest for GetEDRXSetting {
    type Response = (ConfigureEDRX, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CEDRXS?\r",)
    }
}

impl AtParseLine for ConfigureEDRX {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+CEDRXS:")
            .ok_or(AtParseErr::from("not an EDRX-setting response"))?;
        let mut config = Self {
            n: EDRXSetting::Disable,
            act_type: AcTType::CatM,
            requested_edrx_value: EdrxCycleLength::_5,
        };
        for (i, part) in line.split_terminator(',').enumerate() {
            let part = part.trim();
            match i {
                0 => {
                    config.n = match part {
                        "0" => EDRXSetting::Disable,
                        "1" => EDRXSetting::Enable,
                        "2" => EDRXSetting::EnableWithAutoReport,
                        _ => return Err(AtParseErr::from("Not a valid EDRXSetting")),
                    }
                }
                1 => {
                    config.act_type = match part {
                        "4" => AcTType::CatM,
                        "5" => AcTType::NbIot,
                        _ => return Err(AtParseErr::from("Not a valid EDRXSetting")),
                    }
                }
                2 => {
                    config.requested_edrx_value = u8::from_str_radix(part, 2)
                        .map_err(|_| AtParseErr::from("not a edrx value"))?
                        .try_into()?
                }
                _ => return Err(AtParseErr::from("Not a valid EDRXSetting")),
            }
        }
        Ok(config)
    }
}

impl AtResponse for ConfigureEDRX {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::ConfigureEDRX(configure_edrx) => Some(configure_edrx),
            _ => None,
        }
    }
}

/// AT+CEDRX=?
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TestEDRX;

impl AtRequest for TestEDRX {
    type Response = GenericOk;
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CEDRXS=?\r",)
    }
}
