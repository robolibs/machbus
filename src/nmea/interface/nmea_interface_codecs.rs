use super::definitions::{
    BatteryStatusData, COG_RESOLUTION, CURRENT_RESOLUTION, DEPTH_RESOLUTION, DOP_RESOLUTION,
    DistanceCalculationType, EngineData, FLUID_CAPACITY_RESOLUTION, FLUID_LEVEL_RESOLUTION,
    FluidLevelData, FluidType, GNSSDOPData, GNSSDOPMode, GNSSFixType, GNSSSystem,
    GnssSatsInViewData, HEADING_RESOLUTION, HUMIDITY_RESOLUTION, HeadingReference, HumidityData,
    HumiditySource, LAT_LON_RESOLUTION, LocalTimeOffsetData, MagneticVariationSource,
    NavigationData, OutsideEnvironmentalData, POSITION_DELTA_RESOLUTION,
    POSITION_DELTA_TIME_RESOLUTION, PRESSURE_RESOLUTION, PositionDeltaHighPrecisionRapidUpdateData,
    PressureData, PressureSource, ROT_RESOLUTION, RPM_RESOLUTION, ReferenceStationType, RudderData,
    RudderDirection, SPEED_RESOLUTION, SatelliteInfo, SpeedWaterData, SpeedWaterRefType,
    SystemTimeData, TEMPERATURE_RESOLUTION, TemperatureData, TemperatureSource, TimeSource,
    VOLTAGE_RESOLUTION, WIND_DIR_RESOLUTION, WIND_SPEED_RESOLUTION, WaterDepthData, WindData,
    WindReference, XTE_RESOLUTION, XTEData, XTEMode,
};
use super::position::GNSSPosition;
use crate::geo::Wgs;
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{
    PGN_ATTITUDE, PGN_BATTERY_STATUS, PGN_ENGINE_PARAMS_RAPID, PGN_FLUID_LEVEL,
    PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_DOPS, PGN_GNSS_POSITION_DATA, PGN_GNSS_POSITION_DELTA,
    PGN_GNSS_POSITION_RAPID, PGN_GNSS_SATELLITES_IN_VIEW, PGN_HEADING_TRACK, PGN_HUMIDITY,
    PGN_LOCAL_TIME_OFFSET, PGN_MAGNETIC_VARIATION, PGN_NAVIGATION_DATA, PGN_OUTSIDE_ENVIRONMENTAL,
    PGN_PRESSURE, PGN_RATE_OF_TURN, PGN_RUDDER, PGN_SPEED_WATER, PGN_SYSTEM_TIME, PGN_TEMPERATURE,
    PGN_WATER_DEPTH, PGN_WIND_DATA, PGN_XTE,
};
use crate::net::types::Pgn;
use alloc::vec::Vec;
use core::f64::consts::TAU;

/// NMEA 2000 PGNs that [`NMEAInterface`] currently dispatches.
///
/// This is the explicit selected-subset inventory for the high-level
/// navigation, environment, power, propulsion, and steering facade. It is not a
/// claim that every NMEA 2000 PGN is implemented.
pub const NMEA2000_INTERFACE_PGNS: [Pgn; 22] = [
    PGN_GNSS_POSITION_RAPID,
    PGN_GNSS_COG_SOG_RAPID,
    PGN_GNSS_POSITION_DELTA,
    PGN_ATTITUDE,
    PGN_RATE_OF_TURN,
    PGN_GNSS_POSITION_DATA,
    PGN_GNSS_DOPS,
    PGN_MAGNETIC_VARIATION,
    PGN_WIND_DATA,
    PGN_TEMPERATURE,
    PGN_HUMIDITY,
    PGN_PRESSURE,
    PGN_OUTSIDE_ENVIRONMENTAL,
    PGN_ENGINE_PARAMS_RAPID,
    PGN_FLUID_LEVEL,
    PGN_BATTERY_STATUS,
    PGN_WATER_DEPTH,
    PGN_SPEED_WATER,
    PGN_XTE,
    PGN_RUDDER,
    PGN_SYSTEM_TIME,
    PGN_HEADING_TRACK,
];

const ACTUAL_PRESSURE_RESOLUTION: f64 = 0.1;
const DEPTH_OFFSET_RESOLUTION: f64 = 0.001;
const DEPTH_RANGE_RESOLUTION: f64 = 10.0;
#[cfg(feature = "default")]
const NEAR_INTEGER_EPSILON: f64 = 1.0e-9;
const MAX_LATITUDE_I32_RAW: i32 = 900_000_000;
const MAX_LONGITUDE_I32_RAW: i32 = 1_800_000_000;
const MAX_LATITUDE_I64_RAW: i64 = 900_000_000_000_000_000;
const MAX_LONGITUDE_I64_RAW: i64 = 1_800_000_000_000_000_000;
const NMEA_CIRCULAR_ANGLE_MAX_RAW: u16 = 62_831;
const MAX_TIME_OF_DAY_RAW: u32 = 864_010_000;
const ENGINE_TILT_TRIM_MIN_RAW: i8 = -100;
const ENGINE_TILT_TRIM_MAX_RAW: i8 = 100;
const ENGINE_TILT_TRIM_RESERVED_RAW: u8 = 0x7D;
const ENGINE_TILT_TRIM_ERROR_RAW: u8 = 0x7E;
const ENGINE_TILT_TRIM_NOT_AVAILABLE_RAW: u8 = 0x7F;

fn scaled_trunc_near_integer(value: f64, resolution: f64) -> f64 {
    let scaled = value / resolution;
    #[cfg(feature = "embedded")]
    {
        scaled
    }
    #[cfg(feature = "default")]
    {
        let rounded = scaled.round();
        if (scaled - rounded).abs() <= NEAR_INTEGER_EPSILON {
            rounded
        } else {
            scaled.trunc()
        }
    }
}

fn scaled_u16(value: f64, resolution: f64) -> u16 {
    if !value.is_finite() {
        return u16::MAX;
    }
    scaled_trunc_near_integer(value, resolution).clamp(0.0, (u16::MAX - 3) as f64) as u16
}

fn scaled_u8_with_reserved_band(value: f64, resolution: f64) -> u8 {
    if !value.is_finite() {
        return u8::MAX;
    }
    scaled_trunc_near_integer(value, resolution).clamp(0.0, (u8::MAX - 3) as f64) as u8
}

fn scaled_circular_angle_u16(value: f64, resolution: f64) -> u16 {
    if !value.is_finite() {
        return u16::MAX;
    }
    let max_raw = f64::from(NMEA_CIRCULAR_ANGLE_MAX_RAW).min(TAU / resolution);
    scaled_trunc_near_integer(value, resolution).clamp(0.0, max_raw) as u16
}

fn scaled_u32(value: f64, resolution: f64) -> u32 {
    if !value.is_finite() {
        return u32::MAX;
    }
    scaled_trunc_near_integer(value, resolution).clamp(0.0, (u32::MAX - 3) as f64) as u32
}

fn scaled_i16(value: f64, resolution: f64) -> i16 {
    if !value.is_finite() {
        return i16::MAX;
    }
    scaled_trunc_near_integer(value, resolution).clamp(i16::MIN as f64, (i16::MAX - 3) as f64)
        as i16
}

fn scaled_i32(value: f64, resolution: f64) -> i32 {
    if !value.is_finite() {
        return i32::MAX;
    }
    scaled_trunc_near_integer(value, resolution).clamp(i32::MIN as f64, (i32::MAX - 3) as f64)
        as i32
}

fn scaled_latitude_i32(value: f64) -> i32 {
    if !value.is_finite() {
        return i32::MAX;
    }
    scaled_trunc_near_integer(value, LAT_LON_RESOLUTION)
        .clamp(-MAX_LATITUDE_I32_RAW as f64, MAX_LATITUDE_I32_RAW as f64) as i32
}

fn scaled_longitude_i32(value: f64) -> i32 {
    if !value.is_finite() {
        return i32::MAX;
    }
    scaled_trunc_near_integer(value, LAT_LON_RESOLUTION)
        .clamp(-MAX_LONGITUDE_I32_RAW as f64, MAX_LONGITUDE_I32_RAW as f64) as i32
}

