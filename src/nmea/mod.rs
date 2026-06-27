//! NMEA 2000 + NMEA 0183 protocol layer.
//!
//! Mirrors the C++ `machbus::nmea::*` namespace. Pure-data
//! definitions plus a pump-style interface that decodes the most
//! common N2K PGNs and produces send-payload helpers. Network
//! management (PGN 126993/126996/126998) and an NMEA0183 serial
//! parser are also included.

pub mod catalog;
pub mod definitions;
pub mod interface;
pub mod n2k_management;
pub mod position;
pub mod serial_gnss;

use crate::net::pgn_defs::{
    PGN_ATTITUDE, PGN_BATTERY_STATUS, PGN_CONFIG_INFO, PGN_ENGINE_PARAMS_RAPID, PGN_FLUID_LEVEL,
    PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_DOPS, PGN_GNSS_POSITION_DATA, PGN_GNSS_POSITION_DELTA,
    PGN_GNSS_POSITION_RAPID, PGN_HEADING_TRACK, PGN_HEARTBEAT_N2K, PGN_HUMIDITY,
    PGN_MAGNETIC_VARIATION, PGN_OUTSIDE_ENVIRONMENTAL, PGN_PRESSURE, PGN_PRODUCT_INFO,
    PGN_RATE_OF_TURN, PGN_RUDDER, PGN_SPEED_WATER, PGN_SYSTEM_TIME, PGN_TEMPERATURE,
    PGN_WATER_DEPTH, PGN_WIND_DATA, PGN_XTE,
};
use crate::net::types::Pgn;

/// Explicit selected NMEA 2000 PGN inventory implemented by this crate.
///
/// The inventory combines the high-level [`NMEAInterface`] dispatcher and the
/// [`N2KManagement`] helper. It is deliberately smaller than the full NMEA
/// 2000 PGN catalog.
pub const NMEA2000_SELECTED_PGNS: [Pgn; 25] = [
    PGN_SYSTEM_TIME,
    PGN_HEARTBEAT_N2K,
    PGN_PRODUCT_INFO,
    PGN_CONFIG_INFO,
    PGN_RUDDER,
    PGN_HEADING_TRACK,
    PGN_RATE_OF_TURN,
    PGN_ATTITUDE,
    PGN_MAGNETIC_VARIATION,
    PGN_ENGINE_PARAMS_RAPID,
    PGN_FLUID_LEVEL,
    PGN_BATTERY_STATUS,
    PGN_SPEED_WATER,
    PGN_WATER_DEPTH,
    PGN_GNSS_POSITION_RAPID,
    PGN_GNSS_COG_SOG_RAPID,
    PGN_GNSS_POSITION_DELTA,
    PGN_GNSS_POSITION_DATA,
    PGN_XTE,
    PGN_GNSS_DOPS,
    PGN_WIND_DATA,
    PGN_OUTSIDE_ENVIRONMENTAL,
    PGN_TEMPERATURE,
    PGN_HUMIDITY,
    PGN_PRESSURE,
];

pub use catalog::{NMEA_PGN_CATALOG, NmeaPgnEntry, NmeaPgnStatus, catalog_status};
pub use definitions::*;
pub use interface::{NMEA2000_INTERFACE_PGNS, NMEAConfig, NMEAInterface};
pub use n2k_management::{
    N2K_REQUEST_TIMEOUT_MS, N2KConfigInfo, N2KHeartbeat, N2KManagement, N2KManagementConfig,
    N2KOutbound, N2KProductInfo, NMEA2000_MANAGEMENT_PGNS, PendingRequest,
};
pub use position::{GNSSBatch, GNSSPosition};
pub use serial_gnss::{NmeaUtcDateTime, SerialGNSS, SerialGNSSConfig};
