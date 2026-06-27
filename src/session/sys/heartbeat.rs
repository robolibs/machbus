//! Stack-owned ISO 11783 heartbeat workflow.
//!
//! The pure sender/tracker state machines live in
//! [`crate::j1939::heartbeat`]. This module wires them into [`Stack`], so the
//! stack can periodically broadcast `PGN_HEARTBEAT`, cache peer sequences,
//! report missed peers, and reject malformed heartbeat frames before they
//! mutate tracker state.

use crate::net::types::Address;

/// Event emitted for stack-owned heartbeat activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatEvent {
    /// A valid one-byte heartbeat was received.
    Received { source: Address, sequence: u8 },
    /// A tracked peer missed another heartbeat timeout window.
    Missed { source: Address, missed_count: u32 },
    /// This stack broadcast a heartbeat sequence.
    Sent { sequence: u8 },
}
