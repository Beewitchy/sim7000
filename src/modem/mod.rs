mod command;
mod context;
pub mod power;

use crate::{
    BuildIo, Error, ModemPower, PowerState,
    at_command::{
        At, AtRequest, CharacterSet, GetPinStatus, NetworkMode, SelectMessageService,
        SetSmsMessageFormat, SetTeCharacterSet, ShowSystemMode, SmsMessageFormat, ate, cbatchk,
        ccid, cclk,
        cedrxs::{self, AcTType, EDRXSetting, EdrxCycleLength},
        cereg,
        cfgri::{self, RiPinMode},
        cfun, cgact, cgmr, cgnapn, cgnscold, cgnscpy,
        cgnsmod::{self, WorkMode},
        cgnspwr, cgnsurc, cgnsxtra, cgreg,
        cmee::{self, CMEErrorMode},
        cmgd::{DeleteFlag, DeleteSms},
        cmgr::{ReadSms, SmsMessage},
        cmgs::{self, SendSmsMessage},
        cmnb::{self, NbMode},
        cnact, cncfg,
        cnmi::{SetSmsIndication, SmsIndicationMode, SmsMtMode},
        cnmp, cntp, cntpcid, cops, cpowd,
        cpsi::{self},
        creg, csclk, csq, gsn, httptofs,
        ipr::{self, BaudRate},
        unsolicited::{CPin, NetworkRegistration, NewSmsIndex, RegistrationStatus},
    },
    gnss::Gnss,
    log,
    pump::{RawIoPump, RxPump, TxPump},
    read::ModemReader,
    tcp::{ConnectError, TcpStream},
    voltage::VoltageWarner,
};
pub use command::{AT_DEFAULT_TIMEOUT, CommandRunner, RawAtCommand};
pub use context::*;
use embassy_sync::{
    blocking_mutex::raw::RawMutex, channel::Receiver, mutex::Mutex, signal::Signal,
};
use embassy_time::{Duration, Timer, with_timeout};
use futures::{FutureExt, select_biased};
use heapless::{String, Vec};

use self::{command::ExpectResponse, power::PowerSignalBroadcaster};

pub struct Uninitialized;
pub struct Disabled;
pub struct Enabled;
pub struct Sleeping;

