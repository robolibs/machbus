//! Lightweight protocol-facing geodesy types.
//!
//! `machbus` owns the WGS84/ECEF/local-frame data shapes used by protocol
//! structs. Hosted builds with `geo-concord` delegate calculations to
//! `concord`; embedded builds keep dependency-free fallbacks so the public data
//! model does not pull robotics geometry into `no_std`.

use alloc::vec::Vec;

#[cfg(feature = "embedded")]
const EARTH_A_M: f64 = 6_378_137.0;

/// WGS84 latitude/longitude/altitude in degrees and metres.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Wgs {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: f64,
}

impl Wgs {
    #[must_use]
    pub const fn new(latitude: f64, longitude: f64, altitude: f64) -> Self {
        Self {
            latitude,
            longitude,
            altitude,
        }
    }
}

/// Geographic reference point. Kept as an alias because `concord` uses the same
/// representation for `Geo` and `Wgs`.
pub type Geo = Wgs;

/// Earth-centered, Earth-fixed Cartesian point in metres.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Ecf {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Ecf {
    #[must_use]
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

pub mod frame {
    use super::Geo;

    /// East/north/up local tangent point in metres.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Enu {
        east: f64,
        north: f64,
        up: f64,
        origin: Geo,
    }

    impl Enu {
        #[must_use]
        pub const fn new(east: f64, north: f64, up: f64, origin: Geo) -> Self {
            Self {
                east,
                north,
                up,
                origin,
            }
        }

        #[must_use]
        pub const fn east(self) -> f64 {
            self.east
        }

        #[must_use]
        pub const fn north(self) -> f64 {
            self.north
        }

        #[must_use]
        pub const fn up(self) -> f64 {
            self.up
        }

        #[must_use]
        pub const fn x(self) -> f64 {
            self.east
        }

        #[must_use]
        pub const fn y(self) -> f64 {
            self.north
        }

        #[must_use]
        pub const fn z(self) -> f64 {
            self.up
        }

        #[must_use]
        pub const fn ref_origin(self) -> Geo {
            self.origin
        }
    }

    /// North/east/down local tangent point in metres.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct Ned {
        north: f64,
        east: f64,
        down: f64,
        origin: Geo,
    }

    impl Ned {
        #[must_use]
        pub const fn new(north: f64, east: f64, down: f64, origin: Geo) -> Self {
            Self {
                north,
                east,
                down,
                origin,
            }
        }

        #[must_use]
        pub const fn north(self) -> f64 {
            self.north
        }

        #[must_use]
        pub const fn east(self) -> f64 {
            self.east
        }

        #[must_use]
        pub const fn down(self) -> f64 {
            self.down
        }

        #[must_use]
        pub const fn x(self) -> f64 {
            self.north
        }

        #[must_use]
        pub const fn y(self) -> f64 {
            self.east
        }

        #[must_use]
        pub const fn z(self) -> f64 {
            self.down
        }

        #[must_use]
        pub const fn ref_origin(self) -> Geo {
            self.origin
        }
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<Wgs> for concord::Wgs {
    fn from(value: Wgs) -> Self {
        Self::new(value.latitude, value.longitude, value.altitude)
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<concord::Wgs> for Wgs {
    fn from(value: concord::Wgs) -> Self {
        Self::new(value.latitude, value.longitude, value.altitude)
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<Ecf> for concord::Ecf {
    fn from(value: Ecf) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<concord::Ecf> for Ecf {
    fn from(value: concord::Ecf) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<frame::Enu> for concord::frame::Enu {
    fn from(value: frame::Enu) -> Self {
        Self::new(
            value.east(),
            value.north(),
            value.up(),
            value.ref_origin().into(),
        )
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<concord::frame::Enu> for frame::Enu {
    fn from(value: concord::frame::Enu) -> Self {
        Self::new(
            value.east(),
            value.north(),
            value.up(),
            value.ref_origin().into(),
        )
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<frame::Ned> for concord::frame::Ned {
    fn from(value: frame::Ned) -> Self {
        Self::new(
            value.north(),
            value.east(),
            value.down(),
            value.ref_origin().into(),
        )
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
impl From<concord::frame::Ned> for frame::Ned {
    fn from(value: concord::frame::Ned) -> Self {
        Self::new(
            value.north(),
            value.east(),
            value.down(),
            value.ref_origin().into(),
        )
    }
}

#[must_use]
#[cfg(any(feature = "default", feature = "cli"))]
pub fn to_ecf(wgs: Wgs) -> Ecf {
    concord::to_ecf(wgs.into()).into()
}

#[must_use]
#[cfg(feature = "embedded")]
pub fn to_ecf(wgs: Wgs) -> Ecf {
    // Embedded fallback intentionally avoids libm. It is a stable,
    // dependency-free placeholder for protocol code that only needs a
    // Cartesian-shaped value; richer conversions require `geo-concord`.
    //
    // NOTE the units do not match the hosted implementation: `x` is metres
    // while `y` and `z` are the raw degrees. Anything that treats the result as
    // a metric Cartesian triple — differencing two of them for a distance, for
    // instance — is wrong by orders of magnitude. `GNSSPosition::distance_to`
    // is therefore not compiled on this profile.
    Ecf::new(EARTH_A_M + wgs.altitude, wgs.latitude, wgs.longitude)
}

#[must_use]
#[cfg(any(feature = "default", feature = "cli"))]
pub fn to_wgs(ecf: Ecf) -> Wgs {
    concord::to_wgs(ecf.into()).into()
}

#[must_use]
#[cfg(feature = "embedded")]
pub fn to_wgs(ecf: Ecf) -> Wgs {
    Wgs::new(ecf.y, ecf.z, ecf.x - EARTH_A_M)
}

#[must_use]
#[cfg(any(feature = "default", feature = "cli"))]
pub fn to_enu(origin: Geo, wgs: Wgs) -> frame::Enu {
    concord::to_enu(origin.into(), wgs.into()).into()
}

#[must_use]
#[cfg(any(feature = "default", feature = "cli"))]
pub fn to_ned(origin: Geo, wgs: Wgs) -> frame::Ned {
    concord::to_ned(origin.into(), wgs.into()).into()
}

#[must_use]
#[cfg(feature = "embedded")]
fn metres_per_degree(_origin: Geo) -> (f64, f64) {
    // Dependency-free approximation for `no_std` builds without libm.
    (111_320.0, 111_320.0)
}

#[must_use]
#[cfg(feature = "embedded")]
pub fn to_enu(origin: Geo, wgs: Wgs) -> frame::Enu {
    let (metres_per_deg_lat, metres_per_deg_lon) = metres_per_degree(origin);
    frame::Enu::new(
        (wgs.longitude - origin.longitude) * metres_per_deg_lon,
        (wgs.latitude - origin.latitude) * metres_per_deg_lat,
        wgs.altitude - origin.altitude,
        origin,
    )
}

#[must_use]
#[cfg(feature = "embedded")]
pub fn to_ned(origin: Geo, wgs: Wgs) -> frame::Ned {
    let enu = to_enu(origin, wgs);
    frame::Ned::new(enu.north(), enu.east(), -enu.up(), origin)
}

#[must_use]
pub fn batch_to_ecf(wgs_coords: &[Wgs]) -> Vec<Ecf> {
    wgs_coords.iter().copied().map(to_ecf).collect()
}

#[must_use]
pub fn batch_to_wgs(ecf_coords: &[Ecf]) -> Vec<Wgs> {
    ecf_coords.iter().copied().map(to_wgs).collect()
}

#[must_use]
pub fn batch_to_enu(origin: Geo, wgs_coords: &[Wgs]) -> Vec<frame::Enu> {
    wgs_coords
        .iter()
        .copied()
        .map(|wgs| to_enu(origin, wgs))
        .collect()
}

#[must_use]
pub fn batch_to_ned(origin: Geo, wgs_coords: &[Wgs]) -> Vec<frame::Ned> {
    wgs_coords
        .iter()
        .copied()
        .map(|wgs| to_ned(origin, wgs))
        .collect()
}

#[must_use]
#[cfg(any(feature = "default", feature = "cli"))]
pub fn batch_to_wgs_from_enu(enu_coords: &[frame::Enu]) -> Vec<Wgs> {
    enu_coords
        .iter()
        .copied()
        .map(|enu| concord::to_wgs_from_enu(enu.into()).into())
        .collect()
}

#[must_use]
#[cfg(feature = "embedded")]
pub fn batch_to_wgs_from_enu(enu_coords: &[frame::Enu]) -> Vec<Wgs> {
    enu_coords
        .iter()
        .copied()
        .map(|enu| {
            let origin = enu.ref_origin();
            let (metres_per_deg_lat, metres_per_deg_lon) = metres_per_degree(origin);
            Wgs::new(
                origin.latitude + enu.north() / metres_per_deg_lat,
                origin.longitude + enu.east() / metres_per_deg_lon,
                origin.altitude + enu.up(),
            )
        })
        .collect()
}

#[must_use]
#[cfg(any(feature = "default", feature = "cli"))]
pub fn batch_to_wgs_from_ned(ned_coords: &[frame::Ned]) -> Vec<Wgs> {
    ned_coords
        .iter()
        .copied()
        .map(|ned| concord::to_wgs_from_ned(ned.into()).into())
        .collect()
}

#[must_use]
#[cfg(feature = "embedded")]
pub fn batch_to_wgs_from_ned(ned_coords: &[frame::Ned]) -> Vec<Wgs> {
    let enu: Vec<frame::Enu> = ned_coords
        .iter()
        .copied()
        .map(|ned| frame::Enu::new(ned.east(), ned.north(), -ned.down(), ned.ref_origin()))
        .collect();
    batch_to_wgs_from_enu(&enu)
}

/// Path geometry for curvature-based guidance.
///
/// ISOBUS autosteer is commanded as a path **curvature**, so an autonomy client
/// has to turn a pose error into κ every cycle. These helpers were missing
/// entirely — `geo` offered frame conversions only, with no bearing, no
/// cross-track and no curvature — leaving every caller to derive them.
///
/// Everything here is deliberately free of trigonometry and square roots, so it
/// compiles and runs identically on `no_std` targets without libm.
pub mod guidance {
    /// Metres per kilometre — the ISO 11783-7 curvature SLOT is in km⁻¹ while
    /// vehicle geometry is naturally in m⁻¹.
    const M_PER_KM: f64 = 1000.0;

    /// Pure-pursuit curvature to a goal point given in the **vehicle frame**:
    /// `forward_m` ahead of the axle, `left_m` to the left (both metres).
    ///
    /// **Sign: left-positive**, matching the usual robotics body frame (x
    /// forward, y left). This is the *geometry* convention, and it is the
    /// opposite of the wire's — see [`curvature_to_goal_per_km`], which is the
    /// one to feed to a guidance command.
    ///
    /// Uses the exact form `κ = 2·y / L²` with `L² = x² + y²`, so no square root
    /// or trigonometry is needed and the result is exact rather than a
    /// small-angle approximation.
    ///
    /// Returns `None` when the goal is at the vehicle (no path is defined) or
    /// either coordinate is non-finite.
    #[must_use]
    pub fn curvature_to_goal_per_m(forward_m: f64, left_m: f64) -> Option<f64> {
        if !forward_m.is_finite() || !left_m.is_finite() {
            return None;
        }
        let squared_distance = forward_m * forward_m + left_m * left_m;
        if squared_distance <= 0.0 {
            return None;
        }
        Some(2.0 * left_m / squared_distance)
    }

    /// [`curvature_to_goal_per_m`] in the km⁻¹ unit **and sign** the wire uses.
    ///
    /// AEF 023 RIG 2 D.7.2.1: "Curvature is positive when the vehicle is moving
    /// forward and turning to the driver's **right**." The body-frame helper is
    /// left-positive, so the sign is flipped here — at the one boundary where
    /// geometry becomes a wire value. Getting this backwards steers the machine
    /// the opposite way to the commanded path and the guidance loop diverges
    /// instead of converging.
    #[must_use]
    pub fn curvature_to_goal_per_km(forward_m: f64, left_m: f64) -> Option<f64> {
        curvature_to_goal_per_m(forward_m, left_m).map(|k| -k * M_PER_KM)
    }

    /// Curvature (km⁻¹) of a turn of `radius_m`. A zero or non-finite radius is
    /// straight ahead, not an infinite curvature.
    #[must_use]
    pub fn curvature_per_km_from_radius(radius_m: f64) -> f64 {
        if radius_m.is_finite() && radius_m != 0.0 {
            M_PER_KM / radius_m
        } else {
            0.0
        }
    }

    /// Turn radius in metres for a curvature in km⁻¹, or `None` for straight.
    #[must_use]
    pub fn radius_m_from_curvature_per_km(curvature_per_km: f64) -> Option<f64> {
        if curvature_per_km.is_finite() && curvature_per_km != 0.0 {
            Some(M_PER_KM / curvature_per_km)
        } else {
            None
        }
    }

    /// Curvature (km⁻¹) from a robotics-style twist: `κ = ω / v`.
    ///
    /// `yaw_rate_rad_s` is **left-positive** (counter-clockwise), the usual
    /// robotics convention; the result is in the wire's **right-positive** sign
    /// per AEF 023 D.7.2.1, so it is negated here.
    ///
    /// `min_speed_mps` is a **physical** floor, not an epsilon: below it a yaw
    /// rate does not define a forward path, and dividing anyway turns odometry
    /// noise into a full-lock command.
    #[must_use]
    pub fn curvature_per_km_from_twist(
        linear_mps: f64,
        yaw_rate_rad_s: f64,
        min_speed_mps: f64,
    ) -> f64 {
        if !linear_mps.is_finite()
            || !yaw_rate_rad_s.is_finite()
            || linear_mps.abs() <= min_speed_mps.abs()
        {
            return 0.0;
        }
        -(yaw_rate_rad_s / linear_mps) * M_PER_KM
    }

    /// Signed lateral offset of point `p` from the infinite line through `a`
    /// heading along the **unit** vector `dir`, in the same units as the inputs.
    /// Positive is left of the heading.
    ///
    /// Takes a unit direction so no square root is required; build it from a
    /// heading with `(cos, sin)` on hosted targets, or from two path points
    /// normalised by the caller.
    #[must_use]
    pub fn cross_track_error(p: (f64, f64), a: (f64, f64), dir: (f64, f64)) -> f64 {
        let (dx, dy) = (p.0 - a.0, p.1 - a.1);
        // 2D cross product of the unit heading with the offset.
        dir.0 * dy - dir.1 * dx
    }
}

#[cfg(test)]
mod guidance_tests {
    use super::guidance::*;

    #[test]
    fn pure_pursuit_matches_the_geometric_circle() {
        // A goal 10 m ahead and 0 m across is straight.
        assert_eq!(curvature_to_goal_per_m(10.0, 0.0), Some(0.0));

        // A goal directly abeam at 5 m left sits on a circle of radius 2.5 m,
        // i.e. curvature 0.4 m^-1: kappa = 2y/L^2 = 10/25.
        let k = curvature_to_goal_per_m(0.0, 5.0).unwrap();
        assert!((k - 0.4).abs() < 1e-12);

        // Body frame is left-positive (x forward, y left); magnitudes match.
        let left = curvature_to_goal_per_m(10.0, 2.0).unwrap();
        let right = curvature_to_goal_per_m(10.0, -2.0).unwrap();
        assert!((left + right).abs() < 1e-12);
        assert!(left > 0.0);

        // Degenerate goal yields no path rather than an infinity.
        assert_eq!(curvature_to_goal_per_m(0.0, 0.0), None);
        assert_eq!(curvature_to_goal_per_m(f64::NAN, 1.0), None);
    }

    /// AEF 023 RIG 2 D.7.2.1: "Curvature is positive when the vehicle is moving
    /// forward and turning to the driver's right." The helpers that produce a
    /// wire value must carry that sign, not the body frame's.
    #[test]
    fn wire_curvature_is_positive_turning_right() {
        // Goal 10 m ahead, 2 m to the driver's right.
        let right = curvature_to_goal_per_km(10.0, -2.0).unwrap();
        assert!(
            right > 0.0,
            "a goal to the right must encode as positive curvature, got {right}"
        );
        let left = curvature_to_goal_per_km(10.0, 2.0).unwrap();
        assert!(left < 0.0, "a goal to the left must be negative");
        assert!((left + right).abs() < 1e-12);

        // The km helper is the metre helper mirrored, not merely rescaled.
        let per_m = curvature_to_goal_per_m(10.0, -2.0).unwrap();
        assert!((right + per_m * 1000.0).abs() < 1e-9);

        // A left-positive yaw rate turning left is negative on the wire.
        let turning_left = curvature_per_km_from_twist(2.0, 0.04, 0.05);
        assert!(turning_left < 0.0, "got {turning_left}");
        let turning_right = curvature_per_km_from_twist(2.0, -0.04, 0.05);
        assert!((turning_right - 20.0).abs() < 1e-9, "got {turning_right}");
    }

    #[test]
    fn radius_and_curvature_round_trip_in_wire_units() {
        // 50 m radius is 20 km^-1, the worked example in the autosteer guide.
        assert!((curvature_per_km_from_radius(50.0) - 20.0).abs() < 1e-9);
        assert!((radius_m_from_curvature_per_km(20.0).unwrap() - 50.0).abs() < 1e-9);

        // Straight is a zero curvature, not an infinite radius.
        assert_eq!(curvature_per_km_from_radius(0.0), 0.0);
        assert_eq!(radius_m_from_curvature_per_km(0.0), None);
    }

    #[test]
    fn twist_respects_the_physical_speed_floor() {
        // 2 m/s with 0.04 rad/s is 0.02 m^-1 = 20 km^-1 = a 50 m radius. The
        // yaw rate is left-positive and the result is wire sign, so a left turn
        // reads negative (AEF 023 D.7.2.1).
        assert!((curvature_per_km_from_twist(2.0, 0.04, 0.05) + 20.0).abs() < 1e-9);

        // Below the floor, odometry noise must not become a full-lock command.
        assert_eq!(curvature_per_km_from_twist(1e-6, 0.04, 0.05), 0.0);
        assert_eq!(curvature_per_km_from_twist(0.0, 1.0, 0.05), 0.0);
    }

    #[test]
    fn cross_track_is_signed_left_positive() {
        // Heading due north (+y); a point 3 m east is 3 m to the right.
        let xte = cross_track_error((3.0, 10.0), (0.0, 0.0), (0.0, 1.0));
        assert!(
            (xte + 3.0).abs() < 1e-12,
            "east of a northward line is right"
        );

        let xte_left = cross_track_error((-3.0, 10.0), (0.0, 0.0), (0.0, 1.0));
        assert!((xte_left - 3.0).abs() < 1e-12);

        // On the line is zero regardless of how far along.
        assert!(cross_track_error((0.0, 99.0), (0.0, 0.0), (0.0, 1.0)).abs() < 1e-12);
    }
}
