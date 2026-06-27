//! NMEA 2000 PGN catalog / coverage ledger.
//!
//! GAP.md (NMEA 2000) asks to "mark every PGN as implemented, selected-
//! out-of-scope, missing …" and warns against claiming full Appendix A/B
//! support. This is the repo-owned ledger for machbus's *own* NMEA 2000
//! surface: each PGN the crate defines a constant for is tagged
//! [`NmeaPgnStatus::Implemented`] (a runtime decoder exists) or
//! [`NmeaPgnStatus::NotImplemented`] (constant only — no decoder yet). It
//! is intentionally *not* the full Appendix catalog,
//! and contains no standard prose — only PGN numbers (public network
//! identifiers) and repo-owned short names.
//!
//! The implemented rows are kept honest by a test that cross-checks them
//! against [`crate::nmea::NMEA2000_INTERFACE_PGNS`] and the management set.

use crate::net::pgn_defs as pgn;
use crate::net::types::Pgn;

/// Coverage status of a PGN in machbus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NmeaPgnStatus {
    /// A runtime decoder/handler exists for this PGN.
    Implemented,
    /// The PGN number is defined but no decoder exists yet.
    NotImplemented,
}

/// One catalog row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NmeaPgnEntry {
    pub pgn: Pgn,
    pub name: &'static str,
    pub status: NmeaPgnStatus,
}

use NmeaPgnStatus::{Implemented, NotImplemented};