fn scaled_i24(value: f64, resolution: f64) -> i32 {
    if !value.is_finite() {
        return 0x7F_FFFF;
    }
    scaled_trunc_near_integer(value, resolution).clamp(-0x80_0000 as f64, 0x7F_FFFC as f64) as i32
}

fn signed_i16_data_is_available(raw: i16) -> bool {
    raw != i16::MAX && raw != i16::MAX - 1
}

fn signed_i16_data_is_reserved(raw: i16) -> bool {
    raw == i16::MAX - 2
}

fn signed_i32_data_is_available(raw: i32) -> bool {
    raw != i32::MAX && raw != i32::MAX - 1
}

fn signed_i32_data_is_reserved(raw: i32) -> bool {
    raw == i32::MAX - 2
}

fn signed_i64_data_is_available(raw: i64) -> bool {
    raw != i64::MAX && raw != i64::MAX - 1
}

fn signed_i64_data_is_reserved(raw: i64) -> bool {
    raw == i64::MAX - 2
}

fn signed_i24_data_is_available(raw: i32) -> bool {
    raw != 0x7F_FFFF && raw != 0x7F_FFFE
}

fn signed_i24_data_is_reserved(raw: i32) -> bool {
    raw == 0x7F_FFFD
}

fn signed_i8_engine_tilt_trim_is_special(raw: u8) -> bool {
    matches!(
        raw,
        ENGINE_TILT_TRIM_RESERVED_RAW
            | ENGINE_TILT_TRIM_ERROR_RAW
            | ENGINE_TILT_TRIM_NOT_AVAILABLE_RAW
    )
}

fn circular_angle_raw_is_canonical(raw: u16) -> bool {
    raw <= NMEA_CIRCULAR_ANGLE_MAX_RAW || raw == 0xFFFF
}

fn signed_i8_engine_tilt_trim_is_supported(raw: u8) -> bool {
    if signed_i8_engine_tilt_trim_is_special(raw) {
        return false;
    }
    let value = raw as i8;
    (ENGINE_TILT_TRIM_MIN_RAW..=ENGINE_TILT_TRIM_MAX_RAW).contains(&value)
}

fn engine_tilt_trim_to_wire(value: i8) -> u8 {
    value
        .clamp(ENGINE_TILT_TRIM_MIN_RAW, ENGINE_TILT_TRIM_MAX_RAW)
        .to_le_bytes()[0]
}

fn nmea_sequence_id_to_wire(value: u8) -> u8 {
    if nmea_sequence_id_is_canonical(value) {
        value
    } else {
        0xFF
    }
}

fn nmea_u16_with_reserved_band_to_wire(value: u16) -> u16 {
    if matches!(value, 0xFFFD | 0xFFFE) {
        0xFFFF
    } else {
        value
    }
}

fn time_of_day_seconds_to_wire(value: f64) -> u32 {
    if !value.is_finite() {
        return 0xFFFF_FFFF;
    }
    scaled_trunc_near_integer(value, 0.0001).clamp(0.0, MAX_TIME_OF_DAY_RAW as f64) as u32
}

fn pressure_source_to_wire(source: PressureSource) -> u8 {
    match source {
        PressureSource::Reserved => PressureSource::Unavailable.as_u8(),
        _ => source.as_u8(),
    }
}

fn i24_from_le_bytes(bytes: [u8; 3]) -> i32 {
    let raw = (bytes[0] as i32) | ((bytes[1] as i32) << 8) | ((bytes[2] as i32) << 16);
    if raw & 0x80_0000 != 0 {
        raw | !0xFF_FFFF
    } else {
        raw
    }
}

fn write_i24_le(out: &mut [u8], value: i32) {
    let raw = (value as u32) & 0x00FF_FFFF;
    out[0] = (raw & 0xFF) as u8;
    out[1] = ((raw >> 8) & 0xFF) as u8;
    out[2] = ((raw >> 16) & 0xFF) as u8;
}

fn classic_can_payload(msg: &Message) -> Option<&[u8; 8]> {
    msg.data.as_slice().try_into().ok()
}

fn classic_can_payload_with_ff_tail(msg: &Message, tail_start: usize) -> Option<&[u8; 8]> {
    let data = classic_can_payload(msg)?;
    if data[tail_start..].iter().any(|b| *b != 0xFF) {
        return None;
    }
    Some(data)
}

// ─── Config ───────────────────────────────────────────────────────────

/// Listen toggles. Mirrors the C++ `NMEAConfig`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NMEAConfig {
    pub listen_rapid_position: bool,
    pub listen_cog_sog: bool,
    pub listen_position_delta: bool,
    pub listen_attitude: bool,
    pub listen_rate_of_turn: bool,
    pub listen_position_detail: bool,
    pub listen_gnss_dops: bool,
    pub listen_magnetic_variation: bool,
    pub listen_wind: bool,
    pub listen_temperature: bool,
    pub listen_humidity: bool,
    pub listen_pressure: bool,
    pub listen_outside_environmental: bool,
    pub listen_engine: bool,
    pub listen_fluid_level: bool,
    pub listen_battery: bool,
    pub listen_depth: bool,
    pub listen_speed_water: bool,
    pub listen_xte: bool,
    pub listen_rudder: bool,
    pub listen_system_time: bool,
    pub listen_heading: bool,
    pub listen_local_time_offset: bool,
    pub listen_gnss_sats_in_view: bool,
    pub listen_navigation_data: bool,
}

impl Default for NMEAConfig {
    fn default() -> Self {
        Self {
            listen_rapid_position: true,
            listen_cog_sog: true,
            listen_position_delta: true,
            listen_attitude: true,
            listen_rate_of_turn: true,
            listen_position_detail: true,
            listen_gnss_dops: false,
            listen_magnetic_variation: false,
            listen_wind: false,
            listen_temperature: false,
            listen_humidity: false,
            listen_pressure: false,
            listen_outside_environmental: false,
            listen_engine: false,
            listen_fluid_level: false,
            listen_battery: false,
            listen_depth: false,
            listen_speed_water: false,
            listen_xte: false,
            listen_rudder: false,
            listen_system_time: false,
            listen_heading: false,
            listen_local_time_offset: false,
            listen_gnss_sats_in_view: false,
            listen_navigation_data: false,
        }
    }
}

impl NMEAConfig {
    /// Toggle the GNSS/navigation receive profile without enabling
    /// unrelated NMEA 2000 environmental, engine, battery, rudder, or
    /// depth groups.
    ///
    /// This profile covers the navigation PGNs surfaced by
    /// `stack.gnss()`: rapid/detail position, COG/SOG, heading, attitude,
    /// rate of turn, DOPs, magnetic variation, and system time.
    #[must_use]
    pub const fn with_gnss_navigation(mut self, enable: bool) -> Self {
        self.listen_rapid_position = enable;
        self.listen_cog_sog = enable;
        self.listen_position_delta = enable;
        self.listen_attitude = enable;
        self.listen_rate_of_turn = enable;
        self.listen_position_detail = enable;
        self.listen_gnss_dops = enable;
        self.listen_magnetic_variation = enable;
        self.listen_system_time = enable;
        self.listen_heading = enable;
        self.listen_local_time_offset = enable;
        self.listen_gnss_sats_in_view = enable;
        self.listen_navigation_data = enable;
        self
    }

    #[must_use]
    pub const fn with_all(mut self, enable: bool) -> Self {
        self.listen_rapid_position = enable;
        self.listen_cog_sog = enable;
        self.listen_position_delta = enable;
        self.listen_attitude = enable;
        self.listen_rate_of_turn = enable;
        self.listen_position_detail = enable;
        self.listen_gnss_dops = enable;
        self.listen_magnetic_variation = enable;
        self.listen_wind = enable;
        self.listen_temperature = enable;
        self.listen_humidity = enable;
        self.listen_pressure = enable;
        self.listen_outside_environmental = enable;
        self.listen_engine = enable;
        self.listen_fluid_level = enable;
        self.listen_battery = enable;
        self.listen_depth = enable;
        self.listen_speed_water = enable;
        self.listen_xte = enable;
        self.listen_rudder = enable;
        self.listen_system_time = enable;
        self.listen_heading = enable;
        self.listen_local_time_offset = enable;
        self.listen_gnss_sats_in_view = enable;
        self.listen_navigation_data = enable;
        self
    }
}

