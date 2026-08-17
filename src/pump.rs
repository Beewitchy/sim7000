use core::future::Future;
use embassy_futures::select::{Either, Either4, select, select4};
use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    channel::Sender,
    pipe::Pipe,
    signal::Signal,
    watch,
    zerocopy_channel::{Receiver as ZerocopyReceiver, Sender as ZerocopySender},
};
use embassy_time::{Duration, Instant, with_timeout};
use embedded_io_async::{Read, Write};
use heapless::Vec;

use crate::{
    BuildIo, BuildIoConfig, Error, PowerState, SplitIo, StateSignal,
    at_command::{
        AtParseErr, AtParseLine, ResponseCode, cfun,
        unsolicited::{
            self, GnssReport, NetworkRegistration, PowerDown, RegistrationStatus, Urc,
            VoltageWarning,
        },
    },
    log,
    modem::{AppNetworkMap, RawAtCommand, ReadyState, TcpContext, power::PowerSignalListener},
    read::ModemReader,
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

#[derive(Clone)]
pub struct RxLine {
    line: heapless::String<599>,
    views: usize,
}

impl RxLine {
    pub const fn new() -> Self {
        Self { line: heapless::String::new(), views: 0 }
    }
}

pub struct RxPump<'context, M: RawMutex, const TCP_SLOTS: usize> {
    pub(crate) reader: ModemReader<'context, M>,
    pub(crate) generic_response: ZerocopySender<'context, M, ResponseCode>,
    // pub(crate) lines_sender: ZerocopySender<'context, M, RxLine>,
    pub(crate) tcp: &'context TcpContext<M, TCP_SLOTS>,
    pub(crate) gnss: &'context Signal<M, GnssReport>,
    pub(crate) voltage_warning: &'context Signal<M, VoltageWarning>,
    pub(crate) local_time: watch::Sender<'context, M, unsolicited::Psuttz, 1>,
    pub(crate) ready: watch::Sender<'context, M, ReadyState, 2>,
    pub(crate) registration_events: &'context StateSignal<M, NetworkRegistration>,
    pub(crate) sms_indices: Sender<'context, M, unsolicited::NewSmsIndex, 5>,
    pub(crate) pdp_status: watch::Sender<'context, M, AppNetworkMap, 1>,
    pub(crate) active_signal: PowerSignalListener<'context, M>,
}

impl<'context, M: RawMutex, const TCP_SLOTS: usize> RxPump<'context, M, TCP_SLOTS> {
    /// Reset status that is no longer valid after power down
    pub fn clear_online_status(&mut self, new_state: PowerState) {
        if matches!(new_state, PowerState::Sleeping | PowerState::Off) {
            self.sms_indices.clear();
            self.pdp_status.clear();
        }
        if new_state == PowerState::Off {
            self.local_time.clear();
            self.ready.clear();
            self.registration_events.clear();
        }
    }
}

