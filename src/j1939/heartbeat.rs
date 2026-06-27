//! ISO 11783-7 §8 Heartbeat protocol — sender + receiver state
//! machines.
//!
//! Mirrors the C++ `machbus::j1939::heartbeat.hpp`. Wire byte 0
//! carries a sequence number with these special values:
//!
//! | Value | Meaning |
//! |---|---|
//! | 0–250 | Normal rolling sequence |
//! | 251 (`INIT`) | Initial / sender reset |
//! | 252–253 | Reserved (receivers ignore) |
//! | 254 (`SENDER_ERROR`) | Sender error indicator (one-shot) |
//! | 255 (`SHUTDOWN`) | Graceful shutdown (one-shot) |
//!
//! The C++ `HeartbeatProtocol` (which embeds `IsoNet&` and tracks
//! peers) is not ported — combine [`HeartbeatSender`] with
//! [`HeartbeatTracker`] and route via `IsoNet::send` /
//! `register_pgn_callback`.

use alloc::{collections::BTreeMap, format, vec::Vec};

use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn::pgn_is_valid;
use crate::net::pgn_defs::{PGN_ECU_TO_TC, PGN_HEARTBEAT};
use crate::net::types::{Address, Pgn};

// ─── Wire constants ────────────────────────────────────────────────────

pub mod hb_seq {
    pub const INIT: u8 = 251;
    pub const RESERVED_LOW: u8 = 252;
    pub const RESERVED_HIGH: u8 = 253;
    pub const SENDER_ERROR: u8 = 254;
    pub const SHUTDOWN: u8 = 255;
    pub const MAX_NORMAL: u8 = 250;
}

pub const HB_INTERVAL_MS: u32 = 100;
pub const HB_COMM_ERROR_TIMEOUT_MS: u32 = 300;
pub const HB_RECOVERY_COUNT: u8 = 8;
pub const HB_MAX_JUMP: u8 = 3;
pub const HB_MAX_MISS_EVENTS_PER_UPDATE: u32 = 64;

/// Destination-specific PGN used by the ISO heartbeat request payload.
///
/// Public AgIsoStack++ emits heartbeat requests as PGN `0xCC00` frames with
/// payload bytes `requested-pgn[0..3]`, `interval-ms[0..2]`, then three
/// `0xFF` reserved bytes. That PGN is also used by ISO 11783-10 ECU→TC
/// messages, so stack layers must demultiplex by payload/role instead of
/// assuming every `0xCC00` frame is TC process data.
pub const PGN_HEARTBEAT_REQUEST: Pgn = PGN_ECU_TO_TC;

/// ISO 11783 heartbeat request payload carried on [`PGN_HEARTBEAT_REQUEST`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatRequest {
    pub requested_pgn: Pgn,
    pub interval_ms: u16,
}

impl HeartbeatRequest {
    #[must_use]
    pub const fn new(requested_pgn: Pgn, interval_ms: u16) -> Self {
        Self {
            requested_pgn,
            interval_ms,
        }
    }

    #[must_use]
    pub const fn for_heartbeat(interval_ms: u16) -> Self {
        Self::new(PGN_HEARTBEAT, interval_ms)
    }

    pub fn encode(&self) -> Result<[u8; 8]> {
        if !pgn_is_valid(self.requested_pgn) {
            return Err(Error::invalid_data(format!(
                "heartbeat requested PGN 0x{:X} exceeds the 18-bit J1939/ISOBUS PGN range",
                self.requested_pgn
            )));
        }
        Ok([
            (self.requested_pgn & 0xFF) as u8,
            ((self.requested_pgn >> 8) & 0xFF) as u8,
            ((self.requested_pgn >> 16) & 0xFF) as u8,
            (self.interval_ms & 0xFF) as u8,
            (self.interval_ms >> 8) as u8,
            0xFF,
            0xFF,
            0xFF,
        ])
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || data[5..8] != [0xFF, 0xFF, 0xFF] {
            return None;
        }
        let requested_pgn = (data[0] as Pgn) | ((data[1] as Pgn) << 8) | ((data[2] as Pgn) << 16);
        if !pgn_is_valid(requested_pgn) {
            return None;
        }
        let interval_ms = u16::from_le_bytes([data[3], data[4]]);
        Some(Self {
            requested_pgn,
            interval_ms,
        })
    }