// ─── NMEAInterface ────────────────────────────────────────────────────

/// Pump-style NMEA 2000 codec + state cache.
pub struct NMEAInterface {
    config: NMEAConfig,
    latest_position: Option<GNSSPosition>,

    pub on_position: Event<GNSSPosition>,
    pub on_cog: Event<f64>,
    pub on_sog: Event<f64>,
    pub on_position_delta: Event<PositionDeltaHighPrecisionRapidUpdateData>,
    /// `(yaw, pitch, roll)` in radians.
    pub on_attitude: Event<(f64, f64, f64)>,
    pub on_wind: Event<WindData>,
    pub on_temperature: Event<TemperatureData>,
    pub on_engine: Event<EngineData>,
    pub on_depth: Event<WaterDepthData>,
    pub on_heading: Event<f64>,
    pub on_system_time: Event<SystemTimeData>,
    pub on_local_time_offset: Event<LocalTimeOffsetData>,
    pub on_gnss_sats_in_view: Event<GnssSatsInViewData>,
    pub on_navigation_data: Event<NavigationData>,
    pub on_gnss_dops: Event<GNSSDOPData>,
    pub on_magnetic_variation: Event<f64>,
    pub on_rudder: Event<RudderData>,
    pub on_fluid_level: Event<FluidLevelData>,
    pub on_battery: Event<BatteryStatusData>,
    pub on_speed_water: Event<SpeedWaterData>,
    pub on_xte: Event<XTEData>,
    pub on_humidity: Event<HumidityData>,
    pub on_pressure: Event<PressureData>,
    pub on_outside_environmental: Event<OutsideEnvironmentalData>,
}

impl NMEAInterface {
    #[must_use]
    pub fn new(config: NMEAConfig) -> Self {
        Self {
            config,
            latest_position: None,
            on_position: Event::new(),
            on_cog: Event::new(),
            on_sog: Event::new(),
            on_position_delta: Event::new(),
            on_attitude: Event::new(),
            on_wind: Event::new(),
            on_temperature: Event::new(),
            on_engine: Event::new(),
            on_depth: Event::new(),
            on_heading: Event::new(),
            on_system_time: Event::new(),
            on_local_time_offset: Event::new(),
            on_gnss_sats_in_view: Event::new(),
            on_navigation_data: Event::new(),
            on_gnss_dops: Event::new(),
            on_magnetic_variation: Event::new(),
            on_rudder: Event::new(),
            on_fluid_level: Event::new(),
            on_battery: Event::new(),
            on_speed_water: Event::new(),
            on_xte: Event::new(),
            on_humidity: Event::new(),
            on_pressure: Event::new(),
            on_outside_environmental: Event::new(),
        }
    }

    #[inline]
    #[must_use]
    pub fn latest_position(&self) -> Option<GNSSPosition> {
        self.latest_position
    }

    // ─── Send builders ────────────────────────────────────────────────