impl<'context, M, const TCP_SLOTS: usize> Pump for RxPump<'context, M, TCP_SLOTS>
where
    M: RawMutex,
{
    type Err = Error;

    async fn pump(&mut self) -> Result<(), Self::Err> {
        // Execute read_line then check power signal, in that order:
        //  read_line will return immediately if there is already
        //  data available, so the power signal will only be handled
        //  when reading is complete for the currently available data,
        //  making sure all packets are handled before resetting
        //  the Rx state.
        let (line, read_instant) = match select(
            self.reader.read_line(),
            self.active_signal.wait_for_changed_not(PowerState::On),
        )
        .await
        {
            Either::First(line) => line?,
            Either::Second(new_state) => {
                self.clear_online_status(new_state);
                return Ok(());
            }
        };

        if line.is_empty() {
            log::warn!("received empty line from modem");
        }

        let read_instant = read_instant.copied().unwrap_or_else(|| Instant::now());

        // First try to parse it as an unsolicited message
        match Urc::from_line(&line, &read_instant) {
            Err(AtParseErr::Mismatch) => {
                // Fall through to ResponseCode parser
            }
            Err(AtParseErr::Parsing(_err)) => {
                log::error!("error parsing urc: '{:?}', error: {:?}", line, _err);
                return Err(Error::UnknownResponse);
            }
            Ok(message) => {
                log::debug!("Got URC: {:?}", line);
                match message {
                    Urc::NetworkRegistration(registration) => {
                        log::info!("registration status: {:?}", registration);
                        self.registration_events.signal(registration);
                        return Ok(());
                    }
                    Urc::ReceiveHeader(header) => {
                        let mut length = header.length;
                        let connection = header.connection;
                        log::debug!("Reading {} bytes from modem", length);
                        while length > 0 {
                            log::debug!("remaining read: {}", length);
                            let mut buf = Vec::<u8, 365>::new();
                            buf.resize_default(usize::min(length, buf.capacity()))
                                .unwrap();
                            self.reader.read_exact(&mut buf).await?;
                            length -= buf.len();
                            log::debug!(
                                "Sending {} bytes to tcp connection {}",
                                buf.len(),
                                connection
                            );
                            self.tcp.slots[connection].peek().rx.write_all(&buf).await;
                            log::debug!("Bytes sent to tcp connection {}", connection);
                        }
                        log::debug!("Done sending to tcp connection {}", connection);
                        return Ok(());
                    }
                    Urc::Cmti(message) => {
                        if let Err(e) = self.sms_indices.try_send(message) {
                            log::debug!("Failed to send SMS index: {:?}", e);
                            return Err(Error::Transmit);
                        }
                        return Ok(());
                    }
                    Urc::ConnectionMessage(message) => {
                        let slot = &self.tcp.slots[message.index];
                        slot.peek().events.send(message.message);
                        return Ok(());
                    }
                    Urc::Dst(dst) => {
                        log::debug!("Got dst update {:?}.", dst);
                        self.local_time.send_modify(move |local_time| {
                            let current = local_time.get_or_insert_default();
                            current.tz_offset = Some(crate::at_command::cclk::types::set_dst(
                                current.tz_offset,
                                dst,
                            ));
                        });
                        return Ok(());
                    }
                    Urc::GnssReport(report) => {
                        self.gnss.signal(report);
                        return Ok(());
                    }
                    Urc::VoltageWarning(warning) => {
                        self.voltage_warning.signal(warning);
                        return Ok(());
                    }
                    Urc::PowerDown(PowerDown::UnderVoltage) => {
                        self.ready.send(ReadyState::PowerDown);
                        self.voltage_warning.signal(VoltageWarning::UnderVoltage);
                        return Ok(());
                    }
                    Urc::PowerDown(PowerDown::OverVoltage) => {
                        self.ready.send(ReadyState::PowerDown);
                        self.voltage_warning.signal(VoltageWarning::OverVoltage);
                        return Ok(());
                    }
                    Urc::PowerDown(PowerDown::Normal) => {
                        self.ready.send(ReadyState::PowerDown);
                        // This can actually be a solicited response but without a
                        // stateful parser there's no way to know, so just assume
                        // it is and put it in the response queue
                        let mut buf = self.generic_response.send().await;
                        *buf = ResponseCode::PowerDown(PowerDown::Normal);
                        buf.send_done();
                        return Ok(());
                    }
                    Urc::Psuttz(ttz) => {
                        log::debug!("Got local-time update {:?}.", ttz);
                        self.local_time.send(ttz);
                        return Ok(());
                    }
                    Urc::CPin(cpin) => {
                        match cpin {
                            unsolicited::CPin::Ready => self.ready.send(ReadyState::SimReady),
                            unsolicited::CPin::NotInserted | unsolicited::CPin::NotReady => {}
                        }
                        // This can actually be a solicited response but without a
                        // stateful parser there's no way to know, so just assume
                        // it is and put it in the response queue
                        let mut buf = self.generic_response.send().await;
                        *buf = ResponseCode::CPin(cpin);
                        buf.send_done();
                        return Ok(());
                    }
                    Urc::CFun(cfun) => {
                        match cfun.0 {
                            cfun::Functionality::Full => self.ready.send(ReadyState::Ready),
                            _ => {}
                        }
                        return Ok(());
                    }
                    Urc::AppNetworkActive(unsolicited::AppNetworkActive { id, active }) => {
                        self.pdp_status.send_modify(|status| {
                            if let Some(id) = id {
                                let _ = status.get_or_insert_default().status.insert(id, active);
                            }
                        });
                        return Ok(());
                    }
                    Urc::Ready(unsolicited::Ready) => {
                        self.ready.send(ReadyState::Ready);
                        return Ok(());
                    }
                    Urc::SmsReady(unsolicited::SmsReady) => {
                        self.ready.send(ReadyState::SmsReady);
                        return Ok(());
                    }
                    _ => log::warn!("Unhandled URC: {:?}", message),
                }
                return Ok(());
            }
        }
        // If it's not a URC, try to parse it as a regular response code
        match ResponseCode::from_line(&line, &read_instant) {
            Err(AtParseErr::Mismatch) => {}
            Err(AtParseErr::Parsing(_err)) => {
                log::error!("error parsing response: {:?}, error: {:?}", line, _err);
                return Err(Error::UnknownResponse);
            }
            Ok(mut response) => {
                // Sms messages are a bit of a special case,
                // first comes the info and then the message on a new line
                // and a sms message can't be unambiguously parsed seperatly
                if let ResponseCode::SmsMessage(sms) = &mut response {
                    log::info!("Got SMS from: {:?}, reading message", sms.sender);
                    let (line, _) = self.reader.read_line().await?;

                    if line.is_empty() {
                        log::warn!("received empty line from modem");
                    }

                    sms.message = line[..line.len()].try_into().unwrap_or_default();
                } else {
                    log::debug!("Got generic response: {:?}", response);
                }
                let mut buf =
                    with_timeout(Duration::from_secs(10), self.generic_response.send()).await?;
                *buf = response;
                buf.send_done();
                return Ok(());
            }
        }
        log::warn!("Got unknown response: {:?}", line);
        // Present the error to user code (via the pump runner) so it can be handled there
        Err(Error::UnknownResponse)
    }
}

