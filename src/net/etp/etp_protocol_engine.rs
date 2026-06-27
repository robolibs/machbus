use alloc::{format, vec, vec::Vec};

#[cfg(feature = "embedded")]
use crate::fixed::{FixedBytes, FixedMessage, FixedSlots};

use super::constants::{
    BROADCAST_ADDRESS, ETP_MAX_DATA_LENGTH, ETP_TIMEOUT_T1_MS, NULL_ADDRESS, TP_BYTES_PER_FRAME,
    TP_MAX_DATA_LENGTH, TP_MAX_PACKETS_PER_CTS,
};
use super::error::{Error, Result};
use super::event::Event;
use super::frame::Frame;
use super::identifier::Identifier;
use super::pgn::pgn_is_valid;
use super::pgn_defs::{PGN_ETP_CM, PGN_ETP_DT};
use super::session::{
    SessionState, TransportAbortEvent, TransportAbortReason, TransportDirection, TransportSession,
    TransportStats,
};
use super::types::{Address, Pgn, Priority};

// ─── ETP Connection-Management byte codes ──────────────────────────────
pub mod etp_cm {
    pub const RTS: u8 = 0x14;
    pub const CTS: u8 = 0x15;
    pub const DPO: u8 = 0x16;
    pub const EOMA: u8 = 0x17;
    pub const ABORT: u8 = 0xFF;
}

/// Default cap for simultaneous ETP transmit and receive sessions.
pub const ETP_DEFAULT_MAX_SESSIONS: usize = 16;

#[cfg(feature = "embedded")]
type EtpSessions = FixedSlots<TransportSession, ETP_DEFAULT_MAX_SESSIONS>;
#[cfg(feature = "default")]
type EtpSessions = Vec<TransportSession>;

/// Receive-admission summary for an advertised ETP transfer size.
///
/// This is intentionally allocation-free so very large, protocol-maximum
/// transfer profiles can be audited without reserving the advertised payload
/// buffer. Actual receive-session allocation still happens only after an RTS
/// frame is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EtpReceiveProfile {
    pub total_bytes: u32,
    pub total_packets: u32,
    pub max_receive_bytes: u32,
}

/// Borrowed, allocation-free ETP/CMDT transmit helper for `embedded-fixed`.
///
/// This mirrors the lower-level [`crate::net::TpCmdtTx`] shape for payloads
/// larger than TP can carry. The application owns the payload slice, sends
/// [`Self::rts`], converts peer CTS windows into [`Self::set_window`], and
/// transmits the DPO + DT frames returned by
/// [`Self::pending_data_frames_fixed`].
#[cfg(feature = "embedded")]
#[derive(Debug, Clone)]
pub struct EtpCmdtTx<'a> {
    pgn: Pgn,
    payload: &'a [u8],
    source: Address,
    destination: Address,
    total_bytes: u32,
    total_packets: u32,
    next_packet: u32,
    packets_to_send: u8,
}

#[cfg(feature = "embedded")]
impl<'a> EtpCmdtTx<'a> {
    /// Create a borrowed ETP/CMDT transmit session.
    pub fn new(pgn: Pgn, payload: &'a [u8], source: Address, destination: Address) -> Result<Self> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        if !valid_etp_source(source) {
            return Err(Error::invalid_address(source));
        }
        if !valid_etp_destination(destination) {
            return Err(Error::invalid_address(destination));
        }
        if payload.len() as u32 > ETP_MAX_DATA_LENGTH {
            return Err(Error::buffer_overflow());
        }
        if payload.len() as u32 <= TP_MAX_DATA_LENGTH {
            return Err(Error::invalid_state("use TP for <= 1785 bytes"));
        }

        let total_bytes = payload.len() as u32;
        Ok(Self {
            pgn,
            payload,
            source,
            destination,
            total_bytes,
            total_packets: total_bytes.div_ceil(TP_BYTES_PER_FRAME),
            next_packet: 1,
            packets_to_send: 0,
        })
    }

    /// Initial ETP.RTS frame to send to the peer.
    #[must_use]
    pub fn rts(&self) -> Frame {
        make_rts_fields(self.pgn, self.total_bytes, self.source, self.destination)
    }

    /// Apply a peer ETP.CTS window.
    ///
    /// A zero-packet CTS hold is accepted and produces no pending DPO/DT
    /// frames. Non-zero windows are capped to the ETP/TP maximum packets per
    /// CTS and to the remaining payload.
    pub fn set_window(&mut self, next_packet: u32, packets: u8) -> Result<()> {
        if packets == 0 {
            self.next_packet = next_packet.max(1).min(self.total_packets);
            self.packets_to_send = 0;
            return Ok(());
        }
        if next_packet == 0 || next_packet > self.total_packets {
            return Err(Error::invalid_state("invalid ETP CTS next packet"));
        }

        let remaining = self.total_packets - (next_packet - 1);
        self.next_packet = next_packet;
        self.packets_to_send = packets.min(remaining.min(TP_MAX_PACKETS_PER_CTS) as u8);
        Ok(())
    }

    /// Emit DPO plus pending DT frames for the current CTS window into fixed
    /// storage.
    pub fn pending_data_frames_fixed<const N: usize>(&mut self) -> Result<FixedSlots<Frame, N>> {
        if self.packets_to_send == 0 {
            return Ok(FixedSlots::new());
        }
        let needed = 1usize.saturating_add(self.packets_to_send as usize);
        if needed > N {
            return Err(Error::buffer_overflow());
        }

        let mut frames = FixedSlots::new();
        frames
            .push(make_dpo_fields(
                self.pgn,
                self.source,
                self.destination,
                self.packets_to_send,
                self.next_packet - 1,
            ))
            .map_err(|_| Error::buffer_overflow())?;

        for window_index in 0..self.packets_to_send {
            let packet = self.next_packet + window_index as u32;
            let payload_offset = (packet - 1) as usize * TP_BYTES_PER_FRAME as usize;
            let mut data = [0xFFu8; 8];
            data[0] = window_index + 1;
            for j in 0..TP_BYTES_PER_FRAME as usize {
                let idx = payload_offset + j;
                if idx < self.payload.len() {
                    data[j + 1] = self.payload[idx];
                }
            }
            let id =
                Identifier::encode(Priority::Lowest, PGN_ETP_DT, self.source, self.destination);
            frames
                .push(Frame::new(id, data, 8))
                .map_err(|_| Error::buffer_overflow())?;
        }

        self.next_packet = self.next_packet.saturating_add(self.packets_to_send as u32);
        self.packets_to_send = 0;
        Ok(frames)
    }

    /// True once all payload packets have been emitted.
    #[inline]
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.next_packet > self.total_packets
    }
}

