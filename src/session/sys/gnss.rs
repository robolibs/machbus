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
}

// Silence dead-code lint for `Address` import — used in match arms
// for completeness even though no current branch consumes it.
