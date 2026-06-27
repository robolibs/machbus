//! `stack.diag()` — diagnostics handle.
//!
//! Wraps the j1939 codec layer (`Dtc`, `DiagnosticLamps`, `DmDtcList`)
//! with active/previous DTC bookkeeping plus periodic DM1 broadcast.
//! Inbound `PGN_DM1` messages from peers are decoded and re-emitted
//! as [`DiagEvent::Dm1Received`] on the unified [`Event`] queue.

use crate::j1939::{DiagnosticLamps, Dm7Command, Dm8TestResult, Dm13Signals, Dm22Message, Dtc};
use crate::net::types::Address;

/// Diagnostics-related events on the unified queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagEvent {
    /// A peer broadcast a DM1 — its active DTC list and lamp status.
    Dm1Received {
        source: Address,
        active: Vec<Dtc>,
        lamps: DiagnosticLamps,
    },
    /// We added a new DTC to our own active list.
    Raised(Dtc),
    /// We moved a DTC from active → previously-active.
    Cleared(Dtc),
    /// A peer sent a DM7 non-continuous monitor test command.
    Dm7Command {
        source: Address,
        command: Dm7Command,
    },
    /// A peer sent a DM8 non-continuous monitor test result.
    Dm8Result {
        source: Address,
        result: Dm8TestResult,
    },
    /// A peer sent a DM13 stop/start broadcast control command.
    Dm13Signals {
        source: Address,
        signals: Dm13Signals,
    },
    /// A peer sent a DM22 individual-DTC clear/reset request or response.
    Dm22Message {
        source: Address,
        message: Dm22Message,
    },
}
