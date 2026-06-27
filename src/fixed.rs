//! Fixed-capacity helpers for the `embedded-fixed` profile.
//!
//! This module is intentionally small and dependency-free. It provides
//! allocation-free building blocks that embedded applications can use around the
//! heap-backed protocol core today, and that later internal migrations can
//! reuse when replacing selected `VecDeque`/pending-queue/message-payload paths.

use core::{fmt, ops};

use crate::net::{
    Address, BROADCAST_ADDRESS, DataSpan, Error, Frame, Message, NULL_ADDRESS, Pgn, Priority,
    Result as NetResult,
};

/// Fixed-capacity FIFO queue backed by an inline `[Option<T>; N]` ring.
///
/// `push_back` never allocates: it returns the item to the caller on overflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedQueue<T, const N: usize> {
    buf: [Option<T>; N],
    head: usize,
    len: usize,
}

impl<T, const N: usize> Default for FixedQueue<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> FixedQueue<T, N> {
    /// Construct an empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: [(); N].map(|()| None),
            head: 0,
            len: 0,
        }
    }

    /// Maximum number of queued items.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Current number of queued items.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// `true` if no items are queued.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `true` if the queue cannot accept another item.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Push one item onto the back of the queue.
    ///
    /// Returns the original item when the queue is full.
    pub fn push_back(&mut self, item: T) -> core::result::Result<(), T> {
        if self.len == N {
            return Err(item);
        }
        let idx = self.physical_index(self.len);
        self.buf[idx] = Some(item);
        self.len += 1;
        Ok(())
    }

    /// Pop one item from the front of the queue.
    pub fn pop_front(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.buf[self.head].take();
        self.head = self.advance(self.head);
        self.len -= 1;
        if self.len == 0 {
            self.head = 0;
        }
        item
    }

    /// Borrow the front item.
    #[must_use]
    pub fn front(&self) -> Option<&T> {
        (self.len > 0)
            .then(|| self.buf[self.head].as_ref())
            .flatten()
    }

    /// Remove all queued items.
    pub fn clear(&mut self) {
        while self.pop_front().is_some() {}
    }

    #[inline]
    fn advance(&self, idx: usize) -> usize {
        if N == 0 { 0 } else { (idx + 1) % N }
    }

    #[inline]
    fn physical_index(&self, logical: usize) -> usize {
        if N == 0 { 0 } else { (self.head + logical) % N }
    }
}

/// Fixed queue for `(port, Frame)` transport traffic.
pub type FixedFrameQueue<const N: usize> = FixedQueue<(u8, Frame), N>;

/// Fixed-capacity packed slot list backed by inline `[Option<T>; N>]` storage.
///
/// Unlike [`FixedQueue`], this type supports indexed lookup and `swap_remove`.
/// It is useful for small protocol session tables where order is not
/// semantically important and removals should not shift every following item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedSlots<T, const N: usize> {
    buf: [Option<T>; N],
    len: usize,
}

impl<T, const N: usize> Default for FixedSlots<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> FixedSlots<T, N> {
    /// Construct an empty slot list.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buf: [(); N].map(|()| None),
            len: 0,
        }
    }

    /// Maximum number of stored items.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of stored items.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// `true` when no items are stored.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `true` when no more items can be appended.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Iterate over stored items.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.buf[..self.len]
            .iter()
            .map(|slot| slot.as_ref().expect("fixed slots packed invariant"))
    }

    /// Mutably iterate over stored items.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.buf[..self.len]
            .iter_mut()
            .map(|slot| slot.as_mut().expect("fixed slots packed invariant"))
    }

    /// Append one item.
    ///
    /// Returns the original item when the slot list is full.
    pub fn push(&mut self, item: T) -> core::result::Result<(), T> {
        if self.len == N {
            return Err(item);
        }
        self.buf[self.len] = Some(item);
        self.len += 1;
        Ok(())
    }

    /// Remove one item by swapping in the last item.
    pub fn swap_remove(&mut self, idx: usize) -> Option<T> {
        if idx >= self.len {
            return None;
        }
        let last_idx = self.len - 1;
        let removed = self.buf[idx].take();
        if idx != last_idx {
            self.buf[idx] = self.buf[last_idx].take();
        }
        self.len -= 1;
        removed
    }

    /// Remove all items.
    pub fn clear(&mut self) {
        while self.len > 0 {
            let _ = self.swap_remove(self.len - 1);
        }
    }
}

impl<T, const N: usize> ops::Index<usize> for FixedSlots<T, N> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len);
        self.buf[index]
            .as_ref()
            .expect("fixed slots packed invariant")
    }
}

impl<T, const N: usize> ops::IndexMut<usize> for FixedSlots<T, N> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        assert!(index < self.len);
        self.buf[index]
            .as_mut()
            .expect("fixed slots packed invariant")
    }
}

/// Error returned by fixed-capacity buffers when an operation would exceed the
/// inline storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedCapacityError {
    /// Maximum number of bytes/items the fixed container can hold.
    pub capacity: usize,
    /// Number of bytes/items requested by the failed operation.
    pub requested: usize,
}

