//! ISO 11783-9 §4.4 Tractor Facilities + TECU classification.
//!
//! Mirrors the C++ `machbus::isobus::implement::tractor_facilities.hpp`.
//! Hosts the canonical [`TecuClass`] enum (re-exported from
//! `isobus::tractor_ecu` for convenience).
//!
//! Two PGNs share the [`TractorFacilities`] payload:
//!
//! - `PGN_TRACTOR_FACILITIES_RESPONSE` (0xFE09) — TECU broadcast.
//! - `PGN_REQUIRED_TRACTOR_FACILITIES` (0xFE0A) — implement → TECU.
//!
//! The C++ `TractorFacilitiesInterface` (IsoNet-coupled) is
//! intentionally not ported.

use alloc::{collections::BTreeMap, vec::Vec};

use crate::net::pgn_defs::{PGN_REQUIRED_TRACTOR_FACILITIES, PGN_TRACTOR_FACILITIES_RESPONSE};
use crate::net::types::{Address, Pgn};

/// ISO 11783-9 §4.4.2 TECU classification (base class).
///
/// Canonical home for `TecuClass`. The wider
/// [`TecuClassification`](crate::isobus::tractor_ecu::TecuClassification)
/// (base class + N/F/G/P/M addendum flags) lives in
/// [`crate::isobus::tractor_ecu`] which re-exports this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TecuClass {
    /// Basic (speed, hitch, PTO, power management).
    #[default]
    Class1 = 1,
    /// Full measurements (distance, direction, draft, lighting, aux flow).
    Class2 = 2,
    /// Accepts commands (hitch cmd, PTO cmd, aux valve cmd).
    Class3 = 3,
}

impl TecuClass {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            1 => Some(Self::Class1),
            2 => Some(Self::Class2),
            3 => Some(Self::Class3),
            _ => None,
        }
    }
}

/// PGN-routing tag for [`TractorFacilities`] sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TractorFacilitiesRole {
    /// `PGN_TRACTOR_FACILITIES_RESPONSE` (0xFE09) — what the TECU
    /// supports.
    #[default]
    Response,
    /// `PGN_REQUIRED_TRACTOR_FACILITIES` (0xFE0A) — what the implement
    /// requires.
    Required,
}

impl TractorFacilitiesRole {
    #[must_use]
    pub const fn pgn(self) -> Pgn {
        match self {
            Self::Response => PGN_TRACTOR_FACILITIES_RESPONSE,
            Self::Required => PGN_REQUIRED_TRACTOR_FACILITIES,
        }
    }
}

/// Bit-packed tractor facility flags. Bit layout matches the C++
/// (and ISO 11783-9 §4.4) byte-by-byte:
///
/// - byte 0: Class 1 + start of Class 2.
/// - byte 1: Class 2 (cont.) + Class 3.
/// - byte 2: Front (F) addendum + Navigation (N) + Guidance (G).
/// - byte 3: Powertrain (P) + v2 Class 3 limit/exit bits.
/// - byte 4: v2 aux-valve exit + front-F v2 limit/exit bits (low 6
///   bits; bits 6–7 reserved = 1).
/// - bytes 5–7: reserved (`0xFF`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TractorFacilities {
    // Class 1.
    pub rear_hitch_position: bool,
    pub rear_hitch_in_work: bool,
    pub rear_pto_speed: bool,
    pub rear_pto_engagement: bool,
    pub wheel_based_speed: bool,
    pub ground_based_speed: bool,

    // Class 2.
    pub ground_based_distance: bool,
    pub ground_based_direction: bool,
    pub wheel_based_distance: bool,
    pub wheel_based_direction: bool,
    pub rear_draft: bool,
    pub lighting: bool,
    pub aux_valve_flow: bool,

    // Class 3.
    pub rear_hitch_command: bool,
    pub rear_pto_command: bool,
    pub aux_valve_command: bool,

    // Front (F) addendum.
    pub front_hitch_position: bool,
    pub front_hitch_in_work: bool,
    pub front_pto_speed: bool,
    pub front_pto_engagement: bool,
    pub front_hitch_command: bool,
    pub front_pto_command: bool,

    // Navigation (N) addendum.
    pub navigation: bool,

    // Guidance (G) addendum.
    pub guidance: bool,

    // Powertrain (P) addendum.
    pub machine_selected_speed: bool,
    pub machine_selected_speed_command: bool,

    // TECU v2 Class 3: limit-status / exit-code support (rear).
    pub rear_hitch_limit_status: bool,
    pub rear_hitch_exit_code: bool,
    pub rear_pto_engagement_request: bool,
    pub rear_pto_speed_limit_status: bool,
    pub rear_pto_exit_code: bool,
    pub aux_valve_limit_status: bool,
    pub aux_valve_exit_code: bool,

    // TECU v2 F addendum: front limit/exit.
    pub front_hitch_limit_status: bool,
    pub front_hitch_exit_code: bool,
    pub front_pto_engagement_request: bool,
    pub front_pto_speed_limit_status: bool,
    pub front_pto_exit_code: bool,
}

