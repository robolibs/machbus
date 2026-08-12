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
    /// A peer's sequence broke the ISO 11783-7 §8.3.3 rules — repeated, or
    /// advanced by more than 3. The previous tracker stored the sequence
    /// without validating it, so a peer jumping by 50 read as healthy.
    SequenceError { source: Address, sequence: u8 },
    /// No valid heartbeat from a peer within the §8.3.4 300 ms window.
    CommError { source: Address },
    /// A peer reported its own fault (sequence 254).
    SenderError { source: Address },
    /// A peer announced an orderly shutdown (sequence 255).
    GracefulShutdown { source: Address },
    /// A peer recovered: 8 consecutive correct heartbeats after an error.
    Recovered { source: Address },
}