// todo: ellie (17.05.2026) - Implement different modem behaviors
pub trait ModemBehavior<M: RawMutex> {
    fn post_init(&self, commands: &mut CommandRunner<'_, M>) -> Result<(), Error>;
    fn authenticate(&self, commands: &mut CommandRunner<'_, M>) -> Result<(), Error>;
}

pub struct Modem<'m, P, M: RawMutex, const N: usize> {
    context: &'m Shared<M, N>,
    active_signal: PowerSignalBroadcaster<'m>,
    commands: Mutex<M, CommandRunner<'m, M>>,
    power: P,
    apn: Option<heapless::String<63>>,
    ap_username: &'static str,
    ap_password: &'static str,
    automatic_registration: bool,
    user_network_priority: Vec<RadioAccessTechnology, 3>,
    current_network_priority: Vec<RadioAccessTechnology, 3>,
    /// Time given to each RAT before trying the next
    auto_reg_timeout: Duration,
    reg_retries: usize,
}

const MODEM_POWER_TIMEOUT: Duration = Duration::from_secs(30);
const NET_REG_DEFAULT: NetworkRegistration = NetworkRegistration {
    status: RegistrationStatus::NotRegistered,
    lac: None,
    ci: None,
};

/// Helper macro that repeatedly attempts to evaluate an expression that returns a result.
///
/// Returns the Result yielded by the expression if
/// - the expression returns `Ok` at any point,
/// - or the expression returns `Err` $attempts time in a row.
macro_rules! try_retry {
    (($label:literal, $attempts:literal, $delay: expr), $e:expr) => {{
        let mut attempt = 0;
        loop {
            let r = $e;

            if r.is_ok() || attempt >= $attempts {
                break r;
            }

            attempt += 1;
            log::warn!(
                "{} failed, attempt {}/{}, retrying after {:?}",
                $label,
                attempt,
                $attempts,
                $delay
            );
            Timer::after($delay).await;
        }
    }};
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ReadyState {
    #[default]
    None,
    Ready,
    SmsReady,
    PowerDown,
}

impl<'m, P: ModemPower, M: RawMutex, const TCP_SLOTS: usize> Modem<'m, P, M, TCP_SLOTS> {
    pub fn new<'c, I: BuildIo>(
        io: I,
        power: P,
        context: &'m mut ModemContext<'c, M, TCP_SLOTS>,
    ) -> Result<
        (
            Modem<'m, P, M, TCP_SLOTS>,
            RawIoPump<'m, I, M>,
            TxPump<'m, M>,
            RxPump<'m, M, TCP_SLOTS>,
        ),
        Error,
    > {
        let (commands_sender, commands_receiver) = context.packet_channels.commands.split();
        let (response_sender, response_receiver) = context.packet_channels.generic_response.split();
        let commands = Mutex::new(
            CommandRunner::new(commands_sender, response_receiver)
                .with_timeout(Some(AT_DEFAULT_TIMEOUT)),
        );
        let modem = Modem {
            commands,
            active_signal: context.shared.active_signal.publisher(),
            context: &context.shared,
            power,
            apn: None,
            ap_username: "",
            ap_password: "",
            automatic_registration: false,
            user_network_priority: [
                RadioAccessTechnology::LteCatM1,
                RadioAccessTechnology::Gsm,
                RadioAccessTechnology::LteNbIot,
            ]
            .into_iter()
            .collect(),
            current_network_priority: [
                RadioAccessTechnology::LteCatM1,
                RadioAccessTechnology::Gsm,
                RadioAccessTechnology::LteNbIot,
            ]
            .into_iter()
            .collect(),
            auto_reg_timeout: Duration::from_secs(60),
            reg_retries: 0,
        };

        let io_pump = RawIoPump {
            io,
            rx: &context.shared.rx_pipe,
            tx: &context.shared.tx_pipe,
            power_state: PowerState::Off,
            active_signal: context.shared.active_signal.subscribe(),
        };

        let rx_pump = RxPump {
            reader: ModemReader::new(&context.shared.rx_pipe),
            generic_response: response_sender,
            registration_events: &context.shared.registration_events,
            tcp: &context.shared.tcp,
            gnss: context.shared.gnss_slot.peek(),
            voltage_warning: context.shared.voltage_slot.peek(),
            ready: context.shared.ready.sender(),
            sms_indices: context.shared.sms_indices.sender(),
            pdp_status: context.shared.pdp_status.sender(),
        };

        let tx_pump = TxPump {
            writer: &context.shared.tx_pipe,
            commands: commands_receiver,
        };

        Ok((modem, io_pump, tx_pump, rx_pump))
    }

    async fn wait_for_ready(&mut self, max_tries: u64) -> Result<(), Error> {
        // Wait for ready state, cycle power if necessary
        let Some(mut ready) = self.context.ready.receiver() else {
            return Err(Error::InvalidContext);
        };
        if matches!(self.power.state(), PowerState::Off) {
            with_timeout(MODEM_POWER_TIMEOUT, self.power.enable()).await?;
        }
        self.active_signal.broadcast(PowerState::On);
        for attempt in 0..max_tries {
            if with_timeout(
                Duration::from_secs(attempt * 5),
                ready.get_and(|ready| matches!(ready, ReadyState::Ready)),
            )
            .await
            .is_ok()
            {
                break;
            }
            // It's possible the RDY message from the modem was missed--
            //  try sending an AT to get a response. I believe a non-error
            //  response indicates the modem is up
            if let Ok(mut commands) = self.commands.try_lock() {
                if commands.run(At).await.is_ok() {
                    break;
                }
            }
            // Deactivating and rebooting seems to be often not needed,
            //  so only try it every other attempt
            if attempt % 2 != 0 {
                self.deactivate().await?;
                with_timeout(MODEM_POWER_TIMEOUT, self.power.enable()).await?;
            }
        }
        Ok(())
    }

    pub async fn init(&mut self, config: RegistrationConfig) -> Result<(), Error> {
        log::info!("initializing modem");

        self.wait_for_ready(3).await?;

        let mut commands = self.commands.lock().await;

        // todo: ellie (16.05.2026) - flow control configuration
        // let set_flow_control = ifc::SetFlowControl {
        //     dce_by_dte: ifc::FlowControl::Hardware,
        //     dte_by_dce: ifc::FlowControl::Hardware,
        // };

        // // Turn on hardware flow control, the modem does not save this state on reboot.
        // // We need to set it as fast as possible to avoid dropping bytes.
        // for _ in 0..5 {
        //     if let Ok(Ok(_)) = with_timeout(Duration::from_millis(2000), async {
        //         commands.run(set_flow_control).await
        //     })
        //     .await
        //     {
        //         break;
        //     }
        // }

        try_retry!(
            ("AT", 4, Duration::from_millis(250)),
            commands.run(At).await
        )?;

        // todo: ellie (16.05.2026) - sleep & slow-clock configuration
        commands.run(csclk::SetSlowClock(false)).await?;

        commands.run(ipr::SetBaudRate(BaudRate::Hz115200)).await?;
        // commands.run(set_flow_control).await?;
        commands
            .run(cmee::ConfigureCMEErrors(CMEErrorMode::Numeric))
            .await?;

        match config.network_mode {
            NetworkModeConfig::Automatic {
                priority,
                timeout,
                reg_retries,
            } => {
                if let Some(prio) = priority {
                    self.current_network_priority = prio;
                }
                self.automatic_registration = true;
                self.auto_reg_timeout = timeout;
                self.reg_retries = reg_retries;
            }
            NetworkModeConfig::Manual {
                network_mode,
                nb_mode,
            } => {
                commands.run(cnmp::SetNetworkMode(network_mode)).await?;
                commands.run(cmnb::SetNbMode(nb_mode)).await?;
            }
        }

        // todo: ellie (16.05.2026) - add Ri interrupt pin & bat check configuration
        commands.run(cfgri::ConfigureRiPin(RiPinMode::Off)).await?;
        commands.run(cbatchk::EnableVBatCheck(false)).await?;

        // TODO: SIM7000
        // let current_edrx = commands
        //     .run(cedrxs::GetEDRXSetting)
        //     .await
        //     .ok()
        //     .map(|(current_edrx, _)| current_edrx);

        // let configure_edrx = cedrxs::ConfigureEDRX::from(config.edrx);
        // if current_edrx.is_none_or(|current_edrx| current_edrx != configure_edrx) {
        //     try_retry!(
        //         ("CEDRX", 5, Duration::from_millis(200)),
        //         commands.run(configure_edrx).await
        //     )?;
        // }

        drop(commands);

        Ok(())
    }

    pub fn set_apn(&mut self, apn: Option<heapless::String<63>>) {
        self.apn = apn;
    }

    pub fn set_ap_username(&mut self, ap_username: &'static str) {
        self.ap_username = ap_username;
    }

    pub fn set_ap_password(&mut self, ap_password: &'static str) {
        self.ap_password = ap_password;
    }

    pub async fn activate(&mut self) -> Result<(), Error> {
        log::info!("activating modem");

        self.async_drop().await?;

        self.wait_for_ready(2).await?;

        let mut commands = self.commands.lock().await;

        // let set_flow_control = ifc::SetFlowControl {
        //     dce_by_dte: ifc::FlowControl::Hardware,
        //     dte_by_dce: ifc::FlowControl::Hardware,
        // };

        // for _ in 0..5 {
        //     if let Ok(Ok(_)) = with_timeout(Duration::from_millis(2000), async {
        //         commands.run(set_flow_control).await
        //     })
        //     .await
        //     {
        //         break;
        //     }
        // }
        commands.run(ate::SetEcho(false)).await?;
        commands
            .run(cmee::ConfigureCMEErrors(CMEErrorMode::Numeric))
            .await?;

        let _ = with_timeout(Duration::from_secs(30), self.wait_for_sim(&mut commands))
            .await
            .map_err(|_| Error::SimUnavailable)?;

        let _ = commands
            .run(cfun::SetFunctionality(cfun::Functionality::Full, None))
            .await?;

        // CREG, CEREG, and CGREG are each necessary based on what network mode we're using
        // (GSM, LTE, etc). But for simplicity's sake we set up URCs for all of them. This is also
        // what Simcom recommends. These commands can fail spuriously though, so we run each in a
        // retry loop up to 5 times.
        try_retry!(
            ("CREG", 5, Duration::from_secs(1)),
            commands
                .run(creg::ConfigureRegistrationUrc::EnableReg)
                .await
        )?;
        try_retry!(
            ("CEREG", 5, Duration::from_secs(1)),
            commands
                .run(cereg::ConfigureRegistrationUrc::EnableReg)
                .await
        )?;
        try_retry!(
            ("CGREG", 5, Duration::from_secs(1)),
            commands
                .run(cgreg::ConfigureRegistrationUrc::EnableReg)
                .await
        )?;

        let _ = commands.run(cgreg::GetRegistrationStatus).await;

        if self.automatic_registration {
            let active_mode = self.automatic_registration(&mut commands).await?;

            // re-order the priority list
            if let Some(index) = self
                .current_network_priority
                .iter()
                .position(|mode| *mode == active_mode)
            {
                let element = self.current_network_priority.remove(index);
                self.current_network_priority
                    .insert(0, element)
                    .expect("we just removed an element");
            }
        } else {
            for _ in 0..self.reg_retries + 1 {
                let _ = commands.run(cgreg::GetRegistrationStatus).await;
                match self.wait_for_registration().await {
                    Ok(_) => break,
                    _ => {}
                }
                embassy_time::Timer::after_secs(2).await;
            }
        }
        let (_pdp_states, _) = commands.run(cgact::GetPdpContextActivation).await?;
        log::info!("registered to network");

        // TODO: SIM7000
        // commands.run(cipshut::ShutConnections).await?;

        let apn = match &self.apn {
            Some(apn) => apn,
            None => {
                log::debug!("no default APN set, checking network for suggested APN.");
                let (network_apn, _) = commands.run(cgnapn::GetNetworkApn).await?;
                let Some(apn) = network_apn.apn else {
                    log::debug!("no APN set");
                    return Err(Error::NoApn);
                };
                self.apn.insert(apn)
            }
        };

        log::info!("authenticating with apn {:?}", apn);

        commands
            .run(cncfg::PdpConfigure {
                apn: self.apn.as_ref().ok_or(Error::NoApn)?.clone(),
                username: self.ap_username.try_into().unwrap_or_default(),
                password: self.ap_password.try_into().unwrap_or_default(),
            })
            .await?;

        commands
            .run(cnact::SetAppNetworkPDP(cnact::CNActPDP {
                pdp_index: 0,
                mode: cnact::CnactMode::Active,
                address: None,
            }))
            .await?;

        let _ = commands.run(ShowSystemMode).await?;

        // todo: ellie (17.05.2026) - different command set compatibility
        // TODO: SIM7000
        // let _ = commands.run(cipmux::EnableMultiIpConnection(true)).await;
        // commands
        //     .run(cstt::StartTask {
        //         apn: apn.clone(),
        //         username: self.ap_username.try_into().unwrap_or_default(),
        //         password: self.ap_password.try_into().unwrap_or_default(),
        //     })
        //     .await?;

        // datasheet specifies 85 seconds max response time
        // commands
        //     .run_with_timeout(Some(Duration::from_secs(86)), ciicr::StartGprs)
        //     .await?;

        // let (_ip, _) = commands.run(cifsrex::GetLocalIpExt).await?;

        log::info!("modem successfully activated");
        Ok(())
    }

    /// Resets the network priority to the priority provided when initializing [Modem::init] with [NetworkModeConfig::Automatic]
    ///
    /// If not initialized with [NetworkModeConfig::Automatic], this function has no effect
    pub fn reset_network_priority(&mut self) {
        self.current_network_priority = self.user_network_priority.clone();
    }

    /// Connect to the first available radio access technology (RAT).
    /// If connected using LTE-CatM or GSM, set that RAT as first priority for next registration attempt
    ///
    /// Returns which technology ends up being used, or a timeout error
    async fn automatic_registration(
        &self,
        commands: &mut CommandRunner<'_, M>,
    ) -> Result<RadioAccessTechnology, Error> {
        for mode in &self.current_network_priority {
            // Sometimes SIM isn't detected--cycle it to try
            // get it working
            for retry in 0..self.reg_retries + 1 {
                match mode {
                    RadioAccessTechnology::LteCatM1 => {
                        commands.run(cnmp::SetNetworkMode(NetworkMode::Lte)).await?;
                        commands.run(cmnb::SetNbMode(NbMode::CatM)).await?;
                    }
                    RadioAccessTechnology::Gsm => {
                        commands.run(cnmp::SetNetworkMode(NetworkMode::Gsm)).await?;
                    }
                    RadioAccessTechnology::LteNbIot => {
                        commands.run(cnmp::SetNetworkMode(NetworkMode::Lte)).await?;
                        commands.run(cmnb::SetNbMode(NbMode::NbIot)).await?;
                    }
                }

                log::info!("Trying {:?}...", mode);
                let _ = commands.run(cgreg::GetRegistrationStatus).await;
                match with_timeout(self.auto_reg_timeout, self.wait_for_registration()).await {
                    Ok(Ok(_)) => {
                        log::info!("Registered using {:?}", mode);
                        return Ok(*mode);
                    }
                    _ => {}
                }

                log::debug!("Retry {}...", retry);
                embassy_time::Timer::after_secs(2).await;
            }
        }

        Err(Error::Timeout)
    }

    async fn wait_for_sim(&self, commands: &mut CommandRunner<'_, M>) -> Result<(), Error> {
        loop {
            let (pin_status, _) = commands.run(GetPinStatus).await?;
            match pin_status {
                CPin::NotReady => {}
                CPin::NotInserted => {}
                CPin::Ready => return Ok(()),
            }
        }
    }

    pub async fn deactivate(&mut self) -> Result<(), Error> {
        if !matches!(self.power.state(), PowerState::Off)
            && matches!(self.context.ready.try_get(), Some(ReadyState::Ready))
        {
            log::trace!("sending power-down command");
            let mut commands = self.commands.lock().await;
            // result ignored because power-off should proceed regardless
            let _ = commands
                .run_with_timeout(
                    Some(Duration::from_secs(10)),
                    cpowd::PowerDown(cpowd::Mode::Normal),
                )
                .await;
        }
        self.context.sms_state.signal(SmsState::Unavailable);
        self.active_signal.broadcast(PowerState::Off);
        self.context.registration_events.signal(NET_REG_DEFAULT);
        self.context.tcp.disconnect_all().await;

        if !matches!(self.power.state(), PowerState::Off) {
            with_timeout(MODEM_POWER_TIMEOUT, self.power.disable())
                .await
                .map_err(Error::from)?;
        }

        // Run drop work after powering down: this allows drop commands
        //  to avoid unnecessary work for the powered-down state
        let drop_result = self.async_drop().await;

        self.context.ready.sender().send(ReadyState::None);

        drop_result
    }

    pub async fn reset(&mut self) -> Result<(), Error> {
        // Flush drops before resetting--since the system will come
        //  back up it may be useful to run the commands in online
        //  mode
        let drop_result = self.async_drop().await;

        self.active_signal.broadcast(PowerState::Off);
        self.context.registration_events.signal(NET_REG_DEFAULT);
        self.context.tcp.disconnect_all().await;
        // modem needs to be enabled for reset
        if let PowerState::Off = self.power.state() {
            self.power.enable().await;
        }
        self.power.reset().await;

        drop_result
    }

    /// Wait until the modem has registered to a cell tower.
    pub async fn wait_for_registration(&self) -> Result<(), Error> {
        log::debug!("waiting for cell registration");
        let wait_for_registration = async move {
            self.context
                .registration_events
                .compare_wait(|r| {
                    [
                        RegistrationStatus::RegisteredHome,
                        RegistrationStatus::RegisteredRoaming,
                    ]
                    .contains(&r.status)
                })
                .await;
        };

        let warn_on_long_wait = async {
            for i in 1.. {
                Timer::after(Duration::from_secs(20)).await;
                log::warn!(
                    "modem registration seems to be taking a long time ({}s)...",
                    i * 20
                );
            }
        };

        select_biased! {
            _ = wait_for_registration.fuse() => Ok(()),
            _ = warn_on_long_wait.fuse() => unreachable!(),
            _ = Timer::after(Duration::from_secs(10 * 60)).fuse() => Err(Error::Timeout),
        }
    }

    /// Execute queued drop commands
    ///
    /// You should call this between dropping & re-establishing sockets in order
    ///  to clean up state.
    ///
    /// This is done manually to allow the drop process to take exclusive control of the commands channels.
    pub async fn async_drop(&mut self) -> Result<(), Error> {
        let drop_channel = self.context.drop_channel.receiver();
        if drop_channel.is_empty() {
            return Ok(())
        }
        let mut active_signal = self.context.active_signal.subscribe();
        let mut current_power_state = self.power.state();
        while !drop_channel.is_empty() {
            select_biased! {
                power_state = active_signal.listen().fuse() => {
                    current_power_state = power_state;
                }
                drop_message = drop_channel.receive().fuse() => {
                    if current_power_state == PowerState::On {
                        // run drop command, abort if power state changes
                        let result = select_biased! {
                            power_state = active_signal.listen().fuse() => {
                                current_power_state = power_state;
                                Ok(())
                            }
                            result = async {
                                // run drop command
                                let mut runner = self.commands.lock().await;
                                drop_message.run(&mut runner).await
                            }.fuse() => result,
                        };

                        // clean up regardless of whether drop command succeeded
                        drop_message.clean_up(self.context);
                        result?;
                    } else {
                        drop_message.clean_up(self.context);
                    }
                },
            }
        }

        Ok(())
    }

    pub async fn activate_sms(&mut self) -> Result<(), Error> {
        self.async_drop().await?;

        let mut commands = self.commands.lock().await;
        // Set up SMS stuff
        try_retry!(
            ("CMGF", 5, Duration::from_secs(1)),
            commands
                .run(SetSmsMessageFormat(SmsMessageFormat::Text))
                .await
        )?;
        commands.run(SelectMessageService).await?;
        commands.run(SetTeCharacterSet(CharacterSet::GSM)).await?;
        commands
            .run(SetSmsIndication {
                mode: SmsIndicationMode::BufferWhenLinkBusy,
                routing: SmsMtMode::Index,
            })
            .await?;
        self.context.sms_state.signal(SmsState::Available);
        Ok(())
    }

    pub async fn get_sms_stream(&mut self) -> (SmsStream<'_, 'm, M>, SmsSignal<'m, M>) {
        let sms_indicies = self.context.sms_indices.receiver();
        (
            SmsStream {
                sms_indicies,
                commands: &self.commands,
                // state: &self.context.sms_state,
            },
            SmsSignal {
                inner: &self.context.sms_state,
            },
        )
    }
    pub async fn send_sms(&mut self, destination: &str, message: &str) -> Result<(), Error> {
        self.async_drop().await?;

        let mut commands = self.commands.lock().await;
        commands
            .run(cmgs::SendSms {
                destination: destination.try_into().unwrap_or_default(),
            })
            .await?;
        commands
            .run(SendSmsMessage(message.try_into().unwrap_or_default()))
            .await?;
        Ok(())
    }

    pub async fn read_sms(&mut self) -> Result<SmsMessage, Error> {
        self.async_drop().await?;

        let index =
            with_timeout(Duration::from_secs(1), self.context.sms_indices.receive()).await?;
        log::info!("Reading SMS at index: {:?}", index);

        let mut commands = self.commands.lock().await;
        let (sms, _) = commands.run(ReadSms { index: index.index }).await?;
        if let Err(e) = commands
            .run(DeleteSms(DeleteFlag::Index(index.index)))
            .await
        {
            log::warn!("Failed to delete sms: {:?}", e);
        };

        Ok(sms)
    }

    pub async fn connect_tcp(
        &mut self,
        host: &str,
        port: u16,
    ) -> Result<TcpStream<'_, 'm, M>, ConnectError> {
        self.async_drop().await?;

        let tcp_context = self.context.tcp.claim().ok_or(ConnectError::NoFreeSlots)?;

        TcpStream::connect(
            tcp_context,
            host,
            port,
            &self.context.drop_channel,
            &self.commands,
        )
        .await
    }

    pub async fn configure_gnss_background(&mut self) -> Result<(), Error> {
        let mut commands = self.commands.lock().await;
        commands
            .run(cgnsurc::ConfigureGnssUrc {
            period: 4, // TODO
        })
            .await?;
        Ok(())
    }

    /// Start gnss & enable the work-modes for the given systems
    ///
    /// The work mode priority list will be used to select either the first available or next
    ///  available system, based on the current settings.
    pub async fn claim_gnss(
        &mut self,
        work_mode_priority: &WorkModePriority<GnssSystem, 4>,
    ) -> Result<Option<Gnss<'_, 'm, M>>, Error> {
        self.async_drop().await?;

        let Some(reports) = self.context.gnss_slot.claim() else {
            return Ok(None);
        };

        let mut commands = self.commands.lock().await;

        commands.run(cgnspwr::SetGnssPower(true)).await?;

        let (current_set, _) = commands.run(cgnsmod::GetGnssWorkModeSet).await?;

        let allow_multiple = (current_set.glonass as u8
            + current_set.beidou as u8
            + current_set.galilean as u8
            + current_set.qzss.unwrap_or(WorkMode::Stop) as u8)
            > 1;

        let mut workmode_set = cgnsmod::GnssWorkModeSet {
            glonass: WorkMode::Stop,
            beidou: WorkMode::Stop,
            galilean: WorkMode::Stop,
            qzss: current_set.qzss.map(|_| WorkMode::Stop),
        };
        for system in &work_mode_priority.list {
            match system {
                GnssSystem::Galileo => {
                    if matches!(work_mode_priority.selection, SelectionMode::First)
                        || current_set.galilean == cgnsmod::WorkMode::Stop
                    {
                        workmode_set.galilean = WorkMode::Start;
                    }
                }
                GnssSystem::BeiDou => {
                    if matches!(work_mode_priority.selection, SelectionMode::First)
                        || current_set.beidou == cgnsmod::WorkMode::Stop
                    {
                        workmode_set.beidou = WorkMode::Start;
                    }
                }
                GnssSystem::GLONASS => {
                    if matches!(work_mode_priority.selection, SelectionMode::First)
                        || current_set.glonass == cgnsmod::WorkMode::Stop
                    {
                        workmode_set.glonass = WorkMode::Start;
                    }
                }
                GnssSystem::QZSS => {
                    if matches!(work_mode_priority.selection, SelectionMode::First)
                        || current_set.qzss == Some(cgnsmod::WorkMode::Stop)
                    {
                        workmode_set.qzss = Some(WorkMode::Start);
                    }
                }
            }
            if !allow_multiple {
                break;
            }
        }
        commands
            .run(cgnsmod::SetGnssWorkModeSet(Some(workmode_set)))
            .await?;

        Ok(Some(Gnss::new(
            reports,
            &self.commands,
            self.context.active_signal.subscribe(),
            &self.context.drop_channel,
            Duration::from_secs(20),
        )))
    }

    /// Sync the network time protocol
    pub async fn sync_ntp(
        &mut self,
        ntp_server: &str,
        timezone: u16,
    ) -> Result<cntp::NetworkTime, Error> {
        self.async_drop().await?;

        let mut commands = self.commands.lock().await;

        commands
            .run(cnact::SetAppNetworkPDP(cnact::CNActPDP {
                pdp_index: 1,
                mode: cnact::CnactMode::Active,
                address: None,
            }))
            .await?;

        // TODO: SIM7000
        // let apn = self.apn.as_ref().ok_or(Error::NoApn)?.clone();
        // commands
        //     .run(sapbr::BearerSettings {
        //         cmd_type: sapbr::CmdType::SetBearerParameters,
        //         con_param_type: sapbr::ConParamType::Apn,
        //         apn: apn.clone(),
        //     })
        //     .await?;
        // commands
        //     .run(sapbr::BearerSettings {
        //         cmd_type: sapbr::CmdType::OpenBearer,
        //         con_param_type: sapbr::ConParamType::Apn,
        //         apn,
        //     })
        //     .await?;

        commands.run(cntpcid::SetGprsBearerProfileId(1)).await?;

        commands
            .run(cntp::SynchronizeNetworkTime {
                ntp_server: ntp_server.try_into().unwrap_or_default(),
                timezone,
                cid: 1,
            })
            .await?;
        let (_, network_time) = commands
            .run_with_timeout(Some(Duration::from_secs(60)), cntp::Execute)
            .await?;

        commands
            .run(cnact::SetAppNetworkPDP(cnact::CNActPDP {
                pdp_index: 1,
                mode: cnact::CnactMode::Deactive,
                address: None,
            }))
            .await?;

        Ok(network_time)
    }

    pub async fn query_local_time(&mut self) -> Result<cclk::UtcTime, Error> {
        self.run_command_with_timeout(Some(Duration::from_secs(60)), cclk::GetTime::new())
            .await
            .map(|(utc, _)| utc.time)
    }

    pub async fn download_xtra(
        &mut self,
        urls: impl IntoIterator<Item = &str>,
        system: Option<GnssSystem>,
    ) -> Result<(), Error> {
        self.async_drop().await?;

        // XTRA file server:
        // 1. iot1.xtracloud.net
        // 2. iot2.xtracloud.net
        // 3. iot3.xtracloud.net
        // XTRA file:
        // 1. GPS+GLO: xtra3gr_72h.bin
        // 2. GPS+BDS: xtra3gc_72h.bin
        // 3. GPS+GAL: xtra3ge_72h.bin
        // 4. GPS+QZSS: xtra3gj_72h.bin
        // 5. GPS: xtra3g_72h.bin
        let xtra_file = match system {
            Some(GnssSystem::GLONASS) => "xtra3gr_72h.bin",
            Some(GnssSystem::BeiDou) => "xtra3gc_72h.bin",
            Some(GnssSystem::Galileo) => "xtra3ge_72h.bin",
            Some(GnssSystem::QZSS) => "xtra3gj_72h.bin",
            None => "xtra3g_72h.bin",
        };

        let mut commands = self.commands.lock().await;

        let (status, _) = commands.run(cnact::GetAppNetworkPDP).await?;
        let activate_pdp = !status
            .into_iter()
            .find_map(|item| {
                if item.pdp_index == 0 {
                    Some(matches!(item.mode, cnact::CnactMode::Active))
                } else {
                    None
                }
            })
            .unwrap_or(false);
        if activate_pdp {
            commands
                .run(cnact::SetAppNetworkPDP(cnact::CNActPDP {
                    pdp_index: 0,
                    mode: cnact::CnactMode::Active,
                    address: None,
                }))
                .await?;
        }

        let _ = commands
            .run_with_timeout(
                Some(Duration::from_secs(30)),
                cntp::EnableLocalTimestamp(true),
            )
            .await?;

        // TODO: SIM7000
        // commands
        //     .run(cnact::SetAppNetwork {
        //         mode: cnact::CnactMode::Active,
        //         apn: self.apn.as_ref().ok_or(Error::NoApn)?.clone(),
        //     })
        //     .await?;

        // sometimes we aren't able to download the file the first couple of times
        let retry_count = 5;
        let timeout = 50;
        let command_timeout = Some(Duration::from_secs(retry_count as u64 * timeout as u64 + 1));
        let mut status_code = httptofs::StatusCode::BadRequest;
        for url in urls {
            let mut url: heapless::String<64> = url.try_into().map_err(|_| Error::BufferOverflow)?;
            url.push('/').map_err(|_| Error::BufferOverflow)?;
            url.push_str(xtra_file).map_err(|_| Error::BufferOverflow)?;
            let dl_info = commands
                .run_with_timeout(
                    command_timeout,
                    httptofs::DownloadToFileSystem {
                        // unclear which xtra file to use, the size differs depending on server
                        // so they might contain more/different data or different satellite networks
                        // also, sometimes the server is scuffed
                        url,
                        file_path: "/customer/Xtra3.bin".try_into().unwrap_or_default(),
                        retry_count: Some(retry_count),
                        timeout: Some(timeout),
                    },
                )
                .await?
                .1;
            status_code = dl_info.status_code;
            if status_code.success().is_ok() && dl_info.data_length > 0 {
                break;
            }
        }

        if activate_pdp {
            commands
                .run(cnact::SetAppNetworkPDP(cnact::CNActPDP {
                    pdp_index: 0,
                    mode: cnact::CnactMode::Deactive,
                    address: None,
                }))
                .await?;
        }

        status_code.success().map_err(Error::Httptofs)
    }

    /// Enable the use of XTRA file for faster, more accurate GNSS fixes. Similar to assisted gps.
    ///
    /// Before calling this function, make sure the XTRA file has been downloaded. [Modem::download_xtra]
    pub async fn cold_start_with_xtra(&mut self) -> Result<cgnsxtra::GnssXtraInfo, Error> {
        self.async_drop().await?;

        let mut commands = self.commands.lock().await;

        commands.run(cgnscpy::CopyXtraFile).await?.0.success()?;
        let (info, _) = commands.run(cgnsxtra::ValidateGnssXtra).await?;
        commands
            .run(cgnsxtra::GnssXtra(cgnsxtra::ToggleXtra::Enable))
            .await?;

        commands.run(cgnscold::GnssColdStart).await?.1.success()?;

        Ok(info)
    }

    pub async fn claim_voltage_warner(&mut self) -> Option<VoltageWarner<'_, M>> {
        VoltageWarner::take(&self.context.voltage_slot)
    }

