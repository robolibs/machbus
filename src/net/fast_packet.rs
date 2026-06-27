//! NMEA2000 Fast Packet Protocol (9–223 byte payloads).
//!
//! Mirrors the C++ `machbus::net::FastPacketProtocol`.
//!
//! # Frame layout
//!
//! - **First frame**: `[seq:3 | frame_counter:5][total_bytes][6 data bytes]`
//! - **Subsequent**: `[seq:3 | frame_counter:5][7 data bytes]`
//!
//! `seq` is the high 3 bits of byte 0 (0..=7), `frame_counter` the
//! low 5 bits (0..=31). The first frame of a transfer has
//! `frame_counter == 0`. Receivers correlate frames by `(source, pgn,
//! seq)`; mismatched frame counters discard the in-flight session.

#[cfg(feature = "default")]
use alloc::vec;
use alloc::vec::Vec;

#[cfg(feature = "embedded")]
use crate::fixed::FixedBytes;
#[cfg(feature = "embedded")]
use crate::fixed::{FixedMessage, FixedSlots};

use super::constants::{
    BROADCAST_ADDRESS, CAN_DATA_LENGTH, FAST_PACKET_MAX_DATA, NULL_ADDRESS, TP_TIMEOUT_T1_MS,
};
use super::error::{Error, Result};
use super::frame::Frame;
use super::identifier::Identifier;
use super::message::Message;
use super::pgn::pgn_is_valid;
use super::session::TransportStats;
use super::types::{Address, Pgn, Priority};

/// Bytes carried by the **first** frame of a fast packet.
pub const FIRST_FRAME_DATA: usize = 6;
/// Bytes carried by every **subsequent** frame.
pub const SUBSEQUENT_FRAME_DATA: usize = 7;

/// Default cap for simultaneous Fast Packet receive sessions.
pub const FAST_PACKET_DEFAULT_MAX_RX_SESSIONS: usize = 32;
#[cfg(feature = "embedded")]
const FAST_PACKET_MAX_DATA_BYTES: usize = FAST_PACKET_MAX_DATA as usize;

#[cfg(feature = "embedded")]
type FastPacketPayload = FixedBytes<FAST_PACKET_MAX_DATA_BYTES>;
#[cfg(feature = "default")]
type FastPacketPayload = Vec<u8>;

#[derive(Debug, Clone)]
struct FastPacketSession {
    pgn: Pgn,
    data: FastPacketPayload,
    total_bytes: u32,
    bytes_received: u32,
    source_address: Address,
    sequence_counter: u8,
    expected_frame: u8,
    timer_ms: u32,
    last_timestamp_us: u64,
}

#[cfg(feature = "embedded")]
type FastPacketSessions = FixedSlots<FastPacketSession, FAST_PACKET_DEFAULT_MAX_RX_SESSIONS>;
#[cfg(feature = "default")]
type FastPacketSessions = Vec<FastPacketSession>;

/// NMEA2000 Fast Packet sender / reassembler.
#[derive(Debug)]
pub struct FastPacketProtocol {
    rx_sessions: FastPacketSessions,
    tx_sequence_counter: u8,
    max_rx_sessions: usize,
    stats: TransportStats,
}

impl Default for FastPacketProtocol {
    fn default() -> Self {
        Self {
            rx_sessions: FastPacketSessions::new(),
            tx_sequence_counter: 0,
            max_rx_sessions: FAST_PACKET_DEFAULT_MAX_RX_SESSIONS,
            stats: TransportStats::default(),
        }
    }
}

impl FastPacketProtocol {
    pub const MAX_DATA_LENGTH: u32 = FAST_PACKET_MAX_DATA;

    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a protocol instance with a cap for in-flight receive sessions.
    #[must_use]
    pub fn with_max_rx_sessions(max_rx_sessions: usize) -> Self {
        let mut this = Self::new();
        this.set_max_rx_sessions(max_rx_sessions);
        this
    }

    /// Set the in-flight receive-session cap. A value of `0` drops all new
    /// multi-frame receive sessions.
    pub fn set_max_rx_sessions(&mut self, max_rx_sessions: usize) {
        #[cfg(feature = "embedded")]
        let max_rx_sessions = max_rx_sessions.min(FAST_PACKET_DEFAULT_MAX_RX_SESSIONS);
        self.max_rx_sessions = max_rx_sessions;
    }

    #[inline]
    #[must_use]
    pub const fn max_rx_sessions(&self) -> usize {
        self.max_rx_sessions
    }

    /// Snapshot the lossy-path counters accumulated by this protocol object.
    #[inline]
    #[must_use]
    pub const fn stats(&self) -> TransportStats {
        self.stats
    }