/// Result of one [`EtpRxFixed::process_frame`] call.
#[cfg(feature = "embedded")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EtpRxFixedOutcome<const N: usize> {
    /// Optional ETP.CM response frame to transmit immediately.
    pub response: Option<Frame>,
    /// Completed fixed-capacity message, if this frame finished the transfer.
    pub message: Option<FixedMessage<N>>,
}

#[cfg(feature = "embedded")]
impl<const N: usize> Default for EtpRxFixedOutcome<N> {
    fn default() -> Self {
        Self {
            response: None,
            message: None,
        }
    }
}

/// Single-session, allocation-free ETP receiver for `embedded-fixed`.
///
/// ETP can advertise very large transfers, so embedded applications usually
/// need an explicit application-level bound. `EtpRxFixed<N>` accepts one
/// transfer whose advertised size fits `N`, reassembles it into inline storage,
/// and returns CTS/EOMA/abort response frames for the caller to transmit.
#[cfg(feature = "embedded")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EtpRxFixed<const N: usize> {
    active: bool,
    pgn: Pgn,
    source: Address,
    destination: Address,
    priority: Priority,
    total_bytes: u32,
    total_packets: u32,
    bytes_transferred: u32,
    cts_window_size: u8,
    dpo_packet_offset: u32,
    packets_to_receive: u8,
    last_sequence: u8,
    data: FixedBytes<N>,
}

#[cfg(feature = "embedded")]
impl<const N: usize> Default for EtpRxFixed<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "embedded")]
impl<const N: usize> EtpRxFixed<N> {
    /// Create an idle receiver.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            pgn: 0,
            source: NULL_ADDRESS,
            destination: NULL_ADDRESS,
            priority: Priority::Default,
            total_bytes: 0,
            total_packets: 0,
            bytes_transferred: 0,
            cts_window_size: 0,
            dpo_packet_offset: 0,
            packets_to_receive: 0,
            last_sequence: 0,
            data: FixedBytes::new(),
        }
    }

    /// Returns `true` while an ETP transfer is being reassembled.
    #[inline]
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Abort the local receive state without transmitting an ETP abort frame.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Process one ETP.CM or ETP.DT frame.
    pub fn process_frame(&mut self, frame: &Frame) -> Result<EtpRxFixedOutcome<N>> {
        if matches!(frame.pgn(), PGN_ETP_CM | PGN_ETP_DT) && frame.length < 8 {
            return Ok(EtpRxFixedOutcome::default());
        }
        if matches!(frame.pgn(), PGN_ETP_CM | PGN_ETP_DT)
            && (!valid_etp_source(frame.source()) || !valid_etp_destination(frame.destination()))
        {
            return Ok(EtpRxFixedOutcome::default());
        }

        match frame.pgn() {
            PGN_ETP_CM => self.process_cm(frame),
            PGN_ETP_DT => self.process_dt(frame),
            _ => Ok(EtpRxFixedOutcome::default()),
        }
    }

    fn process_cm(&mut self, frame: &Frame) -> Result<EtpRxFixedOutcome<N>> {
        let control_byte = frame.data[0];
        let src = frame.source();
        let dst = frame.destination();
        let cm_pgn = pgn_from_cm_bytes(&frame.data);

        if !etp_cm_reserved_bytes_are_canonical(control_byte, &frame.data) || !pgn_is_valid(cm_pgn)
        {
            return Ok(EtpRxFixedOutcome::default());
        }

        match control_byte {
            etp_cm::RTS => {
                let total_bytes = (frame.data[1] as u32)
                    | ((frame.data[2] as u32) << 8)
                    | ((frame.data[3] as u32) << 16)
                    | ((frame.data[4] as u32) << 24);
                if !(TP_MAX_DATA_LENGTH + 1..=ETP_MAX_DATA_LENGTH).contains(&total_bytes) {
                    return Ok(EtpRxFixedOutcome {
                        response: Some(make_abort_fields(
                            dst,
                            src,
                            cm_pgn,
                            TransportAbortReason::UnexpectedDataSize,
                        )),
                        message: None,
                    });
                }
                if self.active {
                    return Ok(EtpRxFixedOutcome {
                        response: Some(make_abort_fields(
                            dst,
                            src,
                            cm_pgn,
                            TransportAbortReason::AlreadyInSession,
                        )),
                        message: None,
                    });
                }
                if total_bytes as usize > N {
                    return Ok(EtpRxFixedOutcome {
                        response: Some(make_abort_fields(
                            dst,
                            src,
                            cm_pgn,
                            TransportAbortReason::ResourcesUnavailable,
                        )),
                        message: None,
                    });
                }

                let mut data = FixedBytes::new();
                data.resize(total_bytes as usize, 0xFF)
                    .map_err(|_| Error::buffer_overflow())?;
                let total_packets = total_bytes.div_ceil(TP_BYTES_PER_FRAME);
                let cts_count = total_packets.min(TP_MAX_PACKETS_PER_CTS) as u8;
                self.active = true;
                self.pgn = cm_pgn;
                self.source = src;
                self.destination = dst;
                self.priority = frame.priority();
                self.total_bytes = total_bytes;
                self.total_packets = total_packets;
                self.bytes_transferred = 0;
                self.cts_window_size = cts_count;
                self.dpo_packet_offset = 0;
                self.packets_to_receive = 0;
                self.last_sequence = 0;
                self.data = data;

                Ok(EtpRxFixedOutcome {
                    response: Some(make_cts(dst, src, cts_count, 1, cm_pgn)),
                    message: None,
                })
            }

            etp_cm::DPO => {
                if !self.active
                    || self.source != src
                    || self.destination != dst
                    || self.pgn != cm_pgn
                {
                    return Ok(EtpRxFixedOutcome::default());
                }
                let num_packets = frame.data[1];
                let packet_offset = (frame.data[2] as u32)
                    | ((frame.data[3] as u32) << 8)
                    | ((frame.data[4] as u32) << 16);
                let expected_packet_offset = self.bytes_transferred / TP_BYTES_PER_FRAME;
                let remaining_packets =
                    (self.total_bytes - self.bytes_transferred).div_ceil(TP_BYTES_PER_FRAME);
                let valid = num_packets != 0
                    && num_packets <= self.cts_window_size
                    && (num_packets as u32) <= remaining_packets
                    && packet_offset == expected_packet_offset;
                if !valid {
                    let response = make_abort_fields(
                        self.destination,
                        self.source,
                        self.pgn,
                        TransportAbortReason::BadSequence,
                    );
                    self.reset();
                    return Ok(EtpRxFixedOutcome {
                        response: Some(response),
                        message: None,
                    });
                }

                self.dpo_packet_offset = packet_offset;
                self.packets_to_receive = num_packets;
                self.last_sequence = 0;
                Ok(EtpRxFixedOutcome::default())
            }

            etp_cm::ABORT => {
                if self.active
                    && self.pgn == cm_pgn
                    && ((self.source == src && self.destination == dst)
                        || (self.source == dst && self.destination == src))
                {
                    self.reset();
                }
                Ok(EtpRxFixedOutcome::default())
            }

            _ => Ok(EtpRxFixedOutcome::default()),
        }
    }

    fn process_dt(&mut self, frame: &Frame) -> Result<EtpRxFixedOutcome<N>> {
        if !self.active || frame.source() != self.source || frame.destination() != self.destination
        {
            return Ok(EtpRxFixedOutcome::default());
        }

        if self.packets_to_receive == 0 {
            let response = make_abort_fields(
                self.destination,
                self.source,
                self.pgn,
                TransportAbortReason::BadSequence,
            );
            self.reset();
            return Ok(EtpRxFixedOutcome {
                response: Some(response),
                message: None,
            });
        }

        let seq = frame.data[0];
        let expected = self.last_sequence.saturating_add(1);
        if seq == 0 || seq != expected {
            let response = make_abort_fields(
                self.destination,
                self.source,
                self.pgn,
                TransportAbortReason::BadSequence,
            );
            self.reset();
            return Ok(EtpRxFixedOutcome {
                response: Some(response),
                message: None,
            });
        }

        let byte_offset = (self.dpo_packet_offset + seq as u32 - 1) * TP_BYTES_PER_FRAME;
        for i in 0..TP_BYTES_PER_FRAME {
            let abs = (byte_offset + i) as usize;
            if abs < self.total_bytes as usize {
                self.data.as_mut_slice()[abs] = frame.data[i as usize + 1];
            }
        }
        self.bytes_transferred = (byte_offset + TP_BYTES_PER_FRAME).min(self.total_bytes);
        self.last_sequence = seq;

        if self.bytes_transferred >= self.total_bytes {
            let message = FixedMessage {
                pgn: self.pgn,
                data: self.data,
                source: self.source,
                destination: self.destination,
                priority: self.priority,
                timestamp_us: frame.timestamp_us,
            };
            let response = make_eoma(self.destination, self.source, self.total_bytes, self.pgn);
            self.reset();
            return Ok(EtpRxFixedOutcome {
                response: Some(response),
                message: Some(message),
            });
        }

        if seq >= self.packets_to_receive {
            let next_packet = self.dpo_packet_offset + seq as u32 + 1;
            let remaining_packets =
                (self.total_bytes - self.bytes_transferred).div_ceil(TP_BYTES_PER_FRAME);
            let next_count = remaining_packets.min(TP_MAX_PACKETS_PER_CTS) as u8;
            self.cts_window_size = next_count;
            self.packets_to_receive = 0;
            return Ok(EtpRxFixedOutcome {
                response: Some(make_cts(
                    self.destination,
                    self.source,
                    next_count,
                    next_packet,
                    self.pgn,
                )),
                message: None,
            });
        }

        Ok(EtpRxFixedOutcome::default())
    }
}

