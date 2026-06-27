//! Common types for the transport-layer protocols (TP, ETP).
//!
//! Mirrors the C++ `machbus::net::session.hpp`. The numeric values of
//! [`TransportAbortReason`] are wire-compatible with ISO 11783-3
//! abort byte codes.

use alloc::vec::Vec;

use super::constants::{
    BROADCAST_ADDRESS, NULL_ADDRESS, TP_BYTES_PER_FRAME, TP_MAX_PACKETS_PER_CTS,
};
use super::types::{Address, Pgn, Priority};

/// Whether a session is sending or receiving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TransportDirection {
    Transmit,
    #[default]
    Receive,
}

/// Wire-format abort reason byte (ISO 11783-3 §5.13.5 / J1939-21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TransportAbortReason {
    #[default]
    None = 0,
    AlreadyInSession = 1,
    ResourcesUnavailable = 2,
    Timeout = 3,
    ConnectionModeError = 4,
    MaxRetransmitsExceeded = 5,
    UnexpectedPgn = 6,
    BadSequence = 7,
    DuplicateSequence = 8,
    UnexpectedDataSize = 9,
}

impl TransportAbortReason {
    #[inline]
    #[must_use]
    pub const fn from_u8(value: u8) -> Self {
        match Self::try_from_u8(value) {
            Some(reason) => reason,
            None => Self::None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn try_from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::AlreadyInSession),
            2 => Some(Self::ResourcesUnavailable),
            3 => Some(Self::Timeout),
            4 => Some(Self::ConnectionModeError),
            5 => Some(Self::MaxRetransmitsExceeded),
            6 => Some(Self::UnexpectedPgn),
            7 => Some(Self::BadSequence),
            8 => Some(Self::DuplicateSequence),
            9 => Some(Self::UnexpectedDataSize),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Explicit observability counters for lossy transport paths.
///
/// TP, ETP, and NMEA2000 Fast Packet intentionally drop malformed or
/// unactionable input instead of panicking. These counters let embedded stacks
/// and test harnesses verify that those defensive paths are exercised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TransportStats {
    /// Incoming CM/DT/Fast-Packet frames that could not be associated with a
    /// valid receive path, had invalid shape, or were discarded by policy.
    pub dropped_frames: u64,
    /// In-flight sessions removed before normal completion.
    pub dropped_sessions: u64,
    /// Abort frames generated locally.
    pub aborts_sent: u64,
    /// Abort frames received from a peer and matched to a local session.
    pub aborts_received: u64,
    /// Sessions terminated by elapsed protocol timers.
    pub timeouts: u64,
    /// New sends or receives rejected because configured resource caps or
    /// allocation limits were exceeded.
    pub resource_rejections: u64,
}

impl TransportStats {
    #[inline]
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.dropped_frames == 0
            && self.dropped_sessions == 0
            && self.aborts_sent == 0
            && self.aborts_received == 0
            && self.timeouts == 0
            && self.resource_rejections == 0
    }
}

/// FSM state of a transport session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SessionState {
    #[default]
    None,
    /// Sender after RTS, awaiting CTS.
    WaitingForCTS,
    /// Sender clear to transmit DT frames.
    SendingData,
    /// Sender finished window, awaiting EoMA.
    WaitingForEndOfMsg,
    /// Receiver mid-stream (BAM).
    ReceivingData,
    /// Receiver after CTS, awaiting next window of DT.
    WaitingForData,
    /// All bytes transferred and acknowledged.
    Complete,
    /// Aborted by either side.
    Aborted,
}

/// Mutable state of one TP / ETP session.
#[derive(Debug, Clone)]
pub struct TransportSession {
    pub direction: TransportDirection,
    pub state: SessionState,
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub total_bytes: u32,
    pub bytes_transferred: u32,
    pub source_address: Address,
    pub destination_address: Address,
    pub can_port: u8,
    pub priority: Priority,

    // ─── DT sequence tracking ────────────────────────────────────────
    pub last_sequence: u8,
    pub packets_to_send: u8,
    pub next_packet_to_send: u8,
    /// Value advertised in TP.RTS byte 4.
    ///
    /// Some stacks configure this independently from the sender's internal
    /// response to a peer CTS window. Keep it separate from
    /// [`Self::max_packets_per_cts`] so compatibility tests can pin the wire
    /// byte without under-sending when a peer sends a larger CTS window.
    pub advertised_packets_per_cts: u8,
    pub max_packets_per_cts: u8,

    // ─── CTS windowing (receiver side) ──────────────────────────────
    pub cts_window_start: u8,
    pub cts_window_size: u8,

    // ─── Sender retransmit accounting ──────────────────────────────
    /// Number of duplicate/backward CTS windows accepted for this transmit
    /// session. This is reset when the peer advances the next-packet pointer.
    pub retransmit_count: u8,

    // ─── ETP DPO offset for the current window ──────────────────────
    pub dpo_packet_offset: u32,

