//! Common control function (ECU) state shared by [`InternalCf`] and
//! [`PartnerCf`].
//!
//! Mirrors the C++ `machbus::net::ControlFunction` plus the [`CfType`]
//! and [`CfState`] enums.
//!
//! [`InternalCf`]: super::internal_cf::InternalCf
//! [`PartnerCf`]: super::partner_cf::PartnerCf

use super::constants::{MAX_ADDRESS, NULL_ADDRESS};
use super::name::Name;
use super::types::Address;

/// Whether a control function is local (we own it) or remote (we
/// observe / partner with it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CfType {
    /// Local ECU owned by this application.
    Internal,
    /// Remote ECU we have not specifically targeted.
    #[default]
    External,
    /// Remote ECU we explicitly track via NAME filtering.
    Partnered,
}

/// Operational state of a control function on the bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CfState {
    /// Address claimed; permitted to send/receive.
    Online,
    /// No claimed address (yet, or after losing contention).
    #[default]
    Offline,
}

/// Common fields tracked for any control function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlFunction {
    pub name: Name,
    pub address: Address,
    pub can_port: u8,
    pub r#type: CfType,
    pub state: CfState,
}

impl Default for ControlFunction {
    fn default() -> Self {
        // Address must default to NULL_ADDRESS (0xFE) — "no address
        // claimed yet" — *not* 0, which is a valid claimable address
        // and would falsely satisfy `address_valid()`.
        Self {
            name: Name::default(),
            address: NULL_ADDRESS,
            can_port: 0,
            r#type: CfType::default(),
            state: CfState::default(),
        }
    }
}

impl ControlFunction {
    #[must_use]
    pub const fn new(name: Name, can_port: u8, r#type: CfType) -> Self {
        Self {
            name,
            address: NULL_ADDRESS,
            can_port,
            r#type,
            state: CfState::Offline,
        }
    }

    /// `true` when [`Self::address`] is in the valid range
    /// `0..=MAX_ADDRESS` (i.e. not [`NULL_ADDRESS`] / broadcast).
    #[inline]
    #[must_use]
    pub const fn address_valid(&self) -> bool {
        self.address <= MAX_ADDRESS
    }

    #[inline]
    #[must_use]
    pub const fn is_online(&self) -> bool {
        matches!(self.state, CfState::Online)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_offline_external_null_address() {
        let cf = ControlFunction::default();
        assert_eq!(cf.address, NULL_ADDRESS);
        assert_eq!(cf.r#type, CfType::External);
        assert_eq!(cf.state, CfState::Offline);
        assert!(!cf.address_valid()); // NULL_ADDRESS = 0xFE > MAX_ADDRESS = 0xFD
        assert!(!cf.is_online());
    }

    #[test]
    fn address_valid_at_boundary() {
        for (addr, expected) in [
            (0, true),
            (MAX_ADDRESS, true),
            (NULL_ADDRESS, false),
            (0xFF, false),
        ] {
            let cf = ControlFunction {
                address: addr,
                ..Default::default()
            };
            assert_eq!(cf.address_valid(), expected, "addr=0x{addr:02X}");
        }
    }

    #[test]
    fn online_state_query() {
        let cf = ControlFunction {
            state: CfState::Online,
            ..Default::default()
        };
        assert!(cf.is_online());
        let cf = ControlFunction {
            state: CfState::Offline,
            ..Default::default()
        };
        assert!(!cf.is_online());
    }

    #[test]
    fn new_marks_internal_type() {
        let cf = ControlFunction::new(Name::default(), 0, CfType::Internal);
        assert_eq!(cf.r#type, CfType::Internal);
    }
}
