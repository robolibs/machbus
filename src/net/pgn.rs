//! PGN classification helpers and the static metadata lookup table.
//!
//! Mirrors the C++ `machbus::net::pgn` namespace. Helper functions take
//! a [`Pgn`] (a `u32` alias) directly; promoting them to methods is
//! deferred until [`Pgn`] is itself promoted to a newtype.

use super::pgn_defs::*;
use super::types::Pgn;

/// Static metadata for a known PGN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PgnInfo {
    pub pgn: Pgn,
    pub name: &'static str,
    pub data_length: u32,
    pub default_priority: u32,
    pub is_broadcast: bool,
}

/// Static lookup table mirroring the C++ `PGN_TABLE`.
///
/// Order is preserved verbatim; do not sort — some C++ tests assume
/// the C++ layout. Use [`pgn_lookup`] for value-based access.
pub const PGN_TABLE: &[PgnInfo] = &[
    PgnInfo {
        pgn: PGN_REQUEST,
        name: "Request",
        data_length: 3,
        default_priority: 6,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_ADDRESS_CLAIMED,
        name: "Address Claimed",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_COMMANDED_ADDRESS,
        name: "Commanded Address",
        data_length: 9,
        default_priority: 6,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_TP_CM,
        name: "TP.CM",
        data_length: 8,
        default_priority: 7,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_TP_DT,
        name: "TP.DT",
        data_length: 8,
        default_priority: 7,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_ETP_CM,
        name: "ETP.CM",
        data_length: 8,
        default_priority: 7,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_ETP_DT,
        name: "ETP.DT",
        data_length: 8,
        default_priority: 7,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_ACKNOWLEDGMENT,
        name: "Acknowledgment",
        data_length: 8,
        default_priority: 6,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_DM1,
        name: "DM1",
        data_length: 0,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_DM2,
        name: "DM2",
        data_length: 0,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_DM3,
        name: "DM3",
        data_length: 0,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_DM11,
        name: "DM11",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_HEARTBEAT,
        name: "Heartbeat",
        data_length: 1,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_TIME_DATE,
        name: "Time/Date",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_VEHICLE_SPEED,
        name: "Vehicle Speed",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_WHEEL_SPEED,
        name: "Wheel Speed",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_GROUND_SPEED,
        name: "Ground Speed",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_MACHINE_SPEED,
        name: "Machine Speed",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_LANGUAGE_COMMAND,
        name: "Language Command",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_MAINTAIN_POWER,
        name: "Maintain Power",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_GUIDANCE_MACHINE,
        name: "Guidance Machine",
        data_length: 8,
        default_priority: 3,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_GUIDANCE_SYSTEM,
        name: "Guidance System",
        data_length: 8,
        default_priority: 3,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_SHORTCUT_BUTTON,
        name: "Shortcut Button",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_VT_TO_ECU,
        name: "VT to ECU",
        data_length: 8,
        default_priority: 6,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_ECU_TO_VT,
        name: "ECU to VT",
        data_length: 8,
        default_priority: 6,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_TC_TO_ECU,
        name: "TC to ECU",
        data_length: 8,
        default_priority: 6,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_ECU_TO_TC,
        name: "ECU to TC",
        data_length: 8,
        default_priority: 6,
        is_broadcast: false,
    },
    PgnInfo {
        pgn: PGN_WORKING_SET_MASTER,
        name: "Working Set Master",
        data_length: 8,
        default_priority: 6,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_GNSS_POSITION,
        name: "GNSS Position Rapid",
        data_length: 8,
        default_priority: 2,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_GNSS_COG_SOG,
        name: "GNSS COG/SOG",
        data_length: 8,
        default_priority: 2,
        is_broadcast: true,
    },
    PgnInfo {
        pgn: PGN_GNSS_POSITION_DETAIL,
        name: "GNSS Position Data",
        data_length: 0,
        default_priority: 6,
        is_broadcast: true,
    },
];

/// Look up static metadata for a known PGN. Returns `None` if the PGN
/// is not in [`PGN_TABLE`].
#[must_use]
pub fn pgn_lookup(pgn: Pgn) -> Option<PgnInfo> {
    PGN_TABLE.iter().copied().find(|info| info.pgn == pgn)
}

/// PDU2 format: PF byte ≥ 240. PDU2 messages are broadcast / global
/// (the "PS" byte is the group extension, not a destination).
#[inline]
#[must_use]
pub const fn pgn_is_pdu2(pgn: Pgn) -> bool {
    pgn_pdu_format(pgn) >= 240
}

/// Deprecated alias; use [`pgn_is_pdu2`] instead.
///
/// Kept for source compatibility with the C++ API.
#[inline]
#[must_use]
#[deprecated(note = "use pgn_is_pdu2")]
pub const fn pgn_is_broadcast(pgn: Pgn) -> bool {
    pgn_is_pdu2(pgn)
}

