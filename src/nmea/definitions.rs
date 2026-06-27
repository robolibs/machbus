//! NMEA 2000 protocol definitions: enums, resolution constants, and
//! typed message data structures.
//!
//! Mirrors the C++ `machbus::nmea::definitions.hpp`. Pure data, no
//! IsoNet coupling.

#![allow(missing_docs)]
#![allow(non_camel_case_types)]

// Content-named child files keep this module under the project 2000-LOC ceiling.
// They are included into this same module so visibility and behavior stay unchanged.
include!("definitions/nmea_core_definitions.rs");
include!("definitions/nmea_navigation_ais_definitions.rs");
