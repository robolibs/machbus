//! Read-only view over message bytes with defensive out-of-bounds reads.
//!
//! Mirrors the C++ `machbus::net::DataSpan`. The defensive-default
//! semantics (returning `0xFF` / `0xFFFF` / `0xFFFFFFFF` on OOB rather
//! than panicking) are part of the contract and used pervasively in
//! protocol decoders, where missing bytes are conventionally
//! interpreted as "field not provided".

use super::bitfield;
use alloc::vec::Vec;

/// Read-only view over a slice of bytes with safe, defensive accessors.
#[derive(Debug, Clone, Copy)]
pub struct DataSpan<'a> {
    data: &'a [u8],
}

impl<'a> DataSpan<'a> {
    /// Build a span from a borrowed slice.
    #[inline]
    #[must_use]
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    /// Empty span; useful as a default.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self { data: &[] }
    }

    /// Borrow the underlying slice.
    #[inline]
    #[must_use]
    pub const fn as_slice(&self) -> &'a [u8] {
        self.data
    }

    /// Pointer to the first byte.
    #[inline]
    #[must_use]
    pub fn data_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }

    #[inline]
    #[must_use]
    pub const fn size(&self) -> usize {
        self.data.len()
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Sub-view from `offset`, taking up to `count` bytes. If `offset`
    /// is past the end, returns an empty span. If `count` exceeds the
    /// remaining bytes, the span is clamped.
    #[inline]
    #[must_use]
    pub fn subspan(&self, offset: usize, count: usize) -> DataSpan<'a> {
        if offset >= self.data.len() {
            return DataSpan::empty();
        }
        let remaining = self.data.len() - offset;
        let actual = count.min(remaining);
        DataSpan {
            data: &self.data[offset..offset + actual],
        }
    }

    /// Sub-view from `offset` to the end. Empty if `offset` is past
    /// the end.
    #[inline]
    #[must_use]
    pub fn subspan_from(&self, offset: usize) -> DataSpan<'a> {
        if offset >= self.data.len() {
            return DataSpan::empty();
        }
        DataSpan {
            data: &self.data[offset..],
        }
    }

    /// Return the byte at `idx`, or `0xFF` if out of bounds.
    #[inline]
    #[must_use]
    pub fn at(&self, idx: usize) -> u8 {
        self.data.get(idx).copied().unwrap_or(0xFF)
    }

    /// Same as [`Self::at`], named to match the C++ `get_u8`.
    #[inline]
    #[must_use]
    pub fn get_u8(&self, offset: usize) -> u8 {
        self.at(offset)
    }

    /// Read a little-endian `u16` at `offset`. Returns `0xFFFF` if the
    /// span is too short.
    #[inline]
    #[must_use]
    pub fn get_u16_le(&self, offset: usize) -> u16 {
        let Some(end) = offset.checked_add(2) else {
            return 0xFFFF;
        };
        if end > self.data.len() {
            return 0xFFFF;
        }
        bitfield::unpack_u16_le(&self.data[offset..])
    }

    /// Read a little-endian `u32` at `offset`. Returns `0xFFFF_FFFF`
    /// if the span is too short.
    #[inline]
    #[must_use]
    pub fn get_u32_le(&self, offset: usize) -> u32 {
        let Some(end) = offset.checked_add(4) else {
            return 0xFFFF_FFFF;
        };
        if end > self.data.len() {
            return 0xFFFF_FFFF;
        }
        bitfield::unpack_u32_le(&self.data[offset..])
    }

    /// Read a little-endian `u64` at `offset`. Returns `0xFFFF_FFFF_FFFF_FFFF`
    /// if the span is too short.
    #[inline]
    #[must_use]
    pub fn get_u64_le(&self, offset: usize) -> u64 {
        let Some(end) = offset.checked_add(8) else {
            return 0xFFFF_FFFF_FFFF_FFFF;
        };
        if end > self.data.len() {
            return 0xFFFF_FFFF_FFFF_FFFF;
        }
        bitfield::unpack_u64_le(&self.data[offset..])
    }

    /// Read a single bit at byte `byte_offset`, bit `bit` (0..=7).
    /// Returns `false` if either index is out of bounds.
    #[inline]
    #[must_use]
    pub fn get_bit(&self, byte_offset: usize, bit: u8) -> bool {
        if byte_offset >= self.data.len() || bit > 7 {
            return false;
        }
        bitfield::get_bit(self.data[byte_offset], bit)
    }

    /// Iterate over the bytes of the span.
    #[inline]
    pub fn iter(&self) -> core::slice::Iter<'a, u8> {
        self.data.iter()
    }
}