    /// Run a single AT command on the modem. Use with care.
    pub async fn run_command<C, Response>(&self, command: C) -> Result<Response, Error>
    where
        C: AtRequest<Response = Response>,
        Response: ExpectResponse<M>,
    {
        self.commands.lock().await.run(command).await
    }

    /// Run a single AT command on the modem with the specified timeout. Use with care.
    pub async fn run_command_with_timeout<C, Response>(
        &self,
        timeout: Option<Duration>,
        command: C,
    ) -> Result<Response, Error>
    where
        C: AtRequest<Response = Response>,
        Response: ExpectResponse<M>,
    {
        self.commands
            .lock()
            .await
            .run_with_timeout(timeout, command)
            .await
    }

    pub async fn query_system_info(&mut self) -> Result<cpsi::SystemInfo, Error> {
        self.run_command(cpsi::GetSystemInfo)
            .await
            .map(|(system_info, _)| system_info)
    }

    pub async fn query_signal(&mut self) -> Result<csq::SignalQuality, Error> {
        self.run_command(csq::GetSignalQuality)
            .await
            .map(|(response, _)| response)
    }

    /// Query the current cellular network operator.
    ///
    /// This command can take up to 120 seconds to run.
    pub async fn query_operator_info(&mut self) -> Result<cops::OperatorInfo, Error> {
        // max response time is 120 seconds
        self.run_command_with_timeout(Some(Duration::from_secs(121)), cops::GetOperatorInfo)
            .await
            .map(|(response, _)| response)
    }