impl FixedCapacityError {
    #[must_use]
    pub const fn new(capacity: usize, requested: usize) -> Self {
        Self {
            capacity,
            requested,
        }
    }
}

impl fmt::Display for FixedCapacityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "fixed capacity {} exceeded by request for {}",
            self.capacity, self.requested
        )
    }
}

/// Fixed-capacity byte buffer backed by inline storage.
///
/// This is the allocation-free payload half of the `embedded-fixed` profile.
/// It is intentionally small: callers choose the maximum payload size at the
/// type level and receive an error instead of an allocation or truncation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedBytes<const N: usize> {
    data: [u8; N],
    len: usize,
}

impl<const N: usize> Default for FixedBytes<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> FixedBytes<N> {
    /// Construct an empty fixed byte buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            data: [0; N],
            len: 0,
        }
    }

    /// Construct a fixed byte buffer by copying `bytes`.
    pub fn from_slice(bytes: &[u8]) -> core::result::Result<Self, FixedCapacityError> {
        let mut out = Self::new();
        out.extend_from_slice(bytes)?;
        Ok(out)
    }

    /// Maximum byte capacity.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> usize {
        N
    }

    /// Number of stored bytes.
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// `true` when no bytes are stored.
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// `true` when no more bytes can be appended.
    #[inline]
    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len == N
    }

    /// Borrow the initialized byte slice.
    #[inline]
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.data[..self.len]
    }

    /// Mutably borrow the initialized byte slice.
    #[inline]
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data[..self.len]
    }

    /// Borrow as a protocol [`DataSpan`].
    #[inline]
    #[must_use]
    pub fn span(&self) -> DataSpan<'_> {
        DataSpan::new(self.as_slice())
    }

    /// Append one byte.
    pub fn push(&mut self, byte: u8) -> core::result::Result<(), FixedCapacityError> {
        if self.len == N {
            return Err(FixedCapacityError::new(N, self.len + 1));
        }
        self.data[self.len] = byte;
        self.len += 1;
        Ok(())
    }

    /// Append a slice.
    pub fn extend_from_slice(
        &mut self,
        bytes: &[u8],
    ) -> core::result::Result<(), FixedCapacityError> {
        let requested = self.len.saturating_add(bytes.len());
        if requested > N {
            return Err(FixedCapacityError::new(N, requested));
        }
        self.data[self.len..requested].copy_from_slice(bytes);
        self.len = requested;
        Ok(())
    }

    /// Resize the initialized byte range.
    ///
    /// Growing fills new bytes with `value`; shrinking only changes the visible
    /// length. This never allocates.
    pub fn resize(
        &mut self,
        new_len: usize,
        value: u8,
    ) -> core::result::Result<(), FixedCapacityError> {
        if new_len > N {
            return Err(FixedCapacityError::new(N, new_len));
        }
        if new_len > self.len {
            self.data[self.len..new_len].fill(value);
        }
        self.len = new_len;
        Ok(())
    }

    /// Remove all bytes.
    pub fn clear(&mut self) {
        self.len = 0;
    }
}

impl<const N: usize> AsRef<[u8]> for FixedBytes<N> {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

/// Fixed-capacity protocol message envelope.
///
/// Unlike [`crate::net::Message`], the payload never allocates or grows. This
/// is useful for single-frame and bounded application messages in
/// `embedded-fixed` code. Larger TP/ETP/Fast Packet traffic can still use the
/// heap-backed embedded `Message` path until those reassembly buffers are
/// migrated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedMessage<const N: usize> {
    pub pgn: Pgn,
    pub data: FixedBytes<N>,
    pub source: Address,
    pub destination: Address,
    pub priority: Priority,
    pub timestamp_us: u64,
}

impl<const N: usize> Default for FixedMessage<N> {
    fn default() -> Self {
        Self {
            pgn: 0,
            data: FixedBytes::new(),
            source: NULL_ADDRESS,
            destination: BROADCAST_ADDRESS,
            priority: Priority::Default,
            timestamp_us: 0,
        }
    }
}

impl<const N: usize> FixedMessage<N> {
    /// Construct a fixed message by copying `payload`.
    pub fn new(
        pgn: Pgn,
        payload: &[u8],
        source: Address,
    ) -> core::result::Result<Self, FixedCapacityError> {
        Ok(Self {
            pgn,
            data: FixedBytes::from_slice(payload)?,
            source,
            ..Self::default()
        })
    }

    /// Construct a fixed message with complete addressing metadata.
    pub fn with_addressing(
        pgn: Pgn,
        payload: &[u8],
        source: Address,
        destination: Address,
        priority: Priority,
    ) -> core::result::Result<Self, FixedCapacityError> {
        Ok(Self {
            pgn,
            data: FixedBytes::from_slice(payload)?,
            source,
            destination,
            priority,
            timestamp_us: 0,
        })
    }

