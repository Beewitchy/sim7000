use heapless::String;

use super::{RequestType, CommandGroup, AtRequest, GenericOk};

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConnectMode {
    Tcp,
    Udp,
}

/// AT+CIPSTART=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Connect {
    /// Which connection slot to use (Multi-IP mode)
    pub number: usize,

    /// TCP or UDP
    pub mode: ConnectMode,

    /// IP or domain name
    pub destination: String<100>,

    pub port: u16,
}

impl AtRequest for Connect {
    type Response = GenericOk; // TODO: should have its own type
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        let mode = match self.mode {
            ConnectMode::Tcp => "TCP",
            ConnectMode::Udp => "UDP",
        };

        write!(
            buf,
            "+CIPSTART={},\"{mode}\",\"{}\",\"{}\"",
            self.number, self.destination.as_str(), self.port
        )
    }
}