/// machbus's NMEA 2000 PGN coverage ledger (not the full Appendix A/B).
pub const NMEA_PGN_CATALOG: &[NmeaPgnEntry] = &[
    // ─ Implemented: network management ─
    e(pgn::PGN_SYSTEM_TIME, "System Time", Implemented),
    e(pgn::PGN_PRODUCT_INFO, "Product Information", Implemented),
    e(
        pgn::PGN_CONFIG_INFO,
        "Configuration Information",
        Implemented,
    ),
    e(pgn::PGN_HEARTBEAT_N2K, "Heartbeat", Implemented),
    // ─ Implemented: GNSS / navigation ─
    e(
        pgn::PGN_GNSS_POSITION_RAPID,
        "Position, Rapid Update",
        Implemented,
    ),
    e(
        pgn::PGN_GNSS_COG_SOG_RAPID,
        "COG & SOG, Rapid Update",
        Implemented,
    ),
    e(pgn::PGN_GNSS_POSITION_DELTA, "Position Delta", Implemented),
    e(
        pgn::PGN_GNSS_POSITION_DATA,
        "GNSS Position Data",
        Implemented,
    ),
    e(pgn::PGN_GNSS_DOPS, "GNSS DOPs", Implemented),
    e(pgn::PGN_HEADING_TRACK, "Vessel Heading", Implemented),
    e(pgn::PGN_RATE_OF_TURN, "Rate of Turn", Implemented),
    e(pgn::PGN_ATTITUDE, "Attitude", Implemented),
    e(
        pgn::PGN_MAGNETIC_VARIATION,
        "Magnetic Variation",
        Implemented,
    ),
    e(pgn::PGN_XTE, "Cross-Track Error", Implemented),
    e(pgn::PGN_RUDDER, "Rudder", Implemented),
    // ─ Implemented: speed / depth ─
    e(pgn::PGN_SPEED_WATER, "Speed, Water-referenced", Implemented),
    e(pgn::PGN_WATER_DEPTH, "Water Depth", Implemented),
    // ─ Implemented: engine / electrical ─
    e(
        pgn::PGN_ENGINE_PARAMS_RAPID,
        "Engine Parameters, Rapid",
        Implemented,
    ),
    e(pgn::PGN_FLUID_LEVEL, "Fluid Level", Implemented),
    e(pgn::PGN_BATTERY_STATUS, "Battery Status", Implemented),
    // ─ Implemented: environment ─
    e(pgn::PGN_WIND_DATA, "Wind Data", Implemented),
    e(
        pgn::PGN_OUTSIDE_ENVIRONMENTAL,
        "Outside Environmental",
        Implemented,
    ),
    e(pgn::PGN_TEMPERATURE, "Temperature", Implemented),
    e(pgn::PGN_HUMIDITY, "Humidity", Implemented),
    e(pgn::PGN_PRESSURE, "Pressure", Implemented),
    // ─ Not implemented: AIS family (GAP-confirmed out of scope) ─
    e(
        pgn::PGN_AIS_CLASS_A_POSITION,
        "AIS Class A Position",
        NotImplemented,
    ),
    e(
        pgn::PGN_AIS_CLASS_B_POSITION,
        "AIS Class B Position",
        NotImplemented,
    ),
    e(
        pgn::PGN_AIS_ATON_REPORT,
        "AIS Aids-to-Navigation",
        NotImplemented,
    ),
    e(
        pgn::PGN_AIS_CLASS_A_STATIC,
        "AIS Class A Static",
        NotImplemented,
    ),
    // ─ Not implemented: navigation / route ─
    e(pgn::PGN_NAVIGATION_DATA, "Navigation Data", NotImplemented),
    e(
        pgn::PGN_ROUTE_WP_INFO,
        "Route/WP Information",
        NotImplemented,
    ),
    e(pgn::PGN_WAYPOINT_LIST, "Waypoint List", NotImplemented),
    e(
        pgn::PGN_GNSS_SATELLITES_IN_VIEW,
        "Satellites in View",
        NotImplemented,
    ),
    // ─ Not implemented: engine / electrical detail ─
    e(
        pgn::PGN_ENGINE_PARAMS_DYNAMIC,
        "Engine Parameters, Dynamic",
        NotImplemented,
    ),
    e(
        pgn::PGN_TRANSMISSION_PARAMS,
        "Transmission Parameters",
        NotImplemented,
    ),
    e(
        pgn::PGN_ENGINE_TRIP,
        "Engine Trip Parameters",
        NotImplemented,
    ),
    e(pgn::PGN_CHARGER_STATUS, "Charger Status", NotImplemented),
    e(
        pgn::PGN_BATTERY_CONFIG,
        "Battery Configuration",
        NotImplemented,
    ),
    // ─ Not implemented: windlass / misc ─
    e(
        pgn::PGN_WINDLASS_CONTROL,
        "Windlass Control",
        NotImplemented,
    ),
    e(
        pgn::PGN_WINDLASS_MONITORING,
        "Windlass Monitoring",
        NotImplemented,
    ),
    e(pgn::PGN_DISTANCE_LOG, "Distance Log", NotImplemented),
    e(pgn::PGN_MOB, "Man Overboard", NotImplemented),
    // ─ Not implemented: further public PGNs (numbers/names only) ─
    e(
        pgn::PGN_LOCAL_TIME_OFFSET,
        "Local Time Offset",
        NotImplemented,
    ),
    e(pgn::PGN_HEAVE, "Heave", NotImplemented),
    e(
        pgn::PGN_HEADING_TRACK_CONTROL,
        "Heading/Track Control",
        NotImplemented,
    ),
    e(
        pgn::PGN_BINARY_SWITCH_STATUS,
        "Binary Switch Bank Status",
        NotImplemented,
    ),
    e(
        pgn::PGN_BINARY_SWITCH_CONTROL,
        "Switch Bank Control",
        NotImplemented,
    ),
    e(
        pgn::PGN_DC_DETAILED_STATUS,
        "DC Detailed Status",
        NotImplemented,
    ),
    e(
        pgn::PGN_CHARGER_CONFIG,
        "Charger Configuration Status",
        NotImplemented,
    ),
    e(
        pgn::PGN_CONVERTER_STATUS,
        "Converter (Inverter) Status",
        NotImplemented,
    ),
    e(
        pgn::PGN_DC_VOLTAGE_CURRENT,
        "DC Voltage/Current",
        NotImplemented,
    ),
    e(pgn::PGN_LEEWAY, "Leeway Angle", NotImplemented),
    e(
        pgn::PGN_WINDLASS_OPERATING,
        "Anchor Windlass Operating Status",
        NotImplemented,
    ),
    e(
        pgn::PGN_AIS_SAFETY_MSG,
        "AIS Safety-Related Broadcast",
        NotImplemented,
    ),
    e(
        pgn::PGN_AIS_CLASS_B_STATIC_A,
        "AIS Class B Static Data A",
        NotImplemented,
    ),
    e(
        pgn::PGN_AIS_CLASS_B_STATIC_B,
        "AIS Class B Static Data B",
        NotImplemented,
    ),
    e(
        pgn::PGN_ENVIRONMENTAL_PARAMS,
        "Environmental Parameters",
        NotImplemented,
    ),
    e(pgn::PGN_SET_PRESSURE, "Set Pressure", NotImplemented),
    e(
        pgn::PGN_TEMPERATURE_EXT,
        "Temperature, Extended Range",
        NotImplemented,
    ),
    e(
        pgn::PGN_METEOROLOGICAL,
        "Meteorological Station Data",
        NotImplemented,
    ),
    e(pgn::PGN_TRIM_TAB, "Trim Tab Status", NotImplemented),
    e(pgn::PGN_DIRECTION_DATA, "Direction Data", NotImplemented),
];

