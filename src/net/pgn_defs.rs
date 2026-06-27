//! Every PGN constant used in the stack — ISOBUS (ISO 11783),
//! J1939, and NMEA2000.
//!
//! Mirrors the C++ `machbus::net::pgn_defs`. Values **must remain
//! bit-identical** to the C++ source for cross-stack interoperability.

use super::types::Pgn;

// ─── Core Protocol (ISO 11783-3/5, J1939-21) ────────────────────────────
pub const PGN_REQUEST: Pgn = 0xEA00;
pub const PGN_ADDRESS_CLAIMED: Pgn = 0xEE00;
pub const PGN_COMMANDED_ADDRESS: Pgn = 0xFED8;
pub const PGN_TP_CM: Pgn = 0xEC00;
pub const PGN_TP_DT: Pgn = 0xEB00;
pub const PGN_ETP_CM: Pgn = 0xC800;
pub const PGN_ETP_DT: Pgn = 0xC700;
pub const PGN_ACKNOWLEDGMENT: Pgn = 0xE800;
pub const PGN_REQUEST2: Pgn = 0xC900;
pub const PGN_TRANSFER: Pgn = 0xCA00;

// ─── Proprietary Messages (ISO 11783-3, Section 5.4.6) ──────────────────
/// Destination-specific, up to 1785 bytes.
pub const PGN_PROPRIETARY_A: Pgn = 0xEF00;
/// Extended data page Proprietary A.
pub const PGN_PROPRIETARY_A2: Pgn = 0x1EF00;
/// Broadcast range: `0xFF00..=0xFFFF`.
pub const PGN_PROPRIETARY_B_BASE: Pgn = 0xFF00;

// ─── Diagnostics (J1939-73) ──────────────────────────────────────────────
pub const PGN_DM1: Pgn = 0xFECA;
pub const PGN_DM2: Pgn = 0xFECB;
pub const PGN_DM3: Pgn = 0xFECC;
/// Driver's Information Message.
pub const PGN_DM4: Pgn = 0xFECD;
/// Pending DTCs.
pub const PGN_DM6: Pgn = 0xFECF;
/// Command Non-Continuously Monitored Test.
pub const PGN_DM7: Pgn = 0xE300;
/// Test Results.
pub const PGN_DM8: Pgn = 0xFED5;
/// Product/Software ID.
pub const PGN_DM10: Pgn = 0xFDA4;
pub const PGN_DM11: Pgn = 0xFED3;
/// Emissions-Related Active DTCs.
pub const PGN_DM12: Pgn = 0xFED1;
pub const PGN_DM13: Pgn = 0xDF00;
/// Memory access request.
pub const PGN_DM14: Pgn = 0xD900;
/// Memory access response.
pub const PGN_DM15: Pgn = 0xD800;
/// Binary data transfer.
pub const PGN_DM16: Pgn = 0xD700;
/// Diagnostic Readiness 2.
pub const PGN_DM21: Pgn = 0xC200;
pub const PGN_DM22: Pgn = 0xC300;
/// Previously MIL-OFF DTCs.
pub const PGN_DM23: Pgn = 0xFDB2;
/// DM25 Expanded Freeze Frame (SAE J1939-73; PGN 64951).
pub const PGN_DM25: Pgn = 0xFDB7;
pub const PGN_DIAGNOSTIC_PROTOCOL_ID: Pgn = 0xFD32;
pub const PGN_ECU_IDENTIFICATION: Pgn = 0xFDC5;

// ─── Network Management (ISO 11783-5) ────────────────────────────────────
pub const PGN_HEARTBEAT: Pgn = 0xF0E4;
/// PGN 37632 (ISO 11783-5, Sec 4.4.3).
pub const PGN_NAME_MANAGEMENT: Pgn = 0x9300;
pub const PGN_SOFTWARE_ID: Pgn = 0xFEDA;
pub const PGN_PRODUCT_IDENTIFICATION: Pgn = 0xFC8D;
pub const PGN_CF_FUNCTIONALITIES: Pgn = 0xFC8E;
pub const PGN_WORKING_SET_MASTER: Pgn = 0xFE0D;
pub const PGN_LANGUAGE_COMMAND: Pgn = 0xFE0F;
pub const PGN_MAINTAIN_POWER: Pgn = 0xFE47;

