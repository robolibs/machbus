//! Decoded multi-byte CAN message (post-reassembly for TP/ETP/FastPacket).
//!
//! Mirrors the C++ `machbus::net::Message`. Read accessors delegate to
//! [`DataSpan`] for consistency with single-frame decoders. Setters
//! grow the underlying buffer with `0xFF` padding to match the C++
//! `set_*` semantics.

use super::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use super::data_span::DataSpan;
use super::pgn::pgn_is_pdu2;
use super::types::{Address, Pgn, Priority};
use alloc::vec::Vec;

/// Decoded message: PGN-tagged payload of arbitrary length plus
/// addressing metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub source: Address,
    pub destination: Address,
    pub priority: Priority,
    pub timestamp_us: u64,
}

impl Default for Message {
    fn default() -> Self {
        Self {
            pgn: 0,
            data: Vec::new(),
            source: NULL_ADDRESS,
            destination: BROADCAST_ADDRESS,
            priority: Priority::Default,
            timestamp_us: 0,
        }
    }
}

impl Message {
    /// Convenience constructor: `data` is moved in, address defaults to
    /// broadcast, priority to [`Priority::Default`].
    #[must_use]
    pub fn new(pgn: Pgn, data: Vec<u8>, source: Address) -> Self {
        Self {
            pgn,
            data,
            source,
            ..Self::default()
        }
    }

    /// Full-detail constructor matching the C++ signature.
    #[must_use]
    pub fn with_addressing(
        pgn: Pgn,
        data: Vec<u8>,
        source: Address,
        destination: Address,
        priority: Priority,
    ) -> Self {
        Self {
            pgn,
            data,
            source,
            destination,
            priority,
            timestamp_us: 0,
        }
    }