impl TractorFacilities {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];

        // Byte 0: Class 1 + start of Class 2.
        data[0] = (u8::from(self.rear_hitch_position))
            | (u8::from(self.rear_hitch_in_work) << 1)
            | (u8::from(self.rear_pto_speed) << 2)
            | (u8::from(self.rear_pto_engagement) << 3)
            | (u8::from(self.wheel_based_speed) << 4)
            | (u8::from(self.ground_based_speed) << 5)
            | (u8::from(self.ground_based_distance) << 6)
            | (u8::from(self.ground_based_direction) << 7);

        // Byte 1: Class 2 (cont.) + Class 3.
        data[1] = (u8::from(self.wheel_based_distance))
            | (u8::from(self.wheel_based_direction) << 1)
            | (u8::from(self.rear_draft) << 2)
            | (u8::from(self.lighting) << 3)
            | (u8::from(self.aux_valve_flow) << 4)
            | (u8::from(self.rear_hitch_command) << 5)
            | (u8::from(self.rear_pto_command) << 6)
            | (u8::from(self.aux_valve_command) << 7);

        // Byte 2: F + N + G addenda.
        data[2] = (u8::from(self.front_hitch_position))
            | (u8::from(self.front_hitch_in_work) << 1)
            | (u8::from(self.front_pto_speed) << 2)
            | (u8::from(self.front_pto_engagement) << 3)
            | (u8::from(self.front_hitch_command) << 4)
            | (u8::from(self.front_pto_command) << 5)
            | (u8::from(self.navigation) << 6)
            | (u8::from(self.guidance) << 7);

        // Byte 3: P + v2 Class 3 limit/exit (rear).
        data[3] = (u8::from(self.machine_selected_speed))
            | (u8::from(self.machine_selected_speed_command) << 1)
            | (u8::from(self.rear_hitch_limit_status) << 2)
            | (u8::from(self.rear_hitch_exit_code) << 3)
            | (u8::from(self.rear_pto_engagement_request) << 4)
            | (u8::from(self.rear_pto_speed_limit_status) << 5)
            | (u8::from(self.rear_pto_exit_code) << 6)
            | (u8::from(self.aux_valve_limit_status) << 7);

        // Byte 4: v2 aux-valve exit + front-F v2 limit/exit (bits 6–7
        // reserved = 1).
        data[4] = 0xC0
            | (u8::from(self.aux_valve_exit_code))
            | (u8::from(self.front_hitch_limit_status) << 1)
            | (u8::from(self.front_hitch_exit_code) << 2)
            | (u8::from(self.front_pto_engagement_request) << 3)
            | (u8::from(self.front_pto_speed_limit_status) << 4)
            | (u8::from(self.front_pto_exit_code) << 5);

        // Bytes 5–7: reserved 0xFF (already set).
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || data[4] & 0xC0 != 0xC0 || data[5..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        let mut f = Self {
            // Byte 0.
            rear_hitch_position: bit(data[0], 0),
            rear_hitch_in_work: bit(data[0], 1),
            rear_pto_speed: bit(data[0], 2),
            rear_pto_engagement: bit(data[0], 3),
            wheel_based_speed: bit(data[0], 4),
            ground_based_speed: bit(data[0], 5),
            ground_based_distance: bit(data[0], 6),
            ground_based_direction: bit(data[0], 7),
            // Byte 1.
            wheel_based_distance: bit(data[1], 0),
            wheel_based_direction: bit(data[1], 1),
            rear_draft: bit(data[1], 2),
            lighting: bit(data[1], 3),
            aux_valve_flow: bit(data[1], 4),
            rear_hitch_command: bit(data[1], 5),
            rear_pto_command: bit(data[1], 6),
            aux_valve_command: bit(data[1], 7),
            // Byte 2.
            front_hitch_position: bit(data[2], 0),
            front_hitch_in_work: bit(data[2], 1),
            front_pto_speed: bit(data[2], 2),
            front_pto_engagement: bit(data[2], 3),
            front_hitch_command: bit(data[2], 4),
            front_pto_command: bit(data[2], 5),
            navigation: bit(data[2], 6),
            guidance: bit(data[2], 7),
            // Byte 3.
            machine_selected_speed: bit(data[3], 0),
            machine_selected_speed_command: bit(data[3], 1),
            rear_hitch_limit_status: bit(data[3], 2),
            rear_hitch_exit_code: bit(data[3], 3),
            rear_pto_engagement_request: bit(data[3], 4),
            rear_pto_speed_limit_status: bit(data[3], 5),
            rear_pto_exit_code: bit(data[3], 6),
            aux_valve_limit_status: bit(data[3], 7),
            ..Self::default()
        };
        f.aux_valve_exit_code = bit(data[4], 0);
        f.front_hitch_limit_status = bit(data[4], 1);
        f.front_hitch_exit_code = bit(data[4], 2);
        f.front_pto_engagement_request = bit(data[4], 3);
        f.front_pto_speed_limit_status = bit(data[4], 4);
        f.front_pto_exit_code = bit(data[4], 5);
        Some(f)
    }

    #[must_use]
    pub fn with_class1_all(mut self) -> Self {
        self.rear_hitch_position = true;
        self.rear_hitch_in_work = true;
        self.rear_pto_speed = true;
        self.rear_pto_engagement = true;
        self.wheel_based_speed = true;
        self.ground_based_speed = true;
        self
    }

    #[must_use]
    pub fn with_class2_all(mut self) -> Self {
        self.ground_based_distance = true;
        self.ground_based_direction = true;
        self.wheel_based_distance = true;
        self.wheel_based_direction = true;
        self.rear_draft = true;
        self.lighting = true;
        self.aux_valve_flow = true;
        self
    }

    #[must_use]
    pub fn with_class3_all(mut self) -> Self {
        self.rear_hitch_command = true;
        self.rear_pto_command = true;
        self.aux_valve_command = true;
        self
    }

    #[must_use]
    pub fn with_class3_v2_all(mut self) -> Self {
        self.rear_hitch_limit_status = true;
        self.rear_hitch_exit_code = true;
        self.rear_pto_engagement_request = true;
        self.rear_pto_speed_limit_status = true;
        self.rear_pto_exit_code = true;
        self.aux_valve_limit_status = true;
        self.aux_valve_exit_code = true;
        self
    }

    #[must_use]
    pub fn with_front_v2_all(mut self) -> Self {
        self.front_hitch_limit_status = true;
        self.front_hitch_exit_code = true;
        self.front_pto_engagement_request = true;
        self.front_pto_speed_limit_status = true;
        self.front_pto_exit_code = true;
        self
    }
}