// ─── Virtual Terminal (ISO 11783-6) ──────────────────────────────────────
pub const PGN_VT_TO_ECU: Pgn = 0xE600;
pub const PGN_ECU_TO_VT: Pgn = 0xE700;
pub const PGN_SHORTCUT_BUTTON: Pgn = 0xFD02;

// ─── Task Controller (ISO 11783-10) ──────────────────────────────────────
pub const PGN_TC_TO_ECU: Pgn = 0xCB00;
pub const PGN_ECU_TO_TC: Pgn = 0xCC00;

// ─── Vehicle / Machine Speed (J1939 / ISO 11783-7) ───────────────────────
pub const PGN_TIME_DATE: Pgn = 0xFEE6;
pub const PGN_VEHICLE_SPEED: Pgn = 0xFEF1;
pub const PGN_WHEEL_SPEED: Pgn = 0xFE48;
pub const PGN_GROUND_SPEED: Pgn = 0xFE49;
pub const PGN_MACHINE_SPEED: Pgn = 0xF022;

// ─── Guidance (ISO 11783-7) ──────────────────────────────────────────────
pub const PGN_GUIDANCE_MACHINE: Pgn = 0xFE44;
pub const PGN_GUIDANCE_SYSTEM: Pgn = 0xFE45;

// ─── Auxiliary Functions (ISO 11783-11) ──────────────────────────────────
pub const PGN_AUX_ASSIGNMENT: Pgn = 0xFD20;
pub const PGN_AUX_INPUT_STATUS: Pgn = 0xFD21;
pub const PGN_AUX_INPUT_TYPE2: Pgn = 0xFD22;

// ─── File Transfer (ISO 11783-13) ────────────────────────────────────────
/// PGN 43776 (Server → Client).
pub const PGN_FILE_SERVER_TO_CLIENT: Pgn = 0xAB00;
/// PGN 43520 (Client → Server).
pub const PGN_FILE_CLIENT_TO_SERVER: Pgn = 0xAA00;

// ─── Network Interconnection Unit (ISO 11783-4) ──────────────────────────
/// PGN 60672 (NIU message, Sec 6.5).
pub const PGN_NIU_NETWORK_MSG: Pgn = 0xED00;

// ─── Sequence Control (ISO 11783-14) ─────────────────────────────────────
/// PGN 36352 (SCM → SCC).
pub const PGN_SC_MASTER_STATUS: Pgn = 0x8E00;
/// PGN 36096 (SCC → SCM).
pub const PGN_SC_CLIENT_STATUS: Pgn = 0x8D00;

// ─── Tractor Implement Management (ISO 11783-7/9) ────────────────────────
pub const PGN_FRONT_PTO: Pgn = 0xFE54;
pub const PGN_REAR_PTO: Pgn = 0xF003;
pub const PGN_FRONT_HITCH: Pgn = 0xFE08;
pub const PGN_REAR_HITCH: Pgn = 0xF005;
pub const PGN_AUX_VALVE_0_7: Pgn = 0xFE20;
pub const PGN_AUX_VALVE_8_15: Pgn = 0xFE21;
pub const PGN_AUX_VALVE_16_23: Pgn = 0xFE22;
pub const PGN_AUX_VALVE_24_31: Pgn = 0xFE23;

// ─── Tractor Commands (ISO 11783-7 Section 11) ───────────────────────────
pub const PGN_REAR_HITCH_CMD: Pgn = 0xFE50;
pub const PGN_FRONT_HITCH_CMD: Pgn = 0xFE51;
pub const PGN_REAR_PTO_CMD: Pgn = 0xFE52;
pub const PGN_FRONT_PTO_CMD: Pgn = 0xFE53;
pub const PGN_AUX_VALVE_CMD: Pgn = 0xFE30;
pub const PGN_TRACTOR_CONTROL_MODE: Pgn = 0xFE0B;
pub const PGN_MACHINE_SELECTED_SPEED_CMD: Pgn = 0xFD43;

