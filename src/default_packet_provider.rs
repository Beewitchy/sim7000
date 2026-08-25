use core::cell::{Ref, RefCell, RefMut};
use embassy_sync::blocking_mutex::raw::{RawMutex};
use embassy_sync::blocking_mutex::Mutex;

use crate::{PacketProvider, PacketRef, PacketStorage, BorrowDataMut, BorrowData};


struct PacketBuf<const MTU: usize> {
    buf: [u8; MTU],
    free: bool,
}

impl<const MTU: usize> PacketBuf<MTU> {
    const DEFAULT: PacketBuf<MTU> = PacketBuf::new();

    pub(crate) const fn new() -> Self {
        Self {
            buf: [0; MTU],
            free: true,
        }
    }
}

struct State<const MTU: usize, const N: usize> {
    packets: [PacketBuf<MTU>; N],
}

impl<const MTU: usize, const N: usize> State<MTU, N> {
    pub(crate) const fn new() -> Self {
        Self {
            packets: [PacketBuf::DEFAULT; N],
        }
    }

    fn alloc(&mut self) -> Option<BufferRef<MTU>> {
        for (idx, packet) in self.packets.iter_mut().enumerate() {
            if packet.free {
                // info!("[{}] alloc {}", id.0, idx);
                packet.free = false;
                packet.buf.iter_mut().for_each(|b| *b = 0);
                return Some(BufferRef {
                    idx,
                    buf: packet.buf.as_mut_ptr(),
                });
            }
        }
        None
    }

    fn free(&mut self, p_ref: BufferRef<MTU>) {
        // info!("[{}] free {}", id.0, p_ref.idx);
        self.packets[p_ref.idx].free = true;
    }

    fn available(&mut self) -> usize {
        self.packets.iter().filter(|p| p.free).count()
    }
}

/// A packet pool that holds a fixed size pool of packet buffers
pub struct StaticPacketProvider<M: RawMutex, const MTU: usize, const N: usize> {
    state: Mutex<M, RefCell<State<MTU, N>>>,
}

impl<M: RawMutex, const MTU: usize, const N: usize> Default for StaticPacketProvider<M, MTU, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<M: RawMutex, const MTU: usize, const N: usize> StaticPacketProvider<M, MTU, N> {
    /// Create a new packet pool with the given QoS policy
    const fn new() -> Self {
        Self {
            state: Mutex::new(RefCell::new(State::new())),
        }
    }

    fn alloc(&self) -> Option<BufferRef<MTU>> {
        self.state.lock(|state| {
            let mut state = state.borrow_mut();
            state.alloc()
        })
    }

    fn free(&self, p_ref: BufferRef<MTU>) {
        self.state.lock(|state| {
            let mut state = state.borrow_mut();
            state.free(p_ref);
        });
    }

    fn available(&self) -> usize {
        self.state.lock(|state| {
            let mut state = state.borrow_mut();
            state.available()
        })
    }
}

/// Represents a reference to a packet.
#[repr(C)]
pub struct BufferRef<const MTU: usize> {
    idx: usize,
    buf: *mut u8,
}

pub trait BufferPool {
    type Buffer;
    const MTU: usize;
    fn alloc(&self) -> Self::Buffer;
    fn free(&self, buffer: Self::Buffer);
}

impl<M: RawMutex + 'static, const MTU: usize, const N: usize> PacketProvider for StaticPacketProvider<M, MTU, N> {
    type Packet = StaticPacket<Self, MTU>;
    fn allocate(&self, min_size: usize) -> Option<Self::Packet> {
        self.alloc().map(|p| StaticPacket {
            p_ref: p,
            pool: &self,
        })
    }
}

impl<M: RawMutex, const MTU: usize, const N: usize> BufferPool for StaticPacketProvider<M, MTU, N> {
    type Buffer = BufferRef<MTU>;

    const MTU: usize = MTU;

    fn alloc(&self) -> Self::Buffer {
        self.alloc()
    }

    fn free(&self, buffer: Self::Buffer) {
        self.free(buffer)
    }
}

/// A reference to a buffer from [StaticPacketProvider]
pub struct StaticPacket<Pool: BufferPool + 'static, const MTU: usize> {
    p_ref: RefCell<BufferRef<MTU>>,
    pool: &'static Pool,
}


impl<Pool: BufferPool + 'static, const MTU: usize> PacketRef for StaticPacket<Pool, MTU> {
    type PacketData = StaticPacket<Pool, MTU>;

    fn capacity(&self) -> usize {
        MTU
    }
}

impl<Pool: BufferPool + 'static, const MTU: usize> BorrowData for StaticPacket<Pool, MTU> {
    type Borrowed<'b> = Ref<'b, StaticPacket<Pool, MTU>>;

    fn borrow_data(&self) -> Self::Borrowed<'_> {
        self.p_ref.borrow()
    }
}

impl<Pool: BufferPool + 'static, const MTU: usize> BorrowDataMut for StaticPacket<Pool, MTU> {
    type BorrowedMut<'b> = RefMut<'b, StaticPacket<Pool, MTU>>;

    fn borrow_data_mut(&self) -> Self::BorrowedMut<'_> {
        self.p_ref.borrow_mut()
    }
}

impl<T> PacketRef for &T where T:PacketRef {
    type PacketData = T::PacketData;

    fn capacity(&self) -> usize {
        T::capacity(self)
    }
}

impl<T> BorrowData for &T where T: BorrowData {
    type Borrowed<'b> = T::Borrowed<'b> where Self: 'b;

    fn borrow_data(&self) -> Self::Borrowed<'_> {
        T::borrow_data(self)
    }
}

impl<T> BorrowDataMut for &T where T: BorrowDataMut {
    type BorrowedMut<'b> = T::BorrowedMut<'b> where Self: 'b;

    fn borrow_data_mut(&self) -> Self::BorrowedMut<'_> {
        T::borrow_data_mut(self)
    }
}

impl<Pool: BufferPool + 'static, const MTU: usize> PacketStorage for StaticPacket<Pool, MTU> {
}

impl<Pool: BufferPool + 'static, const MTU: usize> AsRef<[u8]> for StaticPacket<Pool, MTU> {
    fn as_ref(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.p_ref.buf, MTU) }
    }
}

impl<Pool: BufferPool + 'static, const MTU: usize> AsMut<[u8]> for StaticPacket<Pool, MTU> {
    fn as_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.p_ref.buf, MTU) }
    }
}

impl<Pool: BufferPool + 'static, const MTU: usize> Drop for StaticPacket<Pool, MTU> {
    fn drop(&mut self) {
        self.pool.free(&self.p_ref);
    }
}

#[cfg(test)]
mod tests {
    use embassy_sync::blocking_mutex::raw::NoopRawMutex;

    use super::*;

    #[test]
    fn test_none_qos() {
        let pool: StaticPacketProvider<NoopRawMutex, 27, 8> = StaticPacketProvider::new();

        let a1 = pool.alloc();
        assert!(a1.is_some());
        let a2 = pool.alloc();
        assert!(a2.is_some());
        let a3 = pool.alloc();
        assert!(a3.is_some());
        let a4 = pool.alloc();
        assert!(a4.is_some());
        let a5 = pool.alloc();
        assert!(a5.is_some());
        let a6 = pool.alloc();
        assert!(a6.is_some());
        let a7 = pool.alloc();
        assert!(a7.is_some());

        let b1 = pool.alloc();
        assert!(b1.is_some());

        let b2 = pool.alloc();
        assert!(b2.is_none());
    }
}