    /// Build a fixed message from a single CAN frame.
    pub fn from_frame(frame: &Frame) -> core::result::Result<Self, FixedCapacityError> {
        Ok(Self {
            pgn: frame.pgn(),
            data: FixedBytes::from_slice(frame.payload())?,
            source: frame.source(),
            destination: frame.destination(),
            priority: frame.priority(),
            timestamp_us: frame.timestamp_us,
        })
    }

    /// Build a fixed message from the heap-backed embedded/hosted
    /// [`crate::net::Message`] representation.
    pub fn from_message(message: &Message) -> core::result::Result<Self, FixedCapacityError> {
        Ok(Self {
            pgn: message.pgn,
            data: FixedBytes::from_slice(&message.data)?,
            source: message.source,
            destination: message.destination,
            priority: message.priority,
            timestamp_us: message.timestamp_us,
        })
    }

    /// Convert a bounded single-frame payload back into a CAN frame.
    pub fn to_frame(&self) -> NetResult<Frame> {
        if self.data.len() > 8 {
            return Err(Error::buffer_overflow());
        }
        Frame::try_from_message_at(
            self.priority,
            self.pgn,
            self.source,
            self.destination,
            self.data.as_slice(),
            self.timestamp_us,
        )
    }

    /// Borrow the payload as a [`DataSpan`].
    #[inline]
    #[must_use]
    pub fn span(&self) -> DataSpan<'_> {
        self.data.span()
    }

    /// Payload length.
    #[inline]
    #[must_use]
    pub const fn size(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Identifier, pgn_defs::PGN_REQUEST};

    #[test]
    fn queue_wraps_and_preserves_order() {
        let mut q = FixedQueue::<u8, 3>::new();
        assert_eq!(q.capacity(), 3);
        assert!(q.is_empty());
        assert_eq!(q.push_back(1), Ok(()));
        assert_eq!(q.push_back(2), Ok(()));
        assert_eq!(q.pop_front(), Some(1));
        assert_eq!(q.push_back(3), Ok(()));
        assert_eq!(q.push_back(4), Ok(()));
        assert!(q.is_full());
        assert_eq!(q.push_back(5), Err(5));
        assert_eq!(q.pop_front(), Some(2));
        assert_eq!(q.pop_front(), Some(3));
        assert_eq!(q.pop_front(), Some(4));
        assert_eq!(q.pop_front(), None);
    }

    #[test]
    fn zero_capacity_queue_never_accepts_items() {
        let mut q = FixedQueue::<u8, 0>::new();
        assert_eq!(q.capacity(), 0);
        assert!(q.is_full());
        assert_eq!(q.push_back(1), Err(1));
        assert_eq!(q.pop_front(), None);
    }

    #[test]
    fn fixed_slots_support_index_and_swap_remove() {
        let mut slots = FixedSlots::<u8, 3>::new();
        assert_eq!(slots.push(10), Ok(()));
        assert_eq!(slots.push(20), Ok(()));
        assert_eq!(slots.push(30), Ok(()));
        assert!(slots.is_full());
        assert_eq!(slots.push(40), Err(40));
        assert_eq!(slots[1], 20);
        slots[1] = 21;
        assert_eq!(slots.swap_remove(1), Some(21));
        assert_eq!(slots.len(), 2);
        assert_eq!(slots[0], 10);
        assert_eq!(slots[1], 30);
        assert_eq!(slots.swap_remove(99), None);
    }

    #[test]
    fn fixed_bytes_rejects_overflow_without_truncating() {
        let mut b = FixedBytes::<3>::from_slice(&[1, 2]).unwrap();
        assert_eq!(b.as_slice(), &[1, 2]);
        assert_eq!(b.push(3), Ok(()));
        assert_eq!(b.as_slice(), &[1, 2, 3]);
        assert_eq!(
            b.extend_from_slice(&[4]),
            Err(FixedCapacityError {
                capacity: 3,
                requested: 4,
            })
        );
        assert_eq!(b.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn fixed_message_round_trips_single_frame_payload() {
        let frame = Frame::new(
            Identifier::encode(Priority::Default, PGN_REQUEST, 0x80, BROADCAST_ADDRESS),
            [0x00, 0xEE, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            3,
        );

        let msg = FixedMessage::<8>::from_frame(&frame).unwrap();
        assert_eq!(msg.pgn, PGN_REQUEST);
        assert_eq!(msg.source, 0x80);
        assert_eq!(msg.data.as_slice(), &[0x00, 0xEE, 0x00]);
        assert_eq!(msg.span().get_u16_le(0), 0xEE00);

        let roundtrip = msg.to_frame().unwrap();
        assert_eq!(roundtrip.pgn(), PGN_REQUEST);
        assert_eq!(roundtrip.payload(), &[0x00, 0xEE, 0x00]);
    }

    #[test]
    fn fixed_message_reports_payload_capacity() {
        assert_eq!(
            FixedMessage::<2>::new(PGN_REQUEST, &[1, 2, 3], 0x80).unwrap_err(),
            FixedCapacityError {
                capacity: 2,
                requested: 3,
            }
        );
    }
}
