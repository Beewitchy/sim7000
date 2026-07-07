use super::{AtParseErr, AtParseLine, AtRequest, AtResponse, GenericOk, ResponseCode};

/// AT+CSQ
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GetSignalQuality;

impl AtRequest for GetSignalQuality {
    type Response = (SignalQuality, GenericOk);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        write!(buf, "AT+CSQ\r")
    }
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SignalQuality {
    pub rssi: Option<i32>,
    pub signal_quality: Option<f32>,
}

impl SignalQuality {
    /// Signal strength percenage
    pub fn signal_strength(&self) -> Option<f32> {
        self.rssi.map(|rssi| {
            // normalize rssi to 0, then percent can be calculated
            let normalized_rssi = rssi + 115;
            100.0 * (normalized_rssi as f32 / 63f32)
        })
    }
}

impl AtParseLine for SignalQuality {
    fn from_line(line: &str) -> Result<Self, AtParseErr> {
        let line = line.strip_prefix("+CSQ:").ok_or("Missing '+CSG: '")?.trim();
        let (rssi, ber) = line.split_once(',').ok_or("Missing ','")?;
        let rssi: u8 = rssi.parse()?;
        let ber: u8 = ber.parse()?;

        let rssi: Option<i32> = match rssi {
            0 => Some(-115),
            1 => Some(-111),
            i @ 2..=31 => Some(-110 + (i as i32 - 2) * 2),
            99 => None,
            _ => return Err("Invalid RSSI value".into()),
        };

        let bit_error_rate = match ber {
            0 => Some(0.14),
            1 => Some(0.28),
            2 => Some(0.57),
            3 => Some(1.13),
            4 => Some(2.26),
            5 => Some(4.53),
            6 => Some(9.05),
            7 => Some(18.10),
            99 => None,
            _ => return Err("Invalid BER value".into()),
        };

        let signal_quality = bit_error_rate.map(|ber| 100.0 - ber);

        Ok(SignalQuality {
            rssi,
            signal_quality,
        })
    }
}

impl AtResponse for SignalQuality {
    fn from_generic(code: &mut ResponseCode) -> Option<&mut Self> {
        match code {
            ResponseCode::SignalQuality(sq) => Some(sq),
            _ => None,
        }
    }
}
