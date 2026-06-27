//! Bit-level access helpers and little-endian byte pack/unpack utilities.
//!
//! Mirrors the C++ `machbus::net::bitfield` namespace. Generic functions
//! are constrained to a sealed [`UnsignedInt`] trait; the small set of
//! implementations (`u8`, `u16`, `u32`, `u64`, `u128`, `usize`) covers
//! every use site in the stack.

mod sealed {
    pub trait Sealed {}
    impl Sealed for u8 {}
    impl Sealed for u16 {}
    impl Sealed for u32 {}
    impl Sealed for u64 {}
    impl Sealed for u128 {}
    impl Sealed for usize {}
}

/// Sealed trait identifying the unsigned-integer types we operate on.
pub trait UnsignedInt:
    sealed::Sealed
    + Copy
    + Eq
    + core::ops::BitAnd<Output = Self>
    + core::ops::BitOr<Output = Self>
    + core::ops::Not<Output = Self>
    + core::ops::Shl<u8, Output = Self>
    + core::ops::Shr<u8, Output = Self>
    + core::ops::Sub<Output = Self>
{
    const BIT_WIDTH: u8;
    const ZERO: Self;
    const ONE: Self;
}

macro_rules! impl_unsigned_int {
    ($t:ty) => {
        impl UnsignedInt for $t {
            const BIT_WIDTH: u8 = (core::mem::size_of::<$t>() * 8) as u8;
            const ZERO: Self = 0;
            const ONE: Self = 1;
        }
    };
}

impl_unsigned_int!(u8);
impl_unsigned_int!(u16);
impl_unsigned_int!(u32);
impl_unsigned_int!(u64);
impl_unsigned_int!(u128);
impl_unsigned_int!(usize);

/// Extract `length` bits starting at `start_bit` from `value`.
///
/// Out-of-range arguments return `0`, matching the C++ defensive
/// semantics.
#[inline]
#[must_use]
pub fn get_bits<T: UnsignedInt>(value: T, start_bit: u8, length: u8) -> T {
    if length == 0 || start_bit >= T::BIT_WIDTH {
        return T::ZERO;
    }
    if length >= T::BIT_WIDTH {
        return value >> start_bit;
    }
    let mask = (T::ONE << length) - T::ONE;
    (value >> start_bit) & mask
}

/// Replace `length` bits starting at `start_bit` of `value` with the
/// low `length` bits of `field_value`.
///
/// Out-of-range arguments leave `value` untouched, matching the C++
/// defensive semantics.
#[inline]
#[must_use]
pub fn set_bits<T: UnsignedInt>(value: T, start_bit: u8, length: u8, field_value: T) -> T {
    if length == 0 || start_bit >= T::BIT_WIDTH {
        return value;
    }
    if length >= T::BIT_WIDTH {
        return field_value;
    }
    let mask = (T::ONE << length) - T::ONE;
    let cleared = value & !(mask << start_bit);
    cleared | ((field_value & mask) << start_bit)
}

/// Read a single bit from `value`. Out-of-range bit indices return `false`.
#[inline]
#[must_use]
pub fn get_bit<T: UnsignedInt>(value: T, bit: u8) -> bool {
    if bit >= T::BIT_WIDTH {
        return false;
    }
    ((value >> bit) & T::ONE) != T::ZERO
}

/// Set or clear a single bit in `value`. Out-of-range bit indices leave
/// the value untouched.
#[inline]
#[must_use]
pub fn set_bit<T: UnsignedInt>(value: T, bit: u8, on: bool) -> T {
    if bit >= T::BIT_WIDTH {
        return value;
    }
    if on {
        value | (T::ONE << bit)
    } else {
        value & !(T::ONE << bit)
    }
}

// ─── Little-endian pack / unpack ─────────────────────────────────────────

/// Decode a little-endian `u16` from a 2-byte slice. Caller must ensure
/// `data.len() >= 2`.
#[inline]
#[must_use]
pub fn unpack_u16_le(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[0], data[1]])
}

/// Decode a little-endian `u32` from a 4-byte slice.
#[inline]
#[must_use]
pub fn unpack_u32_le(data: &[u8]) -> u32 {
    u32::from_le_bytes([data[0], data[1], data[2], data[3]])
}

/// Decode a little-endian `u64` from an 8-byte slice.
#[inline]
#[must_use]
pub fn unpack_u64_le(data: &[u8]) -> u64 {
    u64::from_le_bytes([
        data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7],
    ])
}

/// Encode `value` little-endian into a 2-byte slice.
#[inline]
pub fn pack_u16_le(data: &mut [u8], value: u16) {
    let bytes = value.to_le_bytes();
    data[0] = bytes[0];
    data[1] = bytes[1];
}