/// Extended Transport Protocol engine.
pub struct ExtendedTransportProtocol {
    sessions: EtpSessions,
    max_receive_bytes: u32,
    max_sessions: usize,
    stats: TransportStats,

    pub on_complete: Event<TransportSession>,
    pub on_abort: Event<TransportAbortEvent>,
}

impl Default for ExtendedTransportProtocol {
    fn default() -> Self {
        Self::new()
    }
}

impl ExtendedTransportProtocol {
    pub const MAX_DATA_LENGTH: u32 = ETP_MAX_DATA_LENGTH;
    pub const BYTES_PER_FRAME: u32 = TP_BYTES_PER_FRAME;

    #[must_use]
    pub fn new() -> Self {
        Self {
            sessions: EtpSessions::new(),
            max_receive_bytes: ETP_MAX_DATA_LENGTH,
            max_sessions: ETP_DEFAULT_MAX_SESSIONS,
            stats: TransportStats::default(),
            on_complete: Event::new(),
            on_abort: Event::new(),
        }
    }

    /// Build an ETP engine with a receive-side allocation cap.
    ///
    /// The cap is clamped to the ETP protocol maximum. Incoming RTS frames
    /// above this cap are aborted before allocating a reassembly buffer.
    #[must_use]
    pub fn with_max_receive_bytes(max_receive_bytes: u32) -> Self {
        let mut this = Self::new();
        this.set_max_receive_bytes(max_receive_bytes);
        this
    }

    /// Set the receive-side reassembly allocation cap.
    pub fn set_max_receive_bytes(&mut self, max_receive_bytes: u32) {
        self.max_receive_bytes = max_receive_bytes.min(ETP_MAX_DATA_LENGTH);
    }

    #[inline]
    #[must_use]
    pub const fn max_receive_bytes(&self) -> u32 {
        self.max_receive_bytes
    }

    /// Build an ETP engine with a cap for simultaneous active sessions.
    #[must_use]
    pub fn with_max_sessions(max_sessions: usize) -> Self {
        let mut this = Self::new();
        this.set_max_sessions(max_sessions);
        this
    }

    /// Set the active-session cap. A value of `0` rejects new sessions.
    pub fn set_max_sessions(&mut self, max_sessions: usize) {
        #[cfg(feature = "embedded")]
        let max_sessions = max_sessions.min(ETP_DEFAULT_MAX_SESSIONS);
        self.max_sessions = max_sessions;
    }

    #[inline]
    #[must_use]
    pub const fn max_sessions(&self) -> usize {
        self.max_sessions
    }

    /// Snapshot the lossy-path counters accumulated by this protocol object.
    #[inline]
    #[must_use]
    pub const fn stats(&self) -> TransportStats {
        self.stats
    }

    /// Validate an advertised ETP payload size against protocol and local
    /// receive-profile limits without allocating the reassembly buffer.
    pub fn receive_profile_for_advertised_size(
        &self,
        total_bytes: u32,
    ) -> core::result::Result<EtpReceiveProfile, TransportAbortReason> {
        if !(TP_MAX_DATA_LENGTH + 1..=ETP_MAX_DATA_LENGTH).contains(&total_bytes) {
            return Err(TransportAbortReason::UnexpectedDataSize);
        }
        if total_bytes > self.max_receive_bytes {
            return Err(TransportAbortReason::ResourcesUnavailable);
        }
        Ok(EtpReceiveProfile {
            total_bytes,
            total_packets: total_bytes.div_ceil(TP_BYTES_PER_FRAME),
            max_receive_bytes: self.max_receive_bytes,
        })
    }

    /// Reset lossy-path counters without disturbing active sessions.
    #[inline]
    pub fn clear_stats(&mut self) {
        self.stats = TransportStats::default();
    }