// ─── Tractor Facilities (ISO 11783-9) ────────────────────────────────────
pub const PGN_TRACTOR_FACILITIES_RESPONSE: Pgn = 0xFE09;
pub const PGN_REQUIRED_TRACTOR_FACILITIES: Pgn = 0xFE0A;

// ─── Auxiliary Valve Flow (ISO 11783-7 Class 2 TECU) ─────────────────────
pub const PGN_AUX_VALVE_ESTIMATED_FLOW_BASE: Pgn = 0xFE10;
pub const PGN_AUX_VALVE_MEASURED_FLOW_BASE: Pgn = 0xFE20;

// ─── Lighting (ISO 11783-7 Section 4.5) ──────────────────────────────────
pub const PGN_LIGHTING_DATA: Pgn = 0xFE40;
pub const PGN_LIGHTING_COMMAND: Pgn = 0xFE41;

// ─── Speed / Distance / Direction aliases ──────────────────────────────
/// Alias: Wheel-Based Speed & Distance.
pub const PGN_WHEEL_BASED_SPEED_DIST: Pgn = PGN_WHEEL_SPEED;
/// Alias: Ground-Based Speed & Distance.
pub const PGN_GROUND_BASED_SPEED_DIST: Pgn = PGN_GROUND_SPEED;
/// Alias: Machine Selected Speed.
pub const PGN_MACHINE_SELECTED_SPEED: Pgn = PGN_MACHINE_SPEED;

// ─── Guidance Extended (ISO 11783-7 G Addendum) ────────────────────────
pub const PGN_GUIDANCE_MACHINE_INFO: Pgn = 0xAC00;
pub const PGN_GUIDANCE_CURVATURE_CMD: Pgn = 0xFE46;
/// Class xG external steering.
pub const PGN_GUIDANCE_SYSTEM_CMD: Pgn = 0xAD00;

// ─── Drive Strategy (ISO 11783-7 Section 11) ─────────────────────────────
pub const PGN_DRIVE_STRATEGY_CMD: Pgn = 0xFCCE;

// ─── Hitch Roll/Pitch (ISO 11783-7 Section 11) ──────────────────────────
pub const PGN_FRONT_HITCH_ROLL_PITCH_CMD: Pgn = 0xF100;
pub const PGN_REAR_HITCH_ROLL_PITCH_CMD: Pgn = 0xF102;
pub const PGN_HITCH_PTO_COMBINED_CMD: Pgn = 0xFE42;

// ─── Working Set (ISO 11783-7 Section 10) ────────────────────────────────
pub const PGN_WORKING_SET_MEMBER: Pgn = 0xFE0C;

