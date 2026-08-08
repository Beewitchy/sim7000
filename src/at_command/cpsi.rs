use crate::util::collect_array;
use crate::log;

use super::{
    AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode,
    plmn,
};

/// AT+CPSI?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetSystemInfo;

impl AtRequest for GetSystemInfo {
    type Response = (SystemInfo, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CPSI?\r")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SystemMode {
    NoService,
    Gsm,
    LteCatM1,
    LteNbIot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LteFrequencyBand {
    /// E-UTRAN band of the given number.
    /// See https://en.wikipedia.org/wiki/LTE_frequency_bands#Frequency_bands_and_channel_bandwidths
    EUtran(u8),
    /// Unknown band.
    /// See info log `Unknown frequency band 'name' in CPSI response`
    /// for the listed string if this is parsed.
    Unknown
}

impl core::str::FromStr for LteFrequencyBand {
    type Err = AtParseErr;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(s) = s.strip_prefix("") {
            let band = s.parse().map_err(|_| "Failed to parse UTran band number")?;
            Ok(Self::EUtran(band))
        } else {
            Err("Unknown LTE band identifier".into())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GsmModeParameters {
    pub mcc: plmn::Mcc,
    pub mnc: plmn::Mnc,
    pub lac: u16,
    pub cell_id: u16,
    pub absolute_rf_ch_num: u16,
    pub rx_lev: u16,
    pub track_lo_adjust: u16,
    pub c1: u16,
    pub c2: u16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LteModeParameters {
    pub mcc: plmn::Mcc,
    pub mnc: plmn::Mnc,
    pub tac: u16,
    pub serving_cell_id: u32,
    pub phys_cell_id: u16,
    pub freq_band: LteFrequencyBand,
    pub e_ultra_channel_num: u16,
    pub downlink_bandwidth: u8,
    pub uplink_bandwidth: u8,
    pub rsrq: i16,
    pub rsrp: i16,
    pub rssi: i16,
    pub rssnr: i16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SystemModeParameters {
    NoService,
    Gsm(GsmModeParameters),
    LteCatM1(LteModeParameters),
    LteNbIot(LteModeParameters),
}

impl SystemModeParameters {
    /// Returns the empty-variant [SystemMode] corresponding to these parameters
    pub const fn mode(&self) -> SystemMode {
        match self {
            Self::NoService => SystemMode::NoService,
            Self::Gsm(_) => SystemMode::Gsm,
            Self::LteCatM1(_) => SystemMode::LteCatM1,
            Self::LteNbIot(_) => SystemMode::LteNbIot,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum OperationMode {
    Online,
    Offline,
    FactoryTest,
    Reset,
    LowPower,
}

impl core::str::FromStr for OperationMode {
    type Err = AtParseErr;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Online" => Ok(OperationMode::Online),
            "Offline" => Ok(OperationMode::Offline),
            "Factory Test Mode" => Ok(OperationMode::FactoryTest),
            "Reset" => Ok(OperationMode::Reset),
            "Low Power Mode" => Ok(OperationMode::LowPower),
            _ => Err("Failed to parse Operation Mode".into()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SystemInfo {
    pub operation_mode: OperationMode,
    pub system_mode_parameters: SystemModeParameters,
}

impl SystemInfo {
    pub const fn system_mode(&self) -> SystemMode {
        self.system_mode_parameters.mode()
    }
}

impl AtParseLine for SystemInfo {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        use core::str::FromStr as _;
        let line = line.strip_prefix("+CPSI:").ok_or(AtParseErr::Mismatch)?;
        let (system_mode, rest) = line.split_once(',').ok_or("Missing ','")?;

        let system_mode = match system_mode.trim() {
            "NO SERVICE" => SystemMode::NoService,
            "GSM" => SystemMode::Gsm,
            "LTE CAT-M1" => SystemMode::LteCatM1,
            "LTE NB-IOT" => SystemMode::LteNbIot,
            _ => return Err("Failed to parse System Mode".into()),
        };

        let system_info = match system_mode {
            SystemMode::NoService => SystemInfo {
                operation_mode: OperationMode::from_str(rest)?,
                system_mode_parameters: SystemModeParameters::NoService,
            },
            SystemMode::Gsm => {
                let [
                    operation_mode,
                    mcc_mnc,
                    lac,
                    cell_id,
                    absolute_rf_ch_num,
                    rx_lev,
                    track_lo_adjust,
                    c1_c2,
                ] = collect_array(rest.splitn(8, ',')).ok_or("Missing ','")?;
                let (mcc, mnc) = mcc_mnc
                    .split_once('-')
                    .ok_or("Missing '-' in mcc-mnc parameter")?;
                let (c1, c2) = c1_c2
                    .split_once('-')
                    .ok_or("Missing '-' in c1-c2 parameter")?;
                let mcc = plmn::Mcc::from_str(mcc).ok_or("Failed to parse mcc parameter")?;
                let mnc = plmn::Mnc::from_str(mnc).ok_or("Failed to parse mnc parameter")?;
                SystemInfo {
                    operation_mode: OperationMode::from_str(operation_mode)?,
                    system_mode_parameters: SystemModeParameters::Gsm(GsmModeParameters {
                        mcc,
                        mnc,
                        lac: lac.parse()?,
                        cell_id: cell_id.parse()?,
                        absolute_rf_ch_num: absolute_rf_ch_num.parse()?,
                        rx_lev: rx_lev.parse()?,
                        track_lo_adjust: track_lo_adjust.parse()?,
                        c1: c1.parse()?,
                        c2: c2.parse()?,
                    }),
                }
            }
            SystemMode::LteCatM1 | SystemMode::LteNbIot => {
                let [
                    operation_mode,
                    mcc_mnc,
                    tac,
                    serving_cell_id,
                    phys_cell_id,
                    freq_band,
                    e_ultra_channel_num,
                    downlink_bandwidth,
                    uplink_bandwidth,
                    rsrq,
                    rsrp,
                    rssi,
                    rssnr,
                ] = collect_array(rest.splitn(13, ',')).ok_or("Missing ','")?;
                let (mcc, mnc) = mcc_mnc
                    .split_once('-')
                    .ok_or("Missing '-' in mcc-mnc parameter")?;
                let mcc = plmn::Mcc::from_str(mcc).ok_or("Failed to parse mcc parameter")?;
                let mnc = plmn::Mnc::from_str(mnc).ok_or("Failed to parse mnc parameter")?;
                let tac = if let Some(tac) = tac.strip_prefix("0x") {
                    u16::from_str_radix(tac, 16)?
                } else {
                    u16::from_str_radix(tac, 10)?
                };
                let freq_band = match LteFrequencyBand::from_str(freq_band) {
                    Ok(freq_band) => freq_band,
                    Err(err) => {
                        log::info!("Unknown frequency band '{:?}' in CPSI response", freq_band);
                        return Err(err);
                    }
                };
                let lte_mode_parameters = LteModeParameters {
                    mcc,
                    mnc,
                    tac,
                    serving_cell_id: serving_cell_id.parse()?,
                    phys_cell_id: phys_cell_id.parse()?,
                    freq_band,
                    e_ultra_channel_num: e_ultra_channel_num.parse()?,
                    downlink_bandwidth: downlink_bandwidth.parse()?,
                    uplink_bandwidth: uplink_bandwidth.parse()?,
                    rsrq: rsrq.parse()?,
                    rsrp: rsrp.parse()?,
                    rssi: rssi.parse()?,
                    rssnr: rssnr.parse()?,
                };
                SystemInfo {
                    operation_mode: OperationMode::from_str(operation_mode)?,
                    system_mode_parameters: match system_mode {
                        SystemMode::LteCatM1 => SystemModeParameters::LteCatM1(lte_mode_parameters),
                        SystemMode::LteNbIot => SystemModeParameters::LteNbIot(lte_mode_parameters),
                        SystemMode::NoService => unreachable!(),
                        SystemMode::Gsm => unreachable!(),
                    },
                }
            }
        };
        Ok(system_info)
    }
}

impl AtResponse for SystemInfo {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::SystemInfo(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn parse_cpsi() {
        let valid_cpsis = [
            "+CPSI: GSM,Online,240-24,0x28a0,50183,61 EGSM 900,-53,0,58-58",
            "+CPSI: LTE CAT-M1,Online,240-07,0x2AFE,34564631,149,EUTRAN-BAND20,6400,3,3,-12,-81,-52,10",
        ];

        for valid in valid_cpsis {
            assert!(SystemInfo::from_line(valid, &embassy_time::Instant::now()).is_ok());
        }
    }
}
