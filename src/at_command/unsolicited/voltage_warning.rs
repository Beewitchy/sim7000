use crate::at_command::{AtParseErr, AtParseLine};

/// Voltage is out of range for the Sim7000
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VoltageWarning {
    OverVoltage,
    UnderVoltage,
}

impl AtParseLine for VoltageWarning {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let (reason, message) = line.split_once(' ').ok_or(AtParseErr::Mismatch)?;

        if !message.starts_with("WARN") {
            return Err(AtParseErr::Mismatch);
        }

        match reason.trim() {
            "UNDER-VOLTAGE" => Ok(VoltageWarning::UnderVoltage),
            "OVER-VOLTAGE" => Ok(VoltageWarning::OverVoltage),
            _ => Err(AtParseErr::Mismatch),
        }
    }
}