// ─── J1939 Engine / Powertrain ───────────────────────────────────────────
/// Electronic Engine Controller 1.
pub const PGN_EEC1: Pgn = 0x0F004;
/// Electronic Engine Controller 2.
pub const PGN_EEC2: Pgn = 0x0F003;
/// Electronic Engine Controller 3.
pub const PGN_EEC3: Pgn = 0x0FEC0;
/// Engine Temperature 1.
pub const PGN_ET1: Pgn = 0x0FEEE;
/// Engine Temperature 2.
pub const PGN_ET2: Pgn = 0x0FEED;
/// Engine Fluid Level/Pressure.
pub const PGN_EFLP: Pgn = 0x0FEEF;
pub const PGN_ENGINE_HOURS: Pgn = 0x0FEE5;
pub const PGN_FUEL_ECONOMY: Pgn = 0x0FEF2;
pub const PGN_FUEL_CONSUMPTION: Pgn = 0x0FEE9;
/// Electronic Transmission Controller 1 (J1939 PGN 61442).
pub const PGN_ETC1: Pgn = 0x0F002;
/// Electronic Transmission Controller 2. NOTE: the canonical J1939 value is
/// 0xF005, currently aliased by the (also-incorrect) `PGN_REAR_HITCH`; this is
/// left at 0xF006 (unused, non-colliding) until the ISO 11783-7 hitch/PTO PGNs
/// are corrected from a verified source.
pub const PGN_ETC2: Pgn = 0x0F006;
/// Alias for backwards compatibility.
pub const PGN_TRANSMISSION_1: Pgn = PGN_ETC1;
/// Cruise Control / Vehicle Speed.
pub const PGN_CRUISE_CONTROL: Pgn = 0x0FEF1;
/// Torque/Speed Control 1 (J1939 PGN 0, destination-specific).
pub const PGN_TSC1: Pgn = 0x00000;
/// Vehicle Electrical Power 1.
pub const PGN_VEP1: Pgn = 0x0F009;
/// Aftertreatment 1 (intake-gas region; distinct from Ambient Conditions).
pub const PGN_AT1: Pgn = 0x0F00E;
/// Aftertreatment 2.
pub const PGN_AT2: Pgn = 0x0FE46;
/// Ambient Conditions (J1939 PGN 65269).
pub const PGN_AMBIENT_CONDITIONS: Pgn = 0x0FEF5;
pub const PGN_DASH_DISPLAY: Pgn = 0x0FEFC;
/// Vehicle Position (J1939).
pub const PGN_VEHICLE_POSITION: Pgn = 0x0FEF7;
/// Component Identification.
pub const PGN_COMPONENT_ID: Pgn = 0x0FEEB;
/// Vehicle Identification.
pub const PGN_VEHICLE_ID: Pgn = 0x0FEEC;

// ═════════════════════════════════════════════════════════════════════════
// NMEA2000 PGNs
// ═════════════════════════════════════════════════════════════════════════

// ─── NMEA2000 System / Network Management ────────────────────────────────
pub const PGN_ISO_ADDRESS_CLAIM: Pgn = 60928;
pub const PGN_SYSTEM_TIME: Pgn = 126992;
pub const PGN_PRODUCT_INFO: Pgn = 126996;
pub const PGN_CONFIG_INFO: Pgn = 126998;
pub const PGN_HEARTBEAT_N2K: Pgn = 126993;

// ─── NMEA2000 Navigation / GNSS ──────────────────────────────────────────
/// Position, Rapid Update (8B).
pub const PGN_GNSS_POSITION_RAPID: Pgn = 129025;
/// COG & SOG, Rapid Update (8B).
pub const PGN_GNSS_COG_SOG_RAPID: Pgn = 129026;
/// Position Delta, High Precision.
pub const PGN_GNSS_POSITION_DELTA: Pgn = 129027;
/// GNSS Position Data (FP, 43+B).
pub const PGN_GNSS_POSITION_DATA: Pgn = 129029;
/// GNSS DOPs (8B).
pub const PGN_GNSS_DOPS: Pgn = 129539;
pub const PGN_GNSS_SATELLITES_IN_VIEW: Pgn = 129540;
pub const PGN_LOCAL_TIME_OFFSET: Pgn = 129033;
/// Vessel Heading (8B).
pub const PGN_HEADING_TRACK: Pgn = 127250;
/// Rate of Turn (8B).
pub const PGN_RATE_OF_TURN: Pgn = 127251;
/// Heave (8B).
pub const PGN_HEAVE: Pgn = 127252;
/// Attitude (Yaw/Pitch/Roll, 8B).
pub const PGN_ATTITUDE: Pgn = 127257;
/// Magnetic Variation (8B).
pub const PGN_MAGNETIC_VARIATION: Pgn = 127258;

/// Backward-compatible alias.
pub const PGN_GNSS_POSITION: Pgn = PGN_GNSS_POSITION_RAPID;
/// Backward-compatible alias.
pub const PGN_GNSS_COG_SOG: Pgn = PGN_GNSS_COG_SOG_RAPID;
/// Backward-compatible alias.
pub const PGN_GNSS_POSITION_DETAIL: Pgn = PGN_GNSS_POSITION_DATA;