    /// PGN 129025 — position rapid update.
    #[must_use]
    pub fn build_position(pos: &GNSSPosition) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let lat_raw = scaled_latitude_i32(pos.wgs.latitude);
        let lon_raw = scaled_longitude_i32(pos.wgs.longitude);
        data[0..4].copy_from_slice(&lat_raw.to_le_bytes());
        data[4..8].copy_from_slice(&lon_raw.to_le_bytes());
        data
    }

    /// PGN 129026 — COG / SOG rapid.
    #[must_use]
    pub fn build_cog_sog(cog_rad: f64, sog_mps: f64) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = 0xFF;
        data[1] = 0x00;
        let cog_raw = scaled_circular_angle_u16(cog_rad, COG_RESOLUTION);
        data[2..4].copy_from_slice(&cog_raw.to_le_bytes());
        let sog_raw = scaled_u16(sog_mps, SPEED_RESOLUTION);
        data[4..6].copy_from_slice(&sog_raw.to_le_bytes());
        data
    }

    /// PGN 129027 — Position Delta, High Precision Rapid Update.
    #[must_use]
    pub fn build_position_delta(delta: &PositionDeltaHighPrecisionRapidUpdateData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(delta.sid);
        data[1] = scaled_u8_with_reserved_band(delta.time_delta_s, POSITION_DELTA_TIME_RESOLUTION);
        let lat_raw = scaled_i24(delta.latitude_delta_deg, POSITION_DELTA_RESOLUTION);
        let lon_raw = scaled_i24(delta.longitude_delta_deg, POSITION_DELTA_RESOLUTION);
        write_i24_le(&mut data[2..5], lat_raw);
        write_i24_le(&mut data[5..8], lon_raw);
        data
    }

    /// PGN 130306 — wind data.
    #[must_use]
    pub fn build_wind(wind: &WindData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(wind.sid);
        let speed_raw = scaled_u16(wind.speed_mps, WIND_SPEED_RESOLUTION);
        data[1..3].copy_from_slice(&speed_raw.to_le_bytes());
        let dir_raw = scaled_circular_angle_u16(wind.direction_rad, WIND_DIR_RESOLUTION);
        data[3..5].copy_from_slice(&dir_raw.to_le_bytes());
        data[5] = wind.reference.as_u8();
        data
    }

    /// PGN 130312 — temperature.
    #[must_use]
    pub fn build_temperature(temp: &TemperatureData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(temp.sid);
        data[1] = temp.instance;
        data[2] = temp.source.as_u8();
        let temp_raw = scaled_u16(temp.actual_k, TEMPERATURE_RESOLUTION);
        data[3..5].copy_from_slice(&temp_raw.to_le_bytes());
        if temp.set_k > 0.0 || !temp.set_k.is_finite() {
            let set_raw = scaled_u16(temp.set_k, TEMPERATURE_RESOLUTION);
            data[5..7].copy_from_slice(&set_raw.to_le_bytes());
        }
        data
    }

    /// PGN 127488 — engine parameters rapid.
    #[must_use]
    pub fn build_engine_params(engine: &EngineData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = engine.instance;
        let rpm_raw = scaled_u16(engine.rpm, RPM_RESOLUTION);
        data[1..3].copy_from_slice(&rpm_raw.to_le_bytes());
        let boost_raw = scaled_u16(engine.boost_pressure_pa, PRESSURE_RESOLUTION);
        data[3..5].copy_from_slice(&boost_raw.to_le_bytes());
        data[5] = engine_tilt_trim_to_wire(engine.tilt_trim);
        data
    }

    /// PGN 128267 — water depth.
    #[must_use]
    pub fn build_depth(depth: &WaterDepthData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(depth.sid);
        let depth_raw = scaled_u32(depth.depth_m, DEPTH_RESOLUTION);
        data[1..5].copy_from_slice(&depth_raw.to_le_bytes());
        let offset_raw = scaled_i16(depth.offset_m, DEPTH_OFFSET_RESOLUTION);
        data[5..7].copy_from_slice(&(offset_raw as u16).to_le_bytes());
        if depth.range_m > 0.0 || !depth.range_m.is_finite() {
            data[7] = scaled_u8_with_reserved_band(depth.range_m, DEPTH_RANGE_RESOLUTION);
        }
        data
    }

    /// PGN 127250 — heading / track.
    #[must_use]
    pub fn build_heading(heading_rad: f64, deviation_rad: f64, variation_rad: f64) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let h_raw = scaled_circular_angle_u16(heading_rad, HEADING_RESOLUTION);
        data[1..3].copy_from_slice(&h_raw.to_le_bytes());
        let d_raw = scaled_i16(deviation_rad, HEADING_RESOLUTION);
        data[3..5].copy_from_slice(&(d_raw as u16).to_le_bytes());
        let v_raw = scaled_i16(variation_rad, HEADING_RESOLUTION);
        data[5..7].copy_from_slice(&(v_raw as u16).to_le_bytes());
        data[7] = 0x00;
        data
    }

    /// PGN 126992 — system time.
    #[must_use]
    pub fn build_system_time(time: &SystemTimeData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(time.sid);
        data[1] = time.source.as_u8();
        let days_raw = nmea_u16_with_reserved_band_to_wire(time.days_since_epoch);
        data[2..4].copy_from_slice(&days_raw.to_le_bytes());
        let secs_raw = time_of_day_seconds_to_wire(time.seconds_since_midnight);
        data[4..8].copy_from_slice(&secs_raw.to_le_bytes());
        data
    }

    /// PGN 127245 — rudder.
    #[must_use]
    pub fn build_rudder(rudder: &RudderData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = rudder.instance;
        data[1] = 0xF8 | (rudder.direction.as_u8() & 0x07);
        let order_raw = scaled_i16(rudder.angle_order_rad, HEADING_RESOLUTION);
        data[2..4].copy_from_slice(&(order_raw as u16).to_le_bytes());
        let position_raw = scaled_i16(rudder.position_rad, HEADING_RESOLUTION);
        data[4..6].copy_from_slice(&(position_raw as u16).to_le_bytes());
        data
    }

    /// PGN 127505 — fluid level.
    #[must_use]
    pub fn build_fluid_level(fluid: &FluidLevelData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (fluid.instance & 0x0F) | (fluid.r#type.as_u8() << 4);
        let level_raw = scaled_u16(fluid.level_pct, FLUID_LEVEL_RESOLUTION);
        data[1..3].copy_from_slice(&level_raw.to_le_bytes());
        let capacity_raw = scaled_u32(fluid.capacity_l, FLUID_CAPACITY_RESOLUTION);
        data[3..7].copy_from_slice(&capacity_raw.to_le_bytes());
        data
    }

    /// PGN 127508 — battery status.
    #[must_use]
    pub fn build_battery_status(bat: &BatteryStatusData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = bat.instance;
        let voltage_raw = scaled_u16(bat.voltage, VOLTAGE_RESOLUTION);
        data[1..3].copy_from_slice(&voltage_raw.to_le_bytes());
        let current_raw = scaled_i16(bat.current_a, CURRENT_RESOLUTION);
        data[3..5].copy_from_slice(&(current_raw as u16).to_le_bytes());
        data
    }

    /// PGN 128259 — speed, water referenced.
    #[must_use]
    pub fn build_speed_water(spd: &SpeedWaterData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(spd.sid);
        let water_raw = scaled_u16(spd.water_speed_mps, SPEED_RESOLUTION);
        data[1..3].copy_from_slice(&water_raw.to_le_bytes());
        let ground_raw = scaled_u16(spd.ground_speed_mps, SPEED_RESOLUTION);
        data[3..5].copy_from_slice(&ground_raw.to_le_bytes());
        data[5] = spd.reference.as_u8();
        data
    }

    /// PGN 129283 — XTE.
    #[must_use]
    pub fn build_xte(xte: &XTEData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(xte.sid);
        data[1] = xte.mode.as_u8();
        if xte.navigation_terminated {
            data[1] |= 0x40;
        }
        let xte_raw = scaled_i32(xte.xte_m, XTE_RESOLUTION);
        data[2..6].copy_from_slice(&(xte_raw as u32).to_le_bytes());
        data
    }

    /// PGN 130313 — humidity.
    #[must_use]
    pub fn build_humidity(hum: &HumidityData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(hum.sid);
        data[1] = hum.instance;
        data[2] = hum.source.as_u8();
        let actual_raw = scaled_u16(hum.actual_pct, HUMIDITY_RESOLUTION);
        data[3..5].copy_from_slice(&actual_raw.to_le_bytes());
        let set_raw = scaled_u16(hum.set_pct, HUMIDITY_RESOLUTION);
        data[5..7].copy_from_slice(&set_raw.to_le_bytes());
        data
    }

    /// PGN 130314 — pressure.
    #[must_use]
    pub fn build_pressure(pres: &PressureData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(pres.sid);
        data[1] = pres.instance;
        data[2] = pressure_source_to_wire(pres.source);
        let pressure_raw = scaled_i32(pres.pressure_pa, ACTUAL_PRESSURE_RESOLUTION);
        data[3..7].copy_from_slice(&(pressure_raw as u32).to_le_bytes());
        data
    }

    /// PGN 130310 — outside environmental parameters.
    #[must_use]
    pub fn build_outside_environmental(env: &OutsideEnvironmentalData) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = nmea_sequence_id_to_wire(env.sid);
        let water_raw = scaled_u16(env.water_temperature_k, TEMPERATURE_RESOLUTION);
        data[1..3].copy_from_slice(&water_raw.to_le_bytes());
        let outside_raw = scaled_u16(env.outside_temperature_k, TEMPERATURE_RESOLUTION);
        data[3..5].copy_from_slice(&outside_raw.to_le_bytes());
        let pressure_raw = scaled_u16(env.atmospheric_pressure_pa, PRESSURE_RESOLUTION);
        data[5..7].copy_from_slice(&pressure_raw.to_le_bytes());
        data
    }

    /// PGN 127258 — magnetic variation.
    #[must_use]
    pub fn build_magnetic_variation(variation_rad: f64, age_days: u16) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = 0xFF;
        data[1] = 0x00;
        let age_raw = nmea_u16_with_reserved_band_to_wire(age_days);
        data[2..4].copy_from_slice(&age_raw.to_le_bytes());
        let var_raw = scaled_i16(variation_rad, HEADING_RESOLUTION);
        data[4..6].copy_from_slice(&(var_raw as u16).to_le_bytes());
        data
    }

    // ─── Inbound dispatch ─────────────────────────────────────────────

    /// Feed an inbound message; routes by PGN per [`NMEAConfig`] and
    /// fires the relevant event.
    pub fn handle_message(&mut self, msg: &Message) {
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return;
        }
        match msg.pgn {
            PGN_GNSS_POSITION_RAPID if self.config.listen_rapid_position => {
                self.handle_position_rapid(msg);
            }
            PGN_GNSS_COG_SOG_RAPID if self.config.listen_cog_sog => {
                self.handle_cog_sog(msg);
            }
            PGN_GNSS_POSITION_DELTA if self.config.listen_position_delta => {
                self.handle_position_delta(msg);
            }
            PGN_ATTITUDE if self.config.listen_attitude => self.handle_attitude(msg),
            PGN_RATE_OF_TURN if self.config.listen_rate_of_turn => self.handle_rate_of_turn(msg),
            PGN_GNSS_POSITION_DATA if self.config.listen_position_detail => {
                self.handle_position_detail(msg);
            }
            PGN_GNSS_DOPS if self.config.listen_gnss_dops => self.handle_gnss_dops(msg),
            PGN_MAGNETIC_VARIATION if self.config.listen_magnetic_variation => {
                self.handle_magnetic_variation(msg);
            }
            PGN_WIND_DATA if self.config.listen_wind => self.handle_wind(msg),
            PGN_TEMPERATURE if self.config.listen_temperature => self.handle_temperature(msg),
            PGN_HUMIDITY if self.config.listen_humidity => self.handle_humidity(msg),
            PGN_PRESSURE if self.config.listen_pressure => self.handle_pressure(msg),
            PGN_OUTSIDE_ENVIRONMENTAL if self.config.listen_outside_environmental => {
                self.handle_outside_environmental(msg);
            }
            PGN_ENGINE_PARAMS_RAPID if self.config.listen_engine => self.handle_engine(msg),
            PGN_FLUID_LEVEL if self.config.listen_fluid_level => self.handle_fluid_level(msg),
            PGN_BATTERY_STATUS if self.config.listen_battery => self.handle_battery(msg),
            PGN_WATER_DEPTH if self.config.listen_depth => self.handle_depth(msg),
            PGN_SPEED_WATER if self.config.listen_speed_water => self.handle_speed_water(msg),
            PGN_XTE if self.config.listen_xte => self.handle_xte(msg),
            PGN_RUDDER if self.config.listen_rudder => self.handle_rudder(msg),
            PGN_SYSTEM_TIME if self.config.listen_system_time => self.handle_system_time(msg),
            PGN_LOCAL_TIME_OFFSET if self.config.listen_local_time_offset => {
                self.handle_local_time_offset(msg)
            }
            PGN_GNSS_SATELLITES_IN_VIEW if self.config.listen_gnss_sats_in_view => {
                self.handle_gnss_sats_in_view(msg)
            }
            PGN_NAVIGATION_DATA if self.config.listen_navigation_data => {
                self.handle_navigation_data(msg)
            }
            PGN_HEADING_TRACK if self.config.listen_heading => self.handle_heading(msg),
            _ => {}
        }
    }

    fn handle_position_rapid(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload(msg) else {
            return;
        };
        let lat_raw = i32::from_le_bytes(data[0..4].try_into().unwrap());
        let lon_raw = i32::from_le_bytes(data[4..8].try_into().unwrap());
        if !signed_i32_data_is_available(lat_raw)
            || !signed_i32_data_is_available(lon_raw)
            || signed_i32_data_is_reserved(lat_raw)
            || signed_i32_data_is_reserved(lon_raw)
            || !nmea_latitude_i32_raw_is_in_range(lat_raw)
            || !nmea_longitude_i32_raw_is_in_range(lon_raw)
        {
            return;
        }
        let mut pos = GNSSPosition {
            wgs: Wgs::new(
                lat_raw as f64 * LAT_LON_RESOLUTION,
                lon_raw as f64 * LAT_LON_RESOLUTION,
                0.0,
            ),
            timestamp_us: msg.timestamp_us,
            fix_type: GNSSFixType::GNSSFix,
            ..Default::default()
        };
        if let Some(prev) = self.latest_position {
            pos.heading_rad = prev.heading_rad;
            pos.speed_mps = prev.speed_mps;
            pos.satellites_used = prev.satellites_used;
            pos.hdop = prev.hdop;
            pos.pdop = prev.pdop;
        }
        self.latest_position = Some(pos);
        self.on_position.emit(&pos);
    }

    fn handle_cog_sog(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 6) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        if HeadingReference::try_from_u8(data[1]).is_none() {
            return;
        }
        let cog_raw = u16::from_le_bytes([data[2], data[3]]);
        let sog_raw = u16::from_le_bytes([data[4], data[5]]);
        if matches!(cog_raw, 0xFFFD | 0xFFFE)
            || !circular_angle_raw_is_canonical(cog_raw)
            || matches!(sog_raw, 0xFFFD | 0xFFFE)
        {
            return;
        }
        if cog_raw != 0xFFFF {
            let cog = cog_raw as f64 * COG_RESOLUTION;
            self.on_cog.emit(&cog);
            if let Some(p) = self.latest_position.as_mut() {
                p.cog_rad = Some(cog);
            }
        }
        if sog_raw != 0xFFFF {
            let sog = sog_raw as f64 * SPEED_RESOLUTION;
            self.on_sog.emit(&sog);
            if let Some(p) = self.latest_position.as_mut() {
                p.speed_mps = Some(sog);
            }
        }
    }

    fn handle_position_delta(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload(msg) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let time_raw = data[1];
        let lat_raw = i24_from_le_bytes([data[2], data[3], data[4]]);
        let lon_raw = i24_from_le_bytes([data[5], data[6], data[7]]);
        if time_raw >= 0xFD
            || !signed_i24_data_is_available(lat_raw)
            || !signed_i24_data_is_available(lon_raw)
            || signed_i24_data_is_reserved(lat_raw)
            || signed_i24_data_is_reserved(lon_raw)
        {
            return;
        }
        let delta = PositionDeltaHighPrecisionRapidUpdateData {
            sid: data[0],
            time_delta_s: time_raw as f64 * POSITION_DELTA_TIME_RESOLUTION,
            latitude_delta_deg: lat_raw as f64 * POSITION_DELTA_RESOLUTION,
            longitude_delta_deg: lon_raw as f64 * POSITION_DELTA_RESOLUTION,
        };
        if let Some(mut pos) = self.latest_position {
            pos.wgs = Wgs::new(
                pos.wgs.latitude + delta.latitude_delta_deg,
                pos.wgs.longitude + delta.longitude_delta_deg,
                pos.wgs.altitude,
            );
            pos.timestamp_us = msg.timestamp_us;
            self.latest_position = Some(pos);
            self.on_position.emit(&pos);
        }
        self.on_position_delta.emit(&delta);
    }

    fn handle_attitude(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 7) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let yaw_raw = i16::from_le_bytes([data[1], data[2]]);
        let pitch_raw = i16::from_le_bytes([data[3], data[4]]);
        let roll_raw = i16::from_le_bytes([data[5], data[6]]);
        if !signed_i16_data_is_available(yaw_raw)
            || !signed_i16_data_is_available(pitch_raw)
            || !signed_i16_data_is_available(roll_raw)
            || signed_i16_data_is_reserved(yaw_raw)
            || signed_i16_data_is_reserved(pitch_raw)
            || signed_i16_data_is_reserved(roll_raw)
        {
            return;
        }
        let yaw = yaw_raw as f64 * HEADING_RESOLUTION;
        let pitch = pitch_raw as f64 * HEADING_RESOLUTION;
        let roll = roll_raw as f64 * HEADING_RESOLUTION;
        if let Some(p) = self.latest_position.as_mut() {
            p.heading_rad = Some(yaw);
            p.pitch_rad = Some(pitch);
            p.roll_rad = Some(roll);
        }
        self.on_attitude.emit(&(yaw, pitch, roll));
    }

    fn handle_rate_of_turn(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 5) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let rot_raw = i32::from_le_bytes(data[1..5].try_into().unwrap());
        if signed_i32_data_is_reserved(rot_raw) {
            return;
        }
        if signed_i32_data_is_available(rot_raw) {
            let rot = rot_raw as f64 * ROT_RESOLUTION;
            if let Some(p) = self.latest_position.as_mut() {
                p.rate_of_turn_rps = Some(rot);
            }
        }
    }

    fn handle_wind(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 6) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let Some(reference) = WindReference::try_from_u8(data[5]) else {
            return;
        };
        let mut wind = WindData {
            sid: data[0],
            reference,
            ..Default::default()
        };
        let speed_raw = u16::from_le_bytes([data[1], data[2]]);
        let dir_raw = u16::from_le_bytes([data[3], data[4]]);
        if matches!(speed_raw, 0xFFFD | 0xFFFE)
            || matches!(dir_raw, 0xFFFD | 0xFFFE)
            || !circular_angle_raw_is_canonical(dir_raw)
        {
            return;
        }
        if speed_raw != 0xFFFF {
            wind.speed_mps = speed_raw as f64 * WIND_SPEED_RESOLUTION;
        }
        if dir_raw != 0xFFFF {
            wind.direction_rad = dir_raw as f64 * WIND_DIR_RESOLUTION;
        }
        self.on_wind.emit(&wind);
    }

    fn handle_temperature(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 7) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let Some(source) = TemperatureSource::try_from_u8(data[2]) else {
            return;
        };
        let mut temp = TemperatureData {
            sid: data[0],
            instance: data[1],
            source,
            ..Default::default()
        };
        let temp_raw = u16::from_le_bytes([data[3], data[4]]);
        let set_raw = u16::from_le_bytes([data[5], data[6]]);
        if matches!(temp_raw, 0xFFFD | 0xFFFE) || matches!(set_raw, 0xFFFD | 0xFFFE) {
            return;
        }
        if temp_raw != 0xFFFF {
            temp.actual_k = temp_raw as f64 * TEMPERATURE_RESOLUTION;
        }
        if set_raw != 0xFFFF {
            temp.set_k = set_raw as f64 * TEMPERATURE_RESOLUTION;
        }
        self.on_temperature.emit(&temp);
    }

    fn handle_engine(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 6) else {
            return;
        };
        if !signed_i8_engine_tilt_trim_is_supported(data[5]) {
            return;
        }
        let mut engine = EngineData {
            instance: data[0],
            tilt_trim: data[5] as i8,
            ..Default::default()
        };
        let rpm_raw = u16::from_le_bytes([data[1], data[2]]);
        let boost_raw = u16::from_le_bytes([data[3], data[4]]);
        if matches!(rpm_raw, 0xFFFD | 0xFFFE) || matches!(boost_raw, 0xFFFD | 0xFFFE) {
            return;
        }
        if rpm_raw != 0xFFFF {
            engine.rpm = rpm_raw as f64 * RPM_RESOLUTION;
        }
        if boost_raw != 0xFFFF {
            engine.boost_pressure_pa = boost_raw as f64 * PRESSURE_RESOLUTION;
        }
        self.on_engine.emit(&engine);
    }

    fn handle_depth(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload(msg) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let mut depth = WaterDepthData {
            sid: data[0],
            ..Default::default()
        };
        let depth_raw = u32::from_le_bytes(data[1..5].try_into().unwrap());
        let offset_raw = i16::from_le_bytes([data[5], data[6]]);
        if matches!(depth_raw, 0xFFFF_FFFD | 0xFFFF_FFFE) || signed_i16_data_is_reserved(offset_raw)
        {
            return;
        }
        if depth_raw != 0xFFFF_FFFF {
            depth.depth_m = depth_raw as f64 * DEPTH_RESOLUTION;
        }
        if signed_i16_data_is_available(offset_raw) {
            depth.offset_m = offset_raw as f64 * DEPTH_OFFSET_RESOLUTION;
        }
        if matches!(data[7], 0xFD | 0xFE) {
            return;
        }
        if data[7] != 0xFF {
            depth.range_m = data[7] as f64 * DEPTH_RANGE_RESOLUTION;
        }
        self.on_depth.emit(&depth);
    }

    fn handle_heading(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload(msg) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        if HeadingReference::try_from_u8(data[7]).is_none() {
            return;
        }
        let heading_raw = u16::from_le_bytes([data[1], data[2]]);
        let deviation_raw = i16::from_le_bytes([data[3], data[4]]);
        let variation_raw = i16::from_le_bytes([data[5], data[6]]);
        if matches!(heading_raw, 0xFFFD | 0xFFFE)
            || !circular_angle_raw_is_canonical(heading_raw)
            || signed_i16_data_is_reserved(deviation_raw)
            || signed_i16_data_is_reserved(variation_raw)
        {
            return;
        }
        if heading_raw != 0xFFFF {
            let heading = heading_raw as f64 * HEADING_RESOLUTION;
            if let Some(p) = self.latest_position.as_mut() {
                p.heading_rad = Some(heading);
            }
            self.on_heading.emit(&heading);
        }
    }

    fn handle_system_time(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload(msg) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let Some(source) = TimeSource::try_from_u8(data[1]) else {
            return;
        };
        let days_raw = u16::from_le_bytes([data[2], data[3]]);
        let secs_raw = u32::from_le_bytes(data[4..8].try_into().unwrap());
        if !nmea_date_raw_is_canonical(days_raw) || !nmea_time_of_day_raw_is_canonical(secs_raw) {
            return;
        }
        let mut time = SystemTimeData {
            sid: data[0],
            source,
            days_since_epoch: if days_raw == 0xFFFF { 0 } else { days_raw },
            ..Default::default()
        };
        if secs_raw != 0xFFFF_FFFF {
            time.seconds_since_midnight = secs_raw as f64 / 10_000.0;
        }
        self.on_system_time.emit(&time);
    }

    /// PGN 129033 Time & Date / Local Time Offset. Layout (canboat, Apache):
    /// date u16 (days), time u32 (×0.0001 s), local offset i16 (minutes).
    fn handle_local_time_offset(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload(msg) else {
            return;
        };
        let days_raw = u16::from_le_bytes([data[0], data[1]]);
        let secs_raw = u32::from_le_bytes(data[2..6].try_into().unwrap());
        let offset_raw = i16::from_le_bytes([data[6], data[7]]);
        if !nmea_date_raw_is_canonical(days_raw) || !nmea_time_of_day_raw_is_canonical(secs_raw) {
            return;
        }
        let mut out = LocalTimeOffsetData {
            days_since_epoch: if days_raw == 0xFFFF { 0 } else { days_raw },
            ..Default::default()
        };
        if secs_raw != 0xFFFF_FFFF {
            out.seconds_since_midnight = secs_raw as f64 / 10_000.0;
        }
        // 0x7FFF (i16::MAX) is the not-available sentinel for the offset.
        if offset_raw != i16::MAX {
            out.local_offset_minutes = offset_raw;
        }
        self.on_local_time_offset.emit(&out);
    }

    /// PGN 129540 GNSS Sats in View (fast-packet). Layout (canboat, Apache):
    /// header `[sid][mode:2b|rsvd:6b][satsInView]` then a 12-byte per-satellite
    /// set: `[prn][elevation i16 ×0.0001 rad][azimuth u16 ×0.0001 rad]
    /// [snr i16 ×0.01 dB][rangeResiduals i32][status:4b|rsvd:4b]`.
    fn handle_gnss_sats_in_view(&mut self, msg: &Message) {
        let data = &msg.data;
        if data.len() < 3 {
            return;
        }
        let mut out = GnssSatsInViewData {
            sid: data[0],
            sats_in_view: data[2],
            satellites: Vec::new(),
        };
        let mut off = 3;
        while off + 12 <= data.len() {
            let range_resid_raw =
                i32::from_le_bytes([data[off + 7], data[off + 8], data[off + 9], data[off + 10]]);
            out.satellites.push(SatelliteInfo {
                prn: data[off],
                elevation_rad: i16::from_le_bytes([data[off + 1], data[off + 2]]) as f64 * 0.0001,
                azimuth_rad: u16::from_le_bytes([data[off + 3], data[off + 4]]) as f64 * 0.0001,
                snr_db: i16::from_le_bytes([data[off + 5], data[off + 6]]) as f64 * 0.01,
                range_residual_m: range_resid_raw as f64 * 0.00001,
                ..SatelliteInfo::default()
            });
            off += 12;
        }
        self.on_gnss_sats_in_view.emit(&out);
    }

    /// PGN 129284 Navigation Data (distance/bearing to destination waypoint).
    /// Layout (canboat, Apache): `[sid][distance u32 ×0.01 m][flags: bearing-ref:2
    /// |perp:2|arrival:2|calc:2][eta time u32 ×0.0001 s][eta date u16][bearing
    /// orig→dest u16 ×0.0001 rad][bearing pos→dest u16][origin wp u32][dest wp u32]
    /// [dest lat i32 ×1e-7][dest lon i32 ×1e-7][closing vel i16 ×0.01 m/s]`.
    fn handle_navigation_data(&mut self, msg: &Message) {
        let data = &msg.data;
        if data.len() < 34 {
            return;
        }
        let flags = data[5];
        let nav = NavigationData {
            sid: data[0],
            distance_to_wp_m: u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as f64
                * 0.01,
            bearing_reference: HeadingReference::try_from_u8(flags & 0x03).unwrap_or_default(),
            perpendicular_crossed: (flags >> 2) & 0x03 == 1,
            arrival_circle_entered: (flags >> 4) & 0x03 == 1,
            calc_type: DistanceCalculationType::try_from_u8((flags >> 6) & 0x03)
                .unwrap_or_default(),
            eta_time: u32::from_le_bytes([data[6], data[7], data[8], data[9]]) as f64 * 0.0001,
            eta_date: u16::from_le_bytes([data[10], data[11]]) as i16,
            bearing_origin_to_dest_rad: u16::from_le_bytes([data[12], data[13]]) as f64 * 0.0001,
            bearing_pos_to_dest_rad: u16::from_le_bytes([data[14], data[15]]) as f64 * 0.0001,
            origin_wp_number: u32::from_le_bytes([data[16], data[17], data[18], data[19]]),
            dest_wp_number: u32::from_le_bytes([data[20], data[21], data[22], data[23]]),
            dest_latitude: i32::from_le_bytes([data[24], data[25], data[26], data[27]]) as f64
                * 1e-7,
            dest_longitude: i32::from_le_bytes([data[28], data[29], data[30], data[31]]) as f64
                * 1e-7,
            wp_closing_velocity_mps: i16::from_le_bytes([data[32], data[33]]) as f64 * 0.01,
        };
        self.on_navigation_data.emit(&nav);
    }

    fn handle_gnss_dops(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload(msg) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        if data[1] & 0xC0 != 0 {
            return;
        }
        let Some(desired_mode) = GNSSDOPMode::try_from_u8(data[1] & 0x07) else {
            return;
        };
        let Some(actual_mode) = GNSSDOPMode::try_from_u8((data[1] >> 3) & 0x07) else {
            return;
        };
        let mut dops = GNSSDOPData {
            sid: data[0],
            desired_mode,
            actual_mode,
            ..Default::default()
        };
        let hdop_raw = u16::from_le_bytes([data[2], data[3]]);
        let vdop_raw = u16::from_le_bytes([data[4], data[5]]);
        let tdop_raw = u16::from_le_bytes([data[6], data[7]]);
        let Some(hdop) = nmea_u16_scaled_field(hdop_raw, DOP_RESOLUTION) else {
            return;
        };
        let Some(vdop) = nmea_u16_scaled_field(vdop_raw, DOP_RESOLUTION) else {
            return;
        };
        let Some(tdop) = nmea_u16_scaled_field(tdop_raw, DOP_RESOLUTION) else {
            return;
        };
        if let Some(hdop) = hdop {
            dops.hdop = hdop;
        }
        if let Some(vdop) = vdop {
            dops.vdop = vdop;
        }
        if let Some(tdop) = tdop {
            dops.tdop = tdop;
        }
        if let Some(p) = self.latest_position.as_mut() {
            if dops.hdop > 0.0 {
                p.hdop = Some(dops.hdop);
            }
            if dops.vdop > 0.0 {
                p.vdop = Some(dops.vdop);
            }
        }
        self.on_gnss_dops.emit(&dops);
    }

    fn handle_magnetic_variation(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 6) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        if MagneticVariationSource::try_from_u8(data[1]).is_none() {
            return;
        }
        let age_raw = u16::from_le_bytes([data[2], data[3]]);
        if matches!(age_raw, 0xFFFD | 0xFFFE) {
            return;
        }
        let var_raw = i16::from_le_bytes([data[4], data[5]]);
        if signed_i16_data_is_reserved(var_raw) {
            return;
        }
        if signed_i16_data_is_available(var_raw) {
            let v = var_raw as f64 * HEADING_RESOLUTION;
            self.on_magnetic_variation.emit(&v);
        }
    }

    fn handle_rudder(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 6) else {
            return;
        };
        if data[1] & 0xF8 != 0xF8 {
            return;
        }
        let Some(direction) = RudderDirection::try_from_u8(data[1] & 0x07) else {
            return;
        };
        let mut rudder = RudderData {
            instance: data[0],
            direction,
            ..Default::default()
        };
        let order_raw = i16::from_le_bytes([data[2], data[3]]);
        let position_raw = i16::from_le_bytes([data[4], data[5]]);
        if signed_i16_data_is_reserved(order_raw) || signed_i16_data_is_reserved(position_raw) {
            return;
        }
        if signed_i16_data_is_available(order_raw) {
            rudder.angle_order_rad = order_raw as f64 * HEADING_RESOLUTION;
        }
        if signed_i16_data_is_available(position_raw) {
            rudder.position_rad = position_raw as f64 * HEADING_RESOLUTION;
        }
        self.on_rudder.emit(&rudder);
    }

    fn handle_fluid_level(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 7) else {
            return;
        };
        let Some(r#type) = FluidType::try_from_u8(data[0] >> 4) else {
            return;
        };
        let mut fluid = FluidLevelData {
            instance: data[0] & 0x0F,
            r#type,
            ..Default::default()
        };
        let level_raw = u16::from_le_bytes([data[1], data[2]]);
        if matches!(level_raw, 0xFFFD | 0xFFFE) {
            return;
        }
        if level_raw != 0xFFFF {
            fluid.level_pct = level_raw as f64 * FLUID_LEVEL_RESOLUTION;
        }
        let capacity_raw = u32::from_le_bytes(data[3..7].try_into().unwrap());
        if matches!(capacity_raw, 0xFFFF_FFFD | 0xFFFF_FFFE) {
            return;
        }
        if capacity_raw != 0xFFFF_FFFF {
            fluid.capacity_l = capacity_raw as f64 * FLUID_CAPACITY_RESOLUTION;
        }
        self.on_fluid_level.emit(&fluid);
    }

    fn handle_battery(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 5) else {
            return;
        };
        let mut bat = BatteryStatusData {
            instance: data[0],
            ..Default::default()
        };
        let voltage_raw = u16::from_le_bytes([data[1], data[2]]);
        let current_raw = i16::from_le_bytes([data[3], data[4]]);
        if signed_i16_data_is_reserved(current_raw) {
            return;
        }
        let Some(voltage) = nmea_u16_scaled_field(voltage_raw, VOLTAGE_RESOLUTION) else {
            return;
        };
        if let Some(voltage) = voltage {
            bat.voltage = voltage;
        }
        if signed_i16_data_is_available(current_raw) {
            bat.current_a = current_raw as f64 * CURRENT_RESOLUTION;
        }
        self.on_battery.emit(&bat);
    }

    fn handle_speed_water(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 6) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let Some(reference) = SpeedWaterRefType::try_from_u8(data[5]) else {
            return;
        };
        let mut spd = SpeedWaterData {
            sid: data[0],
            reference,
            ..Default::default()
        };
        let water_raw = u16::from_le_bytes([data[1], data[2]]);
        let ground_raw = u16::from_le_bytes([data[3], data[4]]);
        if matches!(water_raw, 0xFFFD | 0xFFFE) || matches!(ground_raw, 0xFFFD | 0xFFFE) {
            return;
        }
        if water_raw != 0xFFFF {
            spd.water_speed_mps = water_raw as f64 * SPEED_RESOLUTION;
        }
        if ground_raw != 0xFFFF {
            spd.ground_speed_mps = ground_raw as f64 * SPEED_RESOLUTION;
        }
        self.on_speed_water.emit(&spd);
    }

    fn handle_xte(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 6) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        if data[1] & !0x4F != 0 {
            return;
        }
        let Some(mode) = XTEMode::try_from_u8(data[1] & 0x0F) else {
            return;
        };
        let mut xte = XTEData {
            sid: data[0],
            mode,
            navigation_terminated: data[1] & 0x40 != 0,
            ..Default::default()
        };
        let xte_raw = i32::from_le_bytes(data[2..6].try_into().unwrap());
        if signed_i32_data_is_reserved(xte_raw) {
            return;
        }
        if signed_i32_data_is_available(xte_raw) {
            xte.xte_m = xte_raw as f64 * XTE_RESOLUTION;
        }
        self.on_xte.emit(&xte);
    }

    fn handle_humidity(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 7) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let Some(source) = HumiditySource::try_from_u8(data[2]) else {
            return;
        };
        let mut hum = HumidityData {
            sid: data[0],
            instance: data[1],
            source,
            ..Default::default()
        };
        let actual_raw = u16::from_le_bytes([data[3], data[4]]);
        let set_raw = u16::from_le_bytes([data[5], data[6]]);
        let Some(actual) = nmea_u16_scaled_field(actual_raw, HUMIDITY_RESOLUTION) else {
            return;
        };
        let Some(set) = nmea_u16_scaled_field(set_raw, HUMIDITY_RESOLUTION) else {
            return;
        };
        if let Some(actual) = actual {
            hum.actual_pct = actual;
        }
        if let Some(set) = set {
            hum.set_pct = set;
        }
        self.on_humidity.emit(&hum);
    }

    fn handle_pressure(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 7) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let Some(source) = PressureSource::try_from_u8(data[2]) else {
            return;
        };
        let mut pres = PressureData {
            sid: data[0],
            instance: data[1],
            source,
            ..Default::default()
        };
        let pressure_raw = i32::from_le_bytes(data[3..7].try_into().unwrap());
        if signed_i32_data_is_reserved(pressure_raw) {
            return;
        }
        if signed_i32_data_is_available(pressure_raw) {
            pres.pressure_pa = pressure_raw as f64 * ACTUAL_PRESSURE_RESOLUTION;
        }
        self.on_pressure.emit(&pres);
    }

    fn handle_outside_environmental(&mut self, msg: &Message) {
        let Some(data) = classic_can_payload_with_ff_tail(msg, 7) else {
            return;
        };
        if !nmea_sequence_id_is_canonical(data[0]) {
            return;
        }
        let mut env = OutsideEnvironmentalData {
            sid: data[0],
            ..Default::default()
        };
        let water = u16::from_le_bytes([data[1], data[2]]);
        let air = u16::from_le_bytes([data[3], data[4]]);
        let pressure = u16::from_le_bytes([data[5], data[6]]);
        if matches!(water, 0xFFFD | 0xFFFE)
            || matches!(air, 0xFFFD | 0xFFFE)
            || matches!(pressure, 0xFFFD | 0xFFFE)
        {
            return;
        }
        if water != 0xFFFF {
            env.water_temperature_k = water as f64 * TEMPERATURE_RESOLUTION;
        }
        if air != 0xFFFF {
            env.outside_temperature_k = air as f64 * TEMPERATURE_RESOLUTION;
        }
        if pressure != 0xFFFF {
            env.atmospheric_pressure_pa = pressure as f64 * PRESSURE_RESOLUTION;
        }
        self.on_outside_environmental.emit(&env);
    }

    /// PGN 129029 — fast-packet GNSS Position Data.
    fn handle_position_detail(&mut self, msg: &Message) {
        if !gnss_position_detail_payload_len_is_canonical(&msg.data) {
            return;
        }
        if !nmea_sequence_id_is_canonical(msg.data[0]) {
            return;
        }
        let position_days_raw = u16::from_le_bytes([msg.data[1], msg.data[2]]);
        let position_time_raw = u32::from_le_bytes(msg.data[3..7].try_into().unwrap());
        if !nmea_date_raw_is_canonical(position_days_raw)
            || !nmea_time_of_day_raw_is_canonical(position_time_raw)
        {
            return;
        }
        let mut pos = GNSSPosition {
            timestamp_us: msg.timestamp_us,
            ..Default::default()
        };
        let lat_raw = i64::from_le_bytes(msg.data[7..15].try_into().unwrap());
        let lon_raw = i64::from_le_bytes(msg.data[15..23].try_into().unwrap());
        if !signed_i64_data_is_available(lat_raw)
            || !signed_i64_data_is_available(lon_raw)
            || signed_i64_data_is_reserved(lat_raw)
            || signed_i64_data_is_reserved(lon_raw)
            || !nmea_latitude_i64_raw_is_in_range(lat_raw)
            || !nmea_longitude_i64_raw_is_in_range(lon_raw)
        {
            return;
        }
        let lat = lat_raw as f64 * 1e-16;
        let lon = lon_raw as f64 * 1e-16;
        pos.wgs = Wgs::new(lat, lon, 0.0);
        let alt_raw = i64::from_le_bytes(msg.data[23..31].try_into().unwrap());
        if signed_i64_data_is_reserved(alt_raw) {
            return;
        }
        if signed_i64_data_is_available(alt_raw) {
            pos.altitude_m = Some(alt_raw as f64 * 1e-6);
        }
        let type_byte = msg.data[31];
        let gnss_system_raw = type_byte & 0x0F;
        let Some(gnss_system) = GNSSSystem::try_from_u8(gnss_system_raw) else {
            return;
        };
        let fix_method = (type_byte >> 4) & 0x0F;
        let Some(fix_type) = GNSSFixType::try_from_u8(fix_method) else {
            return;
        };
        if !gnss_position_detail_integrity_byte_is_canonical(msg.data[32]) {
            return;
        }
        if !nmea_u8_count_raw_is_canonical(msg.data[33]) {
            return;
        }
        pos.gnss_system = gnss_system;
        pos.fix_type = fix_type;
        pos.satellites_used = if msg.data[33] == 0xFF {
            0
        } else {
            msg.data[33]
        };
        let hdop_raw = u16::from_le_bytes([msg.data[34], msg.data[35]]);
        let pdop_raw = u16::from_le_bytes([msg.data[36], msg.data[37]]);
        let Some(hdop) = nmea_u16_scaled_field(hdop_raw, 0.01) else {
            return;
        };
        let Some(pdop) = nmea_u16_scaled_field(pdop_raw, 0.01) else {
            return;
        };
        pos.hdop = hdop;
        pos.pdop = pdop;
        let geoidal_raw = i32::from_le_bytes(msg.data[38..42].try_into().unwrap());
        if signed_i32_data_is_reserved(geoidal_raw) {
            return;
        }
        if let Some(prev) = self.latest_position {
            pos.heading_rad = prev.heading_rad;
            pos.speed_mps = prev.speed_mps;
            pos.cog_rad = prev.cog_rad;
        }
        self.latest_position = Some(pos);
        self.on_position.emit(&pos);
    }
}