    /// Reset lossy-path counters without disturbing active sessions.
    #[inline]
    pub fn clear_stats(&mut self) {
        self.stats = TransportStats::default();
    }

    /// Build the frames for a fast-packet transmission.
    ///
    /// Errors with [`ErrorCode::BufferOverflow`] for `data > 223
    /// bytes` and [`ErrorCode::InvalidState`] for `data ≤ 8 bytes`
    /// (use a single CAN frame instead).
    ///
    /// [`ErrorCode::BufferOverflow`]: super::error::ErrorCode::BufferOverflow
    /// [`ErrorCode::InvalidState`]: super::error::ErrorCode::InvalidState
    pub fn send(&mut self, pgn: Pgn, data: &[u8], source: Address) -> Result<Vec<Frame>> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        if !valid_fast_packet_source(source) {
            return Err(Error::invalid_address(source));
        }
        if data.len() as u32 > Self::MAX_DATA_LENGTH {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }
        if data.len() as u32 <= CAN_DATA_LENGTH {
            return Err(Error::invalid_state("use single frame for <= 8 bytes"));
        }

        let seq = (self.tx_sequence_counter & 0x07) << 5;
        self.tx_sequence_counter = self.tx_sequence_counter.wrapping_add(1);

        let total_frames =
            1 + (data.len() - FIRST_FRAME_DATA).div_ceil(SUBSEQUENT_FRAME_DATA) as u8;

        let mut frames = Vec::with_capacity(total_frames as usize);
        let id = Identifier::encode(
            Priority::Default,
            pgn,
            source,
            super::constants::BROADCAST_ADDRESS,
        );

        // First frame: byte 0 = seq | 0, byte 1 = total length, bytes 2..8 = first 6 data bytes.
        let mut first = [0xFFu8; 8];
        first[0] = seq; // frame counter = 0
        first[1] = data.len() as u8;
        let n_first = FIRST_FRAME_DATA.min(data.len());
        first[2..2 + n_first].copy_from_slice(&data[..n_first]);
        frames.push(Frame::new(id, first, 8));

        // Subsequent frames.
        let mut offset = FIRST_FRAME_DATA;
        for frame_num in 1..total_frames {
            let mut buf = [0xFFu8; 8];
            buf[0] = seq | frame_num;
            for i in 0..SUBSEQUENT_FRAME_DATA {
                if offset + i < data.len() {
                    buf[i + 1] = data[offset + i];
                }
            }
            frames.push(Frame::new(id, buf, 8));
            offset += SUBSEQUENT_FRAME_DATA;
        }