// ─── NMEA2000 Steering / Rudder ──────────────────────────────────────────
pub const PGN_MOB: Pgn = 127233;
pub const PGN_HEADING_TRACK_CONTROL: Pgn = 127237;
pub const PGN_RUDDER: Pgn = 127245;

// ─── NMEA2000 Engine / Propulsion ────────────────────────────────────────
pub const PGN_ENGINE_PARAMS_RAPID: Pgn = 127488;
pub const PGN_ENGINE_PARAMS_DYNAMIC: Pgn = 127489;
pub const PGN_TRANSMISSION_PARAMS: Pgn = 127493;
pub const PGN_ENGINE_TRIP: Pgn = 127497;

// ─── NMEA2000 Electrical / Power ─────────────────────────────────────────
pub const PGN_BINARY_SWITCH_STATUS: Pgn = 127501;
pub const PGN_BINARY_SWITCH_CONTROL: Pgn = 127502;
pub const PGN_FLUID_LEVEL: Pgn = 127505;
pub const PGN_DC_DETAILED_STATUS: Pgn = 127506;
pub const PGN_CHARGER_STATUS: Pgn = 127507;
pub const PGN_BATTERY_STATUS: Pgn = 127508;
pub const PGN_CHARGER_CONFIG: Pgn = 127510;
pub const PGN_BATTERY_CONFIG: Pgn = 127513;
pub const PGN_CONVERTER_STATUS: Pgn = 127750;
pub const PGN_DC_VOLTAGE_CURRENT: Pgn = 127751;

// ─── NMEA2000 Speed / Distance / Depth ───────────────────────────────────
pub const PGN_LEEWAY: Pgn = 128000;
pub const PGN_SPEED_WATER: Pgn = 128259;
pub const PGN_WATER_DEPTH: Pgn = 128267;
pub const PGN_DISTANCE_LOG: Pgn = 128275;

// ─── NMEA2000 Windlass ───────────────────────────────────────────────────
pub const PGN_WINDLASS_CONTROL: Pgn = 128776;
pub const PGN_WINDLASS_OPERATING: Pgn = 128777;
pub const PGN_WINDLASS_MONITORING: Pgn = 128778;

// ─── NMEA2000 Navigation Control ─────────────────────────────────────────
pub const PGN_XTE: Pgn = 129283;
pub const PGN_NAVIGATION_DATA: Pgn = 129284;
pub const PGN_ROUTE_WP_INFO: Pgn = 129285;

// ─── NMEA2000 AIS ────────────────────────────────────────────────────────
pub const PGN_AIS_CLASS_A_POSITION: Pgn = 129038;
pub const PGN_AIS_CLASS_B_POSITION: Pgn = 129039;
pub const PGN_AIS_ATON_REPORT: Pgn = 129041;
pub const PGN_AIS_CLASS_A_STATIC: Pgn = 129794;
pub const PGN_AIS_SAFETY_MSG: Pgn = 129802;
pub const PGN_AIS_CLASS_B_STATIC_A: Pgn = 129809;
pub const PGN_AIS_CLASS_B_STATIC_B: Pgn = 129810;

// ─── NMEA2000 Waypoints ──────────────────────────────────────────────────
pub const PGN_WAYPOINT_LIST: Pgn = 130074;

// ─── NMEA2000 Environment ────────────────────────────────────────────────
pub const PGN_WIND_DATA: Pgn = 130306;
pub const PGN_OUTSIDE_ENVIRONMENTAL: Pgn = 130310;
pub const PGN_ENVIRONMENTAL_PARAMS: Pgn = 130311;
pub const PGN_TEMPERATURE: Pgn = 130312;
pub const PGN_HUMIDITY: Pgn = 130313;
pub const PGN_PRESSURE: Pgn = 130314;
pub const PGN_SET_PRESSURE: Pgn = 130315;
pub const PGN_TEMPERATURE_EXT: Pgn = 130316;
pub const PGN_METEOROLOGICAL: Pgn = 130323;

// ─── NMEA2000 Trim / Direction ───────────────────────────────────────────
pub const PGN_TRIM_TAB: Pgn = 130576;
pub const PGN_DIRECTION_DATA: Pgn = 130577;