impl<'a> From<&'a [u8]> for DataSpan<'a> {
    #[inline]
    fn from(data: &'a [u8]) -> Self {
        DataSpan::new(data)
    }
}

impl<'a> From<&'a Vec<u8>> for DataSpan<'a> {
    #[inline]
    fn from(data: &'a Vec<u8>) -> Self {
        DataSpan::new(data.as_slice())
    }
}

impl<'a, const N: usize> From<&'a [u8; N]> for DataSpan<'a> {
    #[inline]
    fn from(data: &'a [u8; N]) -> Self {
        DataSpan::new(data)
    }
}

impl<'a> IntoIterator for DataSpan<'a> {
    type Item = &'a u8;
    type IntoIter = core::slice::Iter<'a, u8>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.data.iter()
    }
}

impl Default for DataSpan<'_> {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_span_basics() {
        let s = DataSpan::empty();
        assert!(s.is_empty());
        assert_eq!(s.size(), 0);
        assert_eq!(s.at(0), 0xFF);
        assert_eq!(s.get_u16_le(0), 0xFFFF);
        assert_eq!(s.get_u32_le(0), 0xFFFF_FFFF);
        assert_eq!(s.get_u64_le(0), 0xFFFF_FFFF_FFFF_FFFF);
        assert!(!s.get_bit(0, 0));
    }

    #[test]
    fn at_returns_value_or_ff() {
        let buf = [0x10, 0x20, 0x30];
        let s = DataSpan::from(&buf[..]);
        assert_eq!(s.at(0), 0x10);
        assert_eq!(s.at(2), 0x30);
        assert_eq!(s.at(3), 0xFF);
        assert_eq!(s.at(usize::MAX), 0xFF);
    }

    #[test]
    fn typed_reads_in_bounds() {
        let buf = [0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08];
        let s = DataSpan::from(&buf);
        assert_eq!(s.get_u16_le(0), 0x0201);
        assert_eq!(s.get_u32_le(0), 0x0403_0201);
        assert_eq!(s.get_u64_le(0), 0x0807_0605_0403_0201);
    }

    #[test]
    fn typed_reads_oob_return_all_ones() {
        let buf = [0x10, 0x20];
        let s = DataSpan::from(&buf);
        // u16 at offset 1 needs 2 bytes (1, 2) but len is 2 → OOB.
        assert_eq!(s.get_u16_le(1), 0xFFFF);
        // u32 at offset 0 needs 4 bytes; only 2 → OOB.
        assert_eq!(s.get_u32_le(0), 0xFFFF_FFFF);
        // u64 at offset 0 needs 8 bytes; only 2 → OOB.
        assert_eq!(s.get_u64_le(0), 0xFFFF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn typed_reads_huge_offsets_return_all_ones_without_overflow() {
        let buf = [0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80];
        let s = DataSpan::from(&buf);
        assert_eq!(s.get_u16_le(usize::MAX), 0xFFFF);
        assert_eq!(s.get_u32_le(usize::MAX), 0xFFFF_FFFF);
        assert_eq!(s.get_u64_le(usize::MAX), 0xFFFF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn typed_reads_at_exact_boundary_succeed() {
        // size=2, u16 at offset 0 must succeed.
        let buf = [0xAA, 0xBB];
        let s = DataSpan::from(&buf);
        assert_eq!(s.get_u16_le(0), 0xBBAA);

        // size=4, u32 at offset 0 must succeed.
        let buf = [0x11, 0x22, 0x33, 0x44];
        let s = DataSpan::from(&buf);
        assert_eq!(s.get_u32_le(0), 0x4433_2211);
    }

    #[test]
    fn subspan_in_bounds() {
        let buf = [0, 1, 2, 3, 4, 5];
        let s = DataSpan::from(&buf);
        let sub = s.subspan(2, 3);
        assert_eq!(sub.size(), 3);
        assert_eq!(sub.at(0), 2);
        assert_eq!(sub.at(2), 4);
        assert_eq!(sub.at(3), 0xFF);
    }

    #[test]
    fn subspan_oob_offset_is_empty() {
        let buf = [0, 1, 2];
        let s = DataSpan::from(&buf);
        assert!(s.subspan(5, 2).is_empty());
        assert!(s.subspan(3, 2).is_empty());
        assert!(s.subspan(usize::MAX, usize::MAX).is_empty());
        assert!(s.subspan_from(usize::MAX).is_empty());
    }

    #[test]
    fn subspan_count_clamps_to_remaining() {
        let buf = [0, 1, 2, 3];
        let s = DataSpan::from(&buf);
        let sub = s.subspan(1, 100);
        assert_eq!(sub.size(), 3);
    }

    #[test]
    fn subspan_from_slices_to_end() {
        let buf = [10, 20, 30];
        let s = DataSpan::from(&buf);
        let sub = s.subspan_from(1);
        assert_eq!(sub.size(), 2);
        assert_eq!(sub.at(0), 20);
    }

    #[test]
    fn get_bit_in_and_out_of_bounds() {
        let buf = [0b1010_1010];
        let s = DataSpan::from(&buf);
        assert!(s.get_bit(0, 1));
        assert!(!s.get_bit(0, 0));
        assert!(!s.get_bit(0, 8)); // bit OOB
        assert!(!s.get_bit(1, 0)); // byte OOB
    }

    #[test]
    fn from_vec_works() {
        let v: Vec<u8> = vec![1, 2, 3];
        let s: DataSpan<'_> = (&v).into();
        assert_eq!(s.size(), 3);
        assert_eq!(s.at(2), 3);
    }

    #[test]
    fn from_array_works() {
        let arr = [9u8, 8, 7];
        let s: DataSpan<'_> = (&arr).into();
        assert_eq!(s.size(), 3);
        assert_eq!(s.at(0), 9);
    }

    #[test]
    fn iteration_visits_each_byte() {
        let buf = [4u8, 5, 6];
        let s = DataSpan::from(&buf);
        let collected: Vec<u8> = s.iter().copied().collect();
        assert_eq!(collected, vec![4, 5, 6]);
    }

    use proptest::prelude::*;

    fn expected_u16(data: &[u8], offset: usize) -> u16 {
        match offset.checked_add(2) {
            Some(end) if end <= data.len() => u16::from_le_bytes([data[offset], data[offset + 1]]),
            _ => 0xFFFF,
        }
    }

    fn expected_u32(data: &[u8], offset: usize) -> u32 {
        match offset.checked_add(4) {
            Some(end) if end <= data.len() => u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]),
            _ => 0xFFFF_FFFF,
        }
    }

    fn expected_u64(data: &[u8], offset: usize) -> u64 {
        match offset.checked_add(8) {
            Some(end) if end <= data.len() => u64::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]),
            _ => 0xFFFF_FFFF_FFFF_FFFF,
        }
    }

    proptest! {
        #[test]
        fn proptest_accessors_match_slice_or_defensive_sentinel(
            data in proptest::collection::vec(any::<u8>(), 0..=64),
            offset: usize,
            count: usize,
            bit: u8,
        ) {
            let s = DataSpan::new(&data);

            prop_assert_eq!(s.size(), data.len());
            prop_assert_eq!(s.is_empty(), data.is_empty());
            prop_assert_eq!(s.as_slice(), data.as_slice());
            prop_assert_eq!(s.at(offset), data.get(offset).copied().unwrap_or(0xFF));
            prop_assert_eq!(s.get_u8(offset), data.get(offset).copied().unwrap_or(0xFF));
            prop_assert_eq!(s.get_u16_le(offset), expected_u16(&data, offset));
            prop_assert_eq!(s.get_u32_le(offset), expected_u32(&data, offset));
            prop_assert_eq!(s.get_u64_le(offset), expected_u64(&data, offset));

            let sub = s.subspan(offset, count);
            if offset >= data.len() {
                prop_assert!(sub.is_empty());
                prop_assert!(s.subspan_from(offset).is_empty());
            } else {
                let remaining = data.len() - offset;
                let actual = count.min(remaining);
                prop_assert_eq!(sub.as_slice(), &data[offset..offset + actual]);
                prop_assert_eq!(s.subspan_from(offset).as_slice(), &data[offset..]);
            }

            let expected_bit = data
                .get(offset)
                .is_some_and(|byte| bit <= 7 && ((*byte >> bit) & 1) != 0);
            prop_assert_eq!(s.get_bit(offset, bit), expected_bit);
        }
    }
}
