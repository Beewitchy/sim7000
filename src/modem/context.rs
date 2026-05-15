use embassy_sync::{
    blocking_mutex::raw::RawMutex, pipe::Pipe,
    zerocopy_channel::Channel as ZerocopyChannel, channel::Channel,
    signal::Signal,
};

use super::{power::PowerSignal, RawAtCommand, SmsState};
use crate::{
    at_command::{
        unsolicited::{
            ConnectionMessage, GnssReport, NetworkRegistration, NewSmsIndex, RegistrationStatus,
            VoltageWarning,
        },
        ResponseCode,
    },
    drop::DropChannel,
    slot::Slot,
    tcp::TCP_RX_BUF_LEN,
    util::{Lagged, RingChannel},
    StateSignal,
};

pub type TcpRxPipe<M> = Pipe<M, TCP_RX_BUF_LEN>;
pub type TcpEventChannel<M> = RingChannel<M, ConnectionMessage, 8>;

pub(crate) struct PacketChannels<'c, M: RawMutex> {
    pub(crate) commands: ZerocopyChannel<'c, M, RawAtCommand>,
    pub(crate) generic_response: ZerocopyChannel<'c, M, ResponseCode>,
}

impl<'c, M: RawMutex> PacketChannels<'c, M> {
    pub(crate) fn new(commands_buf: &'c mut [RawAtCommand], response_buf: &'c mut [ResponseCode]) -> Self {
        let commands = ZerocopyChannel::new(commands_buf);
        let generic_response = ZerocopyChannel::new(response_buf);
        Self { commands, generic_response, }
    }
}

pub struct Shared<M: RawMutex, const TCP_SLOTS: usize> {
    pub(crate) power_signal: PowerSignal<M>,
    pub(crate) drop_channel: DropChannel<M>,
    pub(crate) sms_indices: Channel<M, NewSmsIndex, 5>,
    pub(crate) sms_state: Signal<M, SmsState>,
    pub(crate) registration_events: StateSignal<M, NetworkRegistration>,
    pub(crate) tcp: TcpContext<M, TCP_SLOTS>,
    pub(crate) gnss_slot: Slot<Signal<M, GnssReport>>,
    pub(crate) voltage_slot: Slot<Signal<M, VoltageWarning>>,
    pub(crate) tx_pipe: Pipe<M, 2048>,
    pub(crate) rx_pipe: Pipe<M, 2048>,
}

pub struct ModemContext<'c, M: RawMutex, const TCP_SLOTS: usize> {
    pub(crate) shared: Shared<M, TCP_SLOTS>,
    pub(crate) packet_channels: PacketChannels<'c, M>,
}

impl<M, const TCP_SLOTS: usize> Shared<M, TCP_SLOTS> where M: RawMutex {
    pub const fn new(tcp: TcpContext<M, TCP_SLOTS>) -> Self {
        Self {
            power_signal: PowerSignal::new(),
            drop_channel: DropChannel::new(),
            sms_indices: Channel::new(),
            sms_state: Signal::new(),
            registration_events: StateSignal::new(NetworkRegistration {
                status: RegistrationStatus::Unknown,
                lac: None,
                ci: None,
            }),
            tcp,
            gnss_slot: Slot::new(Signal::new()),
            voltage_slot: Slot::new(Signal::new()),
            tx_pipe: Pipe::new(),
            rx_pipe: Pipe::new(),
        }
    }
}

impl<'c, M, const TCP_SLOTS: usize> ModemContext<'c, M, TCP_SLOTS> where M: RawMutex {
    pub fn new(tcp: TcpContext<M, TCP_SLOTS>, commands_buf: &'c mut [RawAtCommand], response_buf: &'c mut [ResponseCode]) -> Self {
        Self {
            shared: Shared::new(tcp),
            packet_channels: PacketChannels::new(commands_buf, response_buf),
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

impl<M> TcpSlot<M> where M: RawMutex {
    pub const fn new() -> Self {
        TcpSlot {
            rx: Pipe::new(),
            events: TcpEventChannel::new(),
        }
    }
}

impl<M, const N: usize> TcpContext<M, N> where M: RawMutex {
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

impl<'c, M> TcpToken<'c, M> where M: RawMutex{
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