    /// Decode from a network message and bind the request to the expected PGN
    /// and a usable source address before inspecting the payload.
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if msg.pgn != PGN_HEARTBEAT_REQUEST
            || msg.source == NULL_ADDRESS
            || msg.source == BROADCAST_ADDRESS
            || msg.destination == NULL_ADDRESS
            || msg.destination == BROADCAST_ADDRESS
        {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── Sender ────────────────────────────────────────────────────────────

/// Generates the heartbeat sequence per ISO 11783-7 §8.3:
/// `INIT(251)` first, then `0..=250` rolling, with one-shot
/// `SENDER_ERROR` / `SHUTDOWN` insertions.
#[derive(Debug, Clone, Copy)]
pub struct HeartbeatSender {
    sequence: u8,
    init_sent: bool,
    special_pending: bool,
    timer_ms: u32,
    interval_ms: u32,
}

impl Default for HeartbeatSender {
    fn default() -> Self {
        Self::new(HB_INTERVAL_MS)
    }
}

impl HeartbeatSender {
    #[must_use]
    pub const fn new(interval_ms: u32) -> Self {
        Self {
            sequence: hb_seq::INIT,
            init_sent: false,
            special_pending: false,
            timer_ms: 0,
            interval_ms,
        }
    }

    /// Return the next sequence byte. The first call always emits
    /// [`hb_seq::INIT`]; subsequent calls walk `0..=250` with rollover.
    pub fn next_sequence(&mut self) -> u8 {
        if !self.init_sent {
            self.init_sent = true;
            self.sequence = hb_seq::INIT;
            return hb_seq::INIT;
        }
        if self.special_pending {
            self.special_pending = false;
            return self.sequence;
        }
        if self.sequence >= hb_seq::INIT {
            // After INIT(251), ERROR(254), or SHUTDOWN(255) → restart at 0.
            self.sequence = 0;
        } else if self.sequence >= hb_seq::MAX_NORMAL {
            self.sequence = 0;
        } else {
            self.sequence += 1;
        }
        self.sequence
    }

    /// Schedule [`hb_seq::SENDER_ERROR`] for the next call to
    /// [`Self::next_sequence`]. After it fires, the sequence resumes
    /// at `0`.
    pub fn signal_error(&mut self) {
        self.sequence = hb_seq::SENDER_ERROR;
        self.special_pending = true;
    }

    /// Schedule [`hb_seq::SHUTDOWN`] for the next call.
    pub fn signal_shutdown(&mut self) {
        self.sequence = hb_seq::SHUTDOWN;
        self.special_pending = true;
    }

    /// Advance the cadence timer. Returns `true` exactly when the
    /// configured `interval_ms` has elapsed; the caller should then
    /// call [`Self::next_sequence`] and broadcast the heartbeat.
    pub fn update(&mut self, elapsed_ms: u32) -> bool {
        if self.interval_ms == 0 {
            self.timer_ms = 0;
            return false;
        }
        self.timer_ms = self.timer_ms.saturating_add(elapsed_ms);
        if self.timer_ms >= self.interval_ms {
            self.timer_ms %= self.interval_ms;
            true
        } else {
            false
        }
    }

    pub fn reset(&mut self) {
        self.sequence = hb_seq::INIT;
        self.init_sent = false;
        self.special_pending = false;
        self.timer_ms = 0;
    }

    pub fn set_interval(&mut self, ms: u32) {
        self.interval_ms = ms;
    }

    #[inline]
    #[must_use]
    pub const fn interval(&self) -> u32 {
        self.interval_ms
    }
}

// ─── Receiver ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum HbReceiverState {
    #[default]
    Normal,
    SequenceError,
    CommError,
}

/// State machine that consumes inbound heartbeat sequence bytes and
/// classifies them.
pub struct HeartbeatReceiver {
    state: HbReceiverState,
    last_sequence: u8,
    recovery_counter: u8,
    time_since_last_ms: u32,
    first_received: bool,