        tracing::debug!(
            target: "machbus.transport.fp",
            pgn = pgn,
            bytes = data.len(),
            "fast packet sent",
        );
        Ok(frames)
    }

    /// Build the frames for a fast-packet transmission into fixed-capacity
    /// inline storage.
    ///
    /// Available with `embedded-fixed`. The caller chooses `N`; use `N >= 32`
    /// to cover the protocol maximum of 223 bytes. Smaller values are useful
    /// when an application has a tighter PGN-specific payload bound.
    #[cfg(feature = "embedded")]
    pub fn send_fixed<const N: usize>(
        &mut self,
        pgn: Pgn,
        data: &[u8],
        source: Address,
    ) -> Result<FixedSlots<Frame, N>> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        if !valid_fast_packet_source(source) {
            return Err(Error::invalid_address(source));
        }
        if data.len() as u32 > Self::MAX_DATA_LENGTH {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }
        if data.len() as u32 <= CAN_DATA_LENGTH {
            return Err(Error::invalid_state("use single frame for <= 8 bytes"));
        }

        let total_frames = 1 + (data.len() - FIRST_FRAME_DATA).div_ceil(SUBSEQUENT_FRAME_DATA);
        if total_frames > N {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }

        let seq = (self.tx_sequence_counter & 0x07) << 5;
        self.tx_sequence_counter = self.tx_sequence_counter.wrapping_add(1);
        let id = Identifier::encode(Priority::Default, pgn, source, BROADCAST_ADDRESS);
        let mut frames = FixedSlots::new();

        let mut first = [0xFFu8; 8];
        first[0] = seq;
        first[1] = data.len() as u8;
        let n_first = FIRST_FRAME_DATA.min(data.len());
        first[2..2 + n_first].copy_from_slice(&data[..n_first]);
        frames
            .push(Frame::new(id, first, 8))
            .map_err(|_| Error::buffer_overflow())?;

        let mut offset = FIRST_FRAME_DATA;
        for frame_num in 1..total_frames {
            let mut buf = [0xFFu8; 8];
            buf[0] = seq | frame_num as u8;
            for i in 0..SUBSEQUENT_FRAME_DATA {
                if offset + i < data.len() {
                    buf[i + 1] = data[offset + i];
                }
            }
            frames
                .push(Frame::new(id, buf, 8))
                .map_err(|_| Error::buffer_overflow())?;
            offset += SUBSEQUENT_FRAME_DATA;
        }

        Ok(frames)
    }

    /// Process an incoming fast-packet frame. Returns the reassembled
    /// [`Message`] when the final frame completes a session, otherwise
    /// [`None`].
    pub fn process_frame(&mut self, frame: &Frame) -> Option<Message> {
        self.process_frame_session(frame)
            .map(|session| make_message(&session))
    }

    /// Fixed-capacity variant of [`Self::process_frame`].
    ///
    /// Available with `embedded-fixed`. This lets embedded callers receive a
    /// completed Fast Packet payload without allocating the returned
    /// heap-backed [`Message`]. Choose `N >= 223` to cover the protocol
    /// maximum, or a smaller PGN-specific bound when known.
    #[cfg(feature = "embedded")]
    pub fn process_frame_fixed<const N: usize>(
        &mut self,
        frame: &Frame,
    ) -> Result<Option<FixedMessage<N>>> {
        match self.process_frame_session(frame) {
            Some(session) => match make_fixed_message(&session) {
                Ok(message) => Ok(Some(message)),
                Err(error) => {
                    self.stats.resource_rejections =
                        self.stats.resource_rejections.saturating_add(1);
                    Err(error)
                }
            },
            None => Ok(None),
        }
    }

    fn process_frame_session(&mut self, frame: &Frame) -> Option<FastPacketSession> {
        if frame.length != CAN_DATA_LENGTH as u8 {
            tracing::warn!(
                target: "machbus.transport.fp",
                length = frame.length,
                "dropping non-classic-length fast-packet frame",
            );
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            return None;
        }

        let frame_counter = frame.data[0] & 0x1F;
        let seq_counter = (frame.data[0] >> 5) & 0x07;
        let src = frame.source();
        let pgn = frame.pgn();

        if !valid_fast_packet_source(src) {
            tracing::warn!(
                target: "machbus.transport.fp",
                source = src,
                "dropping fast-packet frame from invalid source address",
            );
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            return None;
        }

        if frame_counter == 0 {
            return self.start_new_session(frame, pgn, src, seq_counter);
        }

        // Find a matching in-progress session.
        let Some(idx) = self.rx_sessions.iter().position(|s| {
            s.source_address == src && s.pgn == pgn && s.sequence_counter == seq_counter
        }) else {
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            return None;
        };

        // Check sequence ordering.
        let expected = self.rx_sessions[idx].expected_frame;
        if frame_counter != expected {
            tracing::warn!(
                target: "machbus.transport.fp",
                expected = expected,
                got = frame_counter,
                "bad sequence — discarding session",
            );
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            self.stats.dropped_sessions = self.stats.dropped_sessions.saturating_add(1);
            self.remove_rx_session(idx);
            return None;
        }

        let session = &mut self.rx_sessions[idx];
        let offset = FIRST_FRAME_DATA + (frame_counter as usize - 1) * SUBSEQUENT_FRAME_DATA;
        let payload = payload_as_mut_slice(&mut session.data);
        for i in 0..SUBSEQUENT_FRAME_DATA {
            let abs = offset + i;
            if abs < session.total_bytes as usize {
                payload[abs] = frame.data[i + 1];
            }
        }
        session.bytes_received = ((offset + SUBSEQUENT_FRAME_DATA) as u32).min(session.total_bytes);
        session.expected_frame = session.expected_frame.wrapping_add(1);
        session.timer_ms = 0;
        session.last_timestamp_us = frame.timestamp_us;

        if session.bytes_received >= session.total_bytes {
            Some(self.remove_rx_session(idx))
        } else {
            None
        }
    }

    /// Drive RX-session timeouts. Sessions with no progress for
    /// [`TP_TIMEOUT_T1_MS`] are dropped.
    pub fn update(&mut self, elapsed_ms: u32) {
        let mut i = 0;
        while i < self.rx_sessions.len() {
            self.rx_sessions[i].timer_ms = self.rx_sessions[i].timer_ms.saturating_add(elapsed_ms);
            if self.rx_sessions[i].timer_ms >= TP_TIMEOUT_T1_MS {
                tracing::warn!(
                    target: "machbus.transport.fp",
                    pgn = self.rx_sessions[i].pgn,
                    "fast packet timeout",
                );
                self.stats.timeouts = self.stats.timeouts.saturating_add(1);
                self.stats.dropped_sessions = self.stats.dropped_sessions.saturating_add(1);
                self.rx_sessions.swap_remove(i);
            } else {
                i += 1;
            }
        }
    }

    /// Number of in-flight RX sessions (test/diagnostic helper).
    #[inline]
    #[must_use]
    pub fn rx_session_count(&self) -> usize {
        self.rx_sessions.len()
    }

    fn start_new_session(
        &mut self,
        frame: &Frame,
        pgn: Pgn,
        src: Address,
        seq_counter: u8,
    ) -> Option<FastPacketSession> {
        let total_bytes = frame.data[1] as u32;
        if total_bytes <= CAN_DATA_LENGTH || total_bytes > Self::MAX_DATA_LENGTH {
            tracing::warn!(
                target: "machbus.transport.fp",
                bytes = total_bytes,
                max = Self::MAX_DATA_LENGTH,
                "dropping malformed fast-packet first frame",
            );
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            if total_bytes > Self::MAX_DATA_LENGTH {
                self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            }
            return None;
        }
        let copy_len = (total_bytes as usize).min(FIRST_FRAME_DATA);
        let Some(data) = new_rx_payload(total_bytes as usize, &frame.data[2..2 + copy_len]) else {
            tracing::warn!(
                target: "machbus.transport.fp",
                bytes = total_bytes,
                "dropping fast-packet first frame because fixed payload storage is too small",
            );
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return None;
        };

        let session = FastPacketSession {
            pgn,
            data,
            total_bytes,
            bytes_received: copy_len as u32,
            source_address: src,
            sequence_counter: seq_counter,
            expected_frame: 1,
            timer_ms: 0,
            last_timestamp_us: frame.timestamp_us,
        };

        if session.bytes_received >= session.total_bytes {
            // Single-frame fast packet: complete in one shot.
            return Some(session);
        }

        // Replace any existing session for this (src, pgn) — sender
        // restarted before completing the previous one with the same sequence
        // counter. Different sequence counters may be in-flight in parallel.
        let existing_idx = self.rx_sessions.iter().position(|s| {
            s.source_address == src && s.pgn == pgn && s.sequence_counter == seq_counter
        });
        if let Some(idx) = existing_idx {
            self.stats.dropped_sessions = self.stats.dropped_sessions.saturating_add(1);
            self.remove_rx_session(idx);
        }

        if self.rx_sessions.len() >= self.max_rx_sessions {
            tracing::warn!(
                target: "machbus.transport.fp",
                max_rx_sessions = self.max_rx_sessions,
                "dropping fast-packet first frame because session cap is full",
            );
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return None;
        }

        if self.push_rx_session(session).is_err() {
            tracing::warn!(
                target: "machbus.transport.fp",
                "dropping fast-packet first frame because fixed session storage is full",
            );
            self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
        }
        None
    }

    #[cfg(feature = "embedded")]
    fn push_rx_session(&mut self, session: FastPacketSession) -> core::result::Result<(), ()> {
        self.rx_sessions.push(session).map_err(|_| ())
    }

    #[cfg(feature = "default")]
    fn push_rx_session(&mut self, session: FastPacketSession) -> core::result::Result<(), ()> {
        self.rx_sessions.push(session);
        Ok(())
    }

    #[cfg(feature = "embedded")]
    fn remove_rx_session(&mut self, idx: usize) -> FastPacketSession {
        self.rx_sessions
            .swap_remove(idx)
            .expect("rx session index came from active session table")
    }

    #[cfg(feature = "default")]
    fn remove_rx_session(&mut self, idx: usize) -> FastPacketSession {
        self.rx_sessions.swap_remove(idx)
    }
}

