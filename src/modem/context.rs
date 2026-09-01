use embassy_sync::{
    blocking_mutex::raw::RawMutex, channel::Channel, pipe::Pipe, signal::Signal, watch::Watch,
};

use super::{ReadyState, SmsState, power::PowerSignal};
use crate::{
    BuildIoConfig, StateSignal,
    at_command::unsolicited::{
        ConnectionMessage, GnssReport, NetworkRegistration, NewSmsIndex, Psuttz,
        RegistrationStatus, VoltageWarning,
    },
    drop::DropChannel,
    read::ReadTimestamp,
    slot::Slot,
    tcp::TCP_RX_BUF_LEN,
    util::{Lagged, RingChannel},
};

pub type TcpRxPipe<M> = Pipe<M, TCP_RX_BUF_LEN>;
pub type TcpEventChannel<M> = RingChannel<M, ConnectionMessage, 8>;

#[derive(Default, Clone, Debug)]
pub struct AppNetworkMap {
    pub status: heapless::LinearMap<u8, bool, 3>,
}

#[cfg(feature = "defmt")]
impl defmt::Format for AppNetworkMap {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "AppNetworkMap {{ ");
        for (k, v) in &self.status {
            defmt::write!(fmt, "{}: {}, ", k, v);
        }
        defmt::write!(fmt, "}}");
    }
}

pub struct Shared<M: RawMutex, const TCP_SLOTS: usize> {
    pub(crate) active_signal: PowerSignal<M>,
    pub(crate) drop_channel: DropChannel<M>,
    pub(crate) sms_indices: Channel<M, NewSmsIndex, 5>,
    pub(crate) sms_state: Signal<M, SmsState>,
    pub(crate) ready: Watch<M, ReadyState, 2>,
    pub(crate) pdp_status: Watch<M, AppNetworkMap, 1>,
    pub(crate) local_time: Watch<M, Psuttz, 1>,
    pub(crate) registration_events: StateSignal<M, NetworkRegistration>,
    pub(crate) tcp: TcpContext<M, TCP_SLOTS>,
    pub(crate) gnss_slot: Slot<Signal<M, GnssReport>>,
    pub(crate) voltage_slot: Slot<Signal<M, VoltageWarning>>,
    pub(crate) io_config: Watch<M, BuildIoConfig, 1>,
}

pub struct ModemContext<M: RawMutex, const TCP_SLOTS: usize, const BUF_LEN: usize> {
    pub(crate) shared: Shared<M, TCP_SLOTS>,
    pub(crate) tx_pipe: Pipe<M, BUF_LEN>,
    pub(crate) rx_pipe: Pipe<M, BUF_LEN>,
    pub(crate) rx_timestamps: Channel<M, ReadTimestamp, 8>,
}

impl<M, const TCP_SLOTS: usize> Shared<M, TCP_SLOTS>
where
    M: RawMutex,
{
    pub const fn new(tcp: TcpContext<M, TCP_SLOTS>) -> Self {
        Self {
            active_signal: PowerSignal::new(),
            drop_channel: DropChannel::new(),
            sms_indices: Channel::new(),
            sms_state: Signal::new(),
            ready: Watch::new_with(ReadyState::PowerDown),
            pdp_status: Watch::new(),
            local_time: Watch::new(),
            registration_events: StateSignal::new(NetworkRegistration {
                status: RegistrationStatus::Unknown,
                lac: None,
                ci: None,
            }),
            tcp,
            gnss_slot: Slot::new(Signal::new()),
            voltage_slot: Slot::new(Signal::new()),
            io_config: Watch::new(),
        }
    }
}

impl<M, const TCP_SLOTS: usize, const BUF_LEN: usize> ModemContext<M, TCP_SLOTS, BUF_LEN>
where
    M: RawMutex,
{
    pub fn new(tcp: TcpContext<M, TCP_SLOTS>) -> Self {
        Self {
            shared: Shared::new(tcp),
            tx_pipe: Pipe::new(),
            rx_pipe: Pipe::new(),
            rx_timestamps: Channel::new(),
        }
    }
}

pub struct TcpSlot<M: RawMutex> {
    pub rx: TcpRxPipe<M>,
    pub events: TcpEventChannel<M>,
}

pub struct TcpContext<M: RawMutex, const N: usize> {
    pub(crate) slots: [Slot<TcpSlot<M>>; N],
}

impl<M> TcpSlot<M>
where
    M: RawMutex,
{
    pub const fn new() -> Self {
        TcpSlot {
            rx: Pipe::new(),
            events: TcpEventChannel::new(),
        }
    }
}

impl<M, const N: usize> TcpContext<M, N>
where
    M: RawMutex,
{
    pub const fn new(slots: [Slot<TcpSlot<M>>; N]) -> Self {
        Self { slots }
    }

    pub fn claim(&self) -> Option<TcpToken<'_, M>> {
        self.slots.iter().enumerate().find_map(|(i, slot)| {
            let TcpSlot { rx, events } = slot.claim()?; // find an unclaimed slot
            Some(TcpToken {
                ordinal: i,
                rx,
                events,
            })
        })
    }

    pub async fn disconnect_all(&self) {
        for slot in &self.slots {
            if slot.is_claimed() {
                slot.peek().events.send(ConnectionMessage::Closed);
            }
        }
    }
}

pub struct TcpToken<'c, M: RawMutex> {
    ordinal: usize,
    rx: &'c TcpRxPipe<M>,
    events: &'c RingChannel<M, ConnectionMessage, 8>,
}

impl<'c, M> TcpToken<'c, M>
where
    M: RawMutex,
{
    pub fn ordinal(&self) -> usize {
        self.ordinal
    }

    pub fn rx(&self) -> &'c TcpRxPipe<M> {
        self.rx
    }

    pub async fn next_message(&self) -> Result<ConnectionMessage, Lagged> {
        self.events.recv().await
    }
}
