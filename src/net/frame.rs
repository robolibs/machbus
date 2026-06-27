//! Single 8-byte CAN frame with structured identifier.
//!
//! Mirrors the C++ `machbus::net::Frame`. This is the protocol-layer
//! frame: it carries a parsed [`Identifier`] (rather than the raw
//! `can_id` with flag bits) and a microsecond receive timestamp.
//!
//! For driver-level CAN backend interop use
//! [`Frame::to_can_frame`] and [`Frame::from_can_frame`].

use super::can_adapter::CanFrame;
use super::error::{Error, Result};
use super::identifier::Identifier;
use super::pgn::pgn_is_valid;
use super::types::{Address, Pgn, Priority};
use alloc::format;

/// Single CAN data frame (up to 8 bytes payload).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Frame {
    pub id: Identifier,
    pub data: [u8; 8],
    /// Number of valid bytes in `data` (`0..=8`). Bytes past `length`
    /// are conventionally `0xFF` (the J1939 "not available" pattern)
    /// when produced via [`Frame::from_message`].
    pub length: u8,
    /// Receive timestamp in microseconds (driver-supplied).
    pub timestamp_us: u64,
}

impl Frame {
    /// Build a frame from an [`Identifier`] and a fully-formed payload
    /// of length `len ≤ 8`.
    #[must_use]
    pub const fn new(id: Identifier, data: [u8; 8], length: u8) -> Self {
        assert!(length <= 8);
        Self {
            id,
            data,
            length,
            timestamp_us: 0,
        }
    }

    /// Fallible version of [`Self::new`] for API boundaries that should
    /// surface malformed DLC/length values instead of panicking or clamping.
    pub fn try_new(id: Identifier, data: [u8; 8], length: u8) -> Result<Self> {
        if length > 8 {
            return Err(Error::invalid_data(format!(
                "CAN frame length {length} exceeds 8 bytes"
            )));
        }
        Ok(Self::new(id, data, length))
    }

    /// High-level constructor: encode (priority, PGN, src, dst) into an
    /// [`Identifier`] and copy a single-frame payload, padding the remainder
    /// with `0xFF`.
    ///
    /// Panics if the PGN is outside the 18-bit J1939/ISOBUS range or if the
    /// payload exceeds the 8-byte CAN data field. Use
    /// [`Self::try_from_message`] at external or user-controlled boundaries.
    #[must_use]
    pub fn from_message(
        prio: Priority,
        pgn: Pgn,
        src: Address,
        dst: Address,
        payload: &[u8],
    ) -> Self {
        Self::try_from_message(prio, pgn, src, dst, payload)
            .expect("Frame::from_message requires a valid PGN and payload length <= 8")
    }

