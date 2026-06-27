//! Manufacturer-proprietary message classification (ISO 11783-3 §5.4.6).
//!
//! Mirrors the C++ `machbus::j1939::proprietary.hpp`.
//!
//! - **Proprietary A** (`PGN_PROPRIETARY_A = 0xEF00`): destination-specific,
//!   up to 1785 bytes via TP. Manufacturer identifies via source NAME.
//! - **Proprietary A2** (`PGN_PROPRIETARY_A2 = 0x1EF00`): extended-data-page
//!   variant of Proprietary A.
//! - **Proprietary B** (`PGN_PROPRIETARY_B_BASE = 0xFF00..=0xFFFF`):
//!   broadcast, 256 PGNs available indexed by group extension byte.

use alloc::vec::Vec;

use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_PROPRIETARY_A, PGN_PROPRIETARY_A2, PGN_PROPRIETARY_B_BASE};
use crate::net::types::{Address, Pgn};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProprietaryMsg {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub source: Address,
    pub destination: Address,
}

impl Default for ProprietaryMsg {
    fn default() -> Self {
        Self {
            pgn: PGN_PROPRIETARY_A,
            data: Vec::new(),
            source: NULL_ADDRESS,
            destination: BROADCAST_ADDRESS,
        }
    }
}

impl ProprietaryMsg {
    /// Build a proprietary message from a received [`Message`].
    ///
    /// Returns [`None`] for non-proprietary PGNs and unusable source
    /// addresses. The payload itself is manufacturer-defined and is copied
    /// without interpretation.
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !is_proprietary_pgn(msg.pgn)
            || !msg.has_usable_source()
            || !msg.has_valid_destination_for_pgn()
        {
            return None;
        }
        Some(Self {
            pgn: msg.pgn,
            data: msg.data.clone(),
            source: msg.source,
            destination: msg.destination,
        })
    }

    #[inline]
    #[must_use]
    pub fn is_proprietary_a(&self) -> bool {
        self.pgn == PGN_PROPRIETARY_A
    }

    #[inline]
    #[must_use]
    pub fn is_proprietary_a2(&self) -> bool {
        self.pgn == PGN_PROPRIETARY_A2
    }

    /// Proprietary B PGNs occupy the broadcast range `0xFF00..=0xFFFF`.
    #[inline]
    #[must_use]
    pub fn is_proprietary_b(&self) -> bool {
        self.pgn >= PGN_PROPRIETARY_B_BASE && self.pgn <= 0xFFFF
    }

    /// Group extension byte (low byte of the PGN). Meaningful only
    /// for Proprietary B.
    #[inline]
    #[must_use]
    pub const fn group_extension(&self) -> u8 {
        (self.pgn & 0xFF) as u8
    }
}

/// Build the PGN for a Proprietary B group extension byte.
#[inline]
#[must_use]
pub const fn proprietary_b_pgn(group_extension: u8) -> Pgn {
    PGN_PROPRIETARY_B_BASE + group_extension as Pgn
}

#[inline]
#[must_use]
pub const fn is_proprietary_pgn(pgn: Pgn) -> bool {
    pgn == PGN_PROPRIETARY_A || pgn == PGN_PROPRIETARY_A2 || is_proprietary_b_pgn(pgn)
}

#[inline]
#[must_use]
pub const fn is_proprietary_b_pgn(pgn: Pgn) -> bool {
    pgn >= PGN_PROPRIETARY_B_BASE && pgn <= 0xFFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_routes_correctly() {
        let a = ProprietaryMsg {
            pgn: PGN_PROPRIETARY_A,
            ..Default::default()
        };
        assert!(a.is_proprietary_a());
        assert!(!a.is_proprietary_b());

        let a2 = ProprietaryMsg {
            pgn: PGN_PROPRIETARY_A2,
            ..Default::default()
        };
        assert!(a2.is_proprietary_a2());

        let b = ProprietaryMsg {
            pgn: PGN_PROPRIETARY_B_BASE + 0x42,
            ..Default::default()
        };
        assert!(b.is_proprietary_b());
        assert_eq!(b.group_extension(), 0x42);
    }

    #[test]
    fn proprietary_b_pgn_helper() {
        assert_eq!(proprietary_b_pgn(0), PGN_PROPRIETARY_B_BASE);
        assert_eq!(proprietary_b_pgn(0xFF), PGN_PROPRIETARY_B_BASE + 0xFF);
    }

    #[test]
    fn from_message_copies_data() {
        let msg = Message::new(PGN_PROPRIETARY_A, vec![1, 2, 3], 0x10);
        let p = ProprietaryMsg::from_message(&msg).unwrap();
        assert_eq!(p.pgn, PGN_PROPRIETARY_A);
        assert_eq!(p.data, vec![1, 2, 3]);
        assert_eq!(p.source, 0x10);
    }
}
