use embassy_futures::select::{Either, select};
use embassy_sync::{blocking_mutex::raw::RawMutex, channel, mutex::Mutex, signal::Signal, watch};
use embassy_time::Instant;
use heapless::Vec;

use crate::{
    Error, PowerState, StateSignal,
    at_command::{AtParseErr, AtParseLine as _, cfun, unsolicited},
    modem::{AppNetworkMap, ReadyState, TcpContext, power::PowerSignalListener},
    read,
};

pub trait Runner {
    fn run(
        &mut self,
        modem_reader: &mut read::ModemReader<'_>,
    ) -> impl Future<Output = Result<(), Error>>;
}

pub trait Parser {
    fn parse(
        &mut self,
        line: &str,
        instant: &Instant,
    ) -> impl Future<Output = Result<Option<unsolicited::Urc>, Error>>;
    fn parse_more(
        &mut self,
        modem_reader: &mut read::ModemReader<'_>,
        urc: unsolicited::Urc,
    ) -> impl Future<Output = Result<Option<unsolicited::Urc>, Error>>;
    fn parse_all(
        &mut self,
        modem_reader: &mut read::ModemReader<'_>,
        urc: unsolicited::Urc,
    ) -> impl Future<Output = Result<(), Error>> {
        async {
            let mut urc = urc;
            loop {
                match self.parse_more(modem_reader, urc).await {
                    Ok(Some(next)) => urc = next,
                    Ok(None) => break Ok(()),
                    Err(err) => break Err(err),
                }
            }
        }
    }
}

pub struct UnsolicitedMessageRunner<'r, 'context, M: RawMutex, const TCP_SLOTS: usize> {
    state: &'r Mutex<M, UnsolicitedMessageState<'context, M, TCP_SLOTS>>,
}

impl<'r, 'context, M: RawMutex, const TCP_SLOTS: usize>
    UnsolicitedMessageRunner<'r, 'context, M, TCP_SLOTS>
{
    pub fn new(state: &'r Mutex<M, UnsolicitedMessageState<'context, M, TCP_SLOTS>>) -> Self {
        Self { state }
    }
}

impl<'r, 'context, M: RawMutex, const TCP_SLOTS: usize> Runner
    for UnsolicitedMessageRunner<'r, 'context, M, TCP_SLOTS>
{
    async fn run(&mut self, modem_reader: &mut read::ModemReader<'_>) -> Result<(), Error> {
        self.state.lock().await.run(modem_reader).await
    }
}


impl<'context, M: RawMutex, const TCP_SLOTS: usize> Runner
    for UnsolicitedMessageState<'context, M, TCP_SLOTS>
{
    async fn run(&mut self, modem_reader: &mut read::ModemReader<'_>) -> Result<(), Error> {
        self.run(modem_reader).await
    }
}

impl<'r, 'context, M: RawMutex, const TCP_SLOTS: usize> Parser
    for UnsolicitedMessageRunner<'r, 'context, M, TCP_SLOTS>
{
    async fn parse(
        &mut self,
        line: &str,
        instant: &Instant,
    ) -> Result<Option<unsolicited::Urc>, Error> {
        self.state.lock().await.parse(line, instant)
    }

    async fn parse_more(
        &mut self,
        modem_reader: &mut read::ModemReader<'_>,
        urc: unsolicited::Urc,
    ) -> Result<Option<unsolicited::Urc>, Error> {
        self.state.lock().await.parse_more(modem_reader, urc).await
    }

    async fn parse_all(
        &mut self,
        modem_reader: &mut read::ModemReader<'_>,
        urc: unsolicited::Urc,
    ) -> Result<(), Error>
    {
        self.state.lock().await.parse_all(modem_reader, urc).await
    }
}

impl<'context, M: RawMutex, const TCP_SLOTS: usize> Parser
    for UnsolicitedMessageState<'context, M, TCP_SLOTS>
{
    async fn parse(
        &mut self,
        line: &str,
        instant: &Instant,
    ) -> Result<Option<unsolicited::Urc>, Error> {
        self.parse(line, instant)
    }

    async fn parse_more(
        &mut self,
        modem_reader: &mut read::ModemReader<'_>,
        urc: unsolicited::Urc,
    ) -> Result<Option<unsolicited::Urc>, Error> {
        self.parse_more(modem_reader, urc).await
    }
}

pub struct UnsolicitedMessageState<'context, M: RawMutex, const TCP_SLOTS: usize> {
    pub(crate) tcp: &'context TcpContext<M, TCP_SLOTS>,
    pub(crate) gnss: &'context Signal<M, unsolicited::GnssReport>,
    pub(crate) voltage_warning: &'context Signal<M, unsolicited::VoltageWarning>,
    pub(crate) local_time: watch::Sender<'context, M, unsolicited::Psuttz, 1>,
    pub(crate) ready: watch::Sender<'context, M, ReadyState, 2>,
    pub(crate) registration_events: &'context StateSignal<M, unsolicited::NetworkRegistration>,
    pub(crate) sms_indices: channel::Sender<'context, M, unsolicited::NewSmsIndex, 5>,
    pub(crate) pdp_status: watch::Sender<'context, M, AppNetworkMap, 1>,
    pub(crate) active_signal: PowerSignalListener<'context, M>,
}

