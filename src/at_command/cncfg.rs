use heapless::String;

use super::{RequestType, CommandGroup, AtRequest, GenericOk};

/// AT+CNCFG=...
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct PdpConfigure {
    // The maximum length of an APN is 63 octets (bytes)
    pub apn: String<63>,
    pub username: String<50>,
    pub password: String<50>,
}

impl AtRequest for PdpConfigure {
    type Response = GenericOk;
    const TYPE: RequestType = RequestType::Command(CommandGroup::Extended);
    fn encode(&self, buf: &mut impl core::fmt::Write) -> core::fmt::Result {
        if !self.username.is_empty() || !self.password.is_empty() {
            write!(
                buf,
                "+CNCFG=0,0,\"{}\",\"{}\",\"{}\",3",
                self.apn.as_str(), self.username.as_str(), self.password.as_str(),
            )
        } else {
            write!(
                buf,
                "+CNCFG=0,0,\"{}\"",
                self.apn.as_str(),
            )
        }
    }
}