/// `true` if the value fits the 18-bit PGN field plus EDP/DP bits
/// (`pgn <= 0x3FFFF`).
#[inline]
#[must_use]
pub const fn pgn_is_valid(pgn: Pgn) -> bool {
    pgn <= 0x3FFFF
}

/// Extract the PDU format byte (PF) from a PGN.
#[inline]
#[must_use]
pub const fn pgn_pdu_format(pgn: Pgn) -> u8 {
    ((pgn >> 8) & 0xFF) as u8
}

/// Return the canonical PGN value for identifier-level routing.
///
/// PDU1 PGNs use the identifier's PS byte as the destination address, so the
/// low byte is not part of the canonical PGN. PDU2 PGNs keep the low byte as
/// the group extension.
#[inline]
#[must_use]
pub const fn pgn_normalize(pgn: Pgn) -> Pgn {
    if pgn_pdu_format(pgn) < 240 {
        pgn & !0xFF
    } else {
        pgn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdu_format_extraction() {
        assert_eq!(pgn_pdu_format(PGN_REQUEST), 0xEA);
        assert_eq!(pgn_pdu_format(PGN_ADDRESS_CLAIMED), 0xEE);
        assert_eq!(pgn_pdu_format(PGN_DM1), 0xFE);
    }

    #[test]
    fn pdu1_vs_pdu2_classification() {
        // PDU1 (PF < 240): destination-specific.
        assert!(!pgn_is_pdu2(PGN_REQUEST)); // 0xEA
        assert!(!pgn_is_pdu2(PGN_ADDRESS_CLAIMED)); // 0xEE
        assert!(!pgn_is_pdu2(PGN_TP_CM)); // 0xEC

        // PDU2 (PF >= 240): broadcast.
        assert!(pgn_is_pdu2(PGN_DM1)); // 0xFE
        assert!(pgn_is_pdu2(PGN_HEARTBEAT)); // 0xF0
        assert!(pgn_is_pdu2(PGN_VEHICLE_SPEED)); // 0xFE
    }

    #[test]
    fn pgn_normalization_preserves_only_pdu2_group_extension() {
        assert_eq!(pgn_normalize(0x00EF42), 0x00EF00);
        assert_eq!(pgn_normalize(0x01EF42), 0x01EF00);
        assert_eq!(pgn_normalize(0x02EF42), 0x02EF00);
        assert_eq!(pgn_normalize(0x03EF42), 0x03EF00);

        assert_eq!(pgn_normalize(0x00F042), 0x00F042);
        assert_eq!(pgn_normalize(0x01F042), 0x01F042);
        assert_eq!(pgn_normalize(0x02F042), 0x02F042);
        assert_eq!(pgn_normalize(0x03F042), 0x03F042);
    }

    #[test]
    fn validity_caps_at_18_bits_plus_edp_dp() {
        assert!(pgn_is_valid(0x00000));
        assert!(pgn_is_valid(0x3FFFF));
        assert!(!pgn_is_valid(0x40000));
        assert!(!pgn_is_valid(0xFFFFFFFF));
    }

    #[test]
    fn lookup_returns_metadata_for_known() {
        let info = pgn_lookup(PGN_HEARTBEAT).expect("heartbeat must be known");
        assert_eq!(info.name, "Heartbeat");
        assert_eq!(info.data_length, 1);
        assert_eq!(info.default_priority, 6);
        assert!(info.is_broadcast);
    }

    #[test]
    fn lookup_returns_none_for_unknown() {
        assert!(pgn_lookup(0xDEAD_BEEF).is_none());
    }

    #[test]
    fn table_size_matches_cpp() {
        // C++ PGN_TABLE has 31 entries (verified by inspection of pgn.hpp).
        assert_eq!(PGN_TABLE.len(), 31);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_pgn_helpers_are_bounded_and_canonical(raw in any::<u32>()) {
            let pf = ((raw >> 8) & 0xFF) as u8;
            let normalized = pgn_normalize(raw);

            prop_assert_eq!(pgn_pdu_format(raw), pf);
            prop_assert_eq!(pgn_is_pdu2(raw), pf >= 240);
            prop_assert_eq!(pgn_is_valid(raw), raw <= 0x3_FFFF);

            if pf < 240 {
                prop_assert_eq!(normalized & 0xFF, 0);
                prop_assert_eq!(normalized, raw & !0xFF);
            } else {
                prop_assert_eq!(normalized, raw);
            }
        }

        #[test]
        fn proptest_known_pgn_table_entries_are_canonical(index in 0usize..PGN_TABLE.len()) {
            let info = PGN_TABLE[index];

            prop_assert!(pgn_is_valid(info.pgn));
            prop_assert_eq!(pgn_normalize(info.pgn), info.pgn);
            prop_assert_eq!(pgn_lookup(info.pgn), Some(info));
        }
    }
}
