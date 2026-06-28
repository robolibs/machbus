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
