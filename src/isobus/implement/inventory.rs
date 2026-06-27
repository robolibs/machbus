//! Implement-message family inventory (ISO 11783-7).
//!
//! GAP.md (ISO 11783-7) asks to "build an implement-message inventory with
//! one row per message family and public API decision", noting that the
//! current surface is codec-level rather than a full implement-ECU
//! application. This is that inventory as typed, queryable code.
//!
//! Each row states the message family, its backing module, and its
//! integration level: `Codec` (encode/decode + reject tests) or `Runtime`
//! (additionally stateful). It contains no standard prose — only the
//! repo-owned family→module→level classification.

/// An ISO 11783-7 implement message family the crate provides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ImplementMessageFamily {
    Guidance,
    Lighting,
    SpeedDistance,
    MachineSpeedCommand,
    TractorCommands,
    TractorFacilities,
    AuxValveStatus,
    DriveStrategy,
}

/// Integration level of a message family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FamilyLevel {
    /// Encode/decode codecs with strict reject tests; no owned state.
    Codec,
    /// Codecs plus a stateful runtime helper.
    Runtime,
}

/// One inventory row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImplementFamilyInfo {
    pub family: ImplementMessageFamily,
    pub module: &'static str,
    pub level: FamilyLevel,
}

use FamilyLevel::{Codec, Runtime};
use ImplementMessageFamily as F;

/// The implement-message family inventory.
pub const IMPLEMENT_FAMILIES: [ImplementFamilyInfo; 8] = [
    ImplementFamilyInfo {
        family: F::Guidance,
        module: "isobus::implement::guidance",
        level: Codec,
    },
    ImplementFamilyInfo {
        family: F::Lighting,
        module: "isobus::implement::lighting",
        level: Runtime,
    },
    ImplementFamilyInfo {
        family: F::SpeedDistance,
        module: "isobus::implement::speed_distance",
        level: Codec,
    },
    ImplementFamilyInfo {
        family: F::MachineSpeedCommand,
        module: "isobus::implement::machine_speed_cmd",
        level: Codec,
    },
    ImplementFamilyInfo {
        family: F::TractorCommands,
        module: "isobus::implement::tractor_commands",
        level: Codec,
    },
    ImplementFamilyInfo {
        family: F::TractorFacilities,
        module: "isobus::implement::tractor_facilities",
        level: Runtime,
    },
    ImplementFamilyInfo {
        family: F::AuxValveStatus,
        module: "isobus::implement::aux_valve_status",
        level: Codec,
    },
    ImplementFamilyInfo {
        family: F::DriveStrategy,
        module: "isobus::implement::drive_strategy",
        level: Codec,
    },
];

/// The inventory row for a family.
#[must_use]
pub fn family_info(family: ImplementMessageFamily) -> ImplementFamilyInfo {
    IMPLEMENT_FAMILIES
        .into_iter()
        .find(|f| f.family == family)
        .expect("every implement family has an inventory row")
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [ImplementMessageFamily; 8] = [
        F::Guidance,
        F::Lighting,
        F::SpeedDistance,
        F::MachineSpeedCommand,
        F::TractorCommands,
        F::TractorFacilities,
        F::AuxValveStatus,
        F::DriveStrategy,
    ];

    #[test]
    fn every_family_has_one_row_with_a_module() {
        assert_eq!(IMPLEMENT_FAMILIES.len(), ALL.len());
        for fam in ALL {
            let rows: Vec<_> = IMPLEMENT_FAMILIES
                .iter()
                .filter(|f| f.family == fam)
                .collect();
            assert_eq!(rows.len(), 1, "{fam:?} must have one row");
            assert!(rows[0].module.starts_with("isobus::implement::"));
        }
    }

    #[test]
    fn levels_reflect_codec_vs_runtime_classification() {
        assert_eq!(family_info(F::Guidance).level, FamilyLevel::Codec);
        assert_eq!(family_info(F::Lighting).level, FamilyLevel::Runtime);
        assert_eq!(
            family_info(F::TractorFacilities).level,
            FamilyLevel::Runtime
        );
    }
}
