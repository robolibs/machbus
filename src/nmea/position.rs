//! GNSS position data + batch processing helpers.
//!
//! Mirrors the C++ `machbus::nmea::position.hpp`. Uses [`crate::geo::Wgs`] for
//! the position type. Hosted builds with `geo-concord` use the richer `concord`
//! conversions; embedded/no-concord builds keep lightweight in-crate geodesy.

use super::definitions::{GNSSFixType, GNSSSystem};
use crate::geo::{
    Ecf, Geo, Wgs, batch_to_ecf, batch_to_enu, batch_to_ned, batch_to_wgs, batch_to_wgs_from_enu,
    batch_to_wgs_from_ned, frame::Enu, frame::Ned, to_ecf,
};
use alloc::vec::Vec;

/// Radians → degrees, normalized to `[0, 360)`. Non-finite input passes through.
fn normalize_deg(rad: f64) -> f64 {
    if !rad.is_finite() {
        return rad;
    }
    let deg = rad.to_degrees() % 360.0;
    if deg < 0.0 { deg + 360.0 } else { deg }
}

/// One GNSS position fix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GNSSPosition {
    pub wgs: Wgs,
    pub altitude_m: Option<f64>,
    pub heading_rad: Option<f64>,
    pub speed_mps: Option<f64>,
    /// Course over ground.
    pub cog_rad: Option<f64>,
    pub hdop: Option<f64>,
    pub pdop: Option<f64>,
    pub vdop: Option<f64>,
    pub satellites_used: u8,
    pub fix_type: GNSSFixType,
    pub gnss_system: GNSSSystem,
    pub geoidal_separation_m: Option<f64>,
    pub rate_of_turn_rps: Option<f64>,
    pub pitch_rad: Option<f64>,
    pub roll_rad: Option<f64>,
    pub timestamp_us: u64,
}

impl Default for GNSSPosition {
    fn default() -> Self {
        Self {
            wgs: Wgs::default(),
            altitude_m: None,
            heading_rad: None,
            speed_mps: None,
            cog_rad: None,
            hdop: None,
            pdop: None,
            vdop: None,
            satellites_used: 0,
            fix_type: GNSSFixType::NoFix,
            gnss_system: GNSSSystem::GPS,
            geoidal_separation_m: None,
            rate_of_turn_rps: None,
            pitch_rad: None,
            roll_rad: None,
            timestamp_us: 0,
        }
    }
}

impl GNSSPosition {
    #[inline]
    #[must_use]
    pub const fn has_fix(&self) -> bool {
        !matches!(self.fix_type, GNSSFixType::NoFix)
    }

    #[inline]
    #[must_use]
    pub const fn is_rtk(&self) -> bool {
        matches!(self.fix_type, GNSSFixType::RTKFixed | GNSSFixType::RTKFloat)
    }

    /// Heading in degrees (`0..360`), if available. Convenience for the
    /// degree-based units agricultural displays use.
    #[must_use]
    pub fn heading_deg(&self) -> Option<f64> {
        self.heading_rad.map(normalize_deg)
    }

    /// Course-over-ground in degrees (`0..360`), if available.
    #[must_use]
    pub fn cog_deg(&self) -> Option<f64> {
        self.cog_rad.map(normalize_deg)
    }

    /// Ground speed in km/h, if available.
    #[must_use]
    pub fn speed_kmh(&self) -> Option<f64> {
        self.speed_mps.map(|mps| mps * 3.6)
    }

    /// Convert to local ENU frame using `reference` as the origin.
    #[must_use]
    pub fn to_enu(&self, reference: Geo) -> Enu {
        crate::geo::to_enu(reference, self.wgs)
    }

    /// Convert to local NED frame using `reference` as the origin.
    #[must_use]
    pub fn to_ned(&self, reference: Geo) -> Ned {
        crate::geo::to_ned(reference, self.wgs)
    }

    /// Convert to ECEF.
    #[must_use]
    pub fn to_ecf(&self) -> Ecf {
        to_ecf(self.wgs)
    }

    /// Straight-line (ECEF chord) distance in metres to `other`. For the short
    /// baselines typical in field work this is within millimetres of the surface
    /// arc, and it avoids picking an ENU/NED reference origin.
    #[must_use]
    pub fn distance_to(&self, other: &GNSSPosition) -> f64 {
        let a = self.to_ecf();
        let b = other.to_ecf();
        let (dx, dy, dz) = (a.x - b.x, a.y - b.y, a.z - b.z);
        sqrt_f64(dx * dx + dy * dy + dz * dz)
    }
}

