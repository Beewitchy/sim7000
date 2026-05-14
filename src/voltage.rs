use embassy_sync::blocking_mutex::raw::RawMutex;
use embassy_sync::signal::Signal;

use crate::at_command::unsolicited::VoltageWarning;
use crate::slot::Slot;

pub struct VoltageWarner<'c, M: RawMutex> {
    pub(crate) signal: &'c Signal<M, VoltageWarning>,
    pub(crate) slot: &'c Slot<Signal<M, VoltageWarning>>,
}

impl<'c, M> VoltageWarner<'c, M> where M: RawMutex {
    pub(crate) fn take(
        slot: &'c Slot<Signal<M, VoltageWarning>>,
    ) -> Option<Self> {
        let signal = slot.claim()?;
        signal.reset();
        Some(VoltageWarner { signal, slot })
    }

    /// Wait for any voltage warning
    pub async fn warning(&self) -> VoltageWarning {
        self.signal.wait().await
    }
}

impl<M> Drop for VoltageWarner<'_, M> where M: RawMutex {
    fn drop(&mut self) {
        self.slot.release();
    }
}
