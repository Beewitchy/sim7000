//! Unsolicited Response Codes

use embassy_time::Instant;
use super::{AtParseErr, AtParseLine};

mod app_pdp;
mod cbm;
mod cds;
mod cereg;
mod cfun;
mod cgreg;
mod cmt;
mod cmti;
mod connection;
mod cpin;
mod creg;
mod cring;
mod ctzv;
mod cusd;
mod dst;
mod network_registration;
mod pdp;
mod power_down;
mod psnwid;
mod psuttz;
mod rdy;
mod receive;
mod remote_ip;
mod sms_ready;
mod ugnsinf;
mod voltage_warning;

pub use app_pdp::{AppNetworkActive};
pub use cbm::Cbm;
pub use cds::Cds;
pub use cfun::CFun;
pub use cmt::Cmt;
pub use cmti::NewSmsIndex;
pub use connection::{Connection, ConnectionMessage};
pub use cpin::CPin;
pub use cring::CRing;
pub use ctzv::Ctzv;
pub use cusd::CUsd;
pub use dst::Dst;
pub use network_registration::{NetworkRegistration, RegistrationStatus};
pub use pdp::GprsDisconnected;
pub use power_down::PowerDown;
pub use psnwid::Pdnwid;
pub use psuttz::Psuttz;
pub use rdy::Ready;
pub use receive::ReceiveHeader;
pub use remote_ip::IncomingConnection;
pub use sms_ready::SmsReady;
pub use ugnsinf::{DateTime, GnssFix, GnssReport};
pub use voltage_warning::VoltageWarning;

/// Unsolicited Response Code
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Urc {
    AppNetworkActive(AppNetworkActive),
    Cbm(Cbm),
    Cds(Cds),
    CFun(CFun),
    Cmt(Cmt),
    Cmti(NewSmsIndex),
    CPin(CPin),
    CRing(CRing),
    CUsd(CUsd),
    ConnectionMessage(Connection),
    Ctzv(Ctzv),
    Dst(Dst),
    GnssReport(GnssReport),
    GprsDisconnected(GprsDisconnected),
    Pdnwid(Pdnwid),
    PowerDown(PowerDown),
    Psuttz(Psuttz),
    Ready(Ready),
    SmsReady(SmsReady),
    ReceiveHeader(ReceiveHeader),
    NetworkRegistration(NetworkRegistration),
    VoltageWarning(VoltageWarning),
}

impl AtParseLine for Urc {
    fn from_line(line: &str, instant: &Instant) -> Result<Self, AtParseErr> {
        /// Returns a function that tries to parse the line into a Urc::T
        fn parse<'a, T: AtParseLine>(
            line: &'a str,
            instant: &'a embassy_time::Instant,
            f: impl Fn(T) -> Urc + 'a,
        ) -> impl Fn() -> Option<Result<Urc, AtParseErr>> + 'a {
            move || match T::from_line(line, instant) {
                Err(AtParseErr::Mismatch) => None,
                Err(err) => Some(Err(err)),
                Ok(response) => Some(Ok(f(response))),
            }
        }

        None
            .or_else(parse(line, instant, Urc::AppNetworkActive))
            .or_else(parse(line, instant, Urc::Cbm))
            .or_else(parse(line, instant, Urc::Cds))
            .or_else(parse(line, instant, Urc::CFun))
            .or_else(parse(line, instant, Urc::Cmt))
            .or_else(parse(line, instant, Urc::Cmti))
            .or_else(parse(line, instant, Urc::CPin))
            .or_else(parse(line, instant, Urc::CRing))
            .or_else(parse(line, instant, Urc::CUsd))
            .or_else(parse(line, instant, Urc::ConnectionMessage))
            .or_else(parse(line, instant, Urc::Ctzv))
            .or_else(parse(line, instant, Urc::Dst))
            .or_else(parse(line, instant, Urc::GnssReport))
            .or_else(parse(line, instant, Urc::GprsDisconnected))
            .or_else(parse(line, instant, Urc::Pdnwid))
            .or_else(parse(line, instant, Urc::PowerDown))
            .or_else(parse(line, instant, Urc::Psuttz))
            .or_else(parse(line, instant, Urc::Ready))
            .or_else(parse(line, instant, Urc::SmsReady))
            .or_else(parse(line, instant, Urc::ReceiveHeader))
            .or_else(parse(line, instant, Urc::NetworkRegistration))
            .or_else(parse(line, instant, Urc::VoltageWarning))
            .unwrap_or(Err(AtParseErr::Mismatch))
    }
}

// TODO
//mod cdnsgip
//mod cmt;
//mod cbm;
//mod cds;