    /// Fallible high-level constructor that rejects invalid PGN high bits and
    /// oversized single-frame payloads before identifier encoding can
    /// normalize/truncate them.
    pub fn try_from_message(
        prio: Priority,
        pgn: Pgn,
        src: Address,
        dst: Address,
        payload: &[u8],
    ) -> Result<Self> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        if payload.len() > 8 {
            return Err(Error::invalid_data(format!(
                "single CAN frame payload length {} exceeds 8 bytes",
                payload.len()
            )));
        }
        let id = Identifier::encode(prio, pgn, src, dst);
        let length = payload.len() as u8;
        let mut data = [0xFFu8; 8];
        let n = length as usize;
        data[..n].copy_from_slice(&payload[..n]);
        Ok(Self {
            id,
            data,
            length,
            timestamp_us: 0,
        })
    }

    /// Same as [`Self::from_message`] but with an explicit timestamp.
    #[must_use]
    pub fn from_message_at(
        prio: Priority,
        pgn: Pgn,
        src: Address,
        dst: Address,
        payload: &[u8],
        timestamp_us: u64,
    ) -> Self {
        Self::try_from_message_at(prio, pgn, src, dst, payload, timestamp_us)
            .expect("Frame::from_message_at requires a valid PGN and payload length <= 8")
    }

    /// Fallible [`Self::from_message_at`] variant.
    pub fn try_from_message_at(
        prio: Priority,
        pgn: Pgn,
        src: Address,
        dst: Address,
        payload: &[u8],
        timestamp_us: u64,
    ) -> Result<Self> {
        let mut f = Self::try_from_message(prio, pgn, src, dst, payload)?;
        f.timestamp_us = timestamp_us;
        Ok(f)
    }

    // ─── Identifier passthroughs ───────────────────────────────────────
    #[inline]
    #[must_use]
    pub const fn pgn(&self) -> Pgn {
        self.id.pgn()
    }

    #[inline]
    #[must_use]
    pub const fn source(&self) -> Address {
        self.id.source()
    }

    #[inline]
    #[must_use]
    pub const fn destination(&self) -> Address {
        self.id.destination()
    }

    #[inline]
    #[must_use]
    pub const fn priority(&self) -> Priority {
        self.id.priority()
    }

    #[inline]
    #[must_use]
    pub const fn is_broadcast(&self) -> bool {
        self.id.is_broadcast()
    }

    /// Borrow the valid portion of the payload.
    #[inline]
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        &self.data[..(self.length as usize).min(8)]
    }

    // ─── CAN adapter interop ─────────────────────────────────────────
    /// Convert to a CAN adapter frame. The frame is always emitted as
    /// extended (29-bit) format — ISOBUS / J1939 / NMEA2000 use only
    /// extended IDs.
    #[must_use]
    pub fn to_can_frame(&self) -> CanFrame {
        CanFrame::make_ext(self.id.raw, self.payload())
    }

    /// Build a [`Frame`] from a CAN adapter frame.
    ///
    /// Returns `None` for non-extended (11-bit standard) frames, RTR
    /// frames, and error frames — none of those are valid in the
    /// ISOBUS / J1939 / NMEA2000 stack.
    #[must_use]
    pub fn from_can_frame(cf: &CanFrame) -> Option<Self> {
        if !cf.is_extended() || cf.is_rtr() || cf.is_err() {
            return None;
        }
        // Read packed fields by value to avoid an unaligned reference.
        let raw_id = cf.id();
        let dlc = { cf.can_dlc };
        if dlc > 8 {
            return None;
        }
        let length = dlc;
        let data = { cf.data };
        Some(Self {
            id: Identifier::from_raw(raw_id),
            data,
            length,
            timestamp_us: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::*;

    #[test]
    fn from_message_pads_with_ff() {
        let f = Frame::from_message(
            Priority::Default,
            PGN_HEARTBEAT,
            0x80,
            0xFF,
            &[0x11, 0x22, 0x33],
        );
        assert_eq!(f.length, 3);
        assert_eq!(f.data[..3], [0x11, 0x22, 0x33]);
        assert_eq!(f.data[3..], [0xFF; 5]);
    }

    #[test]
    fn try_new_rejects_oversize_length() {
        let err = Frame::try_new(Identifier::from_raw(0), [0; 8], 9).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("exceeds 8"));
    }

    #[test]
    fn try_from_message_rejects_oversize_payload() {
        let payload = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let err = Frame::try_from_message(Priority::Default, PGN_REQUEST, 0x10, 0x20, &payload)
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("exceeds 8"));
    }

    #[test]
    fn try_from_message_rejects_invalid_pgn_high_bits() {
        let err =
            Frame::try_from_message(Priority::Default, 0x40000, 0x10, 0x20, &[0]).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidPgn);
    }

    #[test]
    #[should_panic(expected = "payload length <= 8")]
    fn from_message_panics_on_oversize_payload_instead_of_truncating() {
        let payload = [1, 2, 3, 4, 5, 6, 7, 8, 9];
        let _ = Frame::from_message(Priority::Default, PGN_REQUEST, 0x10, 0x20, &payload);
    }

    #[test]
    fn passthrough_accessors_match_identifier() {
        let f = Frame::from_message(Priority::High, PGN_REQUEST, 0x80, 0x42, &[0xFF; 3]);
        assert_eq!(f.priority(), Priority::High);
        assert_eq!(f.pgn(), PGN_REQUEST);
        assert_eq!(f.source(), 0x80);
        assert_eq!(f.destination(), 0x42);
        assert!(!f.is_broadcast());
    }

    #[test]
    fn pdu2_frame_is_broadcast_regardless_of_dst() {
        let f = Frame::from_message(Priority::Default, PGN_DM1, 0x80, 0x42, &[0xAA; 8]);
        assert!(f.is_broadcast());
        assert_eq!(f.destination(), super::super::constants::BROADCAST_ADDRESS);
    }

    #[test]
    fn payload_returns_only_valid_bytes() {
        let f = Frame::from_message(Priority::Default, PGN_REQUEST, 0, 0, &[0x01, 0x02]);
        assert_eq!(f.payload(), &[0x01, 0x02]);
    }

    #[test]
    fn round_trip_through_can_frame() {
        let original = Frame::from_message(
            Priority::High,
            PGN_HEARTBEAT,
            0x80,
            0xFF,
            &[0xDE, 0xAD, 0xBE, 0xEF],
        );
        let cf = original.to_can_frame();
        assert!(cf.is_extended());
        assert_eq!(cf.id(), original.id.raw);

        let restored = Frame::from_can_frame(&cf).expect("ext frame must convert back");
        assert_eq!(restored.id, original.id);
        assert_eq!(restored.length, 4);
        assert_eq!(restored.payload(), original.payload());
    }

    #[test]
    fn standard_frame_is_rejected() {
        let cf = CanFrame::make_std(0x123, &[0xAA]);
        assert!(Frame::from_can_frame(&cf).is_none());
    }

    #[test]
    fn rtr_frame_is_rejected() {
        let cf = CanFrame::make_rtr(0x1234_5678, true);
        assert!(Frame::from_can_frame(&cf).is_none());
    }

    #[test]
    fn error_frame_is_rejected() {
        let cf = CanFrame::make_error(0x42);
        assert!(Frame::from_can_frame(&cf).is_none());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_from_can_frame_accepts_only_extended_data_frames(
            raw_id: u32,
            payload in proptest::collection::vec(any::<u8>(), 0..=32),
        ) {
            let ext = CanFrame::make_ext(raw_id, &payload);
            let restored = Frame::from_can_frame(&ext).expect("extended data frame must decode");
            prop_assert_eq!(restored.id.raw, ext.id());
            prop_assert_eq!(restored.length as usize, payload.len().min(8));
            prop_assert_eq!(restored.payload(), &payload[..payload.len().min(8)]);

            let std = CanFrame::make_std(raw_id, &payload);
            prop_assert!(Frame::from_can_frame(&std).is_none());

            let rtr = CanFrame::make_rtr(raw_id, true);
            prop_assert!(Frame::from_can_frame(&rtr).is_none());

            let err = CanFrame::make_error(raw_id);
            prop_assert!(Frame::from_can_frame(&err).is_none());
        }

        #[test]
        fn proptest_from_can_frame_rejects_malicious_dlc(
            raw_id: u32,
            dlc: u8,
            data: [u8; 8],
        ) {
            let mut cf = CanFrame::make_ext(raw_id, &data);
            cf.can_dlc = dlc;

            if dlc <= 8 {
                let restored = Frame::from_can_frame(&cf).expect("valid DLC must decode");
                prop_assert_eq!(restored.id.raw, cf.id());
                prop_assert_eq!(restored.length, dlc);
                prop_assert_eq!(restored.data, data);
                prop_assert_eq!(restored.payload(), &data[..dlc as usize]);
            } else {
                prop_assert!(Frame::from_can_frame(&cf).is_none());
            }
        }
    }
}
