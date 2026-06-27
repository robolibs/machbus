//! Stack-owned ISO 11783-14 Sequence Control workflow.
//!
//! The lower-level [`crate::isobus::sc::SCMaster`] and
//! [`crate::isobus::sc::SCClient`] types are pump-style codecs/state machines.
//! This module wires them into [`Stack`]: inbound SC status PGNs
//! are routed from `IsoNet`, status payloads are emitted on update, and state
//! transitions land on the unified event queue.

use crate::isobus::sc::SCState;
use crate::net::types::Address;

/// Unified stack event for ISO 11783-14 Sequence Control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScEvent {
    /// Local SC master changed state.
    MasterStateChanged { from: SCState, to: SCState },
    /// Local SC master started a wire-visible step.
    MasterStepStarted { step_id: u16 },
    /// Local SC master completed a wire-visible step.
    MasterStepCompleted { step_id: u16 },
    /// Local SC master completed the whole sequence.
    MasterSequenceComplete,
    /// Local SC master hit a lifecycle timeout.
    MasterTimeout { reason: &'static str },
    /// Local SC master accepted a valid client status from `source`.
    MasterClientStatus { source: Address, state: SCState },
    /// Local SC client changed state after a valid master status or timeout.
    ClientStateChanged { from: SCState, to: SCState },
    /// Local SC client observed a new sequence start.
    ClientSequenceStart,
    /// Local SC client was asked to execute `step_id`.
    ClientStepRequest { step_id: u16 },
    /// Local SC client paused.
    ClientPause,
    /// Local SC client resumed.
    ClientResume,
    /// Local SC client aborted or entered error.
    ClientAbort,
}
