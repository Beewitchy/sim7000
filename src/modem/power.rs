use embassy_sync::{
    blocking_mutex::raw::RawMutex,
    watch::{Watch, Receiver, AnonReceiver, DynSender},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PowerState {
    On,
    Sleeping,
    Off,
}

pub const POWER_SIGNAL_LISTENERS: usize = 12;

/// A PubSub channel for signaling changes in modem power state.
///
/// Make sure that POWER_SIGNAL_LISTENERS is high enough to accomodate your needs.
pub struct PowerSignal<M: RawMutex> {
    channel: Watch<M, PowerState, POWER_SIGNAL_LISTENERS>,
}

pub struct PowerSignalBroadcaster<'a> {
    sender: DynSender<'a, PowerState>,
}

pub struct PowerSignalListener<'a, M: RawMutex> {
    receiver: Receiver<'a, M, PowerState, POWER_SIGNAL_LISTENERS>,
}

pub struct PowerSignalReader<'a, M: RawMutex> {
    receiver: AnonReceiver<'a, M, PowerState, POWER_SIGNAL_LISTENERS>,
}

impl<M: RawMutex> PowerSignal<M> {
    pub const fn new() -> Self {
        Self {
            channel: Watch::new(),
        }
    }

    pub fn listener(&self) -> PowerSignalListener<'_, M> {
        PowerSignalListener {
            receiver: self
                .channel
                .receiver()
                .expect("not enough PowerSignal listener slots"),
        }
    }

    pub fn reader(&self) -> PowerSignalReader<'_, M> {
        PowerSignalReader {
            receiver: self
                .channel
                .anon_receiver()
        }
    }

    pub fn broadcaster(&self) -> PowerSignalBroadcaster<'_> {
        PowerSignalBroadcaster {
            sender: self.channel.dyn_sender(),
        }
    }

    pub fn update(&self, new_state: PowerState) {
        self.channel.sender().send(new_state);
    }

    pub fn clear(&self) {
        self.channel.sender().clear()
    }
}

impl PowerSignalBroadcaster<'_> {
    pub fn broadcast(&mut self, new_state: PowerState) {
        self.sender.send_if_modified(|state| {
            let modified = if let Some(state) = state {
                *state != new_state
            } else { true };
            *state = Some(new_state);
            modified
        })
    }

    pub fn clear(&self) {
        self.sender.clear()
    }
}

impl<M: RawMutex> PowerSignalListener<'_, M> {
    #[inline]
    pub async fn wait_for(&mut self, state: PowerState) {
        self.receiver.get_and(move |new_state| *new_state == state).await;
    }

    #[inline]
    pub async fn wait_for_changed(&mut self, state: PowerState) {
        self.receiver.changed_and(move |new_state| *new_state == state).await;
    }

    #[inline]
    pub async fn wait_for_not(&mut self, state: PowerState) -> PowerState {
        self.receiver.get_and(move |new_state| *new_state != state).await
    }

    #[inline]
    pub async fn wait_for_changed_not(&mut self, state: PowerState) -> PowerState {
        self.receiver.changed_and(move |new_state| *new_state != state).await
    }

    #[inline]
    pub async fn listen(&mut self) -> PowerState {
        self.receiver.changed().await
    }

    #[inline]
    pub fn try_read_current(&mut self) -> Option<PowerState> {
        self.receiver.try_get()
    }
}

impl<M: RawMutex> PowerSignalReader<'_, M> {
    #[inline]
    pub fn try_read_current(&mut self) -> Option<PowerState> {
        self.receiver.try_get()
    }

    #[inline]
    pub fn try_read_changed(&mut self) -> Option<PowerState> {
        self.receiver.try_changed()
    }
}
