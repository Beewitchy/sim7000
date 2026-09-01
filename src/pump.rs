use core::future::Future;
use embassy_futures::select::{Either4, select4};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::Sender,
    pipe::{Reader, Writer},
    signal::Signal,
    watch,
};
use embassy_time::Instant;
use embedded_io_async::{Read, Write};
use heapless::Vec;

use crate::{
    BuildIo, BuildIoConfig, Error, PowerState, SplitIo, at_command::unsolicited::{
        NetworkRegistration, RegistrationStatus,
    },
    log,
    modem::power::PowerSignalListener,
    read::ReadTimestamp,
};

/// Defines work that should be done for a communication task to
/// run the modem.
///
/// Each pump should be run in a loop in it's own embassy task
/// function.
///
/// You can implement error handling in that function, and may
/// also want to use signals to communicate with you main
/// application in case the modem needs to be rebooted after an
/// error.
pub trait Pump {
    type Err;

    /// Runs the communication logic for this pump.
    ///
    /// The future will an error if some unexpected behavior was
    /// observed during communication: when this happens you can
    /// restart the pump immediately, but may also want to power-
    /// cycle or reboot the modem to try and get it back into a
    /// working state.
    fn pump(&mut self) -> impl Future<Output = Result<(), Self::Err>>;
}

/// This pump handles IO with the modem, writing out buffered tx bytes
/// and reading bytes in to the rx buffer, along with read timestamps
/// to help command response processing determine when responses
/// were received.
pub struct RawIoPump<'context, RW, M: RawMutex, const BUF: usize> {
    pub(crate) io: RW,
    pub(crate) rx: Writer<'context, M, BUF>,
    pub(crate) tx: Reader<'context, M, BUF>,
    pub(crate) total_read: usize,
    pub(crate) read_timestamps: Sender<'context, M, ReadTimestamp, 8>,
    pub(crate) io_config: watch::Receiver<'context, M, BuildIoConfig, 1>,
    pub(crate) active_signal: PowerSignalListener<'context, M>,
    pub(crate) power_state: PowerState,
}

impl<'context, RW: 'context + BuildIo, M: RawMutex, const BUF: usize>
    RawIoPump<'context, RW, M, BUF>
{
    pub async fn high_power_pump(&mut self) -> Result<(), Error> {
        let config = self.io_config.get().await;
        let mut io = match self.io.build(&config) {
            Ok(io) => Some(io),
            Err(err) => {
                self.power_state = PowerState::Off;
                return Err(err.into());
            }
        };
        let (mut reader, mut writer) = RW::IO::split(&mut io);
        match select4(
            self.io_config.changed(),
            async {
                log::trace!("Begin Rx");
                loop {
                    let mut rx_buf = [0u8; BUF];
                    let bytes = match reader.read(&mut rx_buf).await {
                        Ok(0) => break Err(Error::Serial),
                        Ok(bytes) => bytes,
                        Err(_) => break Err(Error::Serial),
                    };
                    let timestamp = Instant::now();
                    log::trace!("Rx {:?}", &rx_buf[..bytes]);
                    // todo: ellie (25.08.2026) - Add a expect_timestamps flag on this pump that readers can set, then only send here if the flag is set
                    match self.read_timestamps.try_send(ReadTimestamp {
                        total_read: self.total_read,
                        timestamp,
                    }) {
                        Ok(()) => {},
                        Err(embassy_sync::channel::TrySendError::Full(_)) => log::warn!("Read timestamps buffer is full. Response read times will be inaccurate."),
                    }
                    self.total_read += bytes;
                    self.rx.write_all(&rx_buf[..bytes]).await;
                }
            },
            async {
                log::trace!("Begin Tx");
                loop {
                    let tx_buf = self.tx.fill_buf().await;
                    log::trace!("Tx {:?}", tx_buf);
                    match writer.write_all(tx_buf).await {
                        Ok(()) => {}
                        Err(_) => break Err(Error::Serial),
                    };
                }
            },
            self.active_signal.wait_for(PowerState::Off),
        )
        .await
        {
            Either4::First(_) => {
                // Config updated, cycle this task
                writer.flush().await.map_err(|_| Error::Serial)?;
            }
            Either4::Second(result) => {
                writer.flush().await.map_err(|_| Error::Serial)?;
                result?;
            }
            Either4::Third(result) => {
                result?;
            }
            Either4::Fourth(()) => {
                log::trace!("Pwr {:?}", &PowerState::Off);
                self.power_state = PowerState::Off;
                writer.flush().await.map_err(|_| Error::Serial)?;
            }
        }
        Ok(())
    }

    pub async fn low_power_pump(&mut self) {
        self.power_state = self.active_signal.wait_for_not(PowerState::Off).await;
    }
}

impl<'context, RW: 'context + BuildIo, M: RawMutex, const BUF: usize> Pump
    for RawIoPump<'context, RW, M, BUF>
{
    type Err = Error;

    async fn pump(&mut self) -> Result<(), Self::Err> {
        log::trace!("{:?}", self.power_state);
        if self.power_state != PowerState::Off {
            self.high_power_pump().await?;
        } else {
            self.low_power_pump().await;
        }

        Ok(())
    }
}

pub struct RegistrationHandler<'context, M: RawMutex> {
    context: &'context Signal<M, NetworkRegistration>,
}

impl<'context, M: RawMutex> RegistrationHandler<'context, M> {
    pub async fn pump(&mut self) {
        match self.context.wait().await.status {
            RegistrationStatus::NotRegistered
            | RegistrationStatus::Searching
            | RegistrationStatus::RegistrationDenied
            | RegistrationStatus::Unknown => todo!(),
            RegistrationStatus::RegisteredHome => todo!(),
            RegistrationStatus::RegisteredRoaming => todo!(),
        }
    }
}

#[macro_export]
macro_rules! pump_task {
    ($name:ident, $type:ty) => {
        #[embassy_executor::task]
        pub(crate) async fn $name(mut pump: $type) {
            use ::sim7000_async::pump::Pump;
            loop {
                if let Err(err) = pump.pump().await {
                    #[cfg(feature = "log")]
                    log::error!("Error pumping {} {:?}", stringify!($name), err);
                    #[cfg(feature = "defmt")]
                    defmt::error!("Error pumping {} {:?}", stringify!($name), err);
                }
            }
        }
    };
}