#[inline]
fn gnss_position_detail_payload_len_is_canonical(data: &[u8]) -> bool {
    const BASE_LEN: usize = 43;
    const REFERENCE_STATION_LEN: usize = 4;

    if data.len() < BASE_LEN {
        return false;
    }

    let reference_station_count = match data[42] {
        0xFF => 0usize,
        0xFD | 0xFE => return false,
        count => count as usize,
    };
    let Some(reference_station_bytes) = reference_station_count.checked_mul(REFERENCE_STATION_LEN)
    else {
        return false;
    };
    let Some(expected_len) = BASE_LEN.checked_add(reference_station_bytes) else {
        return false;
    };

    if data.len() != expected_len {
        return false;
    }

    for station_index in 0..reference_station_count {
        let type_offset = BASE_LEN + station_index * REFERENCE_STATION_LEN;
        let station_type = data[type_offset] & 0x0F;
        if !gnss_reference_station_type_is_defined(station_type) {
            return false;
        }
        let age_offset = type_offset + 2;
        let correction_age_raw = u16::from_le_bytes([data[age_offset], data[age_offset + 1]]);
        if matches!(correction_age_raw, 0xFFFD | 0xFFFE) {
            return false;
        }
    }

    true
}

#[inline]
const fn gnss_reference_station_type_is_defined(station_type: u8) -> bool {
    ReferenceStationType::try_from_u8(station_type).is_some()
}

