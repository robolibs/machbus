//! Domain-specific scalar types and the [`Priority`] enum.
//!
//! Mirrors the C++ `machbus::net::types`. Numeric scalar aliases (`u8`,
//! `u32`, etc.) that the C++ pulls from `dp::` are simply Rust primitives.
//!
//! # Newtype tradeoff
//!
//! [`Pgn`] and [`Address`] are kept as transparent aliases (`u32` / `u8`)
//! to match the C++ surface exactly. A future pass may promote them to
//! newtypes once we have call-site experience to judge whether the type
//! safety justifies the friction. This deviation from `PLAN.md` §3 is
//! tracked in `book/src/reference/behavior-differences.md`.

/// Source or destination address on an ISOBUS / J1939 network.
pub type Address = u8;

/// Parameter Group Number — the high-level message-type identifier in
/// J1939 / ISO 11783.
pub type Pgn = u32;

/// 3-bit priority field of a 29-bit CAN identifier.
///
/// Lower numeric values are higher priority on the bus (CAN arbitration
/// is dominant-zero), matching ISO 11783 wire semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(u8)]
pub enum Priority {
    Highest = 0,
    High = 1,
    AboveNormal = 2,
    Normal = 3,
    BelowNormal = 4,
    Low = 5,
    #[default]
    Default = 6,
    Lowest = 7,
}

impl Priority {
    /// Build a [`Priority`] from its 3-bit numeric encoding. Values
    /// outside `0..=7` saturate to [`Priority::Lowest`].
    #[inline]
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Highest,
            1 => Self::High,
            2 => Self::AboveNormal,
            3 => Self::Normal,
            4 => Self::BelowNormal,
            5 => Self::Low,
            6 => Self::Default,
            _ => Self::Lowest,
        }
    }

    /// Build a [`Priority`] from an exact 3-bit numeric encoding.
    ///
    /// Unlike [`Self::from_u8`], this rejects bytes that contain bits outside
    /// the wire priority field.
    #[inline]
    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Highest),
            1 => Some(Self::High),
            2 => Some(Self::AboveNormal),
            3 => Some(Self::Normal),
            4 => Some(Self::BelowNormal),
            5 => Some(Self::Low),
            6 => Some(Self::Default),
            7 => Some(Self::Lowest),
            _ => None,
        }
    }

    /// Return the 3-bit numeric encoding.
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<Priority> for u8 {
    #[inline]
    fn from(p: Priority) -> u8 {
        p.as_u8()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn priority_roundtrip_all_values() {
        for v in 0u8..=7 {
            let p = Priority::from_u8(v);
            assert_eq!(p.as_u8(), v);
        }
    }

    #[test]
    fn priority_from_u8_saturates() {
        for v in 8u8..=255 {
            assert_eq!(Priority::from_u8(v), Priority::Lowest);
        }
    }

    #[test]
    fn priority_default_is_default_variant() {
        assert_eq!(Priority::default(), Priority::Default);
    }

    #[test]
    fn priority_ordering_matches_can_arbitration() {
        // Lower numeric value = higher arbitration priority on the bus.
        assert!(Priority::Highest < Priority::Lowest);
        assert!(Priority::High < Priority::Normal);
    }
}
