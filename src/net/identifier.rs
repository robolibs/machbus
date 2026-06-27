//! 29-bit CAN extended identifier.
//!
//! Mirrors the C++ `machbus::net::Identifier`. Bit layout (MSB first):
//!
//! ```text
//! bit  28..26 | 25 | 24 | 23..16 | 15..8 | 7..0
//!     prio   | EDP| DP |   PF   |  PS   |  SA
//! ```
//!
//! - **EDP / DP** select the data page (combined with PF/PS to form the
//!   18-bit PGN).
//! - **PF (PDU Format)** ≥ 240 → PDU2 (broadcast), PS becomes the group
//!   extension and is part of the PGN.
//! - **PF** < 240 → PDU1 (destination-specific), PS is the destination
//!   address and is **not** part of the PGN.

use super::constants::BROADCAST_ADDRESS;
use super::error::{Error, Result};
use super::pgn::{pgn_is_valid, pgn_normalize};
use super::types::{Address, Pgn, Priority};
use alloc::format;

/// 29-bit CAN extended identifier with structured accessors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Identifier {
    /// Raw 29-bit value. Always masked to `0x1FFF_FFFF`.
    pub raw: u32,
}

impl Identifier {
    /// Wrap a raw 29-bit value (any high bits are masked off).
    #[inline]
    #[must_use]
    pub const fn from_raw(id: u32) -> Self {
        Self {
            raw: id & 0x1FFF_FFFF,
        }
    }

    /// Build a 29-bit identifier from its semantic components.
    ///
    /// `dst` is ignored for PDU2 (broadcast) PGNs — the PS byte from the
    /// PGN is used instead, matching the J1939-21 wire format.
    #[must_use]
    pub const fn encode(prio: Priority, pgn: Pgn, src: Address, dst: Address) -> Self {
        let pgn = pgn_normalize(pgn);
        let mut id: u32 = 0;
        id |= ((prio.as_u8() as u32) & 0x07) << 26;

        let edp = (pgn >> 17) & 0x01;
        let dp = (pgn >> 16) & 0x01;
        let pf = (pgn >> 8) & 0xFF;
        let ps = pgn & 0xFF;

        id |= edp << 25;
        id |= dp << 24;
        id |= pf << 16;

        if pf < 240 {
            // PDU1: PS byte holds destination.
            id |= (dst as u32) << 8;
        } else {
            // PDU2: PS byte is the group extension carried in the PGN.
            id |= ps << 8;
        }
        id |= src as u32;

        Self::from_raw(id)
    }