#[inline]
const fn bit(b: u8, n: u8) -> bool {
    (b >> n) & 0x01 != 0
}

// ─── TECU facility/capability matrix (ISO 11783-9 §4.4) ────────────────
//
// GAP.md (ISO 11783-9) asks for "a public capability/facility matrix".
// This is the typed grouping of every facility flag by TECU class /
// addendum, so a persona/config can be checked against the class it claims.

/// The class or addendum a facility belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FacilityGroup {
    Class1,
    Class2,
    Class3,
    /// Front (F) addendum.
    Front,
    /// Navigation (N) addendum.
    Navigation,
    /// Guidance (G) addendum.
    Guidance,
    /// Powertrain (P) addendum (machine selected speed).
    Powertrain,
    /// TECU v2 limit-status / exit-code extensions.
    V2Extension,
}

/// The facility matrix: each facility name and the group it belongs to.
/// Names match the [`TractorFacilities`] fields.
pub const TECU_FACILITY_MATRIX: &[(&str, FacilityGroup)] = &[
    ("rear_hitch_position", FacilityGroup::Class1),
    ("rear_hitch_in_work", FacilityGroup::Class1),
    ("rear_pto_speed", FacilityGroup::Class1),
    ("rear_pto_engagement", FacilityGroup::Class1),
    ("wheel_based_speed", FacilityGroup::Class1),
    ("ground_based_speed", FacilityGroup::Class1),
    ("ground_based_distance", FacilityGroup::Class2),
    ("ground_based_direction", FacilityGroup::Class2),
    ("wheel_based_distance", FacilityGroup::Class2),
    ("wheel_based_direction", FacilityGroup::Class2),
    ("rear_draft", FacilityGroup::Class2),
    ("lighting", FacilityGroup::Class2),
    ("aux_valve_flow", FacilityGroup::Class2),
    ("rear_hitch_command", FacilityGroup::Class3),
    ("rear_pto_command", FacilityGroup::Class3),
    ("aux_valve_command", FacilityGroup::Class3),
    ("front_hitch_position", FacilityGroup::Front),
    ("front_hitch_in_work", FacilityGroup::Front),
    ("front_pto_speed", FacilityGroup::Front),
    ("front_pto_engagement", FacilityGroup::Front),
    ("front_hitch_command", FacilityGroup::Front),
    ("front_pto_command", FacilityGroup::Front),
    ("navigation", FacilityGroup::Navigation),
    ("guidance", FacilityGroup::Guidance),
    ("machine_selected_speed", FacilityGroup::Powertrain),
    ("machine_selected_speed_command", FacilityGroup::Powertrain),
    ("rear_hitch_limit_status", FacilityGroup::V2Extension),
    ("rear_hitch_exit_code", FacilityGroup::V2Extension),
    ("rear_pto_engagement_request", FacilityGroup::V2Extension),
    ("rear_pto_speed_limit_status", FacilityGroup::V2Extension),
    ("rear_pto_exit_code", FacilityGroup::V2Extension),
    ("aux_valve_limit_status", FacilityGroup::V2Extension),
    ("aux_valve_exit_code", FacilityGroup::V2Extension),
    ("front_hitch_limit_status", FacilityGroup::V2Extension),
    ("front_hitch_exit_code", FacilityGroup::V2Extension),
    ("front_pto_engagement_request", FacilityGroup::V2Extension),
    ("front_pto_speed_limit_status", FacilityGroup::V2Extension),
    ("front_pto_exit_code", FacilityGroup::V2Extension),
];