    pub async fn query_ip(&mut self) -> Result<heapless::Vec<cnact::CNActPDP, 4>, Error> {
        try_retry!(
            ("CNACT", 5, Duration::from_millis(500)),
            self.run_command_with_timeout(Some(Duration::from_secs(1)), cnact::GetAppNetworkPDP)
                .await
        )
        .map(|(response, _)| response)
    }

    pub async fn query_iccid(&mut self) -> Result<ccid::Iccid, Error> {
        self.run_command(ccid::ShowIccid)
            .await
            .map(|(response, _)| response)
    }

    pub async fn query_imei(&mut self) -> Result<String<16>, Error> {
        self.run_command(gsn::GetImei)
            .await
            .map(|(response, _)| response.imei)
    }

    pub async fn query_firmware_version(&mut self) -> Result<cgmr::FwVersion, Error> {
        self.run_command(cgmr::GetFwVersion)
            .await
            .map(|(response, _)| response)
    }

    pub async fn sleep(&mut self) -> Result<(), Error> {
        let drop_result = self.async_drop().await;
        self.active_signal.broadcast(PowerState::Sleeping);
        self.power.sleep().await;
        drop_result
    }

    pub async fn wake(&mut self) {
        self.power.wake().await;
        self.active_signal.broadcast(PowerState::On);
    }
}