const fn e(pgn: Pgn, name: &'static str, status: NmeaPgnStatus) -> NmeaPgnEntry {
    NmeaPgnEntry { pgn, name, status }
}

/// Catalog status for a PGN, or `None` if it is not in the ledger.
#[must_use]
pub fn catalog_status(pgn: Pgn) -> Option<NmeaPgnStatus> {
    NMEA_PGN_CATALOG
        .iter()
        .find(|e| e.pgn == pgn)
        .map(|e| e.status)
}

/// `true` if machbus has a runtime decoder for `pgn`.
#[must_use]
pub fn is_implemented(pgn: Pgn) -> bool {
    catalog_status(pgn) == Some(NmeaPgnStatus::Implemented)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nmea::NMEA2000_INTERFACE_PGNS;

    #[test]
    fn no_duplicate_pgns() {
        for (i, a) in NMEA_PGN_CATALOG.iter().enumerate() {
            for b in &NMEA_PGN_CATALOG[i + 1..] {
                assert_ne!(a.pgn, b.pgn, "duplicate PGN {} in catalog", a.pgn);
            }
        }
    }

    #[test]
    fn every_interface_pgn_is_marked_implemented() {
        for pgn in NMEA2000_INTERFACE_PGNS {
            assert_eq!(
                catalog_status(pgn),
                Some(NmeaPgnStatus::Implemented),
                "interface PGN {pgn} must be catalogued as Implemented"
            );
        }
    }

    #[test]
    fn management_pgns_are_implemented() {
        assert!(is_implemented(pgn::PGN_PRODUCT_INFO));
        assert!(is_implemented(pgn::PGN_CONFIG_INFO));
        assert!(is_implemented(pgn::PGN_HEARTBEAT_N2K));
    }

    #[test]
    fn ais_family_is_not_implemented() {
        assert_eq!(
            catalog_status(pgn::PGN_AIS_CLASS_A_POSITION),
            Some(NmeaPgnStatus::NotImplemented)
        );
        assert_eq!(
            catalog_status(pgn::PGN_AIS_CLASS_B_POSITION),
            Some(NmeaPgnStatus::NotImplemented)
        );
        assert!(!is_implemented(pgn::PGN_AIS_ATON_REPORT));
    }

    #[test]
    fn unknown_pgn_is_absent_from_catalog() {
        assert_eq!(catalog_status(0x00), None);
    }
}
