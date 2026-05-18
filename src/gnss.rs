use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Timer};
use futures::{FutureExt, select_biased};

use crate::at_command::{cgnsinf, unsolicited::{GnssFix, GnssReport}};
use crate::drop::{AsyncDrop, DropChannel, DropMessage};
use crate::modem::CommandRunner;
use crate::modem::power::PowerSignalListener;
use crate::{PowerState, log};

pub const GNSS_SLOTS: usize = 1;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Closed;

pub struct Gnss<'x, 'c, M: RawMutex> {
    /// Receiver of GnssReports.
    ///
    /// A value of None indicates that the modem will not send any more reports.
    reports: Option<&'x Signal<M, GnssReport>>,
    commands: &'x embassy_sync::mutex::Mutex<M, CommandRunner<'c, M>>,
    power_signal: PowerSignalListener<'x, M>,
    _drop: AsyncDrop<'x, M>,

    /// The timeout value for waiting for a report.
    timeout: Duration,
}

impl<'x, 'c, M> Gnss<'x, 'c, M>
where
    M: RawMutex,
{
    pub(crate) fn new(
        reports: &'x Signal<M, GnssReport>,
        commands: &'x embassy_sync::mutex::Mutex<M, CommandRunner<'c, M>>,
        power_signal: PowerSignalListener<'x, M>,
        drop_channel: &'x DropChannel<M>,
        timeout: Duration,
    ) -> Self {
        Gnss {
            reports: Some(reports),
            commands,
            power_signal,
            _drop: AsyncDrop::new(drop_channel, DropMessage::Gnss),
            timeout,
        }
    }

    /// Wait until the next GNSS report.
    pub async fn get_report(&mut self) -> Result<GnssReport, Closed> {
        // TODO: SIM7000
        self.commands.lock().await.run(cgnsinf::GetGnssReport).await.map_err(|_| Closed)?;

        let reports = self.reports.ok_or(Closed)?;
        select_biased! {
            report = reports.wait().fuse() => Ok(report),
            _ = self.power_signal.wait_for(PowerState::Off).fuse() => {
                self.reports = None;
                Err(Closed)
            }
            _ = Timer::after(self.timeout).fuse() => {
                log::warn!("Gnss timed out");
                self.reports = None;
                Err(Closed)
            }
        }
    }

    /// Wait until the GNSS reports a fix on our location.
    pub async fn get_fix(&mut self) -> Result<GnssFix, Closed> {
        loop {
            if let GnssReport::Fix(fix) = self.get_report().await? {
                return Ok(fix);
            }
        }
    }
}
