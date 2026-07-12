use crate::at_command::{AtParseErr, AtParseLine};

/// Indicates whether the app network is active
#[derive(Debug)]
pub struct IncomingConnection {
    pub remote_ip: core::net::IpAddr,
}

impl AtParseLine for IncomingConnection {
    fn from_line(line: &str, _instant: &embassy_time::Instant) -> Result<Self, AtParseErr> {
        let ip = line
            .strip_prefix("REMOTE IP:")
            .ok_or(AtParseErr::Mismatch)?
            .trim();

        use core::str::FromStr as _;

        let remote_ip =
            core::net::IpAddr::from_str(ip).map_err(|_| "Failed to parse IP address")?;

        Ok(IncomingConnection { remote_ip })
    }
}