    pub on_state_change: Event<(HbReceiverState, HbReceiverState)>,
    pub on_shutdown_received: Event<()>,
    pub on_sender_error: Event<()>,
    pub on_reset_received: Event<()>,
}

impl Default for HeartbeatReceiver {
    fn default() -> Self {
        Self::new()
    }
}

impl HeartbeatReceiver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: HbReceiverState::Normal,
            last_sequence: 0xFF,
            recovery_counter: 0,
            time_since_last_ms: 0,
            first_received: false,
            on_state_change: Event::new(),
            on_shutdown_received: Event::new(),
            on_sender_error: Event::new(),
            on_reset_received: Event::new(),
        }
    }

    pub fn process(&mut self, sequence: u8) {
        // 252/253: reserved, ignored.
        if sequence == hb_seq::RESERVED_LOW || sequence == hb_seq::RESERVED_HIGH {
            return;
        }
        self.time_since_last_ms = 0;
        if sequence == hb_seq::SENDER_ERROR {
            self.on_sender_error.emit(&());
            return;
        }
        if sequence == hb_seq::SHUTDOWN {
            self.on_shutdown_received.emit(&());
            return;
        }

        // Recover from CommError on any valid heartbeat.
        if matches!(self.state, HbReceiverState::CommError) {
            let old = self.state;
            self.state = HbReceiverState::Normal;
            self.recovery_counter = 0;
            self.last_sequence = sequence;
            self.on_state_change.emit(&(old, self.state));
            tracing::debug!(target: "machbus.heartbeat.rx", "recovered from CommError");
            return;
        }

        if !self.first_received {
            self.first_received = true;
            self.last_sequence = sequence;
            return;
        }

        if sequence == hb_seq::INIT {
            // Sender reset — synchronize, expect 0 next.
            self.last_sequence = hb_seq::INIT;
            self.on_reset_received.emit(&());
            tracing::debug!(target: "machbus.heartbeat.rx", "sender reset detected (251)");
            return;
        }

        let is_error = if sequence == self.last_sequence {
            true
        } else {
            compute_jump(self.last_sequence, sequence) > HB_MAX_JUMP
        };

        match self.state {
            HbReceiverState::Normal => {
                if is_error {
                    let old = self.state;
                    self.state = HbReceiverState::SequenceError;
                    self.recovery_counter = 0;
                    self.on_state_change.emit(&(old, self.state));
                    tracing::warn!(
                        target: "machbus.heartbeat.rx",
                        last = self.last_sequence,
                        got = sequence,
                        "sequence error",
                    );
                }
            }
            HbReceiverState::SequenceError => {
                if is_error {
                    self.recovery_counter = 0;
                } else {
                    self.recovery_counter = self.recovery_counter.saturating_add(1);
                    if self.recovery_counter >= HB_RECOVERY_COUNT {
                        let old = self.state;
                        self.state = HbReceiverState::Normal;
                        self.recovery_counter = 0;
                        self.on_state_change.emit(&(old, self.state));
                        tracing::debug!(
                            target: "machbus.heartbeat.rx",
                            "recovered from SequenceError",
                        );
                    }
                }
            }
            HbReceiverState::CommError => unreachable!("handled above"),
        }
        self.last_sequence = sequence;
    }

    /// Drive the comm-error timer. Triggers the
    /// `Normal/SequenceError → CommError` transition once
    /// [`HB_COMM_ERROR_TIMEOUT_MS`] elapses without a heartbeat.
    pub fn update(&mut self, elapsed_ms: u32) {
        if !self.first_received {
            return;
        }
        self.time_since_last_ms = self.time_since_last_ms.saturating_add(elapsed_ms);
        if self.time_since_last_ms > HB_COMM_ERROR_TIMEOUT_MS
            && !matches!(self.state, HbReceiverState::CommError)
        {
            let old = self.state;
            self.state = HbReceiverState::CommError;
            self.recovery_counter = 0;
            self.on_state_change.emit(&(old, self.state));
            tracing::warn!(
                target: "machbus.heartbeat.rx",
                age_ms = self.time_since_last_ms,
                "comm error",
            );
        }
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> HbReceiverState {
        self.state
    }

    #[inline]
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        matches!(self.state, HbReceiverState::Normal)
    }
}