    /// Start an ETP transmit session. Errors with
    /// [`ErrorCode::BufferOverflow`] for `data > ETP_MAX_DATA_LENGTH`,
    /// [`ErrorCode::InvalidState`] for `data ≤ 1785 bytes` (use TP) or
    /// for broadcast destination (ETP is connection-mode only).
    ///
    /// [`ErrorCode::BufferOverflow`]: super::error::ErrorCode::BufferOverflow
    /// [`ErrorCode::InvalidState`]: super::error::ErrorCode::InvalidState
    pub fn send(
        &mut self,
        pgn: Pgn,
        data: &[u8],
        source: Address,
        dest: Address,
        port: u8,
        priority: Priority,
    ) -> Result<Vec<Frame>> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_data(format!(
                "ETP target PGN 0x{pgn:X} exceeds the 18-bit J1939/ISOBUS PGN range"
            )));
        }
        if !valid_etp_source(source) {
            return Err(Error::invalid_address(source));
        }
        if !valid_etp_destination(dest) {
            return Err(Error::invalid_address(dest));
        }
        if data.len() as u32 > Self::MAX_DATA_LENGTH {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }
        if data.len() as u32 <= TP_MAX_DATA_LENGTH {
            return Err(Error::invalid_state("use TP for <= 1785 bytes"));
        }
        if self.transmit_dt_path_is_active(source, dest, port) {
            return Err(Error::with_message(
                super::error::ErrorCode::SessionExists,
                "session already active",
            ));
        }

        if self.sessions.len() >= self.max_sessions {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::with_message(
                super::error::ErrorCode::NoResources,
                "no ETP session slots available",
            ));
        }

        let session = TransportSession {
            direction: TransportDirection::Transmit,
            state: SessionState::WaitingForCTS,
            pgn,
            data: data.to_vec(),
            total_bytes: data.len() as u32,
            source_address: source,
            destination_address: dest,
            can_port: port,
            priority,
            ..Default::default()
        };

        let rts = make_rts(&session);
        if self.push_session(session).is_err() {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::with_message(
                super::error::ErrorCode::NoResources,
                "no ETP session slots available",
            ));
        }
        tracing::debug!(
            target: "machbus.transport.etp",
            pgn = pgn,
            bytes = data.len(),
            "ETP RTS sent",
        );
        Ok(vec![rts])
    }

    pub fn process_frame(&mut self, frame: &Frame, port: u8) -> Vec<Frame> {
        if matches!(frame.pgn(), PGN_ETP_CM | PGN_ETP_DT) && frame.length < 8 {
            tracing::warn!(
                target: "machbus.transport.etp",
                pgn = frame.pgn(),
                length = frame.length,
                "dropping short ETP frame",
            );
            self.note_dropped_frame();
            return Vec::new();
        }
        if matches!(frame.pgn(), PGN_ETP_CM | PGN_ETP_DT)
            && (!valid_etp_source(frame.source()) || !valid_etp_destination(frame.destination()))
        {
            tracing::warn!(
                target: "machbus.transport.etp",
                source = frame.source(),
                destination = frame.destination(),
                "dropping ETP frame with invalid endpoint address",
            );
            self.note_dropped_frame();
            return Vec::new();
        }

        match frame.pgn() {
            PGN_ETP_CM => self.handle_cm(frame, port),
            PGN_ETP_DT => self.handle_dt(frame, port),
            _ => Vec::new(),
        }
    }

    /// Drive timeouts. WaitingForCTS / WaitingForData / WaitingForEndOfMsg
    /// all share [`ETP_TIMEOUT_T1_MS`].
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<Frame> {
        let mut emitted = Vec::new();
        let mut i = 0;
        while i < self.sessions.len() {
            self.sessions[i].timer_ms = self.sessions[i].timer_ms.saturating_add(elapsed_ms);
            let timed_out = matches!(
                self.sessions[i].state,
                SessionState::WaitingForCTS
                    | SessionState::WaitingForData
                    | SessionState::WaitingForEndOfMsg
            ) && self.sessions[i].timer_ms >= ETP_TIMEOUT_T1_MS;

            if timed_out {
                tracing::warn!(target: "machbus.transport.etp", pgn = self.sessions[i].pgn, "ETP timeout");
                self.sessions[i].state = SessionState::Aborted;
                let evt = TransportAbortEvent::from_session(
                    &self.sessions[i],
                    TransportAbortReason::Timeout,
                );
                self.on_abort.emit(&evt);
                self.note_timeout();
                self.note_dropped_session();
                self.note_abort_sent();
                emitted.push(make_session_abort(
                    &self.sessions[i],
                    TransportAbortReason::Timeout,
                ));
                self.remove_session(i);
                continue;
            }
            i += 1;
        }
        emitted
    }

    /// Emit DPO + the next batch of DT frames for any session in
    /// [`SessionState::SendingData`].
    pub fn get_pending_data_frames(&mut self) -> Vec<Frame> {
        let mut emitted = Vec::new();
        for s in self.sessions.iter_mut() {
            if s.state == SessionState::SendingData && s.direction == TransportDirection::Transmit {
                emitted.push(make_dpo(s));
                let count = s.packets_to_send;
                emitted.extend(generate_data_frames(s, count));
                if s.bytes_transferred >= s.total_bytes {
                    s.state = SessionState::WaitingForEndOfMsg;
                    s.timer_ms = 0;
                } else {
                    s.state = SessionState::WaitingForCTS;
                    s.timer_ms = 0;
                }
            }
        }
        emitted
    }

    /// Fixed-capacity variant of [`Self::get_pending_data_frames`].
    ///
    /// Available with `embedded-fixed`. It rejects the call before mutating any
    /// session if the caller-provided frame storage cannot hold the pending
    /// DPO plus data frames for the currently open ETP transmit windows.
    #[cfg(feature = "embedded")]
    pub fn get_pending_data_frames_fixed<const N: usize>(
        &mut self,
    ) -> Result<FixedSlots<Frame, N>> {
        let mut needed = 0usize;
        for s in self.sessions.iter() {
            if s.state == SessionState::SendingData && s.direction == TransportDirection::Transmit {
                let remaining = s.total_bytes.saturating_sub(s.bytes_transferred);
                let remaining_frames = remaining.div_ceil(TP_BYTES_PER_FRAME) as usize;
                needed = needed.saturating_add(1);
                needed = needed.saturating_add((s.packets_to_send as usize).min(remaining_frames));
            }
        }
        if needed > N {
            self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
            return Err(Error::buffer_overflow());
        }

        let mut emitted = FixedSlots::new();
        for s in self.sessions.iter_mut() {
            if s.state == SessionState::SendingData && s.direction == TransportDirection::Transmit {
                emitted
                    .push(make_dpo(s))
                    .map_err(|_| Error::buffer_overflow())?;
                let count = s.packets_to_send;
                generate_data_frames_fixed(s, count, &mut emitted)?;
                if s.bytes_transferred >= s.total_bytes {
                    s.state = SessionState::WaitingForEndOfMsg;
                    s.timer_ms = 0;
                } else {
                    s.state = SessionState::WaitingForCTS;
                    s.timer_ms = 0;
                }
            }
        }
        Ok(emitted)
    }

    #[inline]
    pub fn active_sessions_iter(&self) -> impl Iterator<Item = &TransportSession> {
        self.sessions.iter()
    }

    #[cfg(feature = "default")]
    #[inline]
    #[must_use]
    pub fn active_sessions(&self) -> &[TransportSession] {
        &self.sessions
    }

    #[cfg(feature = "embedded")]
    fn push_session(
        &mut self,
        session: TransportSession,
    ) -> core::result::Result<(), TransportSession> {
        self.sessions.push(session)
    }

    #[cfg(feature = "default")]
    fn push_session(
        &mut self,
        session: TransportSession,
    ) -> core::result::Result<(), TransportSession> {
        self.sessions.push(session);
        Ok(())
    }

    #[cfg(feature = "embedded")]
    fn remove_session(&mut self, idx: usize) -> TransportSession {
        self.sessions
            .swap_remove(idx)
            .expect("session index came from active session table")
    }

    #[cfg(feature = "default")]
    fn remove_session(&mut self, idx: usize) -> TransportSession {
        self.sessions.swap_remove(idx)
    }

    fn session_position(
        &self,
        mut predicate: impl FnMut(&TransportSession) -> bool,
    ) -> Option<usize> {
        for (idx, session) in self.sessions.iter().enumerate() {
            if predicate(session) {
                return Some(idx);
            }
        }
        None
    }

    #[inline]
    fn note_dropped_frame(&mut self) {
        self.stats.dropped_frames = self.stats.dropped_frames.saturating_add(1);
    }

    #[inline]
    fn note_dropped_session(&mut self) {
        self.stats.dropped_sessions = self.stats.dropped_sessions.saturating_add(1);
    }

    #[inline]
    fn note_abort_sent(&mut self) {
        self.stats.aborts_sent = self.stats.aborts_sent.saturating_add(1);
    }

    #[inline]
    fn note_abort_received(&mut self) {
        self.stats.aborts_received = self.stats.aborts_received.saturating_add(1);
    }

    #[inline]
    fn note_timeout(&mut self) {
        self.stats.timeouts = self.stats.timeouts.saturating_add(1);
    }

    #[inline]
    fn note_resource_rejection(&mut self) {
        self.stats.resource_rejections = self.stats.resource_rejections.saturating_add(1);
    }

    // ─── Internal handlers ────────────────────────────────────────

    fn handle_cm(&mut self, frame: &Frame, port: u8) -> Vec<Frame> {
        let mut responses = Vec::new();
        let control_byte = frame.data[0];
        let src = frame.source();
        let dst = frame.destination();
        let cm_pgn = pgn_from_cm_bytes(&frame.data);
        if !etp_cm_reserved_bytes_are_canonical(control_byte, &frame.data) {
            tracing::warn!(
                target: "machbus.transport.etp",
                control_byte,
                "dropping ETP CM frame with non-canonical reserved bytes"
            );
            self.note_dropped_frame();
            return responses;
        }
        if !pgn_is_valid(cm_pgn) {
            tracing::warn!(
                target: "machbus.transport.etp",
                pgn = cm_pgn,
                "dropping ETP CM frame with invalid target PGN"
            );
            self.note_dropped_frame();
            return responses;
        }

        match control_byte {
            etp_cm::RTS => {
                let msg_size = (frame.data[1] as u32)
                    | ((frame.data[2] as u32) << 8)
                    | ((frame.data[3] as u32) << 16)
                    | ((frame.data[4] as u32) << 24);
                let profile = match self.receive_profile_for_advertised_size(msg_size) {
                    Ok(profile) => profile,
                    Err(TransportAbortReason::UnexpectedDataSize) => {
                        tracing::warn!(
                            target: "machbus.transport.etp",
                            bytes = msg_size,
                            "rejecting malformed ETP RTS",
                        );
                        let tmp = TransportSession {
                            source_address: dst,
                            destination_address: src,
                            pgn: cm_pgn,
                            ..Default::default()
                        };
                        responses.push(make_abort(&tmp, TransportAbortReason::UnexpectedDataSize));
                        self.note_dropped_frame();
                        self.note_abort_sent();
                        return responses;
                    }
                    Err(reason) => {
                        tracing::warn!(
                            target: "machbus.transport.etp",
                            bytes = msg_size,
                            max_receive_bytes = self.max_receive_bytes,
                            ?reason,
                            "rejecting ETP RTS before allocation",
                        );
                        let tmp = TransportSession {
                            source_address: dst,
                            destination_address: src,
                            pgn: cm_pgn,
                            ..Default::default()
                        };
                        responses.push(make_abort(&tmp, reason));
                        self.note_dropped_frame();
                        if reason == TransportAbortReason::ResourcesUnavailable {
                            self.note_resource_rejection();
                        }
                        self.note_abort_sent();
                        return responses;
                    }
                };
                if profile.total_packets > 0xFF_FFFF {
                    tracing::warn!(
                        target: "machbus.transport.etp",
                        bytes = msg_size,
                        "rejecting malformed ETP RTS",
                    );
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::UnexpectedDataSize));
                    self.note_dropped_frame();
                    self.note_abort_sent();
                    return responses;
                }
                if self.receive_dt_path_is_active(src, dst, port) {
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::AlreadyInSession));
                    self.note_dropped_frame();
                    self.note_abort_sent();
                    return responses;
                }
                if self.sessions.len() >= self.max_sessions {
                    tracing::warn!(
                        target: "machbus.transport.etp",
                        max_sessions = self.max_sessions,
                        "rejecting ETP RTS because session cap is full",
                    );
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::ResourcesUnavailable));
                    self.note_dropped_frame();
                    self.note_resource_rejection();
                    self.note_abort_sent();
                    return responses;
                }
                let data = match rx_buffer(msg_size, self.max_receive_bytes) {
                    Ok(data) => data,
                    Err(reason) => {
                        tracing::warn!(
                            target: "machbus.transport.etp",
                            bytes = msg_size,
                            max_receive_bytes = self.max_receive_bytes,
                            ?reason,
                            "rejecting ETP RTS before allocation",
                        );
                        let tmp = TransportSession {
                            source_address: dst,
                            destination_address: src,
                            pgn: cm_pgn,
                            ..Default::default()
                        };
                        responses.push(make_abort(&tmp, reason));
                        self.note_dropped_frame();
                        if reason == TransportAbortReason::ResourcesUnavailable {
                            self.note_resource_rejection();
                        }
                        self.note_abort_sent();
                        return responses;
                    }
                };
                let session = TransportSession {
                    direction: TransportDirection::Receive,
                    state: SessionState::WaitingForData,
                    pgn: cm_pgn,
                    total_bytes: msg_size,
                    source_address: src,
                    destination_address: dst,
                    can_port: port,
                    priority: frame.priority(),
                    data,
                    cts_window_size: TP_MAX_PACKETS_PER_CTS as u8,
                    ..Default::default()
                };
                let cts = make_cts(dst, src, TP_MAX_PACKETS_PER_CTS as u8, 1, cm_pgn);
                if self.push_session(session).is_err() {
                    let tmp = TransportSession {
                        source_address: dst,
                        destination_address: src,
                        pgn: cm_pgn,
                        ..Default::default()
                    };
                    responses.push(make_abort(&tmp, TransportAbortReason::ResourcesUnavailable));
                    self.note_dropped_frame();
                    self.note_resource_rejection();
                    self.note_abort_sent();
                    return responses;
                }
                responses.push(cts);
                tracing::debug!(
                    target: "machbus.transport.etp",
                    pgn = cm_pgn,
                    bytes = msg_size,
                    "ETP RTS received",
                );
            }

            etp_cm::CTS => {
                let num_packets = frame.data[1];
                let next_pkt = (frame.data[2] as u32)
                    | ((frame.data[3] as u32) << 8)
                    | ((frame.data[4] as u32) << 16);
                let mut abort_index: Option<usize> = None;
                let mut abort_reason = TransportAbortReason::None;
                let mut matched = false;
                for (idx, s) in self.sessions.iter_mut().enumerate() {
                    if !(s.direction == TransportDirection::Transmit
                        && s.source_address == dst
                        && s.destination_address == src
                        && s.pgn == cm_pgn
                        && matches!(
                            s.state,
                            SessionState::WaitingForCTS | SessionState::SendingData
                        )
                        && s.can_port == port)
                    {
                        continue;
                    }

                    matched = true;
                    if s.state == SessionState::SendingData {
                        let is_duplicate_current_window = num_packets != 0
                            && next_pkt != 0
                            && next_pkt <= s.total_packets()
                            && next_pkt == (s.bytes_transferred / 7) + 1
                            && s.packets_to_send
                                == (num_packets as u32)
                                    .min(s.total_packets() - (next_pkt - 1))
                                    .min(TP_MAX_PACKETS_PER_CTS)
                                    as u8;
                        if is_duplicate_current_window {
                            s.timer_ms = 0;
                            break;
                        }
                        tracing::warn!(
                            target: "machbus.transport.etp",
                            packets = num_packets,
                            next_pkt,
                            "ETP CTS received while sender is already sending data",
                        );
                        s.state = SessionState::Aborted;
                        abort_index = Some(idx);
                        abort_reason = TransportAbortReason::ConnectionModeError;
                    } else if num_packets == 0 {
                        s.timer_ms = 0; // CTS hold
                    } else if next_pkt == 0 || next_pkt > s.total_packets() {
                        tracing::warn!(
                            target: "machbus.transport.etp",
                            next_pkt,
                            total_packets = s.total_packets(),
                            "ETP CTS invalid next packet",
                        );
                        s.state = SessionState::Aborted;
                        abort_index = Some(idx);
                        abort_reason = TransportAbortReason::BadSequence;
                    } else {
                        let remaining_packets = s.total_packets() - (next_pkt - 1);
                        let clamped = (num_packets as u32)
                            .min(remaining_packets)
                            .min(TP_MAX_PACKETS_PER_CTS)
                            as u8;
                        s.state = SessionState::SendingData;
                        s.packets_to_send = clamped;
                        s.bytes_transferred = (next_pkt - 1) * 7;
                        s.timer_ms = 0;
                    }
                    tracing::debug!(
                        target: "machbus.transport.etp",
                        packets = num_packets,
                        next_pkt = next_pkt,
                        "ETP CTS",
                    );
                    break;
                }
                if let Some(idx) = abort_index {
                    let evt = TransportAbortEvent::from_session(&self.sessions[idx], abort_reason);
                    self.on_abort.emit(&evt);
                    let abort_frame = make_session_abort(&self.sessions[idx], abort_reason);
                    responses.push(abort_frame);
                    self.note_dropped_frame();
                    self.note_dropped_session();
                    self.note_abort_sent();
                    self.remove_session(idx);
                } else if !matched {
                    self.note_dropped_frame();
                }
            }

            etp_cm::DPO => {
                // Sender → receiver: this DPO group has `num_packets`
                // DT frames at byte offset `packet_offset * 7`.
                let num_packets = frame.data[1];
                let packet_offset = (frame.data[2] as u32)
                    | ((frame.data[3] as u32) << 8)
                    | ((frame.data[4] as u32) << 16);
                let Some(idx) = self.session_position(|s| {
                    s.direction == TransportDirection::Receive
                        && s.source_address == src
                        && s.destination_address == dst
                        && s.pgn == cm_pgn
                        && s.can_port == port
                }) else {
                    self.note_dropped_frame();
                    return responses;
                };

                let expected_packet_offset = self.sessions[idx].bytes_transferred / 7;
                let remaining_packets = (self.sessions[idx].total_bytes
                    - self.sessions[idx].bytes_transferred)
                    .div_ceil(7);
                let packet_window_valid = num_packets != 0
                    && num_packets <= self.sessions[idx].cts_window_size
                    && (num_packets as u32) <= remaining_packets
                    && packet_offset == expected_packet_offset;

                if !packet_window_valid {
                    tracing::warn!(
                        target: "machbus.transport.etp",
                        offset = packet_offset,
                        expected_packet_offset,
                        packets = num_packets,
                        window = self.sessions[idx].cts_window_size,
                        remaining_packets,
                        "ETP DPO invalid window",
                    );
                    responses.push(make_session_abort(
                        &self.sessions[idx],
                        TransportAbortReason::BadSequence,
                    ));
                    self.note_abort_sent();
                    self.sessions[idx].state = SessionState::Aborted;
                    let evt = TransportAbortEvent::from_session(
                        &self.sessions[idx],
                        TransportAbortReason::BadSequence,
                    );
                    self.on_abort.emit(&evt);
                    self.note_dropped_frame();
                    self.note_dropped_session();
                    self.remove_session(idx);
                    return responses;
                }

                {
                    let s = &mut self.sessions[idx];
                    s.dpo_packet_offset = packet_offset;
                    s.cts_window_size = num_packets;
                    s.packets_to_send = num_packets;
                    s.last_sequence = 0; // resets per DPO group
                    s.timer_ms = 0;
                    tracing::debug!(
                        target: "machbus.transport.etp",
                        offset = packet_offset,
                        packets = num_packets,
                        "ETP DPO",
                    );
                }
            }

            etp_cm::EOMA => {
                if let Some(idx) = self.session_position(|s| {
                    s.direction == TransportDirection::Transmit
                        && s.source_address == dst
                        && s.destination_address == src
                        && s.pgn == cm_pgn
                        && s.can_port == port
                }) {
                    let expected_total = self.sessions[idx].total_bytes;
                    let ack_total = (frame.data[1] as u32)
                        | ((frame.data[2] as u32) << 8)
                        | ((frame.data[3] as u32) << 16)
                        | ((frame.data[4] as u32) << 24);
                    if self.sessions[idx].state != SessionState::WaitingForEndOfMsg
                        || ack_total != expected_total
                    {
                        tracing::warn!(
                            target: "machbus.transport.etp",
                            state = ?self.sessions[idx].state,
                            ack_total,
                            expected_total,
                            "dropping ETP EOMA that does not match a completed transmit session"
                        );
                        self.note_dropped_frame();
                        return responses;
                    }
                    let mut session = self.remove_session(idx);
                    session.state = SessionState::Complete;
                    self.on_complete.emit(&session);
                    tracing::debug!(target: "machbus.transport.etp", "ETP complete");
                } else {
                    self.note_dropped_frame();
                }
            }

            etp_cm::ABORT => {
                let Some(reason) = TransportAbortReason::try_from_u8(frame.data[1]) else {
                    self.note_dropped_frame();
                    return responses;
                };
                if let Some(idx) = self.session_position(|s| {
                    s.pgn == cm_pgn
                        && s.can_port == port
                        && ((s.source_address == dst && s.destination_address == src)
                            || (s.source_address == src && s.destination_address == dst))
                }) {
                    self.sessions[idx].state = SessionState::Aborted;
                    let evt = TransportAbortEvent::from_session(&self.sessions[idx], reason);
                    self.on_abort.emit(&evt);
                    self.remove_session(idx);
                    self.note_abort_received();
                    self.note_dropped_session();
                    tracing::warn!(
                        target: "machbus.transport.etp",
                        pgn = cm_pgn,
                        reason = ?reason,
                        "ETP abort received",
                    );
                } else {
                    self.note_dropped_frame();
                }
            }

            _ => self.note_dropped_frame(),
        }
        responses
    }

    fn handle_dt(&mut self, frame: &Frame, port: u8) -> Vec<Frame> {
        let mut responses = Vec::new();
        let src = frame.source();
        let dst = frame.destination();
        let seq = frame.data[0];

        let Some(idx) = self.session_position(|s| {
            s.direction == TransportDirection::Receive
                && s.source_address == src
                && s.destination_address == dst
                && s.can_port == port
                && s.state == SessionState::WaitingForData
        }) else {
            self.note_dropped_frame();
            return responses;
        };

        if self.sessions[idx].packets_to_send == 0 {
            tracing::warn!(target: "machbus.transport.etp", "ETP DT before DPO");
            responses.push(make_session_abort(
                &self.sessions[idx],
                TransportAbortReason::BadSequence,
            ));
            self.note_abort_sent();
            self.sessions[idx].state = SessionState::Aborted;
            let evt = TransportAbortEvent::from_session(
                &self.sessions[idx],
                TransportAbortReason::BadSequence,
            );
            self.on_abort.emit(&evt);
            self.note_dropped_frame();
            self.note_dropped_session();
            self.remove_session(idx);
            return responses;
        }

        let expected = self.sessions[idx].last_sequence + 1;
        if seq != expected {
            tracing::warn!(target: "machbus.transport.etp", seq, expected, "ETP bad seq");
            responses.push(make_session_abort(
                &self.sessions[idx],
                TransportAbortReason::BadSequence,
            ));
            self.note_abort_sent();
            self.sessions[idx].state = SessionState::Aborted;
            let evt = TransportAbortEvent::from_session(
                &self.sessions[idx],
                TransportAbortReason::BadSequence,
            );
            self.on_abort.emit(&evt);
            self.note_dropped_frame();
            self.note_dropped_session();
            self.remove_session(idx);
            return responses;
        }

        let s = &mut self.sessions[idx];
        let byte_offset = (s.dpo_packet_offset + seq as u32 - 1) * 7;
        for i in 0..7u32 {
            let abs = (byte_offset + i) as usize;
            if abs < s.total_bytes as usize {
                s.data[abs] = frame.data[i as usize + 1];
            }
        }
        s.bytes_transferred = (byte_offset + 7).min(s.total_bytes);
        s.last_sequence = seq;
        s.timer_ms = 0;

        if s.bytes_transferred >= s.total_bytes {
            s.state = SessionState::Complete;
            let (eoma_src, eoma_dst, eoma_total, eoma_pgn) = (
                s.destination_address,
                s.source_address,
                s.total_bytes,
                s.pgn,
            );
            let session = self.remove_session(idx);
            responses.push(make_eoma(eoma_src, eoma_dst, eoma_total, eoma_pgn));
            self.on_complete.emit(&session);
            tracing::debug!(target: "machbus.transport.etp", "ETP RX complete");
            return responses;
        }

        if seq >= self.sessions[idx].cts_window_size {
            let s = &mut self.sessions[idx];
            let next_pkt = s.dpo_packet_offset + seq as u32 + 1;
            let remaining_packets = (s.total_bytes - s.bytes_transferred).div_ceil(7);
            // Clamp to u8 *before* casting, otherwise large remainders
            // wrap (e.g. 270 → 14) and stall the transfer.
            let next_count = remaining_packets.min(TP_MAX_PACKETS_PER_CTS) as u8;
            responses.push(make_cts(
                s.destination_address,
                s.source_address,
                next_count,
                next_pkt,
                s.pgn,
            ));
            s.cts_window_size = next_count;
            s.packets_to_send = 0;
        }
        responses
    }

    fn transmit_dt_path_is_active(&self, source: Address, destination: Address, port: u8) -> bool {
        self.sessions.iter().any(|s| {
            s.direction == TransportDirection::Transmit
                && s.source_address == source
                && s.destination_address == destination
                && s.can_port == port
        })
    }

    fn receive_dt_path_is_active(&self, source: Address, destination: Address, port: u8) -> bool {
        self.sessions.iter().any(|s| {
            s.direction == TransportDirection::Receive
                && s.source_address == source
                && s.destination_address == destination
                && s.can_port == port
        })
    }
}

