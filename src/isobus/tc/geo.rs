//! ISO 11783-10 TC-GEO: position-based task control.
//!
//! Mirrors the C++ `machbus::isobus::tc::TCGEOInterface`. Pump-style
//! port:
//!
//! - [`TCGEOInterface::set_position`] / `try_handle_gnss_position` —
//!   inbound updates. [`TCGEOInterface::handle_gnss_position`] remains the
//!   compatibility wrapper that ignores malformed or unrelated frames.
//! - [`TCGEOInterface::position_process_data_payloads`] — emits the
//!   two `PGN_ECU_TO_TC` payloads (latitude + longitude) for the
//!   caller to ship.
//! - [`TCGEOInterface::update`] — re-evaluates prescription rate.
//!
//! `WGS` positions use [`crate::geo::Wgs`]. Hosted builds with `geo-concord`
//! use `concord`'s richer type; embedded/no-concord builds use the lightweight
//! in-crate equivalent.

use alloc::{format, string::String, vec::Vec};

use super::ddi_database::{ddi_is_rate, ddi_lookup, ddi_to_engineering};
use super::objects::DDI;
use super::server_options::ProcessDataCommands;
use crate::geo::Wgs;
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::PGN_GNSS_POSITION;

const TC_GEO_LAT_MIN_RAW: i32 = -900_000_000;
const TC_GEO_LAT_MAX_RAW: i32 = 900_000_000;
const TC_GEO_LON_MIN_RAW: i32 = -1_800_000_000;
const TC_GEO_LON_MAX_RAW: i32 = 1_800_000_000;
const TC_GEO_NOT_AVAILABLE_RAW: i32 = 0x7FFF_FFFF;

/// TC-GEO DDIs (ISO 11783-10).
pub mod geo_ddi {
    use super::DDI;
    pub const ACTUAL_LATITUDE: DDI = DDI(0x0087);
    pub const ACTUAL_LONGITUDE: DDI = DDI(0x0088);
    pub const ACTUAL_ALTITUDE: DDI = DDI(0x0089);
    pub const SETPOINT_LATITUDE: DDI = DDI(0x008A);
    pub const SETPOINT_LONGITUDE: DDI = DDI(0x008B);
}

/// Timestamped WGS84 position.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GeoPoint {
    pub position: Wgs,
    pub timestamp_us: u64,
}

/// Polygon-bounded prescription zone.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PrescriptionZone {
    pub boundary: Vec<Wgs>,
    /// Optional no-application holes inside `boundary`.
    ///
    /// Points on a hole boundary are treated as outside the zone, matching the
    /// conservative "do not apply on exclusion boundaries" behavior used by
    /// [`point_in_prescription_zone`].
    pub holes: Vec<Vec<Wgs>>,
    /// Application rate (DDI-dependent units).
    pub application_rate: i32,
}

/// A named prescription map (collection of zones).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PrescriptionMap {
    pub structure_label: String,
    pub zones: Vec<PrescriptionZone>,
}

// ─── TCGEOInterface ───────────────────────────────────────────────────

/// Pump-style TC-GEO interface.
pub struct TCGEOInterface {
    maps: Vec<PrescriptionMap>,
    current_position: Option<GeoPoint>,
    last_rate: Option<i32>,

    pub on_position_update: Event<GeoPoint>,
    pub on_application_rate_changed: Event<i32>,
    pub on_prescription_map_received: Event<PrescriptionMap>,
}

impl Default for TCGEOInterface {
    fn default() -> Self {
        Self::new()
    }
}

impl TCGEOInterface {
    #[must_use]
    pub fn new() -> Self {
        Self {
            maps: Vec::new(),
            current_position: None,
            last_rate: None,
            on_position_update: Event::new(),
            on_application_rate_changed: Event::new(),
            on_prescription_map_received: Event::new(),
        }
    }

    pub fn set_position(&mut self, position: GeoPoint) {
        self.current_position = Some(position);
        self.on_position_update.emit(&position);
    }

    /// Decode a `PGN_GNSS_POSITION` (129025) message and record the
    /// position. Compatibility wrapper: malformed/unrelated frames and
    /// `0x7FFFFFFF` not-available sentinels are ignored. Use
    /// [`Self::try_handle_gnss_position`] for explicit validation errors.
    pub fn handle_gnss_position(&mut self, msg: &Message) {
        let _ = self.try_handle_gnss_position(msg);
    }