fn make_message(session: &FastPacketSession) -> Message {
    Message {
        pgn: session.pgn,
        data: payload_as_slice(&session.data).to_vec(),
        source: session.source_address,
        destination: super::constants::BROADCAST_ADDRESS,
        priority: Priority::Default,
        timestamp_us: session.last_timestamp_us,
    }
}

#[cfg(feature = "embedded")]
fn make_fixed_message<const N: usize>(session: &FastPacketSession) -> Result<FixedMessage<N>> {
    FixedMessage::with_addressing(
        session.pgn,
        payload_as_slice(&session.data),
        session.source_address,
        super::constants::BROADCAST_ADDRESS,
        Priority::Default,
    )
    .map(|mut message| {
        message.timestamp_us = session.last_timestamp_us;
        message
    })
    .map_err(|_| Error::buffer_overflow())
}

#[cfg(feature = "embedded")]
fn new_rx_payload(total_bytes: usize, first: &[u8]) -> Option<FastPacketPayload> {
    let mut data = FastPacketPayload::new();
    data.resize(total_bytes, 0xFF).ok()?;
    data.as_mut_slice()[..first.len()].copy_from_slice(first);
    Some(data)
}

#[cfg(feature = "default")]
fn new_rx_payload(total_bytes: usize, first: &[u8]) -> Option<FastPacketPayload> {
    let mut data = vec![0xFFu8; total_bytes];
    data[..first.len()].copy_from_slice(first);
    Some(data)
}