    /// Borrow the payload as a [`DataSpan`].
    #[inline]
    #[must_use]
    pub fn span(&self) -> DataSpan<'_> {
        DataSpan::new(&self.data)
    }

    /// Number of bytes in the payload.
    #[inline]
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    #[inline]
    #[must_use]
    pub fn is_broadcast(&self) -> bool {
        self.destination == BROADCAST_ADDRESS
    }

    /// `true` when the source address can identify a real sender.
    #[inline]
    #[must_use]
    pub const fn has_usable_source(&self) -> bool {
        self.source != NULL_ADDRESS && self.source != BROADCAST_ADDRESS
    }

    /// `true` when the destination metadata is compatible with this PGN.
    ///
    /// PDU2 PGNs carry a group-extension byte, not a destination address, so
    /// they are represented as broadcast in [`Message`]. PDU1 PGNs may be
    /// destination-specific or global, but never use the null address as the
    /// destination.
    #[inline]
    #[must_use]
    pub const fn has_valid_destination_for_pgn(&self) -> bool {
        self.destination != NULL_ADDRESS
            && (!pgn_is_pdu2(self.pgn) || self.destination == BROADCAST_ADDRESS)
    }

    /// `true` when PGN, source and destination are a usable envelope for a
    /// full-message decoder before that decoder reads the payload.
    #[inline]
    #[must_use]
    pub const fn has_usable_envelope_for_pgn(&self, expected_pgn: Pgn) -> bool {
        self.pgn == expected_pgn && self.has_usable_source() && self.has_valid_destination_for_pgn()
    }

    // ─── Defensive read helpers (mirror C++) ─────────────────────────
    #[inline]
    #[must_use]
    pub fn get_u8(&self, offset: usize) -> u8 {
        self.span().get_u8(offset)
    }

    #[inline]
    #[must_use]
    pub fn get_u16_le(&self, offset: usize) -> u16 {
        self.span().get_u16_le(offset)
    }

    #[inline]
    #[must_use]
    pub fn get_u32_le(&self, offset: usize) -> u32 {
        self.span().get_u32_le(offset)
    }

    #[inline]
    #[must_use]
    pub fn get_u64_le(&self, offset: usize) -> u64 {
        self.span().get_u64_le(offset)
    }

    #[inline]
    #[must_use]
    pub fn get_bit(&self, byte_offset: usize, bit: u8) -> bool {
        self.span().get_bit(byte_offset, bit)
    }

    /// Extract an arbitrary little-endian bit field of up to 32 bits
    /// starting at `start_bit`. Returns 0 if `length` is 0 or > 32.
    /// Bytes past the end of `data` are treated as zero.
    #[must_use]
    pub fn get_bits(&self, start_bit: usize, length: u8) -> u32 {
        if length == 0 || length > 32 {
            return 0;
        }
        let mut result: u32 = 0;
        for i in 0..length {
            let bit_pos = start_bit + i as usize;
            let byte_idx = bit_pos / 8;
            let bit_idx = (bit_pos % 8) as u8;
            if byte_idx < self.data.len() {
                let bit = ((self.data[byte_idx] >> bit_idx) & 0x01) as u32;
                result |= bit << i;
            }
        }
        result
    }

    // ─── Growing setters (mirror C++) ────────────────────────────────
    /// Set the byte at `offset`, growing the buffer with `0xFF`
    /// padding if necessary.
    pub fn set_u8(&mut self, offset: usize, val: u8) {
        if offset >= self.data.len() {
            self.data.resize(offset + 1, 0xFF);
        }
        self.data[offset] = val;
    }

    /// Write a little-endian `u16` at `offset`, growing as needed.
    pub fn set_u16_le(&mut self, offset: usize, val: u16) {
        let needed = offset + 2;
        if needed > self.data.len() {
            self.data.resize(needed, 0xFF);
        }
        let bytes = val.to_le_bytes();
        self.data[offset] = bytes[0];
        self.data[offset + 1] = bytes[1];
    }

    /// Write a little-endian `u32` at `offset`, growing as needed.
    pub fn set_u32_le(&mut self, offset: usize, val: u32) {
        let needed = offset + 4;
        if needed > self.data.len() {
            self.data.resize(needed, 0xFF);
        }
        let bytes = val.to_le_bytes();
        self.data[offset..offset + 4].copy_from_slice(&bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_message_is_broadcast_with_null_source() {
        let m = Message::default();
        assert_eq!(m.source, NULL_ADDRESS);
        assert_eq!(m.destination, BROADCAST_ADDRESS);
        assert!(m.is_broadcast());
        assert!(!m.has_usable_source());
        assert!(!m.has_usable_envelope_for_pgn(0));
        assert_eq!(m.size(), 0);
    }

    #[test]
    fn message_envelope_validation_rejects_bad_source_and_destination_metadata() {
        let pdu2 = Message::new(0xFECA, vec![], 0x80);
        assert!(pdu2.has_usable_envelope_for_pgn(0xFECA));

        let wrong_pgn = Message::new(0xFECA, vec![], 0x80);
        assert!(!wrong_pgn.has_usable_envelope_for_pgn(0xFECB));

        let null_source = Message::new(0xFECA, vec![], NULL_ADDRESS);
        assert!(!null_source.has_usable_envelope_for_pgn(0xFECA));

        let broadcast_source = Message::new(0xFECA, vec![], BROADCAST_ADDRESS);
        assert!(!broadcast_source.has_usable_envelope_for_pgn(0xFECA));

        let pdu2_specific_destination =
            Message::with_addressing(0xFECA, vec![], 0x80, 0x42, Priority::Default);
        assert!(!pdu2_specific_destination.has_valid_destination_for_pgn());
        assert!(!pdu2_specific_destination.has_usable_envelope_for_pgn(0xFECA));

        let pdu1_specific_destination =
            Message::with_addressing(0xEA00, vec![], 0x80, 0x42, Priority::Default);
        assert!(pdu1_specific_destination.has_usable_envelope_for_pgn(0xEA00));

        let pdu1_global_destination =
            Message::with_addressing(0xEA00, vec![], 0x80, BROADCAST_ADDRESS, Priority::Default);
        assert!(pdu1_global_destination.has_usable_envelope_for_pgn(0xEA00));

        let pdu1_null_destination =
            Message::with_addressing(0xEA00, vec![], 0x80, NULL_ADDRESS, Priority::Default);
        assert!(!pdu1_null_destination.has_usable_envelope_for_pgn(0xEA00));
    }

    #[test]
    fn read_helpers_match_data_span() {
        let m = Message::new(0xEA00, vec![0x01, 0x02, 0x03, 0x04], 0x80);
        assert_eq!(m.get_u8(0), 0x01);
        assert_eq!(m.get_u16_le(0), 0x0201);
        assert_eq!(m.get_u32_le(0), 0x0403_0201);
        // OOB defensive defaults.
        assert_eq!(m.get_u8(99), 0xFF);
        assert_eq!(m.get_u16_le(99), 0xFFFF);
        assert_eq!(m.get_u64_le(0), 0xFFFF_FFFF_FFFF_FFFF);
    }

    #[test]
    fn get_bit_in_and_out_of_bounds() {
        let m = Message::new(0, vec![0b1010_1010], 0);
        assert!(m.get_bit(0, 1));
        assert!(!m.get_bit(0, 0));
        assert!(!m.get_bit(0, 8)); // bit OOB
        assert!(!m.get_bit(99, 0)); // byte OOB
    }

    #[test]
    fn get_bits_extracts_arbitrary_field() {
        // bytes: 0xAB 0xCD = bits ... 1100_1101 1010_1011 (LE within byte)
        let m = Message::new(0, vec![0xAB, 0xCD], 0);
        // Low nibble of byte 0:
        assert_eq!(m.get_bits(0, 4), 0x0B);
        // High nibble of byte 0:
        assert_eq!(m.get_bits(4, 4), 0x0A);
        // Full byte 1 starting at bit 8:
        assert_eq!(m.get_bits(8, 8), 0xCD);
        // 16-bit LE field across both bytes:
        assert_eq!(m.get_bits(0, 16), 0xCDAB);
    }

    #[test]
    fn get_bits_zero_length_returns_zero() {
        let m = Message::new(0, vec![0xFF], 0);
        assert_eq!(m.get_bits(0, 0), 0);
    }

    #[test]
    fn get_bits_oversize_length_returns_zero() {
        let m = Message::new(0, vec![0xFF; 8], 0);
        assert_eq!(m.get_bits(0, 33), 0);
    }

    #[test]
    fn get_bits_past_end_yields_zero_filled() {
        // Only 1 byte of data; reading 16 bits past the end zero-fills.
        let m = Message::new(0, vec![0xFF], 0);
        assert_eq!(m.get_bits(0, 16), 0x00FF);
    }

    #[test]
    fn set_u8_grows_with_ff_padding() {
        let mut m = Message::default();
        m.set_u8(3, 0xAA);
        assert_eq!(m.data, vec![0xFF, 0xFF, 0xFF, 0xAA]);
    }

    #[test]
    fn set_u16_le_grows_when_needed() {
        let mut m = Message::default();
        m.set_u16_le(2, 0xCAFE);
        assert_eq!(m.data, vec![0xFF, 0xFF, 0xFE, 0xCA]);
    }

    #[test]
    fn set_u16_le_in_place_does_not_grow() {
        let mut m = Message::new(0, vec![0u8; 4], 0);
        m.set_u16_le(0, 0x1234);
        assert_eq!(m.data, vec![0x34, 0x12, 0x00, 0x00]);
    }

    #[test]
    fn set_u32_le_grows_when_needed() {
        let mut m = Message::default();
        m.set_u32_le(0, 0xDEAD_BEEF);
        assert_eq!(m.data, vec![0xEF, 0xBE, 0xAD, 0xDE]);
    }

    #[test]
    fn round_trip_set_then_get() {
        let mut m = Message::default();
        m.set_u32_le(0, 0xDEAD_BEEF);
        m.set_u16_le(4, 0xCAFE);
        m.set_u8(6, 0x42);
        assert_eq!(m.get_u32_le(0), 0xDEAD_BEEF);
        assert_eq!(m.get_u16_le(4), 0xCAFE);
        assert_eq!(m.get_u8(6), 0x42);
    }
}