pub struct SmsStream<'a, 'c, M: RawMutex> {
    sms_indicies: Receiver<'a, M, NewSmsIndex, 5>,
    commands: &'a Mutex<M, CommandRunner<'c, M>>,
    // state: &'a Signal<CriticalSectionRawMutex, SmsState>,
}

pub struct SmsSignal<'a, M: RawMutex> {
    inner: &'a Signal<M, SmsState>,
}

impl<M: RawMutex> SmsSignal<'_, M> {
    pub async fn wait_for_available(&self) {
        loop {
            let state = self.inner.wait().await;
            if let SmsState::Available = state {
                return;
            }
        }
    }

    pub async fn wait_for_unavailable(&self) {
        loop {
            let state = self.inner.wait().await;
            if let SmsState::Unavailable = state {
                return;
            }
        }
    }
}

pub(crate) enum SmsState {
    Available,
    Unavailable,
}

impl<M: RawMutex> SmsStream<'_, '_, M> {
    pub async fn read_sms(&mut self) -> Result<SmsMessage, Error> {
        let index = with_timeout(Duration::from_secs(1), self.sms_indicies.receive()).await?;
        log::info!("Reading SMS at index: {:?}", index);

        let mut commands = self.commands.lock().await;
        let (sms, _) = commands.run(ReadSms { index: index.index }).await?;

        if index.index > 8 {
            // avoid filling up the sms storage with old messages that for some reason wasn't deleted
            let _ = commands.run(DeleteSms(DeleteFlag::All)).await;
        } else {
            let _ = commands
                .run(DeleteSms(DeleteFlag::Index(index.index)))
                .await;
        }

        Ok(sms)
    }