impl<'context, M: RawMutex, const TCP_SLOTS: usize>
    UnsolicitedMessageState<'context, M, TCP_SLOTS>
{
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

    pub async fn run(&mut self, modem_reader: &mut read::ModemReader<'_>) -> Result<(), Error> {
        loop {
            // Execute read_line then check power signal, in that order:
            //  read_line will return immediately if there is already
            //  data available, so the power signal will only be handled
            //  when reading is complete for the currently available data,
            //  making sure all packets are handled before resetting
            //  the Rx state.
            let (line, read_instant) = match select(
                modem_reader.read_line(),
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
            if let Some(urc) = self.parse(line, read_instant.unwrap_or(&Instant::now()))? {
                self.parse_all(modem_reader, urc).await?;
            }
        }
    }

    pub fn parse(
        &mut self,
        line: &str,
        read_instant: &Instant,
    ) -> Result<Option<unsolicited::Urc>, Error> {
        match unsolicited::Urc::from_line(&line, &read_instant) {
            Err(AtParseErr::Mismatch) => {
                return Err(Error::UnknownResponse);
            }
            Err(AtParseErr::Parsing(_err)) => {
                log::error!("error parsing urc: '{:?}', error: {:?}", line, _err);
                return Err(Error::UnknownResponse);
            }
            Ok(message) => {
                log::debug!("Got URC: {:?}", line);
                match message {
                    unsolicited::Urc::NetworkRegistration(registration) => {
                        log::info!("registration status: {:?}", registration);
                        self.registration_events.signal(registration);
                    }
                    unsolicited::Urc::ReceiveHeader(header) => {
                        return Ok(Some(unsolicited::Urc::ReceiveHeader(header)));
                    }
                    unsolicited::Urc::Cmti(message) => {
                        if let Err(e) = self.sms_indices.try_send(message) {
                            log::debug!("Failed to send SMS index: {:?}", e);
                            return Err(Error::Transmit);
                        }
                    }
                    unsolicited::Urc::ConnectionMessage(message) => {
                        let slot = &self.tcp.slots[message.index];
                        slot.peek().events.send(message.message);
                    }
                    unsolicited::Urc::Dst(dst) => {
                        log::debug!("Got dst update {:?}.", dst);
                        self.local_time.send_modify(move |local_time| {
                            let current = local_time.get_or_insert_default();
                            current.tz_offset = Some(crate::at_command::cclk::types::set_dst(
                                current.tz_offset,
                                dst,
                            ));
                        });
                    }
                    unsolicited::Urc::GnssReport(report) => {
                        self.gnss.signal(report);
                    }
                    unsolicited::Urc::VoltageWarning(warning) => {
                        self.voltage_warning.signal(warning);
                    }
                    unsolicited::Urc::PowerDown(unsolicited::PowerDown::UnderVoltage) => {
                        self.ready.send(ReadyState::PowerDown);
                        self.voltage_warning
                            .signal(unsolicited::VoltageWarning::UnderVoltage);
                    }
                    unsolicited::Urc::PowerDown(unsolicited::PowerDown::OverVoltage) => {
                        self.ready.send(ReadyState::PowerDown);
                        self.voltage_warning
                            .signal(unsolicited::VoltageWarning::OverVoltage);
                    }
                    unsolicited::Urc::PowerDown(unsolicited::PowerDown::Normal) => {
                        self.ready.send(ReadyState::PowerDown);
                    }
                    unsolicited::Urc::Psuttz(ttz) => {
                        log::debug!("Got local-time update {:?}.", ttz);
                        self.local_time.send(ttz);
                    }
                    unsolicited::Urc::CPin(cpin) => match cpin {
                        unsolicited::CPin::Ready => self.ready.send(ReadyState::SimReady),
                        unsolicited::CPin::NotInserted | unsolicited::CPin::NotReady => {}
                    },
                    unsolicited::Urc::CFun(cfun) => match cfun.0 {
                        cfun::Functionality::Full => self.ready.send(ReadyState::Ready),
                        _ => {}
                    },
                    unsolicited::Urc::AppNetworkActive(unsolicited::AppNetworkActive {
                        id,
                        active,
                    }) => {
                        self.pdp_status.send_modify(|status| {
                            if let Some(id) = id {
                                let _ = status.get_or_insert_default().status.insert(id, active);
                            }
                        });
                    }
                    unsolicited::Urc::Ready(unsolicited::Ready) => {
                        self.ready.send(ReadyState::Ready);
                    }
                    unsolicited::Urc::SmsReady(unsolicited::SmsReady) => {
                        self.ready.send(ReadyState::SmsReady);
                    }
                    _ => log::warn!("Unhandled URC: {:?}", message),
                }
            }
        }
        Ok(None)
    }

    pub async fn parse_more(
        &mut self,
        modem_reader: &mut read::ModemReader<'_>,
        urc: unsolicited::Urc,
    ) -> Result<Option<unsolicited::Urc>, Error> {
        match urc {
            unsolicited::Urc::ReceiveHeader(header) => {
                let mut length = header.length;
                let connection = header.connection;
                log::debug!("Reading {} bytes from modem", length);
                while length > 0 {
                    log::debug!("remaining read: {}", length);
                    let mut buf = Vec::<u8, 365>::new();
                    buf.resize_default(usize::min(length, buf.capacity()))
                        .unwrap();
                    modem_reader.read_exact(&mut buf).await?;
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
            }
            _ => {}
        }
        Ok(None)
    }
}
