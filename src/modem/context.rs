use core::cell::OnceCell;

use embassy_sync::{
    blocking_mutex::raw::RawMutex, zerocopy_channel::Channel, mutex::Mutex, pipe::Pipe,
    channel::Channel as CopyChannel,
    signal::Signal,
};

use super::{power::PowerSignal, CommandRunner, RawAtCommand, SmsState};
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

pub type TcpRxPipe<M: RawMutex> = Pipe<M, TCP_RX_BUF_LEN>;
pub type TcpEventChannel<M: RawMutex> = RingChannel<M, ConnectionMessage, 8>;

pub struct ModemContext<M: RawMutex + 'static> {
    pub(crate) power_signal: PowerSignal,
    pub(crate) command_lock: Mutex<M, ()>,
    pub(crate) commands: OnceCell<Channel<'static, M, RawAtCommand>>,
    pub(crate) generic_response: OnceCell<Channel<'static, M, Option<ResponseCode>>>,
    pub(crate) drop_channel: DropChannel<M>,
    pub(crate) tcp: TcpContext<M>,
    pub(crate) sms_indices: CopyChannel<M, NewSmsIndex, 5>,
    pub(crate) sms_state: Signal<M, SmsState>,
    pub(crate) registration_events: StateSignal<M, NetworkRegistration>,
    pub(crate) gnss_slot: Slot<Signal<M, GnssReport>>,
    pub(crate) voltage_slot: Slot<Signal<M, VoltageWarning>>,
    pub(crate) tx_pipe: Pipe<M, 2048>,
    pub(crate) rx_pipe: Pipe<M, 2048>,
}

impl<M> ModemContext<M> where M: RawMutex {
    pub const fn new(tcp: TcpContext<M>) -> Self {
        static COMMAND_BUFFER: [Option<RawAtCommand>; 4] = [None, None, None, None];
        ModemContext {
            power_signal: PowerSignal::new(),
            command_lock: Mutex::new(()),
            commands: OnceCell::new(),
            generic_response: OnceCell::new(),
            drop_channel: DropChannel::new(),
            tcp,
            sms_indices: CopyChannel::new(),
            sms_state: Signal::new(),
            registration_events: StateSignal::new(NetworkRegistration {
                status: RegistrationStatus::Unknown,
                lac: None,
                ci: None,
            }),
            gnss_slot: Slot::new(Signal::new()),
            voltage_slot: Slot::new(Signal::new()),
            tx_pipe: Pipe::new(),
            rx_pipe: Pipe::new(),
        }
    }

    pub fn commands(&self) -> CommandRunner<'_, M> {
        CommandRunner::create(self)
    }
}

pub struct TcpSlot<M: RawMutex> {
    pub rx: TcpRxPipe<M>,
    pub events: TcpEventChannel<M>,
}

pub struct TcpContext<M: RawMutex + 'static> {
    pub(crate) slots: &'static [Slot<TcpSlot<M>>],
}

impl<M> TcpSlot<M> where M: RawMutex {
    pub const fn new() -> Self {
        TcpSlot {
            rx: Pipe::new(),
            events: TcpEventChannel::new(),
        }
    }
}

impl<M> TcpContext<M> where M: RawMutex + 'static {
    pub const fn new(slots: &'static [Slot<TcpSlot<M>>]) -> Self {
        TcpContext { slots }
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
        for slot in self.slots {
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
