//! Implement-bus power supply obligations (ISO 11783-2).
//!
//! The physical layer sets minimum current capacities the implement bus must
//! provide on its switched (ECU_PWR) and unswitched (PWR) power lines, at a
//! nominal supply voltage. The electrical delivery of that power is hardware,
//! but checking a described supply against the minimum capacities is software.
//! This module owns the repo's copy of those numeric limits (structural facts,
//! not standard prose) and a validator. It complements
//! [`crate::net::topology`] for the wiring side.

use alloc::vec::Vec;

/// Minimum current capacity of the switched ECU_PWR line, amps.
pub const ECU_PWR_MIN_CURRENT_A: u8 = 15;
/// Minimum current capacity of the unswitched PWR line, amps.
pub const PWR_MIN_CURRENT_A: u8 = 50;
/// Nominal supply voltage, volts.
pub const NOMINAL_SUPPLY_VOLTAGE_V: u8 = 12;

/// A described implement-bus power supply to check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BusPowerSupply {
    /// Current the ECU_PWR (switched) line can supply, amps.
    pub ecu_pwr_current_a: u8,
    /// Current the PWR (unswitched) line can supply, amps.
    pub pwr_current_a: u8,
}

/// A power-supply shortfall against the minimum capacities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusPowerViolation {
    /// ECU_PWR supplies less than [`ECU_PWR_MIN_CURRENT_A`].
    EcuPwrUndersized { available_a: u8 },
    /// PWR supplies less than [`PWR_MIN_CURRENT_A`].
    PwrUndersized { available_a: u8 },
}

/// Check a supply against the ISO 11783-2 minimum current capacities. An empty
/// result means both lines meet their minimum.
#[must_use]
pub fn validate_bus_power(supply: &BusPowerSupply) -> Vec<BusPowerViolation> {
    let mut out = Vec::new();
    if supply.ecu_pwr_current_a < ECU_PWR_MIN_CURRENT_A {
        out.push(BusPowerViolation::EcuPwrUndersized {
            available_a: supply.ecu_pwr_current_a,
        });
    }
    if supply.pwr_current_a < PWR_MIN_CURRENT_A {
        out.push(BusPowerViolation::PwrUndersized {
            available_a: supply.pwr_current_a,
        });
    }
    out
}

/// `true` if both power lines meet their minimum current capacity.
#[must_use]
pub fn bus_power_is_adequate(supply: &BusPowerSupply) -> bool {
    validate_bus_power(supply).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adequate_supply_has_no_violations() {
        let ok = BusPowerSupply {
            ecu_pwr_current_a: 15,
            pwr_current_a: 50,
        };
        assert!(bus_power_is_adequate(&ok));
        assert!(validate_bus_power(&ok).is_empty());
    }

    #[test]
    fn undersized_lines_are_reported() {
        let bad = BusPowerSupply {
            ecu_pwr_current_a: 10,
            pwr_current_a: 40,
        };
        let v = validate_bus_power(&bad);
        assert!(!bus_power_is_adequate(&bad));
        assert!(v.contains(&BusPowerViolation::EcuPwrUndersized { available_a: 10 }));
        assert!(v.contains(&BusPowerViolation::PwrUndersized { available_a: 40 }));
        assert_eq!(v.len(), 2);
    }
}