// ─── Multi-peer tracker ────────────────────────────────────────────────

/// Tracks heartbeats from multiple remote peers. Records sequence
/// number, missed-heartbeat count, and emits an event when a peer
/// times out (3× `interval`).
pub struct HeartbeatTracker {
    interval_ms: u32,
    peers: BTreeMap<Address, RemotePeer>,
    pub on_heartbeat_received: Event<(Address, u8)>,
    /// `(address, missed_count)`.
    pub on_heartbeat_missed: Event<(Address, u32)>,
}

#[derive(Debug, Clone, Copy)]
struct RemotePeer {
    last_sequence: u8,
    missed_count: u32,
    timer_ms: u32,
}

impl Default for HeartbeatTracker {
    fn default() -> Self {
        Self::new(HB_INTERVAL_MS)
    }
}

impl HeartbeatTracker {
    #[must_use]
    pub fn new(interval_ms: u32) -> Self {
        Self {
            interval_ms,
            peers: BTreeMap::new(),
            on_heartbeat_received: Event::new(),
            on_heartbeat_missed: Event::new(),
        }
    }

    pub fn track(&mut self, address: Address) {
        self.peers.entry(address).or_insert(RemotePeer {
            last_sequence: 0,
            missed_count: 0,
            timer_ms: 0,
        });
    }

    pub fn untrack(&mut self, address: Address) {
        self.peers.remove(&address);
    }

    /// Record an incoming heartbeat message. Untracked peers are
    /// silently ignored; the event still fires so callers can
    /// auto-track on first sight.
    pub fn handle_message(&mut self, msg: &Message) {
        if !msg.has_usable_envelope_for_pgn(PGN_HEARTBEAT) {
            return;
        }
        let seq = match msg.data.first() {
            Some(b) => *b,
            None => return,
        };
        let valid_width = msg.data.len() == 1
            || (msg.data.len() == 8 && msg.data[1..].iter().all(|&byte| byte == 0xFF));
        if !valid_width || seq == hb_seq::RESERVED_LOW || seq == hb_seq::RESERVED_HIGH {
            return;
        }
        self.on_heartbeat_received.emit(&(msg.source, seq));
        if let Some(peer) = self.peers.get_mut(&msg.source) {
            peer.last_sequence = seq;
            peer.missed_count = 0;
            peer.timer_ms = 0;
        }
    }

    /// Drive the per-peer miss-detection timer. After 3× `interval`
    /// without a heartbeat, increments the peer's `missed_count` and
    /// fires [`Self::on_heartbeat_missed`].
    pub fn update(&mut self, elapsed_ms: u32) {
        let threshold = self.interval_ms.saturating_mul(3);
        let mut missed: Vec<(Address, u32)> = Vec::new();
        for (addr, peer) in &mut self.peers {
            if threshold == 0 {
                peer.timer_ms = 0;
                continue;
            }
            peer.timer_ms = peer.timer_ms.saturating_add(elapsed_ms);
            let missed_this_tick = peer.timer_ms / threshold;
            if missed_this_tick == 0 {
                continue;
            }
            peer.timer_ms %= threshold;

            let previous_missed = peer.missed_count;
            peer.missed_count = peer.missed_count.saturating_add(missed_this_tick);

            if missed_this_tick <= HB_MAX_MISS_EVENTS_PER_UPDATE {
                for offset in 1..=missed_this_tick {
                    missed.push((*addr, previous_missed.saturating_add(offset)));
                }
            } else {
                missed.push((*addr, peer.missed_count));
            }
        }
        for evt in missed {
            self.on_heartbeat_missed.emit(&evt);
        }
    }

    #[must_use]
    pub fn last_sequence(&self, address: Address) -> Option<u8> {
        self.peers.get(&address).map(|p| p.last_sequence)
    }

