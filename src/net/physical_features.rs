//! Physical-layer obligation checklist (ISO 11783-2).
//!
//! ISO 11783-2 is overwhelmingly an electrical/mechanical specification, so most
//! of it is not software-implementable. This is the repo-owned ledger of the
//! part's obligations, each tagged by how the crate relates to it: provided in
//! software, inherently hardware (out of software scope), or a software item
//! that is still missing. It mirrors [`crate::net::datalink_features`] for the
//! data-link layer and contains no standard prose — only the feature →
//! module → status classification.

/// A physical-layer obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysicalFeature {
    /// CAN bit timing (segments, sample point, SJW).
    BitTiming,
    /// The 250 kbit/s nominal bit rate.
    Bitrate250k,
    /// Adapter capability / readiness model.
    AdapterCapability,
    /// Bus-load monitoring against the line rate.
    BusLoadMonitor,
    /// Capture / replay / verification harness for bus evidence.
    CaptureHarness,
    /// Bus termination (TBC / split termination, breakaway disconnect).
    Termination,
    /// Transceiver electrical parameters.
    Transceiver,
    /// Cable / physical media parameters.
    Cabling,
    /// Connector definitions and pin allocations.
    Connectors,
    /// Topology rules (bus/stub length, node count, spacing).
    TopologyRules,
    /// Bus / ECU power supply ranges and current capacities.
    BusPower,
    /// CAN fault confinement (error-passive / bus-off) reaction.
    FaultConfinement,
}

/// How the crate relates to a physical-layer obligation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhysStatus {
    /// Provided in software with in-repo tests.
    Implemented,
    /// Inherently hardware/mechanical — out of software scope.
    Hardware,
    /// A software-implementable item that is not done yet.
    Missing,
}

/// One checklist row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PhysicalFeatureRow {
    pub feature: PhysicalFeature,
    pub module: &'static str,
    pub status: PhysStatus,
}

use PhysStatus::{Hardware, Implemented, Missing};
use PhysicalFeature as F;

/// The ISO 11783-2 physical-layer obligation checklist.
pub const PHYSICAL_FEATURES: [PhysicalFeatureRow; 12] = [
    PhysicalFeatureRow {
        feature: F::BitTiming,
        module: "net::can_bus_config",
        status: Implemented,
    },
    PhysicalFeatureRow {
        feature: F::Bitrate250k,
        module: "net::can_bus_config",
        status: Implemented,
    },
    PhysicalFeatureRow {
        feature: F::AdapterCapability,
        module: "net::adapter",
        status: Implemented,
    },
    PhysicalFeatureRow {
        feature: F::BusLoadMonitor,
        module: "net::bus_load",
        status: Implemented,
    },
    PhysicalFeatureRow {
        feature: F::CaptureHarness,
        module: "net::capture",
        status: Implemented,
    },
    PhysicalFeatureRow {
        feature: F::Termination,
        module: "hardware",
        status: Hardware,
    },
    PhysicalFeatureRow {
        feature: F::Transceiver,
        module: "hardware",
        status: Hardware,
    },
    PhysicalFeatureRow {
        feature: F::Cabling,
        module: "hardware",
        status: Hardware,
    },
    PhysicalFeatureRow {
        feature: F::Connectors,
        module: "hardware",
        status: Hardware,
    },
    PhysicalFeatureRow {
        feature: F::TopologyRules,
        module: "net::topology",
        status: Implemented,
    },
    PhysicalFeatureRow {
        feature: F::BusPower,
        module: "net::bus_power",
        status: Implemented,
    },
    // Software-implementable but not yet done (see GAP.md ISO 11783-2).
    PhysicalFeatureRow {
        feature: F::FaultConfinement,
        module: "net::policy (not wired to controller error state)",
        status: Missing,
    },
];

/// The checklist row for a feature.
#[must_use]
pub fn physical_feature(feature: PhysicalFeature) -> PhysicalFeatureRow {
    PHYSICAL_FEATURES
        .into_iter()
        .find(|r| r.feature == feature)
        .expect("every physical-layer feature has a row")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [PhysicalFeature; 12] = [
        F::BitTiming,
        F::Bitrate250k,
        F::AdapterCapability,
        F::BusLoadMonitor,
        F::CaptureHarness,
        F::Termination,
        F::Transceiver,
        F::Cabling,
        F::Connectors,
        F::TopologyRules,
        F::BusPower,
        F::FaultConfinement,
    ];

    #[test]
    fn every_feature_has_one_row_with_honest_status() {
        assert_eq!(PHYSICAL_FEATURES.len(), ALL.len());
        for f in ALL {
            let rows: Vec<_> = PHYSICAL_FEATURES
                .iter()
                .filter(|r| r.feature == f)
                .collect();
            assert_eq!(rows.len(), 1, "{f:?} must have exactly one row");
            assert!(!rows[0].module.is_empty());
        }
        // Implemented rows point at a real net module; hardware rows are tagged.
        assert_eq!(
            physical_feature(F::Bitrate250k).status,
            PhysStatus::Implemented
        );
        assert!(physical_feature(F::BitTiming).module.starts_with("net::"));
        assert_eq!(
            physical_feature(F::Termination).status,
            PhysStatus::Hardware
        );
        assert_eq!(
            physical_feature(F::TopologyRules).status,
            PhysStatus::Implemented
        );
        assert_eq!(
            physical_feature(F::BusPower).status,
            PhysStatus::Implemented
        );
        assert_eq!(
            physical_feature(F::FaultConfinement).status,
            PhysStatus::Missing
        );
    }
}