    /// Decode a `PGN_GNSS_POSITION` (129025) message and record the
    /// position, returning explicit validation errors for wrong PGNs,
    /// invalid source addresses, non-canonical lengths, or not-available /
    /// out-of-range coordinate sentinels.
    pub fn try_handle_gnss_position(&mut self, msg: &Message) -> Result<()> {
        if msg.pgn != PGN_GNSS_POSITION {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        if !msg.has_usable_source() {
            return Err(Error::invalid_address(msg.source));
        }
        if !msg.has_valid_destination_for_pgn() {
            return Err(Error::invalid_address(msg.destination));
        }
        if msg.data.len() != 8 {
            return Err(Error::invalid_data(
                "GNSS rapid position must be exactly 8 bytes",
            ));
        }
        let lat_raw = i32::from_le_bytes(msg.data[0..4].try_into().unwrap_or([0; 4]));
        let lon_raw = i32::from_le_bytes(msg.data[4..8].try_into().unwrap_or([0; 4]));
        if !valid_lat_raw(lat_raw) || !valid_lon_raw(lon_raw) {
            return Err(Error::invalid_data(
                "GNSS rapid position coordinate is not available or out of range",
            ));
        }
        let position = Wgs::new(lat_raw as f64 * 1e-7, lon_raw as f64 * 1e-7, 0.0);
        self.set_position(GeoPoint {
            position,
            timestamp_us: msg.timestamp_us,
        });
        Ok(())
    }

    /// Build the two `PGN_ECU_TO_TC` payloads (latitude then
    /// longitude) the caller should send to the TC. Returns
    /// `Err(InvalidState)` if no position is recorded yet.
    pub fn position_process_data_payloads(&self) -> Result<[[u8; 8]; 2]> {
        let pos = self
            .current_position
            .ok_or_else(|| Error::invalid_state("no position available"))?;
        let lat_fixed = encode_lat_raw(pos.position.latitude)?;
        let lon_fixed = encode_lon_raw(pos.position.longitude)?;
        let lat = encode_value_pd(geo_ddi::ACTUAL_LATITUDE, lat_fixed);
        let lon = encode_value_pd(geo_ddi::ACTUAL_LONGITUDE, lon_fixed);
        Ok([lat, lon])
    }

    pub fn add_prescription_map(&mut self, map: PrescriptionMap) {
        self.on_prescription_map_received.emit(&map);
        self.maps.push(map);
    }

    pub fn clear_prescription_maps(&mut self) {
        self.maps.clear();
        self.last_rate = None;
    }

    #[must_use]
    pub fn prescription_maps(&self) -> &[PrescriptionMap] {
        &self.maps
    }

    #[must_use]
    pub fn current_position(&self) -> Option<GeoPoint> {
        self.current_position
    }

    /// Lookup the application rate for a position. Returns the rate
    /// of the first zone that contains the point, or `None`.
    #[must_use]
    pub fn get_rate_at_position(&self, pos: Wgs) -> Option<i32> {
        for map in &self.maps {
            for zone in &map.zones {
                if point_in_prescription_zone(pos, zone) {
                    return Some(zone.application_rate);
                }
            }
        }
        None
    }

    /// Lookup the application rate for a position and convert it to the
    /// engineering unit defined by the supplied rate DDI.
    ///
    /// Returns `Err(InvalidData)` if `ddi` is unknown, is not a rate DDI, or
    /// if a matched raw rate sits outside the DDI's representable display
    /// range. Returns `Ok(None)` when no prescription zone matches.
    pub fn get_rate_at_position_engineering(&self, pos: Wgs, ddi: DDI) -> Result<Option<f64>> {
        self.get_rate_at_position(pos)
            .map(|raw| prescription_rate_to_engineering(ddi, raw))
            .transpose()
    }

    /// Lookup the application rate for a position and encode it as a
    /// `PGN_ECU_TO_TC` Process Data Value payload for `ddi`.
    ///
    /// The same DDI/range validation as
    /// [`prescription_rate_process_data_payload`] is applied before bytes are
    /// emitted.
    pub fn rate_process_data_payload_at_position(
        &self,
        pos: Wgs,
        ddi: DDI,
    ) -> Result<Option<[u8; 8]>> {
        self.get_rate_at_position(pos)
            .map(|raw| prescription_rate_process_data_payload(ddi, raw))
            .transpose()
    }

    /// Re-evaluate the rate against the current position. Fires
    /// [`Self::on_application_rate_changed`] when the rate changes.
    pub fn update(&mut self, _elapsed_ms: u32) {
        let current_rate = self
            .current_position
            .and_then(|pos| self.get_rate_at_position(pos.position));
        if self.last_rate != current_rate {
            self.last_rate = current_rate;
            let Some(rate) = current_rate else {
                return;
            };
            self.on_application_rate_changed.emit(&rate);
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────

fn encode_value_pd(ddi: DDI, value: i32) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    let ddi_raw: u16 = ddi.into();
    data[0] = ProcessDataCommands::Value.as_u8() & 0x0F;
    data[1] = 0;
    data[2] = (ddi_raw & 0xFF) as u8;
    data[3] = ((ddi_raw >> 8) & 0xFF) as u8;
    data[4..8].copy_from_slice(&value.to_le_bytes());
    data
}

/// Convert an engineering-unit prescription rate into the raw process-data
/// integer for a known ISO 11783 rate DDI.
pub fn prescription_rate_from_engineering(ddi: DDI, engineering_value: f64) -> Result<i32> {
    let (ddi_raw, resolution, min_value, max_value) = validate_rate_ddi(ddi)?;
    if !engineering_value.is_finite() {
        return Err(Error::invalid_data("prescription rate must be finite"));
    }
    if resolution <= 0.0 || !resolution.is_finite() {
        return Err(Error::invalid_data("rate DDI has invalid resolution"));
    }
    let raw = round_f64(engineering_value / resolution);
    if raw < f64::from(min_value) || raw > f64::from(max_value) {
        return Err(Error::invalid_data(format!(
            "prescription rate out of range for DDI 0x{ddi_raw:04X}"
        )));
    }
    Ok(raw as i32)
}

/// Convert a raw prescription-rate process-data value to the engineering unit
/// defined by a known ISO 11783 rate DDI.
pub fn prescription_rate_to_engineering(ddi: DDI, raw_value: i32) -> Result<f64> {
    let (ddi_raw, _resolution, min_value, max_value) = validate_rate_ddi(ddi)?;
    if raw_value < min_value || raw_value > max_value {
        return Err(Error::invalid_data(format!(
            "raw prescription rate out of range for DDI 0x{ddi_raw:04X}"
        )));
    }
    Ok(ddi_to_engineering(ddi_raw, raw_value))
}

/// Encode a raw prescription-rate value as an ECU-to-TC Process Data Value
/// payload for a known ISO 11783 rate DDI.
pub fn prescription_rate_process_data_payload(ddi: DDI, raw_value: i32) -> Result<[u8; 8]> {
    let (ddi_raw, _resolution, min_value, max_value) = validate_rate_ddi(ddi)?;
    if raw_value < min_value || raw_value > max_value {
        return Err(Error::invalid_data(format!(
            "raw prescription rate out of range for DDI 0x{ddi_raw:04X}"
        )));
    }
    Ok(encode_value_pd(ddi, raw_value))
}

fn validate_rate_ddi(ddi: DDI) -> Result<(u16, f64, i32, i32)> {
    let ddi_raw: u16 = ddi.into();
    let Some(definition) = ddi_lookup(ddi_raw) else {
        return Err(Error::invalid_data(format!(
            "unknown prescription rate DDI 0x{ddi_raw:04X}"
        )));
    };
    if !ddi_is_rate(ddi_raw) {
        return Err(Error::invalid_data(format!(
            "DDI 0x{ddi_raw:04X} is not an application rate DDI"
        )));
    }
    Ok((
        ddi_raw,
        definition.resolution,
        definition.min_value,
        definition.max_value,
    ))
}

fn encode_lat_raw(latitude: f64) -> Result<i32> {
    encode_geo_raw(latitude, TC_GEO_LAT_MIN_RAW, TC_GEO_LAT_MAX_RAW, "latitude")
}

fn encode_lon_raw(longitude: f64) -> Result<i32> {
    encode_geo_raw(
        longitude,
        TC_GEO_LON_MIN_RAW,
        TC_GEO_LON_MAX_RAW,
        "longitude",
    )
}

fn encode_geo_raw(value: f64, min_raw: i32, max_raw: i32, field: &str) -> Result<i32> {
    if !value.is_finite() {
        return Err(Error::invalid_data(format!("{field} must be finite")));
    }
    let raw = round_f64(value * 1e7);
    if raw < f64::from(min_raw) || raw > f64::from(max_raw) {
        return Err(Error::invalid_data(format!("{field} out of range")));
    }
    Ok(raw as i32)
}

fn round_f64(value: f64) -> f64 {
    if value >= 0.0 {
        ((value + 0.5) as i64) as f64
    } else {
        ((value - 0.5) as i64) as f64
    }
}

fn valid_lat_raw(raw: i32) -> bool {
    raw != TC_GEO_NOT_AVAILABLE_RAW && (TC_GEO_LAT_MIN_RAW..=TC_GEO_LAT_MAX_RAW).contains(&raw)
}

fn valid_lon_raw(raw: i32) -> bool {
    raw != TC_GEO_NOT_AVAILABLE_RAW && (TC_GEO_LON_MIN_RAW..=TC_GEO_LON_MAX_RAW).contains(&raw)
}

/// Ray-casting point-in-polygon test in WGS84 (lat/lon plane).
///
/// Boundary points are included. Degenerate polygons, non-finite coordinates,
/// and polygons with effectively zero area return `false`.
#[must_use]
pub fn point_in_polygon(point: Wgs, polygon: &[Wgs]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    if !valid_wgs(point) || polygon.iter().any(|p| !valid_wgs(*p)) {
        return false;
    }
    if !polygon_has_area(polygon) {
        return false;
    }
    if polygon_edges(polygon).any(|(a, b)| point_on_segment(point, a, b)) {
        return true;
    }
    let mut inside = false;
    for (a, b) in polygon_edges(polygon) {
        let xi = a.longitude;
        let yi = a.latitude;
        let xj = b.longitude;
        let yj = b.latitude;
        let intersect = (yi > point.latitude) != (yj > point.latitude)
            && point.longitude < (xj - xi) * (point.latitude - yi) / (yj - yi) + xi;
        if intersect {
            inside = !inside;
        }
    }
    inside
}

/// Return whether `point` is inside a prescription zone and outside every
/// exclusion hole. The outer boundary is inclusive; hole boundaries are
/// conservative exclusions.
#[must_use]
pub fn point_in_prescription_zone(point: Wgs, zone: &PrescriptionZone) -> bool {
    point_in_polygon(point, &zone.boundary)
        && !zone.holes.iter().any(|hole| point_in_polygon(point, hole))
}

const POLYGON_EPSILON: f64 = 1e-12;

fn valid_wgs(point: Wgs) -> bool {
    point.latitude.is_finite() && point.longitude.is_finite()
}

fn polygon_has_area(polygon: &[Wgs]) -> bool {
    let twice_area = polygon_edges(polygon).fold(0.0, |area, (a, b)| {
        area + a.longitude * b.latitude - b.longitude * a.latitude
    });
    twice_area.abs() > POLYGON_EPSILON
}

fn polygon_edges(polygon: &[Wgs]) -> impl Iterator<Item = (Wgs, Wgs)> + '_ {
    polygon
        .iter()
        .copied()
        .zip(polygon.iter().copied().cycle().skip(1))
        .take(polygon.len())
}

fn point_on_segment(point: Wgs, a: Wgs, b: Wgs) -> bool {
    let px = point.longitude;
    let py = point.latitude;
    let ax = a.longitude;
    let ay = a.latitude;
    let bx = b.longitude;
    let by = b.latitude;
    let cross = (px - ax) * (by - ay) - (py - ay) * (bx - ax);
    if cross.abs() > POLYGON_EPSILON {
        return false;
    }
    let min_x = ax.min(bx) - POLYGON_EPSILON;
    let max_x = ax.max(bx) + POLYGON_EPSILON;
    let min_y = ay.min(by) - POLYGON_EPSILON;
    let max_y = ay.max(by) + POLYGON_EPSILON;
    px >= min_x && px <= max_x && py >= min_y && py <= max_y
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::tc::ddi_database::ddi;
    use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
    use crate::net::pgn_defs::PGN_GNSS_POSITION;
    use proptest::prelude::*;

    fn wgs(lat: f64, lon: f64) -> Wgs {
        Wgs::new(lat, lon, 0.0)
    }

    #[test]
    fn set_position_fires_event() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut tc = TCGEOInterface::new();
        let log: Rc<RefCell<Vec<GeoPoint>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        tc.on_position_update
            .subscribe(move |&p| lc.borrow_mut().push(p));
        tc.set_position(GeoPoint {
            position: wgs(52.0, 5.0),
            timestamp_us: 1234,
        });
        assert_eq!(log.borrow().len(), 1);
    }

    #[test]
    fn handle_gnss_decodes_lat_lon() {
        let mut tc = TCGEOInterface::new();
        let lat_fixed: i32 = 520_000_000; // 52.0
        let lon_fixed: i32 = 50_000_000; // 5.0
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&lat_fixed.to_le_bytes());
        data.extend_from_slice(&lon_fixed.to_le_bytes());
        let msg = Message::new(PGN_GNSS_POSITION, data, 0x10);
        tc.handle_gnss_position(&msg);
        let pos = tc.current_position().unwrap();
        assert!((pos.position.latitude - 52.0).abs() < 1e-6);
        assert!((pos.position.longitude - 5.0).abs() < 1e-6);
    }