pub struct TxPump<'context, M: RawMutex> {
    pub(crate) writer: &'context Pipe<M, 2048>,
    pub(crate) commands: ZerocopyReceiver<'context, M, RawAtCommand>,
}

impl<'context, M> Pump for TxPump<'context, M>
where
    M: RawMutex,
{
    type Err = Error;

    async fn pump(&mut self) -> Result<(), Self::Err> {
        let command = self.commands.receive().await;
        #[cfg(feature = "defmt")]
        if command.binary {
            log::trace!("Write to modem: {=[u8]:x}", command.bytes.as_slice());
        } else {
            log::trace!("Write to modem: {=[u8]:a}", command.bytes.as_slice());
        }
        #[cfg(not(feature = "defmt"))]
        log::trace!("Write to modem: {:?}", command.as_bytes());

        // `Writer` is infallible. It is fine to ignore these errors.
        let _ = self.writer.write_all(command.as_bytes()).await;
        let _ = self.writer.flush().await;

        command.receive_done();

        Ok(())
    }
}

pub struct RawIoPump<'context, RW, M: RawMutex> {
    pub(crate) io: RW,
    /// sends data to the rx pump
    pub(crate) rx: &'context Pipe<M, 2048>,
    /// reads data from the tx pump
    pub(crate) tx: &'context Pipe<M, 2048>,
    pub(crate) io_config: watch::Receiver<'context, M, BuildIoConfig, 1>,
    pub(crate) active_signal: PowerSignalListener<'context, M>,
    pub(crate) power_state: PowerState,
}

impl<'context, RW: 'context + BuildIo, M: RawMutex> RawIoPump<'context, RW, M> {
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
                let mut rx_buf = [0u8; 256];
                log::trace!("Begin Rx");
                loop {
                    let bytes = match reader.read(&mut rx_buf).await {
                        Ok(0) => break Err(Error::Serial),
                        Ok(bytes) => bytes,
                        Err(_) => break Err(Error::Serial),
                    };
                    log::trace!("Rx {:?}", &rx_buf[..bytes]);
                    self.rx.write_all(&rx_buf[..bytes]).await;
                }
            },
            async {
                let mut tx_buf = [0u8; 256];
                log::trace!("Begin Tx");
                loop {
                    let bytes = self.tx.read(&mut tx_buf).await;
                    log::trace!("Tx {:?}", &tx_buf[..bytes]);
                    match writer.write_all(&tx_buf[..bytes]).await {
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

impl<'context, RW: 'context + BuildIo, M: RawMutex> Pump for RawIoPump<'context, RW, M> {
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