#[inline]
const fn gnss_position_detail_integrity_byte_is_canonical(value: u8) -> bool {
    value & 0xFC == 0xFC
}

#[inline]
const fn nmea_date_raw_is_canonical(value: u16) -> bool {
    value <= 0xFFFC || value == 0xFFFF
}

#[inline]
const fn nmea_time_of_day_raw_is_canonical(value: u32) -> bool {
    value <= MAX_TIME_OF_DAY_RAW || value == 0xFFFF_FFFF
}

#[inline]
const fn nmea_sequence_id_is_canonical(value: u8) -> bool {
    value <= 0xFC || value == 0xFF
}

#[inline]
const fn nmea_u8_count_raw_is_canonical(value: u8) -> bool {
    value <= 0xFC || value == 0xFF
}

#[inline]
fn nmea_u16_scaled_field(raw: u16, resolution: f64) -> Option<Option<f64>> {
    match raw {
        0xFFFD | 0xFFFE => None,
        0xFFFF => Some(None),
        value => Some(Some(f64::from(value) * resolution)),
    }
}

#[inline]
const fn nmea_latitude_i32_raw_is_in_range(value: i32) -> bool {
    value >= -MAX_LATITUDE_I32_RAW && value <= MAX_LATITUDE_I32_RAW
}

#[inline]
const fn nmea_longitude_i32_raw_is_in_range(value: i32) -> bool {
    value >= -MAX_LONGITUDE_I32_RAW && value <= MAX_LONGITUDE_I32_RAW
}

#[inline]
const fn nmea_latitude_i64_raw_is_in_range(value: i64) -> bool {
    value >= -MAX_LATITUDE_I64_RAW && value <= MAX_LATITUDE_I64_RAW
}

#[inline]
const fn nmea_longitude_i64_raw_is_in_range(value: i64) -> bool {
    value >= -MAX_LONGITUDE_I64_RAW && value <= MAX_LONGITUDE_I64_RAW
}

impl Default for NMEAInterface {
    fn default() -> Self {
        Self::new(NMEAConfig::default())
    }
}