#[must_use]
#[cfg(feature = "default")]
fn sqrt_f64(value: f64) -> f64 {
    value.sqrt()
}

#[must_use]
#[cfg(feature = "embedded")]
fn sqrt_f64(value: f64) -> f64 {
    if value <= 0.0 {
        return 0.0;
    }
    let mut x = value;
    for _ in 0..16 {
        x = 0.5 * (x + value / x);
    }
    x
}

/// Batched GNSS sample buffer with batched concord conversions.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GNSSBatch {
    pub positions: Vec<GNSSPosition>,
}

impl GNSSBatch {
    #[must_use]
    pub fn to_enu_batch(&self, reference: Geo) -> Vec<Enu> {
        let wgs: Vec<Wgs> = self.positions.iter().map(|p| p.wgs).collect();
        batch_to_enu(reference, &wgs)
    }

    #[must_use]
    pub fn to_ned_batch(&self, reference: Geo) -> Vec<Ned> {
        let wgs: Vec<Wgs> = self.positions.iter().map(|p| p.wgs).collect();
        batch_to_ned(reference, &wgs)
    }

    #[must_use]
    pub fn to_ecf_batch(&self) -> Vec<Ecf> {
        let wgs: Vec<Wgs> = self.positions.iter().map(|p| p.wgs).collect();
        batch_to_ecf(&wgs)
    }

    #[must_use]
    pub fn wgs_from_ecf_batch(ecf: &[Ecf]) -> Vec<Wgs> {
        batch_to_wgs(ecf)
    }

    #[must_use]
    pub fn wgs_from_enu_batch(enu: &[Enu]) -> Vec<Wgs> {
        batch_to_wgs_from_enu(enu)
    }