    pub async fn send_sms(&mut self, destination: &str, message: &str) -> Result<(), Error> {
        let mut commands = self.commands.lock().await;
        commands
            .run_with_timeout(
                Some(Duration::from_secs(10)),
                cmgs::SendSms {
                    destination: destination.try_into().unwrap_or_default(),
                },
            )
            .await?;
        commands
            .run(SendSmsMessage(message.try_into().unwrap_or_default()))
            .await?;
        Ok(())
    }
}

/// Configure cellular mobile communication and edrx.
pub struct RegistrationConfig {
    pub network_mode: NetworkModeConfig,
    pub edrx: EDRXConfig,
}

#[derive(PartialEq, Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RadioAccessTechnology {
    LteCatM1,
    LteNbIot,
    Gsm,
}

#[derive(PartialEq)]
pub enum NetworkModeConfig {
    /// Custom automatic, not Simcom automatic.
    ///
    /// Goes through the priority list in order. Connects to first available RAT, which will also be set as first priority for next time.
    Automatic {
        /// If none, priority will be: Lte-CatM > GSM > Lte-NbIoT
        priority: Option<Vec<RadioAccessTechnology, 3>>,
        /// How much time is given for each radio access technology before trying the next
        timeout: Duration,
        /// How many times to retry registration immediately
        ///  Sometimes this helps the sim connect quicker,
        ///  rather than redoing the full activation.
        reg_retries: usize,
    },
    /// The modules built-in modes
    Manual {
        network_mode: NetworkMode,
        nb_mode: NbMode,
    },
}