#[cfg(feature = "embedded")]
fn payload_as_slice(payload: &FastPacketPayload) -> &[u8] {
    payload.as_slice()
}

#[cfg(feature = "default")]
fn payload_as_slice(payload: &FastPacketPayload) -> &[u8] {
    payload
}

#[cfg(feature = "embedded")]
fn payload_as_mut_slice(payload: &mut FastPacketPayload) -> &mut [u8] {
    payload.as_mut_slice()
}

#[cfg(feature = "default")]
fn payload_as_mut_slice(payload: &mut FastPacketPayload) -> &mut [u8] {
    payload
}

#[inline]
const fn valid_fast_packet_source(source: Address) -> bool {
    source != NULL_ADDRESS && source != BROADCAST_ADDRESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn pick_frames(payload: &[u8], pgn: Pgn, src: Address) -> Vec<Frame> {
        let mut p = FastPacketProtocol::new();
        p.send(pgn, payload, src).expect("send ok")
    }

    #[test]
    fn send_too_small_is_invalid() {
        let mut p = FastPacketProtocol::new();
        let r = p.send(0xF010, &[0u8; 8], 0x10);
        assert!(r.is_err());
    }

    #[test]
    fn send_too_large_overflows() {
        let mut p = FastPacketProtocol::new();
        let r = p.send(0xF010, &vec![0u8; 224], 0x10);
        assert!(r.is_err());
        assert_eq!(p.stats().resource_rejections, 1);
    }

    #[test]
    fn send_rejects_invalid_pgn_before_identifier_normalization() {
        let mut p = FastPacketProtocol::new();
        let err = p.send(0x4_0000, &[0u8; 9], 0x10).unwrap_err();
        assert_eq!(err.code, super::super::error::ErrorCode::InvalidPgn);
        assert!(
            p.stats().is_empty(),
            "invalid PGNs must fail before allocating transport resources"
        );
    }

    #[test]
    fn send_rejects_null_and_broadcast_source_addresses() {
        for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut p = FastPacketProtocol::new();
            let err = p.send(0xF010, &[0u8; 9], source).unwrap_err();
            assert_eq!(err.code, super::super::error::ErrorCode::InvalidAddress);
            assert!(p.stats().is_empty());
        }
    }

    #[test]
    fn receive_rejects_invalid_source_addresses_before_session_mutation() {
        for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let id = Identifier::encode(Priority::Default, 0xF010, source, BROADCAST_ADDRESS);
            let first = Frame::new(id, [0, 20, 0, 1, 2, 3, 4, 5], 8);
            let mut rx = FastPacketProtocol::new();

            assert!(rx.process_frame(&first).is_none());
            assert_eq!(rx.rx_session_count(), 0);
            assert_eq!(rx.stats().dropped_frames, 1);
        }
    }

    #[test]
    fn receive_respects_zero_session_cap() {
        let frames = pick_frames(&(0..20u8).collect::<Vec<_>>(), 0xF010, 0x10);
        let mut rx = FastPacketProtocol::with_max_rx_sessions(0);

        assert!(rx.process_frame(&frames[0]).is_none());
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().resource_rejections, 1);
    }

    #[test]
    fn receive_drops_new_session_when_cap_is_full() {
        let frames_a = pick_frames(&(0..20u8).collect::<Vec<_>>(), 0xF010, 0x10);
        let frames_b = pick_frames(&(100..120u8).collect::<Vec<_>>(), 0xF010, 0x20);
        let mut rx = FastPacketProtocol::with_max_rx_sessions(1);

        assert!(rx.process_frame(&frames_a[0]).is_none());
        assert_eq!(rx.rx_session_count(), 1);
        assert!(rx.process_frame(&frames_b[0]).is_none());
        assert_eq!(rx.rx_session_count(), 1);
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().resource_rejections, 1);

        let mut completed = None;
        for frame in frames_a.iter().skip(1) {
            completed = rx.process_frame(frame).or(completed);
        }
        assert_eq!(completed.expect("first session completes").source, 0x10);
    }

    #[test]
    fn nine_byte_payload_fits_two_frames() {
        let payload: Vec<u8> = (0..9).collect();
        let frames = pick_frames(&payload, 0xF010, 0x10);
        assert_eq!(frames.len(), 2);
        // First frame: byte 0 = seq|0, byte 1 = total = 9, bytes 2..8 = first 6.
        assert_eq!(frames[0].data[0] & 0x1F, 0); // frame counter 0
        assert_eq!(frames[0].data[1], 9);
        assert_eq!(&frames[0].data[2..8], &payload[..6]);
        // Second frame: bytes 1..8 hold the remaining 3 + 4 padding.
        assert_eq!(frames[1].data[0] & 0x1F, 1);
        assert_eq!(&frames[1].data[1..4], &payload[6..9]);
        assert_eq!(&frames[1].data[4..8], &[0xFF; 4]);
    }

    #[test]
    fn round_trip_through_protocol() {
        let payload: Vec<u8> = (0..50).collect();
        let frames = pick_frames(&payload, 0xF010, 0x10);

        let mut rx = FastPacketProtocol::new();
        let mut completed: Option<Message> = None;
        for f in &frames {
            if let Some(m) = rx.process_frame(f) {
                completed = Some(m);
                break;
            }
        }
        let msg = completed.expect("session completed");
        assert_eq!(msg.pgn, 0xF010);
        assert_eq!(msg.source, 0x10);
        assert_eq!(msg.data, payload);
        assert_eq!(rx.rx_session_count(), 0);
    }

    #[test]
    fn reassembled_message_uses_last_frame_timestamp() {
        let payload: Vec<u8> = (0..20).collect();
        let mut frames = pick_frames(&payload, 0xF010, 0x10);
        for (idx, frame) in frames.iter_mut().enumerate() {
            frame.timestamp_us = 1_000 + idx as u64 * 250;
        }

        let mut rx = FastPacketProtocol::new();
        let mut completed: Option<Message> = None;
        for f in &frames {
            completed = rx.process_frame(f).or(completed);
        }

        let msg = completed.expect("session completed");
        assert_eq!(msg.data, payload);
        assert_eq!(msg.timestamp_us, frames.last().unwrap().timestamp_us);
    }

    #[test]
    fn round_trip_max_size() {
        let payload: Vec<u8> = (0..223u32).map(|n| (n & 0xFF) as u8).collect();
        let frames = pick_frames(&payload, 0xF010, 0x10);

        let mut rx = FastPacketProtocol::new();
        let mut completed: Option<Message> = None;
        for f in &frames {
            if let Some(m) = rx.process_frame(f) {
                completed = Some(m);
            }
        }
        let msg = completed.expect("session completed");
        assert_eq!(msg.data, payload);
    }

    #[test]
    fn malformed_first_frame_lengths_are_rejected() {
        let id = Identifier::encode(Priority::Default, 0xF010, 0x10, 0xFF);
        let mut rx = FastPacketProtocol::new();

        let too_small = Frame::new(id, [0, 8, 0, 1, 2, 3, 4, 5], 8);
        assert!(rx.process_frame(&too_small).is_none());
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 1);

        let too_large = Frame::new(id, [0, 224, 0, 1, 2, 3, 4, 5], 8);
        assert!(rx.process_frame(&too_large).is_none());
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 2);
        assert_eq!(rx.stats().resource_rejections, 1);
    }

    #[test]
    fn short_frame_is_dropped_before_reassembly() {
        let id = Identifier::encode(Priority::Default, 0xF010, 0x10, 0xFF);
        let short = Frame::new(id, [0, 20, 0, 1, 2, 3, 4, 5], 7);
        let mut rx = FastPacketProtocol::new();

        assert!(rx.process_frame(&short).is_none());
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    #[test]
    fn orphan_subsequent_frame_is_counted_as_drop() {
        let frames = pick_frames(&(0..20u8).collect::<Vec<_>>(), 0xF010, 0x10);
        let mut rx = FastPacketProtocol::new();

        assert!(rx.process_frame(&frames[1]).is_none());
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    #[test]
    fn out_of_order_subsequent_frame_drops_session() {
        let payload: Vec<u8> = (0..30).collect();
        let frames = pick_frames(&payload, 0xF010, 0x10);
        let mut rx = FastPacketProtocol::new();

        // Feed first frame normally.
        assert!(rx.process_frame(&frames[0]).is_none());
        assert_eq!(rx.rx_session_count(), 1);

        // Skip the expected frame_counter=1, send frame_counter=2.
        assert!(rx.process_frame(&frames[2]).is_none());
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
    }

    #[test]
    fn restart_replaces_existing_session_for_same_src_pgn() {
        let payload: Vec<u8> = (0..30).collect();
        let frames = pick_frames(&payload, 0xF010, 0x10);
        let mut rx = FastPacketProtocol::new();
        rx.process_frame(&frames[0]);
        assert_eq!(rx.rx_session_count(), 1);

        // Sender starts again (frame_counter == 0).
        let mut p2 = FastPacketProtocol::new();
        let frames2 = p2
            .send(
                0xF010,
                &(0..40u32).map(|n| n as u8).collect::<Vec<_>>(),
                0x10,
            )
            .unwrap();
        rx.process_frame(&frames2[0]);
        assert_eq!(rx.rx_session_count(), 1); // still 1 — replaced, not added
        assert_eq!(rx.stats().dropped_sessions, 1);
    }

    #[test]
    fn same_source_pgn_different_sequence_counters_can_interleave() {
        let payload_a: Vec<u8> = (0..20).collect();
        let payload_b: Vec<u8> = (100..120).map(|n| n as u8).collect();
        let mut sender = FastPacketProtocol::new();
        let frames_a = sender.send(0xF010, &payload_a, 0x10).unwrap();
        let frames_b = sender.send(0xF010, &payload_b, 0x10).unwrap();
        assert_ne!(frames_a[0].data[0] >> 5, frames_b[0].data[0] >> 5);

        let mut rx = FastPacketProtocol::new();
        assert!(rx.process_frame(&frames_a[0]).is_none());
        assert!(rx.process_frame(&frames_b[0]).is_none());
        assert_eq!(rx.rx_session_count(), 2);

        let mut got_a = None;
        let mut got_b = None;
        for f in frames_a
            .iter()
            .skip(1)
            .zip(frames_b.iter().skip(1))
            .flat_map(|(a, b)| [a, b])
        {
            if let Some(m) = rx.process_frame(f) {
                if m.data == payload_a {
                    got_a = Some(m);
                } else if m.data == payload_b {
                    got_b = Some(m);
                }
            }
        }

        assert!(got_a.is_some());
        assert!(got_b.is_some());
        assert_eq!(rx.rx_session_count(), 0);
    }

    #[test]
    fn send_sequence_counter_wraps_after_eight_transfers_and_replaces_old_rx_session() {
        let pgn = 0xF010;
        let source = 0x10;
        let mut tx = FastPacketProtocol::new();
        let payloads: Vec<Vec<u8>> = (0..9u8)
            .map(|base| (0..10u8).map(|offset| base * 16 + offset).collect())
            .collect();
        let streams: Vec<Vec<Frame>> = payloads
            .iter()
            .map(|payload| tx.send(pgn, payload, source).expect("send ok"))
            .collect();

        for (idx, frames) in streams.iter().enumerate() {
            assert_eq!(frames[0].data[0] >> 5, (idx as u8) & 0x07);
            assert_eq!(frames[0].data[0] & 0x1F, 0);
        }
        assert_eq!(streams[0][0].data[0] >> 5, streams[8][0].data[0] >> 5);

        let mut rx = FastPacketProtocol::new();
        for frames in streams.iter().take(8) {
            assert!(rx.process_frame(&frames[0]).is_none());
        }
        assert_eq!(rx.rx_session_count(), 8);

        // The ninth transfer wraps back to sequence 0. A new first frame for
        // the same `(source, PGN, seq)` replaces the stale in-flight seq-0
        // reassembly state instead of growing an unbounded duplicate session.
        assert!(rx.process_frame(&streams[8][0]).is_none());
        assert_eq!(rx.rx_session_count(), 8);

        let msg = rx
            .process_frame(&streams[8][1])
            .expect("wrapped seq-0 stream completes");
        assert_eq!(msg.data, payloads[8]);
        assert_eq!(rx.rx_session_count(), 7);
    }

    #[test]
    fn timeout_drops_pending_session() {
        let payload: Vec<u8> = (0..30).collect();
        let frames = pick_frames(&payload, 0xF010, 0x10);
        let mut rx = FastPacketProtocol::new();
        rx.process_frame(&frames[0]);
        assert_eq!(rx.rx_session_count(), 1);

        rx.update(TP_TIMEOUT_T1_MS / 2);
        assert_eq!(rx.rx_session_count(), 1);
        rx.update(TP_TIMEOUT_T1_MS);
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().timeouts, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
        rx.clear_stats();
        assert!(rx.stats().is_empty());
    }

    #[test]
    fn timeout_drops_parallel_nonzero_sequence_sessions() {
        let mut tx = FastPacketProtocol::new();
        let frames_a = tx
            .send(0xF010, &(0..30u8).collect::<Vec<_>>(), 0x10)
            .unwrap();
        let frames_b = tx
            .send(0xF010, &(100..130u8).collect::<Vec<_>>(), 0x10)
            .unwrap();
        assert_eq!(frames_a[0].data[0] >> 5, 0);
        assert_eq!(frames_b[0].data[0] >> 5, 1);

        let mut rx = FastPacketProtocol::new();
        assert!(rx.process_frame(&frames_a[0]).is_none());
        assert!(rx.process_frame(&frames_b[0]).is_none());
        assert_eq!(rx.rx_session_count(), 2);

        rx.update(TP_TIMEOUT_T1_MS);

        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().timeouts, 2);
        assert_eq!(rx.stats().dropped_sessions, 2);
    }

    #[test]
    fn parallel_sessions_for_different_sources_dont_collide() {
        let payload_a: Vec<u8> = (0..20).collect();
        let payload_b: Vec<u8> = (100..130).map(|n| n as u8).collect();
        let frames_a = pick_frames(&payload_a, 0xF010, 0x10);
        let frames_b = pick_frames(&payload_b, 0xF010, 0x20);

        let mut rx = FastPacketProtocol::new();
        // Interleave first frames, then continuation frames.
        rx.process_frame(&frames_a[0]);
        rx.process_frame(&frames_b[0]);
        assert_eq!(rx.rx_session_count(), 2);

        let mut got_a = None;
        let mut got_b = None;
        for f in frames_a.iter().skip(1).chain(frames_b.iter().skip(1)) {
            if let Some(m) = rx.process_frame(f) {
                if m.source == 0x10 {
                    got_a = Some(m);
                } else if m.source == 0x20 {
                    got_b = Some(m);
                }
            }
        }
        let msg_a = got_a.expect("source 0x10 completed");
        let msg_b = got_b.expect("source 0x20 completed");
        assert_eq!(msg_a.data, payload_a);
        assert_eq!(msg_b.data, payload_b);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(96))]

        #[test]
        fn proptest_fast_packet_round_trip_completes_only_after_final_frame(
            payload in proptest::collection::vec(any::<u8>(), 9..=FAST_PACKET_MAX_DATA as usize),
            pgn_raw in any::<u32>(),
            source in 0u8..=0xFD,
        ) {
            let pgn = pgn_raw & 0x3_FFFF;
            let frames = pick_frames(&payload, pgn, source);
            prop_assert!(frames.len() >= 2);
            let expected_pgn = frames[0].pgn();

            let mut rx = FastPacketProtocol::new();
            for frame in frames.iter().take(frames.len() - 1) {
                prop_assert!(rx.process_frame(frame).is_none());
                prop_assert!(rx.rx_session_count() <= rx.max_rx_sessions());
            }

            let completed = rx
                .process_frame(frames.last().expect("non-empty frame list"))
                .expect("final frame should complete valid fast-packet stream");
            prop_assert_eq!(completed.pgn, expected_pgn);
            prop_assert_eq!(completed.source, source);
            prop_assert_eq!(completed.data, payload);
            prop_assert_eq!(rx.rx_session_count(), 0);
            prop_assert!(rx.stats().is_empty());
        }

        #[test]
        fn proptest_fast_packet_rx_arbitrary_frames_are_bounded(
            inputs in proptest::collection::vec(
                (
                    any::<[u8; 8]>(),
                    0u8..=8,
                    any::<u32>(),
                    any::<u8>(),
                    any::<u8>(),
                    any::<u8>(),
                ),
                0..=128,
            ),
            cap in 0usize..=8,
            elapsed_ms in any::<u32>(),
        ) {
            let mut rx = FastPacketProtocol::with_max_rx_sessions(cap);

            for (data, len, pgn_raw, source, dest, priority) in inputs {
                let frame = Frame::new(
                    Identifier::encode(
                        Priority::from_u8(priority),
                        pgn_raw & 0x3_FFFF,
                        source,
                        dest,
                    ),
                    data,
                    len,
                );

                if let Some(msg) = rx.process_frame(&frame) {
                    prop_assert!((9..=FAST_PACKET_MAX_DATA as usize).contains(&msg.data.len()));
                    prop_assert_eq!(msg.destination, super::super::constants::BROADCAST_ADDRESS);
                }

                prop_assert!(rx.rx_session_count() <= cap);
            }

            rx.update(elapsed_ms);
            prop_assert!(rx.rx_session_count() <= cap);

            let stats = rx.stats();
            prop_assert_eq!(stats.aborts_sent, 0);
            prop_assert_eq!(stats.aborts_received, 0);
        }
    }
}