    /// Fallible identifier construction for paths that must reject
    /// out-of-range PGNs instead of silently dropping high bits at the raw
    /// CAN identifier boundary.
    pub fn try_encode(prio: Priority, pgn: Pgn, src: Address, dst: Address) -> Result<Self> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_data(format!(
                "identifier PGN 0x{pgn:X} exceeds the 18-bit J1939/ISOBUS PGN range"
            )));
        }
        Ok(Self::encode(prio, pgn, src, dst))
    }

    /// Default-priority encoding helper (broadcast destination).
    #[must_use]
    pub const fn encode_default(pgn: Pgn, src: Address) -> Self {
        Self::encode(Priority::Default, pgn, src, BROADCAST_ADDRESS)
    }

    /// Fallible default-priority encoding helper.
    pub fn try_encode_default(pgn: Pgn, src: Address) -> Result<Self> {
        Self::try_encode(Priority::Default, pgn, src, BROADCAST_ADDRESS)
    }

    #[inline]
    #[must_use]
    pub const fn priority(self) -> Priority {
        Priority::from_u8(((self.raw >> 26) & 0x07) as u8)
    }

    #[inline]
    #[must_use]
    pub const fn pdu_format(self) -> u8 {
        ((self.raw >> 16) & 0xFF) as u8
    }

    #[inline]
    #[must_use]
    pub const fn pdu_specific(self) -> u8 {
        ((self.raw >> 8) & 0xFF) as u8
    }

    #[inline]
    #[must_use]
    pub const fn source(self) -> Address {
        (self.raw & 0xFF) as Address
    }

    #[inline]
    #[must_use]
    pub const fn data_page(self) -> bool {
        ((self.raw >> 24) & 0x01) != 0
    }

    #[inline]
    #[must_use]
    pub const fn extended_data_page(self) -> bool {
        ((self.raw >> 25) & 0x01) != 0
    }

    /// `true` if PF ≥ 240 (broadcast / PDU2).
    #[inline]
    #[must_use]
    pub const fn is_pdu2(self) -> bool {
        self.pdu_format() >= 240
    }

    /// Reconstruct the 18-bit PGN from the identifier.
    #[inline]
    #[must_use]
    pub const fn pgn(self) -> Pgn {
        let dp = (self.raw >> 24) & 0x01;
        let edp = (self.raw >> 25) & 0x01;
        let pf = self.pdu_format() as u32;
        if pf < 240 {
            // PDU1: PS is destination, not part of the PGN.
            (edp << 17) | (dp << 16) | (pf << 8)
        } else {
            // PDU2: PS is part of the PGN (group extension).
            (edp << 17) | (dp << 16) | (pf << 8) | (self.pdu_specific() as u32)
        }
    }

    /// Destination address: PS for PDU1; [`BROADCAST_ADDRESS`] for PDU2.
    #[inline]
    #[must_use]
    pub const fn destination(self) -> Address {
        if self.is_pdu2() {
            BROADCAST_ADDRESS
        } else {
            self.pdu_specific()
        }
    }

    /// `true` if this is a broadcast frame (PDU2 *or* PDU1 to the global
    /// destination).
    #[inline]
    #[must_use]
    pub const fn is_broadcast(self) -> bool {
        self.is_pdu2() || self.destination() == BROADCAST_ADDRESS
    }
}

impl From<u32> for Identifier {
    #[inline]
    fn from(raw: u32) -> Self {
        Self::from_raw(raw)
    }
}

impl From<Identifier> for u32 {
    #[inline]
    fn from(id: Identifier) -> u32 {
        id.raw
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::*;

    #[test]
    fn from_raw_masks_high_bits() {
        let id = Identifier::from_raw(0xFFFF_FFFF);
        assert_eq!(id.raw, 0x1FFF_FFFF);
    }

    #[test]
    fn encode_pdu1_request_to_specific_address() {
        // Request PGN (0xEA00) is PDU1: PS = destination.
        let id = Identifier::encode(Priority::Default, PGN_REQUEST, 0x80, 0x42);
        assert_eq!(id.priority(), Priority::Default);
        assert_eq!(id.source(), 0x80);
        assert_eq!(id.pgn(), PGN_REQUEST);
        assert_eq!(id.destination(), 0x42);
        assert_eq!(id.pdu_format(), 0xEA);
        assert_eq!(id.pdu_specific(), 0x42);
        assert!(!id.is_pdu2());
        assert!(!id.is_broadcast());
    }

    #[test]
    fn encode_pdu2_dm1_is_broadcast() {
        // DM1 (0xFECA) is PDU2: PS = 0xCA is the group extension.
        let id = Identifier::encode(Priority::Default, PGN_DM1, 0x80, 0x42);
        assert!(id.is_pdu2());
        assert!(id.is_broadcast());
        assert_eq!(id.destination(), BROADCAST_ADDRESS);
        assert_eq!(id.pgn(), PGN_DM1);
        // dst argument was ignored; PS is 0xCA from PGN.
        assert_eq!(id.pdu_specific(), 0xCA);
    }

    #[test]
    fn encode_default_helper_is_broadcast() {
        let id = Identifier::encode_default(PGN_HEARTBEAT, 0x10);
        assert_eq!(id.priority(), Priority::Default);
        assert_eq!(id.source(), 0x10);
        assert!(id.is_broadcast());
    }

    #[test]
    fn try_encode_rejects_out_of_range_pgn_bits() {
        let err = Identifier::try_encode(Priority::Default, 0x40000, 0x80, 0x42).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));