    #[test]
    fn handle_gnss_requires_expected_pgn_and_valid_source_address() {
        let lat_fixed: i32 = 520_000_000;
        let lon_fixed: i32 = 50_000_000;
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&lat_fixed.to_le_bytes());
        data.extend_from_slice(&lon_fixed.to_le_bytes());

        let mut wrong_pgn = Message::new(PGN_GNSS_POSITION + 1, data.clone(), 0x10);
        let mut tc = TCGEOInterface::new();
        tc.handle_gnss_position(&wrong_pgn);
        assert!(tc.current_position().is_none());

        for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut tc = TCGEOInterface::new();
            wrong_pgn = Message::new(PGN_GNSS_POSITION, data.clone(), bad_source);
            tc.handle_gnss_position(&wrong_pgn);
            assert!(
                tc.current_position().is_none(),
                "TC-GEO must ignore GNSS position from invalid source 0x{bad_source:02X}"
            );
        }
    }

    #[test]
    fn try_handle_gnss_reports_validation_errors_without_cache_mutation() {
        let lat_fixed: i32 = 520_000_000;
        let lon_fixed: i32 = 50_000_000;
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&lat_fixed.to_le_bytes());
        data.extend_from_slice(&lon_fixed.to_le_bytes());

        let mut tc = TCGEOInterface::new();
        let err = tc
            .try_handle_gnss_position(&Message::new(PGN_GNSS_POSITION + 1, data.clone(), 0x10))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidPgn);
        assert!(tc.current_position().is_none());

        let err = tc
            .try_handle_gnss_position(&Message::new(PGN_GNSS_POSITION, data.clone(), NULL_ADDRESS))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidAddress);
        assert!(tc.current_position().is_none());

        let err = tc
            .try_handle_gnss_position(&Message::new(PGN_GNSS_POSITION, vec![0xFF; 7], 0x10))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("exactly 8 bytes"));
        assert!(tc.current_position().is_none());

        let mut unavailable = Vec::with_capacity(8);
        unavailable.extend_from_slice(&TC_GEO_NOT_AVAILABLE_RAW.to_le_bytes());
        unavailable.extend_from_slice(&lon_fixed.to_le_bytes());
        let err = tc
            .try_handle_gnss_position(&Message::new(PGN_GNSS_POSITION, unavailable, 0x10))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("not available"));
        assert!(tc.current_position().is_none());
    }

    #[test]
    fn handle_gnss_skips_not_available_sentinel() {
        let mut tc = TCGEOInterface::new();
        let mut data = Vec::with_capacity(8);
        data.extend_from_slice(&TC_GEO_NOT_AVAILABLE_RAW.to_le_bytes());
        data.extend_from_slice(&0_i32.to_le_bytes());
        let msg = Message::new(PGN_GNSS_POSITION, data, 0x10);
        tc.handle_gnss_position(&msg);
        assert!(tc.current_position().is_none());
    }

    #[test]
    fn handle_gnss_rejects_out_of_range_coordinates_without_overwriting_position() {
        let mut tc = TCGEOInterface::new();
        tc.set_position(GeoPoint {
            position: wgs(52.0, 5.0),
            timestamp_us: 1,
        });

        let mut impossible_lat = Vec::with_capacity(8);
        impossible_lat.extend_from_slice(&(TC_GEO_LAT_MAX_RAW + 1).to_le_bytes());
        impossible_lat.extend_from_slice(&0_i32.to_le_bytes());
        tc.handle_gnss_position(&Message::new(PGN_GNSS_POSITION, impossible_lat, 0x10));
        let pos = tc.current_position().unwrap();
        assert_eq!(pos.timestamp_us, 1);
        assert!((pos.position.latitude - 52.0).abs() < 1e-6);

        let mut impossible_lon = Vec::with_capacity(8);
        impossible_lon.extend_from_slice(&0_i32.to_le_bytes());
        impossible_lon.extend_from_slice(&(TC_GEO_LON_MIN_RAW - 1).to_le_bytes());
        tc.handle_gnss_position(&Message::new(PGN_GNSS_POSITION, impossible_lon, 0x10));
        let pos = tc.current_position().unwrap();
        assert_eq!(pos.timestamp_us, 1);
        assert!((pos.position.longitude - 5.0).abs() < 1e-6);
    }

    #[test]
    fn handle_gnss_rejects_prefix_compatible_overlong_payload() {
        let mut tc = TCGEOInterface::new();
        let lat_fixed: i32 = 520_000_000;
        let lon_fixed: i32 = 50_000_000;
        let mut data = Vec::with_capacity(9);
        data.extend_from_slice(&lat_fixed.to_le_bytes());
        data.extend_from_slice(&lon_fixed.to_le_bytes());
        data.push(0xFF);
        let msg = Message::new(PGN_GNSS_POSITION, data, 0x10);
        tc.handle_gnss_position(&msg);
        assert!(tc.current_position().is_none());
    }

    #[test]
    fn position_payloads_no_position_errors() {
        let tc = TCGEOInterface::new();
        assert!(tc.position_process_data_payloads().is_err());
    }

    #[test]
    fn position_payloads_round_trip_ddis() {
        let mut tc = TCGEOInterface::new();
        tc.set_position(GeoPoint {
            position: wgs(52.123, 5.456),
            timestamp_us: 0,
        });
        let [lat, lon] = tc.position_process_data_payloads().unwrap();
        let lat_ddi = (lat[2] as u16) | ((lat[3] as u16) << 8);
        let lon_ddi = (lon[2] as u16) | ((lon[3] as u16) << 8);
        assert_eq!(lat_ddi, geo_ddi::ACTUAL_LATITUDE);
        assert_eq!(lon_ddi, geo_ddi::ACTUAL_LONGITUDE);
        let lat_val = i32::from_le_bytes(lat[4..8].try_into().unwrap());
        assert_eq!(lat_val, 521_230_000);
    }

    #[test]
    fn position_payloads_reject_non_finite_or_out_of_range_coordinates() {
        let mut tc = TCGEOInterface::new();
        tc.set_position(GeoPoint {
            position: wgs(f64::NAN, 5.0),
            timestamp_us: 0,
        });
        assert!(tc.position_process_data_payloads().is_err());

        tc.set_position(GeoPoint {
            position: wgs(90.000_000_1, 5.0),
            timestamp_us: 0,
        });
        assert!(tc.position_process_data_payloads().is_err());

        tc.set_position(GeoPoint {
            position: wgs(52.0, -180.000_000_1),
            timestamp_us: 0,
        });
        assert!(tc.position_process_data_payloads().is_err());
    }

    #[test]
    fn point_in_polygon_simple_square() {
        let square = vec![wgs(0.0, 0.0), wgs(0.0, 1.0), wgs(1.0, 1.0), wgs(1.0, 0.0)];
        assert!(point_in_polygon(wgs(0.5, 0.5), &square));
        assert!(point_in_polygon(wgs(0.0, 0.5), &square));
        assert!(point_in_polygon(wgs(0.0, 0.0), &square));
        assert!(!point_in_polygon(wgs(2.0, 0.5), &square));
        assert!(!point_in_polygon(wgs(0.5, -0.1), &square));
    }

    #[test]
    fn point_in_polygon_rejects_degenerate() {
        let two_pts = vec![wgs(0.0, 0.0), wgs(1.0, 1.0)];
        assert!(!point_in_polygon(wgs(0.5, 0.5), &two_pts));
        let collinear = vec![wgs(0.0, 0.0), wgs(0.5, 0.5), wgs(1.0, 1.0)];
        assert!(!point_in_polygon(wgs(0.5, 0.5), &collinear));
        let non_finite = vec![
            wgs(0.0, 0.0),
            wgs(0.0, 1.0),
            wgs(f64::NAN, 1.0),
            wgs(1.0, 0.0),
        ];
        assert!(!point_in_polygon(wgs(0.5, 0.5), &non_finite));
    }

    #[test]
    fn rate_lookup_walks_zones() {
        let mut tc = TCGEOInterface::new();
        tc.add_prescription_map(PrescriptionMap {
            structure_label: "Z1".to_string(),
            zones: vec![
                PrescriptionZone {
                    boundary: vec![wgs(0.0, 0.0), wgs(0.0, 1.0), wgs(1.0, 1.0), wgs(1.0, 0.0)],
                    holes: Vec::new(),
                    application_rate: 100,
                },
                PrescriptionZone {
                    boundary: vec![wgs(2.0, 2.0), wgs(2.0, 3.0), wgs(3.0, 3.0), wgs(3.0, 2.0)],
                    holes: Vec::new(),
                    application_rate: 200,
                },
            ],
        });
        assert_eq!(tc.get_rate_at_position(wgs(0.5, 0.5)), Some(100));
        assert_eq!(tc.get_rate_at_position(wgs(2.5, 2.5)), Some(200));
        assert_eq!(tc.get_rate_at_position(wgs(10.0, 10.0)), None);
    }

    #[test]
    fn prescription_rate_uses_ddi_resolution_and_range() {
        let volume_per_area = DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE);
        assert_eq!(
            prescription_rate_from_engineering(volume_per_area, 12.34).unwrap(),
            1234
        );
        assert_eq!(
            prescription_rate_process_data_payload(volume_per_area, 1234).unwrap(),
            [0x03, 0x00, 0x01, 0x00, 0xD2, 0x04, 0x00, 0x00]
        );
        assert!(
            (prescription_rate_to_engineering(volume_per_area, 1234).unwrap() - 12.34).abs() < 1e-9
        );

        let mass_per_area = DDI(ddi::ACTUAL_MASS_PER_AREA_APPLICATION_RATE);
        assert_eq!(
            prescription_rate_from_engineering(mass_per_area, 250.0).unwrap(),
            250
        );
        assert_eq!(
            prescription_rate_to_engineering(mass_per_area, 250).unwrap(),
            250.0
        );
    }

    #[test]
    fn prescription_rate_rejects_unknown_non_rate_and_out_of_range_ddis() {
        assert!(prescription_rate_from_engineering(geo_ddi::ACTUAL_LATITUDE, 1.0).is_err());
        assert!(prescription_rate_from_engineering(DDI(0xFFFF), 1.0).is_err());
        assert!(
            prescription_rate_from_engineering(
                DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE),
                f64::NAN,
            )
            .is_err()
        );
        assert!(
            prescription_rate_from_engineering(
                DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE),
                1.0e20,
            )
            .is_err()
        );
        assert!(
            prescription_rate_process_data_payload(
                DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE),
                -1,
            )
            .is_err()
        );
    }

    #[test]
    fn rate_lookup_can_return_engineering_values_and_process_data_payloads() {
        let mut tc = TCGEOInterface::new();
        tc.add_prescription_map(PrescriptionMap {
            structure_label: "rates".to_string(),
            zones: vec![PrescriptionZone {
                boundary: vec![wgs(0.0, 0.0), wgs(0.0, 1.0), wgs(1.0, 1.0), wgs(1.0, 0.0)],
                holes: Vec::new(),
                application_rate: 100,
            }],
        });
        let ddi = DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE);
        assert_eq!(
            tc.get_rate_at_position_engineering(wgs(0.5, 0.5), ddi)
                .unwrap(),
            Some(1.0)
        );
        assert_eq!(
            tc.rate_process_data_payload_at_position(wgs(0.5, 0.5), ddi)
                .unwrap(),
            Some([0x03, 0x00, 0x01, 0x00, 0x64, 0x00, 0x00, 0x00])
        );
        assert_eq!(
            tc.get_rate_at_position_engineering(wgs(2.0, 2.0), ddi)
                .unwrap(),
            None
        );
        assert!(
            tc.rate_process_data_payload_at_position(wgs(0.5, 0.5), geo_ddi::ACTUAL_LATITUDE)
                .is_err()
        );
    }

    #[test]
    fn update_emits_on_rate_change() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut tc = TCGEOInterface::new();
        tc.add_prescription_map(PrescriptionMap {
            structure_label: "Z".to_string(),
            zones: vec![PrescriptionZone {
                boundary: vec![wgs(0.0, 0.0), wgs(0.0, 1.0), wgs(1.0, 1.0), wgs(1.0, 0.0)],
                holes: Vec::new(),
                application_rate: 42,
            }],
        });
        tc.set_position(GeoPoint {
            position: wgs(0.5, 0.5),
            timestamp_us: 0,
        });
        let log: Rc<RefCell<Vec<i32>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        tc.on_application_rate_changed
            .subscribe(move |&r| lc.borrow_mut().push(r));
        tc.update(0);
        tc.update(0); // second call — same rate, should not re-fire
        assert_eq!(*log.borrow(), vec![42]);
    }

    #[test]
    fn rate_lookup_handles_overlap_holes_boundaries_and_no_fix() {
        let mut tc = TCGEOInterface::new();
        tc.add_prescription_map(PrescriptionMap {
            structure_label: "edge-cases".to_string(),
            zones: vec![
                PrescriptionZone {
                    boundary: vec![wgs(0.0, 0.0), wgs(0.0, 2.0), wgs(2.0, 2.0), wgs(2.0, 0.0)],
                    holes: vec![vec![
                        wgs(0.8, 0.8),
                        wgs(0.8, 1.2),
                        wgs(1.2, 1.2),
                        wgs(1.2, 0.8),
                    ]],
                    application_rate: 100,
                },
                PrescriptionZone {
                    boundary: vec![wgs(1.0, 1.0), wgs(1.0, 3.0), wgs(3.0, 3.0), wgs(3.0, 1.0)],
                    holes: Vec::new(),
                    application_rate: 200,
                },
            ],
        });

        assert_eq!(tc.get_rate_at_position(wgs(0.5, 0.5)), Some(100));
        assert_eq!(tc.get_rate_at_position(wgs(0.0, 1.0)), Some(100));
        assert_eq!(
            tc.get_rate_at_position(wgs(1.5, 1.5)),
            Some(100),
            "overlaps resolve deterministically to the first matching zone"
        );
        assert_eq!(tc.get_rate_at_position(wgs(1.0, 1.0)), Some(200));
        assert_eq!(tc.get_rate_at_position(wgs(0.9, 0.9)), None);
        assert_eq!(tc.get_rate_at_position(wgs(5.0, 5.0)), None);
        assert!(tc.position_process_data_payloads().is_err());
    }

    proptest! {
        #[test]
        fn proptest_gnss_position_ingress_accepts_or_rejects_arbitrary_payloads_without_bad_cache(
            messages in proptest::collection::vec(
                (proptest::collection::vec(any::<u8>(), 0..=12), any::<u64>()),
                0..=128,
            ),
        ) {
            let mut tc = TCGEOInterface::new();
            for (data, timestamp_us) in messages {
                let mut msg = Message::new(PGN_GNSS_POSITION, data, 0x10);
                msg.timestamp_us = timestamp_us;
                tc.handle_gnss_position(&msg);
                if let Some(position) = tc.current_position() {
                    prop_assert!(position.position.latitude.is_finite());
                    prop_assert!(position.position.longitude.is_finite());
                    prop_assert!((-90.0..=90.0).contains(&position.position.latitude));
                    prop_assert!((-180.0..=180.0).contains(&position.position.longitude));
                }
            }
        }
    }
}
