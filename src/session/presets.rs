//! Curated plugin groups (P6) — the new-facade equivalent of the old persona
//! builders. Each returns a `Vec<Box<dyn Plugin>>` for
//! [`SessionBuilder::plug_group`](super::SessionBuilder::plug_group); you can
//! still drop or add individual plugins around a group.
//!
//! ```no_run
//! use machbus::session::{Session, presets};
//! use machbus::prelude::*;
//! let s = Session::builder(Name::default(), 0xF0)
//!     .plug_group(presets::tractor())
//!     .build()
//!     .unwrap();
//! ```

use super::Plugin;
use super::plugins::{
    AutoDrive, Diagnostics, Heartbeat, Implement, Powertrain, ShortcutButton, TcClient, VtClient,
};
use crate::isobus::tc::{DDOP, TCClientConfig};
use crate::isobus::vt::{ObjectPool, VTClientConfig, WorkingSet};

/// A TECU-style tractor: diagnostics, implement-message broadcast/decode, and
/// J1939 powertrain status.
#[must_use]
pub fn tractor() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(Diagnostics::every(1000)),
        Box::new(Implement::new()),
        Box::new(Powertrain::new()),
    ]
}

/// An implement ECU that talks to a Virtual Terminal and a Task Controller:
/// VT client (with the given object pool + working set), TC client (with the
/// given DDOP), and diagnostics.
#[must_use]
pub fn implement(pool: ObjectPool, ws: WorkingSet, ddop: DDOP) -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(VtClient::new(VTClientConfig::default(), pool, ws)),
        Box::new(TcClient::new(TCClientConfig::default(), ddop)),
        Box::new(Diagnostics::every(1000)),
    ]
}

/// An autonomous vehicle controller: the combined [`AutoDrive`] steering and
/// speed surface, the implement messages it reads speed and facilities from,
/// the Auxiliary Shortcut Button it must obey, an ISO 11783-7 §8 heartbeat, and
/// diagnostics.
///
/// The ISB and heartbeat are not optional extras here — they are the two
/// mechanisms by which an operator and a peer respectively can tell this node
/// to stop. Grouping them with `AutoDrive` means a caller cannot assemble an
/// autonomy node that has no way of being stopped.
#[must_use]
pub fn autonomous_vehicle() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(AutoDrive::new()),
        Box::new(Implement::new()),
        Box::new(ShortcutButton::new()),
        Box::new(Heartbeat::every(100)),
        Box::new(Diagnostics::every(1000)),
    ]
}

/// A minimal monitored node: diagnostics + ISO 11783 heartbeat.
#[must_use]
pub fn diagnostic_node() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(Diagnostics::every(1000)),
        Box::new(Heartbeat::every(100)),
    ]
}