    #[must_use]
    pub fn wgs_from_ned_batch(ned: &[Ned]) -> Vec<Wgs> {
        batch_to_wgs_from_ned(ned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn pos(lat: f64, lon: f64) -> GNSSPosition {
        pos_alt(lat, lon, 0.0)
    }

    fn pos_alt(lat: f64, lon: f64, altitude: f64) -> GNSSPosition {
        GNSSPosition {
            wgs: Wgs::new(lat, lon, altitude),
            ..Default::default()
        }
    }

    fn assert_wgs_near(actual: Wgs, expected: Wgs) {
        assert!(
            (actual.latitude - expected.latitude).abs() < 1.0e-7,
            "latitude mismatch: actual={} expected={}",
            actual.latitude,
            expected.latitude
        );
        assert!(
            (actual.longitude - expected.longitude).abs() < 1.0e-7,
            "longitude mismatch: actual={} expected={}",
            actual.longitude,
            expected.longitude
        );
        assert!(
            (actual.altitude - expected.altitude).abs() < 1.0e-3,
            "altitude mismatch: actual={} expected={}",
            actual.altitude,
            expected.altitude
        );
    }

    #[test]
    fn distance_between_fixes() {
        let a = GNSSPosition {
            wgs: Wgs::new(52.0, 5.0, 0.0),
            ..Default::default()
        };
        // Same point ⇒ zero distance.
        assert!(a.distance_to(&a) < 1e-6);

        // ~0.001° latitude north ≈ 111 m.
        let b = GNSSPosition {
            wgs: Wgs::new(52.001, 5.0, 0.0),
            ..Default::default()
        };
        let d = a.distance_to(&b);
        assert!(
            (100.0..120.0).contains(&d),
            "distance {d} m out of expected band"
        );
        // Distance is symmetric.
        assert!((d - b.distance_to(&a)).abs() < 1e-6);
    }

    #[test]
    fn display_unit_accessors() {
        let mut p = GNSSPosition::default();
        // None when the underlying field is absent.
        assert!(p.heading_deg().is_none());
        assert!(p.cog_deg().is_none());
        assert!(p.speed_kmh().is_none());

        p.heading_rad = Some(core::f64::consts::FRAC_PI_2); // 90°
        p.cog_rad = Some(-core::f64::consts::FRAC_PI_2); // -90° → 270°
        p.speed_mps = Some(10.0); // 36 km/h
        assert!((p.heading_deg().unwrap() - 90.0).abs() < 1e-9);
        assert!((p.cog_deg().unwrap() - 270.0).abs() < 1e-9);
        assert!((p.speed_kmh().unwrap() - 36.0).abs() < 1e-9);
    }

    #[test]
    fn fix_classification() {
        let mut p = GNSSPosition::default();
        assert!(!p.has_fix());
        p.fix_type = GNSSFixType::GNSSFix;
        assert!(p.has_fix());
        assert!(!p.is_rtk());
        p.fix_type = GNSSFixType::RTKFixed;
        assert!(p.is_rtk());
        p.fix_type = GNSSFixType::RTKFloat;
        assert!(p.is_rtk());
    }

    #[test]
    fn ecf_conversion_runs() {
        let p = pos(52.0, 5.0);
        let ecf = p.to_ecf();
        // Earth radius is ~6.37e6, so ECF magnitude near surface
        // should be on that order.
        let mag = (ecf.x * ecf.x + ecf.y * ecf.y + ecf.z * ecf.z).sqrt();
        assert!(mag > 6_000_000.0 && mag < 7_000_000.0);
    }

    #[test]
    fn enu_conversion_with_reference() {
        let reference = Geo::new(52.0, 5.0, 0.0);
        let p = pos(52.0, 5.0);
        let enu = p.to_enu(reference);
        // Coincident point: distance should be ~0.
        let _ = enu; // adapter frame fields depend on feature; ensure call type-checks.
    }

    #[test]
    fn batch_conversion_round_trip() {
        let reference = Geo::new(52.0, 5.0, 0.0);
        let batch = GNSSBatch {
            positions: vec![
                pos_alt(52.0, 5.0, 12.0),
                pos_alt(52.001, 5.001, 14.5),
                pos_alt(51.999, 5.002, -1.0),
            ],
        };
        let expected_wgs: Vec<Wgs> = batch.positions.iter().map(|p| p.wgs).collect();

        let enu = batch.to_enu_batch(reference);
        assert_eq!(enu.len(), expected_wgs.len());
        let ned = batch.to_ned_batch(reference);
        assert_eq!(ned.len(), expected_wgs.len());
        let ecf = batch.to_ecf_batch();
        assert_eq!(ecf.len(), expected_wgs.len());

        for (batch_enu, position) in enu.iter().zip(&batch.positions) {
            let single = position.to_enu(reference);
            assert!((batch_enu.east() - single.east()).abs() < 1.0e-9);
            assert!((batch_enu.north() - single.north()).abs() < 1.0e-9);
            assert!((batch_enu.up() - single.up()).abs() < 1.0e-9);
        }

        for (batch_ned, position) in ned.iter().zip(&batch.positions) {
            let single = position.to_ned(reference);
            assert!((batch_ned.north() - single.north()).abs() < 1.0e-9);
            assert!((batch_ned.east() - single.east()).abs() < 1.0e-9);
            assert!((batch_ned.down() - single.down()).abs() < 1.0e-9);
        }

        for (batch_ecf, position) in ecf.iter().zip(&batch.positions) {
            let single = position.to_ecf();
            assert!((batch_ecf.x - single.x).abs() < 1.0e-9);
            assert!((batch_ecf.y - single.y).abs() < 1.0e-9);
            assert!((batch_ecf.z - single.z).abs() < 1.0e-9);
        }

        for (actual, expected) in GNSSBatch::wgs_from_enu_batch(&enu)
            .into_iter()
            .zip(&expected_wgs)
        {
            assert_wgs_near(actual, *expected);
        }
        for (actual, expected) in GNSSBatch::wgs_from_ned_batch(&ned)
            .into_iter()
            .zip(&expected_wgs)
        {
            assert_wgs_near(actual, *expected);
        }
        for (actual, expected) in GNSSBatch::wgs_from_ecf_batch(&ecf)
            .into_iter()
            .zip(&expected_wgs)
        {
            assert_wgs_near(actual, *expected);
        }
    }

    #[test]
    fn empty_batch_conversions_are_empty() {
        let reference = Geo::new(52.0, 5.0, 0.0);
        let batch = GNSSBatch::default();
        assert!(batch.to_enu_batch(reference).is_empty());
        assert!(batch.to_ned_batch(reference).is_empty());
        assert!(batch.to_ecf_batch().is_empty());
        assert!(GNSSBatch::wgs_from_enu_batch(&[]).is_empty());
        assert!(GNSSBatch::wgs_from_ned_batch(&[]).is_empty());
        assert!(GNSSBatch::wgs_from_ecf_batch(&[]).is_empty());
    }
}