    #[must_use]
    pub fn missed_count(&self, address: Address) -> u32 {
        self.peers.get(&address).map_or(0, |p| p.missed_count)
    }
}

/// Forward jump distance with rollover at `MAX_NORMAL + 1` (251
/// values: `0..=250`).
fn compute_jump(from: u8, to: u8) -> u8 {
    if from == hb_seq::INIT {
        return if to == 0 { 1 } else { to.saturating_add(1) };
    }
    if to > from {
        to - from
    } else {
        ((hb_seq::MAX_NORMAL as u16 + 1) - from as u16 + to as u16) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_HEARTBEAT;
    use proptest::prelude::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    // ─── Request payload ───────────────────────────────────────────

    #[test]
    fn heartbeat_request_matches_agisostack_reference_payload() {
        let request = HeartbeatRequest::for_heartbeat(100);
        let encoded = request.encode().unwrap();
        assert_eq!(encoded, [0xE4, 0xF0, 0x00, 0x64, 0x00, 0xFF, 0xFF, 0xFF]);
        assert_eq!(HeartbeatRequest::decode(&encoded), Some(request));
    }

    #[test]
    fn heartbeat_request_rejects_malformed_payloads() {
        assert!(HeartbeatRequest::decode(&[0xE4, 0xF0, 0x00, 0x64, 0x00, 0xFF, 0xFF]).is_none());
        assert!(
            HeartbeatRequest::decode(&[0xE4, 0xF0, 0x00, 0x64, 0x00, 0xFF, 0xFF, 0xFF, 0xFF])
                .is_none()
        );
        assert!(
            HeartbeatRequest::decode(&[0xE4, 0xF0, 0x00, 0x64, 0x00, 0x00, 0xFF, 0xFF]).is_none()
        );
        assert!(
            HeartbeatRequest::decode(&[0xFF, 0xFF, 0xFF, 0x64, 0x00, 0xFF, 0xFF, 0xFF]).is_none()
        );
        assert!(HeartbeatRequest::new(0x40_000, 100).encode().is_err());
    }

    #[test]
    fn heartbeat_request_from_message_binds_pgn_and_source() {
        let request = HeartbeatRequest::for_heartbeat(100);
        let encoded = request.encode().unwrap();
        assert_eq!(
            HeartbeatRequest::from_message(&Message::with_addressing(
                PGN_HEARTBEAT_REQUEST,
                encoded.to_vec(),
                0x41,
                0x42,
                crate::net::Priority::Default,
            )),
            Some(request)
        );
        assert_eq!(
            HeartbeatRequest::from_message(&Message::new(PGN_HEARTBEAT, encoded.to_vec(), 0x41)),
            None
        );
        assert_eq!(
            HeartbeatRequest::from_message(&Message::with_addressing(
                PGN_HEARTBEAT_REQUEST,
                encoded.to_vec(),
                NULL_ADDRESS,
                0x42,
                crate::net::Priority::Default,
            )),
            None
        );
        assert_eq!(
            HeartbeatRequest::from_message(&Message::with_addressing(
                PGN_HEARTBEAT_REQUEST,
                encoded.to_vec(),
                BROADCAST_ADDRESS,
                0x42,
                crate::net::Priority::Default,
            )),
            None
        );
        assert_eq!(
            HeartbeatRequest::from_message(&Message::new(
                PGN_HEARTBEAT_REQUEST,
                encoded.to_vec(),
                0x41
            )),
            None
        );
        assert_eq!(
            HeartbeatRequest::from_message(&Message::with_addressing(
                PGN_HEARTBEAT_REQUEST,
                encoded.to_vec(),
                0x41,
                NULL_ADDRESS,
                crate::net::Priority::Default,
            )),
            None
        );
    }

    // ─── Sender ────────────────────────────────────────────────────

    #[test]
    fn sender_emits_init_first() {
        let mut s = HeartbeatSender::default();
        assert_eq!(s.next_sequence(), hb_seq::INIT);
    }

    #[test]
    fn sender_walks_from_zero_after_init() {
        let mut s = HeartbeatSender::default();
        s.next_sequence(); // INIT
        for expected in 0u8..=5 {
            assert_eq!(s.next_sequence(), expected);
        }
    }

    #[test]
    fn sender_rolls_over_at_max_normal() {
        let mut s = HeartbeatSender::default();
        s.next_sequence(); // INIT
        // Walk to 250 then verify rollover.
        for _ in 0..hb_seq::MAX_NORMAL {
            s.next_sequence();
        }
        assert_eq!(s.next_sequence(), hb_seq::MAX_NORMAL); // 250
        assert_eq!(s.next_sequence(), 0);
    }

    #[test]
    fn sender_signal_error_emits_once_then_resumes_at_zero() {
        let mut s = HeartbeatSender::default();
        s.next_sequence(); // INIT
        s.next_sequence(); // 0
        s.signal_error();
        assert_eq!(s.next_sequence(), hb_seq::SENDER_ERROR);
        // After the special, sequence resumes at 0.
        assert_eq!(s.next_sequence(), 0);
    }

    #[test]
    fn sender_signal_shutdown_emits_once() {
        let mut s = HeartbeatSender::default();
        s.next_sequence(); // INIT
        s.signal_shutdown();
        assert_eq!(s.next_sequence(), hb_seq::SHUTDOWN);
        assert_eq!(s.next_sequence(), 0);
    }

    #[test]
    fn sender_update_fires_at_interval() {
        let mut s = HeartbeatSender::new(100);
        assert!(!s.update(50));
        assert!(s.update(60)); // 110 ms total → fires
    }

    #[test]
    fn sender_long_tick_emits_once_and_preserves_phase() {
        let mut s = HeartbeatSender::new(100);
        assert!(s.update(450));
        assert_eq!(s.timer_ms, 50);
        assert!(!s.update(49));
        assert!(s.update(1));
        assert_eq!(s.timer_ms, 0);
        assert!(!s.update(0));
    }

    #[test]
    fn sender_zero_interval_is_disabled() {
        let mut s = HeartbeatSender::new(0);
        assert!(!s.update(u32::MAX));
        assert_eq!(s.timer_ms, 0);
        assert_eq!(s.next_sequence(), hb_seq::INIT);
    }

    #[test]
    fn sender_reset_returns_to_init() {
        let mut s = HeartbeatSender::default();
        s.next_sequence();
        s.next_sequence();
        s.reset();
        assert_eq!(s.next_sequence(), hb_seq::INIT);
    }

    // ─── Receiver ──────────────────────────────────────────────────

    #[test]
    fn receiver_first_received_silently_records() {
        let mut r = HeartbeatReceiver::new();
        r.process(5);
        assert!(r.first_received);
        assert_eq!(r.last_sequence, 5);
        assert_eq!(r.state(), HbReceiverState::Normal);
    }

    #[test]
    fn receiver_repeated_sequence_triggers_error() {
        let mut r = HeartbeatReceiver::new();
        r.process(5);
        r.process(5); // repeat
        assert_eq!(r.state(), HbReceiverState::SequenceError);
    }

    #[test]
    fn receiver_jump_too_far_triggers_error() {
        let mut r = HeartbeatReceiver::new();
        r.process(5);
        r.process(20); // jump 15 > HB_MAX_JUMP
        assert_eq!(r.state(), HbReceiverState::SequenceError);
    }

    #[test]
    fn receiver_allows_wrap_jump_within_limit() {
        let mut r = HeartbeatReceiver::new();
        r.process(249);
        r.process(1); // 249 → 250 → 0 → 1 is a jump of 3.
        assert_eq!(r.state(), HbReceiverState::Normal);
    }

    #[test]
    fn receiver_rejects_wrap_jump_too_large() {
        let mut r = HeartbeatReceiver::new();
        r.process(249);
        r.process(2); // jump of 4 > HB_MAX_JUMP.
        assert_eq!(r.state(), HbReceiverState::SequenceError);
    }

    #[test]
    fn receiver_recovers_after_eight_good_sequences() {
        let mut r = HeartbeatReceiver::new();
        r.process(5);
        r.process(5); // → SequenceError
        for s in 6u8..=13 {
            // 8 consecutive good sequences
            r.process(s);
        }
        assert_eq!(r.state(), HbReceiverState::Normal);
    }

    #[test]
    fn receiver_comm_error_after_timeout() {
        let mut r = HeartbeatReceiver::new();
        r.process(0); // first_received
        r.update(HB_COMM_ERROR_TIMEOUT_MS + 1);
        assert_eq!(r.state(), HbReceiverState::CommError);
    }

    #[test]
    fn receiver_comm_error_does_not_fire_before_or_at_timeout() {
        let mut r = HeartbeatReceiver::new();
        r.process(0);
        r.update(HB_COMM_ERROR_TIMEOUT_MS);
        assert_eq!(r.state(), HbReceiverState::Normal);
        r.update(1);
        assert_eq!(r.state(), HbReceiverState::CommError);
    }

    #[test]
    fn receiver_recovers_from_comm_error_on_any_heartbeat() {
        let mut r = HeartbeatReceiver::new();
        r.process(0);
        r.update(HB_COMM_ERROR_TIMEOUT_MS + 1);
        assert_eq!(r.state(), HbReceiverState::CommError);
        r.process(1);
        assert_eq!(r.state(), HbReceiverState::Normal);
    }

    #[test]
    fn receiver_special_values_dont_change_state() {
        let mut r = HeartbeatReceiver::new();
        r.process(0);
        let counter = Rc::new(RefCell::new(0u32));
        let c = counter.clone();
        r.on_sender_error.subscribe(move |_| *c.borrow_mut() += 1);

        r.process(hb_seq::SENDER_ERROR);
        r.process(hb_seq::RESERVED_LOW);
        r.process(hb_seq::RESERVED_HIGH);
        assert_eq!(r.state(), HbReceiverState::Normal);
        assert_eq!(*counter.borrow(), 1);
    }

    #[test]
    fn receiver_init_marks_reset() {
        let mut r = HeartbeatReceiver::new();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        r.on_reset_received.subscribe(move |_| *c.borrow_mut() += 1);

        r.process(5);
        r.process(hb_seq::INIT);
        r.process(0); // expect 0 after INIT
        assert_eq!(r.state(), HbReceiverState::Normal);
        assert_eq!(*count.borrow(), 1);
    }

    // ─── Tracker ───────────────────────────────────────────────────

    #[test]
    fn tracker_resets_on_received() {
        let mut t = HeartbeatTracker::new(100);
        t.track(0x10);
        t.update(250);
        let msg = Message::new(PGN_HEARTBEAT, vec![5], 0x10);
        t.handle_message(&msg);
        t.update(60);
        assert_eq!(t.last_sequence(0x10), Some(5));
        assert_eq!(t.missed_count(0x10), 0);
    }

    #[test]
    fn tracker_records_miss_after_3x_interval() {
        let mut t = HeartbeatTracker::new(100);
        t.track(0x10);
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        t.on_heartbeat_missed
            .subscribe(move |_| *c.borrow_mut() += 1);

        t.update(310); // > 300 ms
        assert!(*count.borrow() >= 1);
        assert!(t.missed_count(0x10) >= 1);
    }

    #[test]
    fn tracker_counts_multiple_misses_and_preserves_overshoot() {
        let mut t = HeartbeatTracker::new(100);
        t.track(0x10);
        let events = Rc::new(RefCell::new(Vec::<(Address, u32)>::new()));
        let seen = events.clone();
        t.on_heartbeat_missed
            .subscribe(move |event| seen.borrow_mut().push(*event));

        t.update(650);
        assert_eq!(t.missed_count(0x10), 2);
        assert_eq!(&*events.borrow(), &[(0x10, 1), (0x10, 2)]);

        t.update(249);
        assert_eq!(t.missed_count(0x10), 2);
        t.update(1);
        assert_eq!(t.missed_count(0x10), 3);
        assert_eq!(&*events.borrow(), &[(0x10, 1), (0x10, 2), (0x10, 3)]);
    }

    #[test]
    fn tracker_long_tick_accounts_without_unbounded_event_replay() {
        let mut t = HeartbeatTracker::new(1);
        t.track(0x10);
        let events = Rc::new(RefCell::new(Vec::<(Address, u32)>::new()));
        let seen = events.clone();
        t.on_heartbeat_missed
            .subscribe(move |event| seen.borrow_mut().push(*event));

        t.update(u32::MAX);
        assert_eq!(t.missed_count(0x10), u32::MAX / 3);
        assert_eq!(&*events.borrow(), &[(0x10, u32::MAX / 3)]);
        assert_eq!(t.peers.get(&0x10).unwrap().timer_ms, u32::MAX % 3);
    }

    #[test]
    fn tracker_zero_interval_is_disabled() {
        let mut t = HeartbeatTracker::new(0);
        t.track(0x10);
        t.update(u32::MAX);
        assert_eq!(t.missed_count(0x10), 0);
        assert_eq!(t.peers.get(&0x10).unwrap().timer_ms, 0);
    }

    #[test]
    fn tracker_untrack_removes_peer() {
        let mut t = HeartbeatTracker::new(100);
        t.track(0x10);
        t.untrack(0x10);
        assert_eq!(t.last_sequence(0x10), None);
    }

    proptest! {
        #[test]
        fn proptest_sender_update_is_bounded_and_never_replays_zero_tick(
            interval_ms in 0u32..=10_000,
            elapsed_values in proptest::collection::vec(any::<u32>(), 0..=128),
        ) {
            let mut sender = HeartbeatSender::new(interval_ms);
            for elapsed_ms in elapsed_values {
                let fired = sender.update(elapsed_ms);
                if interval_ms == 0 {
                    prop_assert!(!fired);
                    prop_assert_eq!(sender.timer_ms, 0);
                } else {
                    prop_assert!(sender.timer_ms < interval_ms);
                    prop_assert!(!sender.update(0));
                }
            }
        }

        #[test]
        fn proptest_receiver_accepts_arbitrary_sequence_and_timer_inputs_without_panics(
            actions in proptest::collection::vec((any::<bool>(), any::<u8>(), any::<u32>()), 0..=256),
        ) {
            let mut receiver = HeartbeatReceiver::new();
            for (feed_sequence, sequence, elapsed_ms) in actions {
                if feed_sequence {
                    receiver.process(sequence);
                } else {
                    receiver.update(elapsed_ms);
                }
                prop_assert!(matches!(
                    receiver.state(),
                    HbReceiverState::Normal
                        | HbReceiverState::SequenceError
                        | HbReceiverState::CommError
                ));
                prop_assert!(receiver.recovery_counter <= HB_RECOVERY_COUNT);
                if !receiver.first_received {
                    prop_assert_eq!(receiver.time_since_last_ms, 0);
                }
            }
        }

        #[test]
        fn proptest_tracker_arbitrary_operations_keep_peer_table_bounded(
            interval_ms in 0u32..=10_000,
            operations in proptest::collection::vec(
                (0u8..=3, any::<u8>(), any::<u8>(), any::<u32>()),
                0..=256,
            ),
        ) {
            let mut tracker = HeartbeatTracker::new(interval_ms);
            for (op, address, sequence, elapsed_ms) in operations {
                match op {
                    0 => tracker.track(address),
                    1 => tracker.untrack(address),
                    2 => tracker.handle_message(&Message::new(
                        PGN_HEARTBEAT,
                        vec![sequence],
                        address,
                    )),
                    _ => tracker.update(elapsed_ms),
                }
                prop_assert!(tracker.peers.len() <= usize::from(u8::MAX) + 1);
                for (&peer_address, peer) in &tracker.peers {
                    prop_assert_eq!(tracker.last_sequence(peer_address), Some(peer.last_sequence));
                    prop_assert_eq!(tracker.missed_count(peer_address), peer.missed_count);
                    if interval_ms == 0 {
                        prop_assert_eq!(peer.missed_count, 0);
                    }
                }
            }
        }
    }
}
