//! `stack.gnss()` — GNSS / NMEA 2000 handle.
//!
//! Wraps [`crate::nmea::NMEAInterface`] (already pump-style) plus a
//! shared-state pattern: the inbound PGN callbacks drain raw
//! [`Message`]s into a buffer, and `Stack::tick` flushes them through
//! `NMEAInterface::handle_message` so the cached position stays
//! current. Native `NMEAInterface` events get re-emitted as
//! [`GnssEvent`] entries on the unified queue.

use crate::nmea::{GNSSDOPData, GNSSPosition, SystemTimeData};

/// Inbound GNSS / NMEA events.
#[derive(Debug, Clone, PartialEq)]
pub enum GnssEvent {
    /// Cached position changed (position rapid or detail PGN).
    Position(GNSSPosition),
    /// Course over ground update (radians).
    Cog(f64),
    /// Speed over ground update (m/s).
    Sog(f64),
    /// Heading update (radians).
    Heading(f64),
    /// Magnetic variation (radians).
    MagneticVariation(f64),
    /// Roll/pitch/yaw triple (radians).
    Attitude { yaw: f64, pitch: f64, roll: f64 },
    /// DOPs report.
    Dops(GNSSDOPData),
    /// System time / date update.
    SystemTime(SystemTimeData),
    /// No position update within the configured window. Nothing consumed GNSS
    /// liveness before this existed, so an autonomy path could keep steering to
    /// a curvature derived from a position that had stopped arriving.
    PositionStale { silent_for_ms: u32 },
    /// The receiver reported a method that cannot be steered on — no fix, dead
    /// reckoning, error or unavailable.
    FixDegraded { fix_type: crate::nmea::GNSSFixType },
    /// A usable fix returned after a degraded one. Informational: recovery does
    /// not clear a latched stop.
    FixRestored { fix_type: crate::nmea::GNSSFixType },
}

// Silence dead-code lint for `Address` import — used in match arms
// for completeness even though no current branch consumes it.
