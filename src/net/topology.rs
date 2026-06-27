//! Bus topology rule validator (ISO 11783-2).
//!
//! The physical layer bounds how a segment may be wired: a maximum number of
//! ECUs, a maximum bus length, a maximum simple-stub length, and a minimum
//! spacing between stubs/splices. The electrical derivation of those bounds is
//! hardware, but checking a proposed wiring plan against them is pure software.
//! This module owns the repo's copy of those numeric limits (structural facts,
//! not standard prose) and a validator over a described segment.

/// Maximum ECUs on one bus segment.
pub const MAX_ECUS_PER_SEGMENT: u32 = 30;
/// Maximum total bus length, metres.
pub const MAX_BUS_LENGTH_M: f64 = 40.0;
/// Maximum length of a simple stub, metres.
pub const MAX_SIMPLE_STUB_LENGTH_M: f64 = 3.0;
/// Minimum distance between two stubs/splices, metres.
pub const MIN_STUB_SPACING_M: f64 = 0.5;

/// A proposed bus-segment wiring to check.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BusTopology {
    /// Number of ECUs on the segment.
    pub ecu_count: u32,
    /// Total bus (trunk) length, metres.
    pub bus_length_m: f64,
    /// Longest simple stub on the segment, metres.
    pub longest_stub_m: f64,
    /// Smallest spacing between adjacent stubs/splices, metres.
    pub min_stub_spacing_m: f64,
}

/// A single topology-rule violation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TopologyViolation {
    /// More than [`MAX_ECUS_PER_SEGMENT`] ECUs.
    TooManyEcus { count: u32 },
    /// Bus length exceeds [`MAX_BUS_LENGTH_M`].
    BusTooLong { length_m: f64 },
    /// A simple stub exceeds [`MAX_SIMPLE_STUB_LENGTH_M`].
    StubTooLong { length_m: f64 },
    /// Two stubs/splices are closer than [`MIN_STUB_SPACING_M`].
    StubsTooClose { spacing_m: f64 },
}

/// Check a proposed segment against the ISO 11783-2 topology limits. An empty
/// result means the plan is within all checked limits.
#[must_use]
pub fn validate_topology(topo: &BusTopology) -> Vec<TopologyViolation> {
    let mut out = Vec::new();
    if topo.ecu_count > MAX_ECUS_PER_SEGMENT {
        out.push(TopologyViolation::TooManyEcus {
            count: topo.ecu_count,
        });
    }
    if topo.bus_length_m > MAX_BUS_LENGTH_M {
        out.push(TopologyViolation::BusTooLong {
            length_m: topo.bus_length_m,
        });
    }
    if topo.longest_stub_m > MAX_SIMPLE_STUB_LENGTH_M {
        out.push(TopologyViolation::StubTooLong {
            length_m: topo.longest_stub_m,
        });
    }
    if topo.min_stub_spacing_m < MIN_STUB_SPACING_M {
        out.push(TopologyViolation::StubsTooClose {
            spacing_m: topo.min_stub_spacing_m,
        });
    }
    out
}

/// `true` if `topo` satisfies every checked topology rule.
#[must_use]
pub fn topology_is_valid(topo: &BusTopology) -> bool {
    validate_topology(topo).is_empty()
}

/// Maximum number of Type I WEAK ECUs on a single machine.
pub const MAX_TYPE_I_WEAK_PER_MACHINE: u32 = 3;

/// ISO 11783-2 ECU connection types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcuType {
    /// Standard ECU, no termination.
    TypeI,
    /// ECU with a weak split-termination (limited per machine).
    TypeIWeak,
    /// ECU that contains the bus termination; only valid at a bus end.
    TypeII,
}

impl EcuType {
    /// `true` if this ECU type carries the bus termination.
    #[must_use]
    pub const fn provides_termination(self) -> bool {
        matches!(self, EcuType::TypeII)
    }

    /// `true` if this ECU type may only be placed at an end of the bus.
    #[must_use]
    pub const fn must_be_at_bus_end(self) -> bool {
        matches!(self, EcuType::TypeII)
    }
}

/// Check the ECU-type composition of one machine: the number of Type I WEAK
/// ECUs must not exceed [`MAX_TYPE_I_WEAK_PER_MACHINE`]. Returns the offending
/// count if violated. (Type II bus-end placement is a wiring property checked
/// elsewhere, not derivable from a type list alone.)
#[must_use]
pub fn validate_machine_ecu_types(types: &[EcuType]) -> Option<u32> {
    let weak = types.iter().filter(|t| **t == EcuType::TypeIWeak).count() as u32;
    (weak > MAX_TYPE_I_WEAK_PER_MACHINE).then_some(weak)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compliant_segment_has_no_violations() {
        let ok = BusTopology {
            ecu_count: 30,
            bus_length_m: 40.0,
            longest_stub_m: 3.0,
            min_stub_spacing_m: 0.5,
        };
        assert!(topology_is_valid(&ok));
        assert!(validate_topology(&ok).is_empty());
    }

    #[test]
    fn each_limit_violation_is_reported() {
        let bad = BusTopology {
            ecu_count: 31,
            bus_length_m: 41.0,
            longest_stub_m: 3.5,
            min_stub_spacing_m: 0.4,
        };
        let v = validate_topology(&bad);
        assert!(!topology_is_valid(&bad));
        assert!(v.contains(&TopologyViolation::TooManyEcus { count: 31 }));
        assert!(v.contains(&TopologyViolation::BusTooLong { length_m: 41.0 }));
        assert!(v.contains(&TopologyViolation::StubTooLong { length_m: 3.5 }));
        assert!(v.contains(&TopologyViolation::StubsTooClose { spacing_m: 0.4 }));
        assert_eq!(v.len(), 4);
    }

    #[test]
    fn ecu_type_rules() {
        assert!(EcuType::TypeII.provides_termination());
        assert!(EcuType::TypeII.must_be_at_bus_end());
        assert!(!EcuType::TypeI.provides_termination());
        assert!(!EcuType::TypeIWeak.must_be_at_bus_end());

        // Up to 3 Type I WEAK ECUs per machine is allowed; a 4th is not.
        let ok = [
            EcuType::TypeIWeak,
            EcuType::TypeIWeak,
            EcuType::TypeI,
            EcuType::TypeIWeak,
        ];
        assert_eq!(validate_machine_ecu_types(&ok), None);
        let bad = [EcuType::TypeIWeak; 4];
        assert_eq!(validate_machine_ecu_types(&bad), Some(4));
    }
}
