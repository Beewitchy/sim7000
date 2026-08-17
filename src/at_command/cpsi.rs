use crate::log;
use crate::util::collect_array;

use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, CommandGroup, GenericOk, RequestType, ResponseCode, plmn};

/// AT+CPSI?
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetSystemInfo;

impl AtRequest for GetSystemInfo {
    type Response = (SystemInfo, GenericOk);
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "+CPSI?")
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

/// See https://en.wikipedia.org/wiki/GSM_frequency_bands
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GsmFrequencyBandAndChannel {
    TGSM380,
    TGSM410,
    GSM450(u16),
    GSM480(u16),
    GSM710,
    GSM750(u16),
    TGSM810,
    GSM850(u16),
    PGSM900(u16),
    EGSM900(u16),
    RGSM900(u16),
    TGSM900,
    DCS1800(u16),
    PCS1900(u16),
    /// Unknown band.
    /// See info log `Unknown frequency band 'name' in CPSI response`
    /// for the listed string if this is parsed.
    #[default]
    Unknown,
}

impl core::str::FromStr for GsmFrequencyBandAndChannel {
    type Err = AtParseErr;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (channel, s) = if let Some((channel, rest)) = s.split_once(char::is_whitespace) {
            (u16::from_str_radix(channel, 10).map_err(|_| "Failed to parse channel number")?, rest.trim_start())
        } else {
            (0, s)
        };
        match s {
            "TGSM 380" => Ok(Self::TGSM380),
            "TGSM 410" => Ok(Self::TGSM410),
            "GSM 450" => Ok(Self::GSM450(channel)),
            "GSM 480" => Ok(Self::GSM480(channel)),
            "GSM 710" => Ok(Self::GSM710),
            "GSM 750" => Ok(Self::GSM750(channel)),
            "TGSM 810" => Ok(Self::TGSM810),
            "GSM 850" => Ok(Self::GSM850(channel)),
            "PGSM 900" => Ok(Self::PGSM900(channel)),
            "EGSM 900" => Ok(Self::EGSM900(channel)),
            "RGSM 900" => Ok(Self::RGSM900(channel)),
            "TGSM 900" => Ok(Self::TGSM900),
            "DCS 1800" => Ok(Self::DCS1800(channel)),
            "PCS 1900" => Ok(Self::PCS1900(channel)),
            _ => Err("Unknown GSM band identifier".into()),
        }
    }
}

/// See https://en.wikipedia.org/wiki/LTE_frequency_bands#Frequency_bands_and_channel_bandwidths
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LteFrequencyBand {
    /// E-UTRAN band of the given number.
    EUtran(u8),
    /// Unknown band.
    /// See info log `Unknown frequency band 'name' in CPSI response`
    /// for the listed string if this is parsed.
    #[default]
    Unknown,
}