    pub timer_ms: u32,
}

impl Default for TransportSession {
    fn default() -> Self {
        Self {
            direction: TransportDirection::Receive,
            state: SessionState::None,
            pgn: 0,
            data: Vec::new(),
            total_bytes: 0,
            bytes_transferred: 0,
            source_address: NULL_ADDRESS,
            destination_address: BROADCAST_ADDRESS,
            can_port: 0,
            priority: Priority::Default,
            last_sequence: 0,
            packets_to_send: 0,
            next_packet_to_send: 0,
            advertised_packets_per_cts: TP_MAX_PACKETS_PER_CTS as u8,
            max_packets_per_cts: TP_MAX_PACKETS_PER_CTS as u8,
            cts_window_start: 1,
            cts_window_size: 0,
            retransmit_count: 0,
            dpo_packet_offset: 0,
            timer_ms: 0,
        }
    }
}

impl TransportSession {
    /// Fraction of bytes transferred (`0.0..=1.0`). Empty payload → 0.
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.total_bytes == 0 {
            0.0
        } else {
            self.bytes_transferred as f32 / self.total_bytes as f32
        }
    }

    /// Total DT frames the session must carry (`ceil(total_bytes / 7)`).
    #[must_use]
    pub const fn total_packets(&self) -> u32 {
        self.total_bytes.div_ceil(TP_BYTES_PER_FRAME)
    }

    #[inline]
    #[must_use]
    pub const fn is_broadcast(&self) -> bool {
        self.destination_address == BROADCAST_ADDRESS
    }

    #[inline]
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        matches!(self.state, SessionState::Complete)
    }
}

/// Lightweight, `Copy` summary emitted on session abort. Avoids
/// cloning the full payload buffer for every abort listener.
#[derive(Debug, Clone, Copy)]
pub struct TransportAbortEvent {
    pub pgn: Pgn,
    pub source: Address,
    pub destination: Address,
    pub can_port: u8,
    pub direction: TransportDirection,
    pub reason: TransportAbortReason,
}

impl TransportAbortEvent {
    #[must_use]
    pub fn from_session(session: &TransportSession, reason: TransportAbortReason) -> Self {
        Self {
            pgn: session.pgn,
            source: session.source_address,
            destination: session.destination_address,
            can_port: session.can_port,
            direction: session.direction,
            reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abort_reason_round_trips_through_u8() {
        for r in [
            TransportAbortReason::None,
            TransportAbortReason::Timeout,
            TransportAbortReason::AlreadyInSession,
            TransportAbortReason::BadSequence,
            TransportAbortReason::DuplicateSequence,
            TransportAbortReason::ConnectionModeError,
            TransportAbortReason::MaxRetransmitsExceeded,
        ] {
            assert_eq!(TransportAbortReason::from_u8(r.as_u8()), r);
        }
    }

    #[test]
    fn abort_reason_unknown_byte_is_none() {
        assert_eq!(
            TransportAbortReason::from_u8(99),
            TransportAbortReason::None
        );
    }

    #[test]
    fn total_packets_round_up() {
        for (bytes, packets) in [(7u32, 1u32), (8, 2), (14, 2), (15, 3)] {
            let s = TransportSession {
                total_bytes: bytes,
                ..Default::default()
            };
            assert_eq!(s.total_packets(), packets);
        }
    }

    #[test]
    fn progress_handles_empty() {
        let s = TransportSession::default();
        assert_eq!(s.progress(), 0.0);
    }

    #[test]
    fn progress_half() {
        let s = TransportSession {
            total_bytes: 100,
            bytes_transferred: 50,
            ..Default::default()
        };
        assert!((s.progress() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn defaults_match_cpp_inits() {
        let s = TransportSession::default();
        assert_eq!(s.source_address, NULL_ADDRESS);
        assert_eq!(s.destination_address, BROADCAST_ADDRESS);
        assert!(s.is_broadcast());
        assert_eq!(s.cts_window_start, 1);
        assert_eq!(s.max_packets_per_cts as u32, TP_MAX_PACKETS_PER_CTS);
        assert_eq!(s.retransmit_count, 0);
    }

    #[test]
    fn abort_event_summary_clone_is_cheap() {
        let s = TransportSession {
            pgn: 0xEA00,
            source_address: 0x10,
            destination_address: 0x20,
            can_port: 1,
            ..Default::default()
        };
        let evt = TransportAbortEvent::from_session(&s, TransportAbortReason::Timeout);
        // Copy works without any allocations.
        let _copy = evt;
        assert_eq!(evt.pgn, 0xEA00);
        assert_eq!(evt.reason, TransportAbortReason::Timeout);
    }

    #[test]
    fn transport_stats_default_is_empty() {
        assert!(TransportStats::default().is_empty());

        let stats = TransportStats {
            dropped_frames: 1,
            ..Default::default()
        };
        assert!(!stats.is_empty());
    }
}