        let id = Identifier::try_encode_default(PGN_HEARTBEAT, 0x10).unwrap();
        assert_eq!(id.pgn(), PGN_HEARTBEAT);
        assert_eq!(id.source(), 0x10);
    }

    #[test]
    fn priority_field_round_trip() {
        for prio in [
            Priority::Highest,
            Priority::High,
            Priority::Normal,
            Priority::Default,
            Priority::Lowest,
        ] {
            let id = Identifier::encode(prio, PGN_REQUEST, 0x01, 0x02);
            assert_eq!(id.priority(), prio);
        }
    }

    #[test]
    fn data_pages_extracted_correctly() {
        // PGN 0x2EF00 → bit 17 = EDP = 1, bit 16 = DP = 0, PF = 0xEF (PDU1), PS = 0.
        let id = Identifier::encode(Priority::Default, 0x2EF00, 0x80, 0xFF);
        assert!(id.extended_data_page());
        assert!(!id.data_page());
        assert_eq!(id.pdu_format(), 0xEF);

        // PGN 0x1EF00 → EDP = 0, DP = 1, PF = 0xEF — the C++ "Proprietary A2" PGN.
        let id = Identifier::encode(Priority::Default, 0x1EF00, 0x80, 0xFF);
        assert!(!id.extended_data_page());
        assert!(id.data_page());
        assert_eq!(id.pdu_format(), 0xEF);
    }

    #[test]
    fn pdu1_pdu2_boundary_table_normalizes_identifier_fields() {
        #[derive(Clone, Copy)]
        struct Case {
            input_pgn: Pgn,
            canonical_pgn: Pgn,
            dst: Address,
            raw: u32,
            pdu2: bool,
            ps: u8,
            destination: Address,
        }

        let cases = [
            Case {
                input_pgn: 0x00EF42,
                canonical_pgn: 0x00EF00,
                dst: 0x22,
                raw: 0x18EF_2280,
                pdu2: false,
                ps: 0x22,
                destination: 0x22,
            },
            Case {
                input_pgn: 0x00F042,
                canonical_pgn: 0x00F042,
                dst: 0x22,
                raw: 0x18F0_4280,
                pdu2: true,
                ps: 0x42,
                destination: BROADCAST_ADDRESS,
            },
            Case {
                input_pgn: 0x01EF42,
                canonical_pgn: 0x01EF00,
                dst: 0x33,
                raw: 0x19EF_3380,
                pdu2: false,
                ps: 0x33,
                destination: 0x33,
            },
            Case {
                input_pgn: 0x01F042,
                canonical_pgn: 0x01F042,
                dst: 0x33,
                raw: 0x19F0_4280,
                pdu2: true,
                ps: 0x42,
                destination: BROADCAST_ADDRESS,
            },
            Case {
                input_pgn: 0x02EF42,
                canonical_pgn: 0x02EF00,
                dst: 0x44,
                raw: 0x1AEF_4480,
                pdu2: false,
                ps: 0x44,
                destination: 0x44,
            },
            Case {
                input_pgn: 0x02F042,
                canonical_pgn: 0x02F042,
                dst: 0x44,
                raw: 0x1AF0_4280,
                pdu2: true,
                ps: 0x42,
                destination: BROADCAST_ADDRESS,
            },
            Case {
                input_pgn: 0x03EF42,
                canonical_pgn: 0x03EF00,
                dst: 0x55,
                raw: 0x1BEF_5580,
                pdu2: false,
                ps: 0x55,
                destination: 0x55,
            },
            Case {
                input_pgn: 0x03F042,
                canonical_pgn: 0x03F042,
                dst: 0x55,
                raw: 0x1BF0_4280,
                pdu2: true,
                ps: 0x42,
                destination: BROADCAST_ADDRESS,
            },
        ];

        for case in cases {
            let id = Identifier::encode(Priority::Default, case.input_pgn, 0x80, case.dst);
            assert_eq!(id.raw, case.raw);
            assert_eq!(id.pdu_format(), ((case.canonical_pgn >> 8) & 0xFF) as u8);
            assert_eq!(id.pdu_specific(), case.ps);
            assert_eq!(id.pgn(), case.canonical_pgn);
            assert_eq!(id.is_pdu2(), case.pdu2);
            assert_eq!(id.destination(), case.destination);
        }
    }

    #[test]
    fn u32_conversions() {
        let id = Identifier::encode(Priority::High, PGN_DM1, 0x55, 0xAA);
        let raw: u32 = id.into();
        let id2: Identifier = raw.into();
        assert_eq!(id, id2);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_from_raw_masks_and_decodes_identifier_fields(raw in any::<u32>()) {
            let id = Identifier::from_raw(raw);
            let masked = raw & 0x1FFF_FFFF;
            let pf = ((masked >> 16) & 0xFF) as u8;
            let ps = ((masked >> 8) & 0xFF) as u8;
            let expected_pgn = {
                let edp = (masked >> 25) & 0x01;
                let dp = (masked >> 24) & 0x01;
                if pf < 240 {
                    (edp << 17) | (dp << 16) | ((pf as u32) << 8)
                } else {
                    (edp << 17) | (dp << 16) | ((pf as u32) << 8) | ps as u32
                }
            };

            prop_assert_eq!(id.raw, masked);
            prop_assert_eq!(id.priority(), Priority::from_u8(((masked >> 26) & 0x07) as u8));
            prop_assert_eq!(id.source(), (masked & 0xFF) as u8);
            prop_assert_eq!(id.pdu_format(), pf);
            prop_assert_eq!(id.pdu_specific(), ps);
            prop_assert_eq!(id.data_page(), ((masked >> 24) & 0x01) != 0);
            prop_assert_eq!(id.extended_data_page(), ((masked >> 25) & 0x01) != 0);
            prop_assert_eq!(id.is_pdu2(), pf >= 240);
            prop_assert_eq!(id.pgn(), expected_pgn);
            prop_assert_eq!(
                id.destination(),
                if pf >= 240 { BROADCAST_ADDRESS } else { ps }
            );
            prop_assert_eq!(id.is_broadcast(), pf >= 240 || ps == BROADCAST_ADDRESS);
        }

        #[test]
        fn proptest_encode_decode_round_trip(
            prio_raw in 0u8..=7,
            pgn in 0u32..=0x3FFFF,
            src: u8,
            dst: u8,
        ) {
            let prio = Priority::from_u8(prio_raw);
            let id = Identifier::encode(prio, pgn, src, dst);

            prop_assert_eq!(id.priority(), prio);
            prop_assert_eq!(id.source(), src);

            // For PDU1 (PF < 240) the PS byte of the input PGN is discarded
            // — it is replaced by the destination address on the wire.
            // The decoded PGN is therefore the input with its low byte zeroed.
            let pf = ((pgn >> 8) & 0xFF) as u8;
            let canonical = if pf < 240 { pgn & 0xFFFF_FF00 } else { pgn };
            prop_assert_eq!(id.pgn(), canonical);

            if pf < 240 {
                prop_assert_eq!(id.destination(), dst);
            } else {
                prop_assert_eq!(id.destination(), BROADCAST_ADDRESS);
            }
        }

        #[test]
        fn proptest_encode_ignores_out_of_range_pgn_bits(
            prio_raw in any::<u8>(),
            pgn_raw in any::<u32>(),
            src: u8,
            dst: u8,
        ) {
            let prio = Priority::from_u8(prio_raw);
            let id = Identifier::encode(prio, pgn_raw, src, dst);
            let in_range_pgn = pgn_raw & 0x3_FFFF;
            let expected_pgn = pgn_normalize(in_range_pgn);
            let pf = ((in_range_pgn >> 8) & 0xFF) as u8;

            prop_assert_eq!(id.raw & !0x1FFF_FFFF, 0);
            prop_assert_eq!(id.priority(), prio);
            prop_assert_eq!(id.source(), src);
            prop_assert_eq!(id.pgn(), expected_pgn);
            prop_assert_eq!(
                id.destination(),
                if pf >= 240 { BROADCAST_ADDRESS } else { dst }
            );
        }
    }
}