impl core::str::FromStr for LteFrequencyBand {
    type Err = AtParseErr;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(s) = s.strip_prefix("EUTRAN-BAND") {
            let band = s.parse().map_err(|_| "Failed to parse UTran band number")?;
            Ok(Self::EUtran(band))
        } else {
            Err("Unknown LTE band identifier".into())
        }
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GsmModeParameters {
    pub mcc: plmn::Mcc,
    pub mnc: plmn::Mnc,
    pub lac: u16,
    pub cell_id: u16,
    pub freq_and_channel: GsmFrequencyBandAndChannel,
    pub rx_lev: i16,
    pub track_lo_adjust: u16,
    pub c1: u16,
    pub c2: u16,
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
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

        if let Some((operation_mode, rest)) = rest.split_once(',') {
            let operation_mode = OperationMode::from_str(operation_mode)?;
            let system_mode_parameters = match system_mode {
                SystemMode::NoService => SystemModeParameters::NoService,
                SystemMode::Gsm => {
                    let [
                        mcc_mnc,
                        lac,
                        cell_id,
                        freq_and_channel,
                        rx_lev,
                        track_lo_adjust,
                        c1_c2,
                    ] = collect_array(rest.splitn(7, ',')).ok_or("Missing ','")?;
                    let (mcc, mnc) = mcc_mnc
                        .split_once('-')
                        .ok_or("Missing '-' in mcc-mnc parameter")?;
                    let (c1, c2) = c1_c2
                        .split_once('-')
                        .ok_or("Missing '-' in c1-c2 parameter")?;
                    let mcc = plmn::Mcc::from_str(mcc).ok_or("Failed to parse mcc parameter")?;
                    let mnc = plmn::Mnc::from_str(mnc).ok_or("Failed to parse mnc parameter")?;
                    let lac = if let Some(lac) = lac.strip_prefix("0x") {
                        u16::from_str_radix(lac, 16)
                    } else {
                        u16::from_str_radix(lac, 10)
                    }.map_err(|_| "Failed to parse lac")?;
                    let freq_and_channel =
                        match GsmFrequencyBandAndChannel::from_str(freq_and_channel.trim()) {
                            Ok(freq_and_channel) => freq_and_channel,
                            Err(err) => {
                                log::info!(
                                    "Unknown frequency band '{:?}' in CPSI response",
                                    freq_and_channel
                                );
                                return Err(err);
                            }
                        };
                    SystemModeParameters::Gsm(GsmModeParameters {
                        mcc,
                        mnc,
                        lac,
                        cell_id: cell_id.parse().map_err(|_| "Failed to parse cell id")?,
                        freq_and_channel,
                        rx_lev: rx_lev.parse().map_err(|_| "Failed to parse rx level")?,
                        track_lo_adjust: track_lo_adjust.parse().map_err(|_| "Failed to parse lo adjust")?,
                        c1: c1.parse()?,
                        c2: c2.parse()?,
                    })
                }
                SystemMode::LteCatM1 | SystemMode::LteNbIot => {
                    let [
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
                    ] = collect_array(rest.splitn(12, ',')).ok_or("Missing ','")?;
                    let (mcc, mnc) = mcc_mnc
                        .split_once('-')
                        .ok_or("Missing '-' in mcc-mnc parameter")?;
                    let mcc = plmn::Mcc::from_str(mcc).ok_or("Failed to parse mcc parameter")?;
                    let mnc = plmn::Mnc::from_str(mnc).ok_or("Failed to parse mnc parameter")?;
                    let tac = if let Some(tac) = tac.strip_prefix("0x") {
                        u16::from_str_radix(tac, 16)
                    } else {
                        u16::from_str_radix(tac, 10)
                    }.map_err(|_| "Failed to parse tac")?;
                    let freq_band = match LteFrequencyBand::from_str(freq_band.trim()) {
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
                        serving_cell_id: serving_cell_id.trim().parse()?,
                        phys_cell_id: phys_cell_id.trim().parse()?,
                        freq_band,
                        e_ultra_channel_num: e_ultra_channel_num.trim().parse()?,
                        downlink_bandwidth: downlink_bandwidth.trim().parse()?,
                        uplink_bandwidth: uplink_bandwidth.trim().parse()?,
                        rsrq: rsrq.trim().parse()?,
                        rsrp: rsrp.trim().parse()?,
                        rssi: rssi.trim().parse()?,
                        rssnr: rssnr.trim().parse()?,
                    };
                    match system_mode {
                        SystemMode::LteCatM1 => SystemModeParameters::LteCatM1(lte_mode_parameters),
                        SystemMode::LteNbIot => SystemModeParameters::LteNbIot(lte_mode_parameters),
                        SystemMode::NoService => unreachable!(),
                        SystemMode::Gsm => unreachable!(),
                    }
                }
            };
            Ok(SystemInfo { operation_mode, system_mode_parameters })
        } else {
            let operation_mode = OperationMode::from_str(rest)?;
            Ok(match system_mode {
                SystemMode::NoService => SystemInfo {
                    operation_mode,
                    system_mode_parameters: SystemModeParameters::NoService,
                },
                SystemMode::Gsm => SystemInfo {
                    operation_mode,
                    system_mode_parameters: SystemModeParameters::Gsm(GsmModeParameters::default()),
                },
                SystemMode::LteCatM1 => SystemInfo {
                    operation_mode,
                    system_mode_parameters: SystemModeParameters::LteCatM1(LteModeParameters::default()),
                },
                SystemMode::LteNbIot => SystemInfo {
                    operation_mode,
                    system_mode_parameters: SystemModeParameters::LteNbIot(LteModeParameters::default()),
                }
            })
        }
    }
}

impl AtResponse for SystemInfo {
    #[cfg(any(feature = "log", feature = "defmt"))]
    const RESPONSE_KIND: super::ResponseCodeKind = super::ResponseCodeKind::SystemInfo;
    #[inline]
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::SystemInfo(v) => Some(v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod test {
    use super::super::plmn;
    use super::*;
    #[test]
    fn parse_cpsi() {
        let now = embassy_time::Instant::now();
        assert_eq!(
            SystemInfo::from_line("+CPSI: NO SERVICE,Online", &now).ok(),
            Some(SystemInfo {
                operation_mode: OperationMode::Online,
                system_mode_parameters: SystemModeParameters::NoService,
            })
        );
        assert_eq!(
            SystemInfo::from_line(
                "+CPSI: GSM,Online,240-24,0x28a0,50183,61 EGSM 900,-53,0,58-58",
                &now
            ).unwrap(),
            SystemInfo {
                operation_mode: OperationMode::Online,
                system_mode_parameters: SystemModeParameters::Gsm(GsmModeParameters {
                    mcc: plmn::Mcc::new(240),
                    mnc: plmn::Mnc::new_short(24),
                    lac: 0x28a0,
                    cell_id: 50183,
                    freq_and_channel: GsmFrequencyBandAndChannel::EGSM900(61),
                    rx_lev: -53,
                    track_lo_adjust: 0,
                    c1: 58,
                    c2: 58
                }),
            }
        );
        assert_eq!(
            SystemInfo::from_line(
                "+CPSI: LTE CAT-M1,Online,240-07,0x2AFE,34564631,149,EUTRAN-BAND20,6400,3,3,-12,-81,-52,10",
                &now
            ).unwrap(),
            SystemInfo {
                operation_mode: OperationMode::Online,
                system_mode_parameters: SystemModeParameters::LteCatM1(LteModeParameters {
                    mcc: plmn::Mcc::new(240),
                    mnc: plmn::Mnc::new_short(7),
                    tac: 0x2AFE,
                    serving_cell_id: 34564631,
                    phys_cell_id: 149,
                    freq_band: LteFrequencyBand::EUtran(20),
                    e_ultra_channel_num: 6400,
                    downlink_bandwidth: 3,
                    uplink_bandwidth: 3,
                    rsrq: -12,
                    rsrp: -81,
                    rssi: -52,
                    rssnr: 10
                }),
            }
        );
        assert_eq!(
            SystemInfo::from_line("+CPSI: LTE CAT-M1,Online", &now).unwrap(),
            SystemInfo {
                operation_mode: OperationMode::Online,
                system_mode_parameters: SystemModeParameters::LteCatM1(LteModeParameters::default()),
            }
        );
    }
}