/// Encode `value` little-endian into a 4-byte slice.
#[inline]
pub fn pack_u32_le(data: &mut [u8], value: u32) {
    let bytes = value.to_le_bytes();
    data[..4].copy_from_slice(&bytes);
}

/// Encode `value` little-endian into an 8-byte slice.
#[inline]
pub fn pack_u64_le(data: &mut [u8], value: u64) {
    let bytes = value.to_le_bytes();
    data[..8].copy_from_slice(&bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_bits_extracts_low_nibble() {
        assert_eq!(get_bits(0xABu8, 0, 4), 0x0B);
        assert_eq!(get_bits(0xABu8, 4, 4), 0x0A);
    }

    #[test]
    fn get_bits_zero_length_returns_zero() {
        assert_eq!(get_bits(0xFFFFu16, 4, 0), 0);
    }

    #[test]
    fn get_bits_oob_start_returns_zero() {
        assert_eq!(get_bits(0xFFu8, 8, 4), 0);
        assert_eq!(get_bits(0xFFu8, 9, 4), 0);
    }

    #[test]
    fn get_bits_full_width_shifts_only() {
        assert_eq!(get_bits(0xABu8, 0, 8), 0xAB);
        assert_eq!(get_bits(0xABu8, 4, 8), 0x0A);
        assert_eq!(get_bits(0xABCDu16, 0, 16), 0xABCD);
    }

    #[test]
    fn set_bits_replaces_low_nibble() {
        assert_eq!(set_bits(0xABu8, 0, 4, 0x05), 0xA5);
        assert_eq!(set_bits(0xABu8, 4, 4, 0x05), 0x5B);
    }

    #[test]
    fn set_bits_masks_oversize_field_value() {
        // field_value bits beyond `length` must be ignored.
        assert_eq!(set_bits(0x00u8, 0, 4, 0xFF), 0x0F);
    }

    #[test]
    fn set_bits_zero_length_returns_input() {
        assert_eq!(set_bits(0xABu8, 0, 0, 0xFF), 0xAB);
    }

    #[test]
    fn set_bits_oob_start_returns_input() {
        assert_eq!(set_bits(0xABu8, 9, 4, 0x05), 0xAB);
    }

    #[test]
    fn get_set_bit_round_trip() {
        let v = 0u8;
        let v = set_bit(v, 3, true);
        assert!(get_bit(v, 3));
        assert!(!get_bit(v, 4));
        let v = set_bit(v, 3, false);
        assert!(!get_bit(v, 3));
    }

    #[test]
    fn get_bit_oob_returns_false() {
        assert!(!get_bit(0xFFu8, 8));
    }

    #[test]
    fn pack_unpack_u16_le_round_trip() {
        let mut buf = [0u8; 2];
        pack_u16_le(&mut buf, 0xCAFE);
        assert_eq!(buf, [0xFE, 0xCA]);
        assert_eq!(unpack_u16_le(&buf), 0xCAFE);
    }

    #[test]
    fn pack_unpack_u32_le_round_trip() {
        let mut buf = [0u8; 4];
        pack_u32_le(&mut buf, 0xDEAD_BEEF);
        assert_eq!(buf, [0xEF, 0xBE, 0xAD, 0xDE]);
        assert_eq!(unpack_u32_le(&buf), 0xDEAD_BEEF);
    }

    #[test]
    fn pack_unpack_u64_le_round_trip() {
        let mut buf = [0u8; 8];
        pack_u64_le(&mut buf, 0x0123_4567_89AB_CDEF);
        assert_eq!(buf, [0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x01]);
        assert_eq!(unpack_u64_le(&buf), 0x0123_4567_89AB_CDEF);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_get_set_bits_round_trip_u32(
            value: u32,
            start_bit in 0u8..32,
            length in 1u8..=32,
            field: u32,
        ) {
            // Replace and re-read: the readback must equal the masked field_value.
            let length = length.min(32 - start_bit);
            if length == 0 { return Ok(()); }
            let written = set_bits(value, start_bit, length, field);
            let readback = get_bits(written, start_bit, length);
            let mask = if length == 32 { u32::MAX } else { (1u32 << length) - 1 };
            prop_assert_eq!(readback, field & mask);
        }

        #[test]
        fn proptest_pack_unpack_u32_le(value: u32) {
            let mut buf = [0u8; 4];
            pack_u32_le(&mut buf, value);
            prop_assert_eq!(unpack_u32_le(&buf), value);
        }

        #[test]
        fn proptest_pack_unpack_u64_le(value: u64) {
            let mut buf = [0u8; 8];
            pack_u64_le(&mut buf, value);
            prop_assert_eq!(unpack_u64_le(&buf), value);
        }
    }
}
