use crate::at_command::{AtParseErr, AtParseLine};

/// Indicates whether the app network is active
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppNetworkActive {
    pub id: Option<u8>,
    pub active: bool,
}

impl AtParseLine for AppNetworkActive {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let line = line
            .strip_prefix("+APP PDP:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();
        let (id, state) = line
            .split_once(',')
            .map(|(id, state)| (id.parse().ok(), state))
            .unwrap_or((None, line));
        match state.trim() {
            "ACTIVE" => Ok(AppNetworkActive { id, active: true }),
            "DEACTIVE" => Ok(AppNetworkActive { id, active: false }),
            _ => Err("Expecting 'ACTIVE/DEACTIVE'".into()),
        }
    }
}