/// Configuration of Extended Discontinuous Reception mode
pub enum EDRXConfig {
    Disabled,
    Enabled {
        auto_report: bool,
        act_type: AcTType,
        cycle_length: EdrxCycleLength,
    },
}

impl Default for RegistrationConfig {
    fn default() -> Self {
        RegistrationConfig {
            network_mode: NetworkModeConfig::Automatic {
                priority: None,
                timeout: Duration::from_secs(2 * 60),
                reg_retries: 1,
            },
            edrx: EDRXConfig::Disabled,
        }
    }
}

impl From<EDRXConfig> for cedrxs::ConfigureEDRX {
    fn from(value: EDRXConfig) -> Self {
        match value {
            EDRXConfig::Disabled => cedrxs::ConfigureEDRX {
                n: EDRXSetting::Disable,
                // these values don't matter.
                act_type: AcTType::CatM,
                requested_edrx_value: EdrxCycleLength::_5,
            },
            EDRXConfig::Enabled {
                auto_report,
                act_type,
                cycle_length,
            } => cedrxs::ConfigureEDRX {
                n: if auto_report {
                    EDRXSetting::EnableWithAutoReport
                } else {
                    EDRXSetting::Enable
                },
                act_type,
                requested_edrx_value: cycle_length,
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GnssSystem {
    Galileo,
    BeiDou,
    GLONASS,
    QZSS,
}

#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SelectionMode {
    First,
    Next,
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct WorkModePriority<T, const N: usize> {
    pub list: heapless::Vec<T, N>,
    pub selection: SelectionMode,
}