// ─── Frame builders ────────────────────────────────────────────────────

fn rx_buffer(
    total_bytes: u32,
    max_receive_bytes: u32,
) -> core::result::Result<Vec<u8>, TransportAbortReason> {
    if total_bytes > max_receive_bytes {
        return Err(TransportAbortReason::ResourcesUnavailable);
    }

    let mut data = Vec::new();
    data.try_reserve_exact(total_bytes as usize)
        .map_err(|_| TransportAbortReason::ResourcesUnavailable)?;
    data.resize(total_bytes as usize, 0xFF);
    Ok(data)
}

fn etp_cm_reserved_bytes_are_canonical(control_byte: u8, data: &[u8; 8]) -> bool {
    match control_byte {
        etp_cm::ABORT => data[2..5].iter().all(|&byte| byte == 0xFF),
        _ => true,
    }
}

fn make_rts(s: &TransportSession) -> Frame {
    make_rts_fields(
        s.pgn,
        s.total_bytes,
        s.source_address,
        s.destination_address,
    )
}

fn make_rts_fields(
    pgn: Pgn,
    total_bytes: u32,
    source_address: Address,
    destination_address: Address,
) -> Frame {
    let id = Identifier::encode(
        Priority::Lowest,
        PGN_ETP_CM,
        source_address,
        destination_address,
    );
    let mut data = [0u8; 8];
    data[0] = etp_cm::RTS;
    data[1] = (total_bytes & 0xFF) as u8;
    data[2] = ((total_bytes >> 8) & 0xFF) as u8;
    data[3] = ((total_bytes >> 16) & 0xFF) as u8;
    data[4] = ((total_bytes >> 24) & 0xFF) as u8;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_dpo(s: &TransportSession) -> Frame {
    let packet_offset = s.bytes_transferred / 7;
    make_dpo_fields(
        s.pgn,
        s.source_address,
        s.destination_address,
        s.packets_to_send,
        packet_offset,
    )
}

fn make_dpo_fields(
    pgn: Pgn,
    source_address: Address,
    destination_address: Address,
    packets_to_send: u8,
    packet_offset: u32,
) -> Frame {
    let id = Identifier::encode(
        Priority::Lowest,
        PGN_ETP_CM,
        source_address,
        destination_address,
    );
    let mut data = [0u8; 8];
    data[0] = etp_cm::DPO;
    data[1] = packets_to_send;
    data[2] = (packet_offset & 0xFF) as u8;
    data[3] = ((packet_offset >> 8) & 0xFF) as u8;
    data[4] = ((packet_offset >> 16) & 0xFF) as u8;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_cts(src: Address, dst: Address, num_packets: u8, next_packet: u32, pgn: Pgn) -> Frame {
    let id = Identifier::encode(Priority::Lowest, PGN_ETP_CM, src, dst);
    let mut data = [0u8; 8];
    data[0] = etp_cm::CTS;
    data[1] = num_packets;
    data[2] = (next_packet & 0xFF) as u8;
    data[3] = ((next_packet >> 8) & 0xFF) as u8;
    data[4] = ((next_packet >> 16) & 0xFF) as u8;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_eoma(src: Address, dst: Address, total_bytes: u32, pgn: Pgn) -> Frame {
    let id = Identifier::encode(Priority::Lowest, PGN_ETP_CM, src, dst);
    let mut data = [0u8; 8];
    data[0] = etp_cm::EOMA;
    data[1] = (total_bytes & 0xFF) as u8;
    data[2] = ((total_bytes >> 8) & 0xFF) as u8;
    data[3] = ((total_bytes >> 16) & 0xFF) as u8;
    data[4] = ((total_bytes >> 24) & 0xFF) as u8;
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_abort(s: &TransportSession, reason: TransportAbortReason) -> Frame {
    make_abort_fields(s.source_address, s.destination_address, s.pgn, reason)
}

fn make_abort_fields(
    source_address: Address,
    destination_address: Address,
    pgn: Pgn,
    reason: TransportAbortReason,
) -> Frame {
    let id = Identifier::encode(
        Priority::Lowest,
        PGN_ETP_CM,
        source_address,
        destination_address,
    );
    let mut data = [0xFFu8; 8];
    data[0] = etp_cm::ABORT;
    data[1] = reason.as_u8();
    data[5] = (pgn & 0xFF) as u8;
    data[6] = ((pgn >> 8) & 0xFF) as u8;
    data[7] = ((pgn >> 16) & 0xFF) as u8;
    Frame::new(id, data, 8)
}

fn make_session_abort(s: &TransportSession, reason: TransportAbortReason) -> Frame {
    let (source_address, destination_address) = match s.direction {
        TransportDirection::Transmit => (s.source_address, s.destination_address),
        TransportDirection::Receive => (s.destination_address, s.source_address),
    };
    let wire_session = TransportSession {
        source_address,
        destination_address,
        pgn: s.pgn,
        ..Default::default()
    };
    make_abort(&wire_session, reason)
}

fn generate_data_frames(session: &mut TransportSession, count: u8) -> Vec<Frame> {
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        if session.bytes_transferred >= session.total_bytes {
            break;
        }
        let id = Identifier::encode(
            Priority::Lowest,
            PGN_ETP_DT,
            session.source_address,
            session.destination_address,
        );
        let mut data = [0xFFu8; 8];
        // Sequence resets per DPO group, so this is `i + 1` (1-based).
        data[0] = i + 1;
        for j in 0..7u32 {
            let idx = session.bytes_transferred + j;
            if idx < session.total_bytes {
                data[(j + 1) as usize] = session.data[idx as usize];
            }
        }
        session.bytes_transferred =
            (session.bytes_transferred + TP_BYTES_PER_FRAME).min(session.total_bytes);
        out.push(Frame::new(id, data, 8));
    }
    out
}

#[cfg(feature = "embedded")]
fn generate_data_frames_fixed<const N: usize>(
    session: &mut TransportSession,
    count: u8,
    out: &mut FixedSlots<Frame, N>,
) -> Result<()> {
    for i in 0..count {
        if session.bytes_transferred >= session.total_bytes {
            break;
        }
        let id = Identifier::encode(
            Priority::Lowest,
            PGN_ETP_DT,
            session.source_address,
            session.destination_address,
        );
        let mut data = [0xFFu8; 8];
        // Sequence resets per DPO group, so this is `i + 1` (1-based).
        data[0] = i + 1;
        for j in 0..TP_BYTES_PER_FRAME {
            let idx = session.bytes_transferred + j;
            if idx < session.total_bytes {
                data[(j + 1) as usize] = session.data[idx as usize];
            }
        }
        session.bytes_transferred =
            (session.bytes_transferred + TP_BYTES_PER_FRAME).min(session.total_bytes);
        out.push(Frame::new(id, data, 8))
            .map_err(|_| Error::buffer_overflow())?;
    }
    Ok(())
}

#[inline]
fn pgn_from_cm_bytes(data: &[u8; 8]) -> Pgn {
    (data[5] as u32) | ((data[6] as u32) << 8) | ((data[7] as u32) << 16)
}

#[inline]
const fn valid_etp_source(source: Address) -> bool {
    source != NULL_ADDRESS && source != BROADCAST_ADDRESS
}

#[inline]
const fn valid_etp_destination(destination: Address) -> bool {
    destination != NULL_ADDRESS && destination != BROADCAST_ADDRESS
}