/// Facility names belonging to a group.
#[must_use]
pub fn facilities_in(group: FacilityGroup) -> Vec<&'static str> {
    TECU_FACILITY_MATRIX
        .iter()
        .filter(|(_, g)| *g == group)
        .map(|(name, _)| *name)
        .collect()
}

/// Aggregates the required-tractor-facilities requests from multiple implements
/// into the single union the TECU must provide (ISO 11783-9): the TECU merges
/// per-implement requests and transmits exactly that union, rather than every
/// facility unconditionally.
#[derive(Debug, Clone, Default)]
pub struct RequiredFacilitiesAggregator {
    by_source: BTreeMap<Address, TractorFacilities>,
}

impl RequiredFacilitiesAggregator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record (or replace) the facilities one implement requires.
    pub fn observe(&mut self, source: Address, required: TractorFacilities) {
        self.by_source.insert(source, required);
    }

    /// Drop an implement (e.g. on disconnect); returns its last request.
    pub fn forget(&mut self, source: Address) -> Option<TractorFacilities> {
        self.by_source.remove(&source)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.by_source.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_source.is_empty()
    }

    /// The union of every observed request — the facility set the TECU should
    /// provide. (Computed as the bitwise OR of the canonical encodings, which
    /// preserves the reserved-bit pattern.)
    #[must_use]
    pub fn merged(&self) -> TractorFacilities {
        if self.by_source.is_empty() {
            return TractorFacilities::default();
        }
        let mut bytes = [0u8; 8];
        for f in self.by_source.values() {
            let encoded = f.encode();
            bytes.iter_mut().zip(encoded).for_each(|(b, e)| *b |= e);
        }
        TractorFacilities::decode(&bytes).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_facilities_aggregator_unions_requests() {
        let mut agg = RequiredFacilitiesAggregator::new();
        assert!(agg.is_empty());
        let a = TractorFacilities {
            rear_hitch_position: true,
            ..TractorFacilities::default()
        };
        let b = TractorFacilities {
            lighting: true,
            ..TractorFacilities::default()
        };
        agg.observe(0x80, a);
        agg.observe(0x81, b);
        assert_eq!(agg.len(), 2);

        let merged = agg.merged();
        assert!(merged.rear_hitch_position);
        assert!(merged.lighting);

        // Dropping a source removes its facilities from the union.
        agg.forget(0x80);
        let merged = agg.merged();
        assert!(!merged.rear_hitch_position);
        assert!(merged.lighting);
    }

    #[test]
    fn tecu_class_round_trip() {
        for c in [TecuClass::Class1, TecuClass::Class2, TecuClass::Class3] {
            assert_eq!(TecuClass::from_u8(c.as_u8()), Some(c));
        }
        assert_eq!(TecuClass::from_u8(0), None);
        assert_eq!(TecuClass::from_u8(4), None);
    }

    #[test]
    fn tecu_facility_matrix_groups_every_facility_uniquely() {
        // 38 facility flags on TractorFacilities → 38 matrix rows.
        assert_eq!(TECU_FACILITY_MATRIX.len(), 38);
        let mut seen = alloc::collections::BTreeSet::new();
        for (name, _) in TECU_FACILITY_MATRIX {
            assert!(seen.insert(*name), "duplicate facility {name}");
        }
        // Class 1 baseline is the six core facilities.
        assert_eq!(facilities_in(FacilityGroup::Class1).len(), 6);
        assert!(facilities_in(FacilityGroup::Class1).contains(&"rear_hitch_position"));
        // Commands are Class 3.
        assert!(facilities_in(FacilityGroup::Class3).contains(&"rear_hitch_command"));
        // Guidance/navigation are addenda, one facility each.
        assert_eq!(facilities_in(FacilityGroup::Guidance), vec!["guidance"]);
        assert_eq!(facilities_in(FacilityGroup::Navigation), vec!["navigation"]);
    }

    #[test]
    fn tractor_facilities_role_pgn() {
        assert_eq!(
            TractorFacilitiesRole::Response.pgn(),
            PGN_TRACTOR_FACILITIES_RESPONSE
        );
        assert_eq!(
            TractorFacilitiesRole::Required.pgn(),
            PGN_REQUIRED_TRACTOR_FACILITIES
        );
    }

    #[test]
    fn class1_round_trip() {
        let f = TractorFacilities::default().with_class1_all();
        let bytes = f.encode();
        let decoded = TractorFacilities::decode(&bytes).unwrap();
        assert_eq!(decoded, f);
        // Byte 0 low 6 bits set, ground_based_distance/direction off
        // means byte 0 == 0b0011_1111.
        assert_eq!(bytes[0], 0b0011_1111);
    }

    #[test]
    fn class2_round_trip() {
        let f = TractorFacilities::default().with_class2_all();
        let bytes = f.encode();
        let decoded = TractorFacilities::decode(&bytes).unwrap();
        assert_eq!(decoded, f);
    }

    #[test]
    fn class3_with_v2_round_trip() {
        let f = TractorFacilities::default()
            .with_class3_all()
            .with_class3_v2_all()
            .with_front_v2_all();
        let bytes = f.encode();
        // Reserved bits 6-7 of byte 4 must remain set.
        assert_eq!(bytes[4] & 0xC0, 0xC0);
        let decoded = TractorFacilities::decode(&bytes).unwrap();
        assert_eq!(decoded, f);
    }

    #[test]
    fn full_round_trip() {
        let f = TractorFacilities {
            navigation: true,
            guidance: true,
            front_hitch_position: true,
            front_pto_command: true,
            machine_selected_speed: true,
            machine_selected_speed_command: true,
            ..Default::default()
        };
        let decoded = TractorFacilities::decode(&f.encode()).unwrap();
        assert_eq!(decoded, f);
    }

    #[test]
    fn short_payload_returns_none() {
        assert!(TractorFacilities::decode(&[0u8; 3]).is_none());
        assert!(TractorFacilities::decode(&[0u8; 4]).is_none());
        assert!(TractorFacilities::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn overlong_payload_returns_none() {
        assert!(TractorFacilities::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn reserved_bytes_and_bits_are_rejected() {
        let mut bytes = TractorFacilities::default()
            .with_class3_v2_all()
            .with_front_v2_all()
            .encode();

        bytes[4] &= 0x3F;
        assert!(TractorFacilities::decode(&bytes).is_none());

        let mut bytes = TractorFacilities::default().encode();
        bytes[5] = 0x00;
        assert!(TractorFacilities::decode(&bytes).is_none());
    }
}
