use alloc::{format, vec::Vec};

#[cfg(feature = "embedded")]
use crate::fixed::FixedSlots;
#[cfg(feature = "embedded")]
use crate::fixed::{FixedBytes, FixedMessage};

use super::constants::{
    BROADCAST_ADDRESS, CAN_DATA_LENGTH, NULL_ADDRESS, TP_BAM_INTER_PACKET_MS, TP_BYTES_PER_FRAME,
    TP_MAX_DATA_LENGTH, TP_MAX_PACKETS_PER_CTS, TP_TIMEOUT_T1_MS, TP_TIMEOUT_T3_MS,
    TP_TIMEOUT_T4_MS,
};
use super::error::{Error, Result};
use super::event::Event;
use super::frame::Frame;
use super::identifier::Identifier;
use super::pgn::pgn_is_valid;
use super::pgn_defs::{PGN_TP_CM, PGN_TP_DT};
use super::session::{
    SessionState, TransportAbortEvent, TransportAbortReason, TransportDirection, TransportSession,
    TransportStats,
};
use super::types::{Address, Pgn, Priority};

/// CTS keep-alive interval used while a receiver is paused.
pub const TP_T_HOLD_MS: u32 = 500;

/// Default cap for simultaneous TP transmit and receive sessions.
pub const TP_DEFAULT_MAX_SESSIONS: usize = 32;

/// Default number of duplicate/backward CTS windows accepted before aborting
/// a CMDT transmit session with `MaxRetransmitsExceeded`.
pub const TP_DEFAULT_MAX_RETRANSMITS: u8 = 3;

#[cfg(feature = "embedded")]
type TpSessions = FixedSlots<TransportSession, TP_DEFAULT_MAX_SESSIONS>;
#[cfg(feature = "default")]
type TpSessions = Vec<TransportSession>;

#[cfg(feature = "embedded")]
type TpTimerSessions = FixedSlots<TpTimerSession, TP_DEFAULT_MAX_SESSIONS>;
#[cfg(feature = "default")]
type TpTimerSessions = Vec<TpTimerSession>;

/// Borrowed, allocation-free TP/CMDT transmit helper for `embedded-fixed`.
///
/// This is a lower-level helper for embedded applications that want
/// connection-mode TP without copying the payload into the heap-backed
/// [`TransportProtocol`] session store. The application owns the payload slice,
/// sends [`Self::rts`], converts peer CTS windows into [`Self::set_window`],
/// and transmits the frames returned by [`Self::pending_data_frames_fixed`].
#[cfg(feature = "embedded")]
#[derive(Debug, Clone)]
pub struct TpCmdtTx<'a> {
    pgn: Pgn,
    payload: &'a [u8],
    source: Address,
    destination: Address,
    total_bytes: u32,
    total_packets: u8,
    advertised_packets_per_cts: u8,
    next_sequence: u8,
    packets_to_send: u8,
}

#[cfg(feature = "embedded")]
impl<'a> TpCmdtTx<'a> {
    /// Create a borrowed TP/CMDT transmit session.
    pub fn new(pgn: Pgn, payload: &'a [u8], source: Address, destination: Address) -> Result<Self> {
        Self::with_advertised_packets_per_cts(
            pgn,
            payload,
            source,
            destination,
            TP_MAX_PACKETS_PER_CTS as u8,
        )
    }

    /// Create a borrowed TP/CMDT transmit session with a custom RTS packet
    /// window advertisement.
    pub fn with_advertised_packets_per_cts(
        pgn: Pgn,
        payload: &'a [u8],
        source: Address,
        destination: Address,
        advertised_packets_per_cts: u8,
    ) -> Result<Self> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_pgn(pgn));
        }
        if !valid_tp_source(source) {
            return Err(Error::invalid_address(source));
        }
        if !valid_tp_destination(destination) || destination == BROADCAST_ADDRESS {
            return Err(Error::invalid_address(destination));
        }
        if payload.len() as u32 > TP_MAX_DATA_LENGTH {
            return Err(Error::buffer_overflow());
        }
        if payload.len() as u32 <= CAN_DATA_LENGTH {
            return Err(Error::invalid_state("use single frame for <= 8 bytes"));
        }

        let total_bytes = payload.len() as u32;
        Ok(Self {
            pgn,
            payload,
            source,
            destination,
            total_bytes,
            total_packets: total_bytes.div_ceil(TP_BYTES_PER_FRAME) as u8,
            advertised_packets_per_cts: advertised_packets_per_cts
                .clamp(1, TP_MAX_PACKETS_PER_CTS as u8),
            next_sequence: 1,
            packets_to_send: 0,
        })
    }

    /// Initial RTS frame to send to the peer.
    #[must_use]
    pub fn rts(&self) -> Frame {
        make_rts_fields(
            self.pgn,
            self.total_bytes,
            self.source,
            self.destination,
            self.advertised_packets_per_cts,
        )
    }

    /// Apply a peer CTS window.
    ///
    /// A zero-packet CTS hold is accepted and produces no pending DT frames.
    pub fn set_window(&mut self, next_sequence: u8, packets: u8) -> Result<()> {
        if packets == 0 {
            self.next_sequence = next_sequence.max(1).min(self.total_packets);
            self.packets_to_send = 0;
            return Ok(());
        }
        if next_sequence == 0 || next_sequence > self.total_packets {
            return Err(Error::invalid_state("invalid TP CTS next packet"));
        }

        let remaining = self.total_packets - (next_sequence - 1);
        self.next_sequence = next_sequence;
        self.packets_to_send = packets.min(remaining).min(TP_MAX_PACKETS_PER_CTS as u8);
        Ok(())
    }

    /// Emit pending DT frames for the current CTS window into fixed storage.
    pub fn pending_data_frames_fixed<const N: usize>(&mut self) -> Result<FixedSlots<Frame, N>> {
        if self.packets_to_send as usize > N {
            return Err(Error::buffer_overflow());
        }

        let mut frames = FixedSlots::new();
        let count = self.packets_to_send;
        for offset_packet in 0..count {
            let sequence = self.next_sequence + offset_packet;
            let payload_offset = (sequence as usize - 1) * TP_BYTES_PER_FRAME as usize;
            let mut data = [0xFFu8; 8];
            data[0] = sequence;
            for j in 0..TP_BYTES_PER_FRAME as usize {
                let idx = payload_offset + j;
                if idx < self.payload.len() {
                    data[j + 1] = self.payload[idx];
                }
            }
            let id = Identifier::encode(Priority::Lowest, PGN_TP_DT, self.source, self.destination);
            frames
                .push(Frame::new(id, data, 8))
                .map_err(|_| Error::buffer_overflow())?;
        }

        self.next_sequence = self.next_sequence.saturating_add(count);
        self.packets_to_send = 0;
        Ok(frames)
    }

    /// True once all payload packets have been emitted.
    #[inline]
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.next_sequence > self.total_packets
    }
}

/// Result of one [`TpRxFixed::process_frame`] call.
#[cfg(feature = "embedded")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TpRxFixedOutcome<const N: usize> {
    /// Optional TP.CM response frame to transmit immediately.
    pub response: Option<Frame>,
    /// Completed fixed-capacity message, if this frame finished the transfer.
    pub message: Option<FixedMessage<N>>,
}

#[cfg(feature = "embedded")]
impl<const N: usize> Default for TpRxFixedOutcome<N> {
    fn default() -> Self {
        Self {
            response: None,
            message: None,
        }
    }
}

/// Single-session, allocation-free TP receiver for `embedded-fixed`.
///
/// This helper is intentionally separate from [`TransportProtocol`]. It lets a
/// microcontroller receive one bounded BAM or CMDT transfer into caller-chosen
/// inline storage without using the heap-backed `TransportSession.data` path.
/// Applications that need multiple simultaneous receive paths can instantiate
/// one receiver per endpoint/port or keep them in a fixed slot table.
#[cfg(feature = "embedded")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TpRxFixed<const N: usize> {
    active: bool,
    pgn: Pgn,
    source: Address,
    destination: Address,
    priority: Priority,
    total_bytes: u32,
    total_packets: u8,
    max_packets_per_cts: u8,
    cts_window_start: u8,
    cts_window_size: u8,
    last_sequence: u8,
    broadcast: bool,
    data: FixedBytes<N>,
}

#[cfg(feature = "embedded")]
impl<const N: usize> Default for TpRxFixed<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "embedded")]
impl<const N: usize> TpRxFixed<N> {
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
            max_packets_per_cts: TP_MAX_PACKETS_PER_CTS as u8,
            cts_window_start: 1,
            cts_window_size: 0,
            last_sequence: 0,
            broadcast: false,
            data: FixedBytes::new(),
        }
    }

    /// Returns `true` while a BAM or CMDT transfer is being reassembled.
    #[inline]
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    /// Abort the local receive state without transmitting a TP abort frame.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Process one TP.CM or TP.DT frame.
    ///
    /// Malformed or unrelated frames are dropped and produce the default empty
    /// outcome. Capacity/resource failures are reported on the wire with TP.CM
    /// aborts when the peer used CMDT; BAM has no response path and is dropped.
    pub fn process_frame(&mut self, frame: &Frame) -> Result<TpRxFixedOutcome<N>> {
        if matches!(frame.pgn(), PGN_TP_CM | PGN_TP_DT) && frame.length < CAN_DATA_LENGTH as u8 {
            return Ok(TpRxFixedOutcome::default());
        }
        if matches!(frame.pgn(), PGN_TP_CM | PGN_TP_DT)
            && (!valid_tp_source(frame.source()) || !valid_tp_destination(frame.destination()))
        {
            return Ok(TpRxFixedOutcome::default());
        }

        match frame.pgn() {
            PGN_TP_CM => self.process_cm(frame),
            PGN_TP_DT => self.process_dt(frame),
            _ => Ok(TpRxFixedOutcome::default()),
        }
    }

    fn process_cm(&mut self, frame: &Frame) -> Result<TpRxFixedOutcome<N>> {
        let control_byte = frame.data[0];
        let src = frame.source();
        let dst = frame.destination();
        let cm_pgn = pgn_from_cm_bytes(&frame.data);

        if !tp_cm_reserved_bytes_are_canonical(control_byte, &frame.data)
            || !pgn_is_valid(cm_pgn)
            || (control_byte != tp_cm::BAM && dst == BROADCAST_ADDRESS)
        {
            return Ok(TpRxFixedOutcome::default());
        }

        match control_byte {
            tp_cm::RTS => {
                let total_bytes = frame.data[1] as u32 | ((frame.data[2] as u32) << 8);
                let total_packets = frame.data[3];
                let max_per_cts = frame.data[4];
                if !valid_tp_payload_shape(total_bytes, total_packets) || max_per_cts == 0 {
                    return Ok(TpRxFixedOutcome {
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
                    return Ok(TpRxFixedOutcome {
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
                    return Ok(TpRxFixedOutcome {
                        response: Some(make_abort_fields(
                            dst,
                            src,
                            cm_pgn,
                            TransportAbortReason::ResourcesUnavailable,
                        )),
                        message: None,
                    });
                }

                self.start_receive(
                    cm_pgn,
                    src,
                    dst,
                    frame.priority(),
                    total_bytes,
                    total_packets,
                    false,
                    max_per_cts.min(TP_MAX_PACKETS_PER_CTS as u8),
                )?;
                let cts_count = total_packets.min(self.max_packets_per_cts);
                self.cts_window_size = cts_count;
                Ok(TpRxFixedOutcome {
                    response: Some(make_cts(dst, src, cts_count, 1, cm_pgn)),
                    message: None,
                })
            }

            tp_cm::BAM => {
                let total_bytes = frame.data[1] as u32 | ((frame.data[2] as u32) << 8);
                let total_packets = frame.data[3];
                if self.active
                    || !frame.is_broadcast()
                    || !valid_tp_payload_shape(total_bytes, total_packets)
                    || total_bytes as usize > N
                {
                    return Ok(TpRxFixedOutcome::default());
                }
                self.start_receive(
                    cm_pgn,
                    src,
                    BROADCAST_ADDRESS,
                    frame.priority(),
                    total_bytes,
                    total_packets,
                    true,
                    TP_MAX_PACKETS_PER_CTS as u8,
                )?;
                Ok(TpRxFixedOutcome::default())
            }

            tp_cm::ABORT => {
                if self.active
                    && self.pgn == cm_pgn
                    && ((self.source == src && self.destination == dst)
                        || (self.source == dst && self.destination == src))
                {
                    self.reset();
                }
                Ok(TpRxFixedOutcome::default())
            }

            _ => Ok(TpRxFixedOutcome::default()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn start_receive(
        &mut self,
        pgn: Pgn,
        source: Address,
        destination: Address,
        priority: Priority,
        total_bytes: u32,
        total_packets: u8,
        broadcast: bool,
        max_packets_per_cts: u8,
    ) -> Result<()> {
        let mut data = FixedBytes::new();
        data.resize(total_bytes as usize, 0xFF)
            .map_err(|_| Error::buffer_overflow())?;
        self.active = true;
        self.pgn = pgn;
        self.source = source;
        self.destination = destination;
        self.priority = priority;
        self.total_bytes = total_bytes;
        self.total_packets = total_packets;
        self.max_packets_per_cts = max_packets_per_cts;
        self.cts_window_start = 1;
        self.cts_window_size = 0;
        self.last_sequence = 0;
        self.broadcast = broadcast;
        self.data = data;
        Ok(())
    }

    fn process_dt(&mut self, frame: &Frame) -> Result<TpRxFixedOutcome<N>> {
        if !self.active
            || frame.source() != self.source
            || (!self.broadcast && frame.destination() != self.destination)
        {
            return Ok(TpRxFixedOutcome::default());
        }

        let seq = frame.data[0];
        let expected = self.last_sequence.saturating_add(1);
        if seq == 0 || seq != expected {
            let reason = if seq != 0 && seq <= self.last_sequence {
                TransportAbortReason::DuplicateSequence
            } else {
                TransportAbortReason::BadSequence
            };
            let response = (!self.broadcast)
                .then(|| make_abort_fields(self.destination, self.source, self.pgn, reason));
            self.reset();
            return Ok(TpRxFixedOutcome {
                response,
                message: None,
            });
        }

        let offset = (seq as u32 - 1) * TP_BYTES_PER_FRAME;
        for i in 0..TP_BYTES_PER_FRAME {
            let abs = (offset + i) as usize;
            if abs < self.total_bytes as usize {
                self.data.as_mut_slice()[abs] = frame.data[i as usize + 1];
            }
        }
        self.last_sequence = seq;

        let bytes_transferred = (offset + TP_BYTES_PER_FRAME).min(self.total_bytes);
        if bytes_transferred >= self.total_bytes {
            let message = FixedMessage {
                pgn: self.pgn,
                data: self.data,
                source: self.source,
                destination: self.destination,
                priority: self.priority,
                timestamp_us: frame.timestamp_us,
            };
            let response = (!self.broadcast).then(|| {
                make_eoma(
                    self.destination,
                    self.source,
                    self.total_bytes,
                    self.total_packets,
                    self.pgn,
                )
            });
            self.reset();
            return Ok(TpRxFixedOutcome {
                response,
                message: Some(message),
            });
        }

        if !self.broadcast {
            let in_window = seq - (self.cts_window_start - 1);
            if in_window >= self.cts_window_size {
                let remaining = self.total_packets as u32 - seq as u32;
                let next_count = (remaining as u8).min(self.max_packets_per_cts);
                self.cts_window_start = seq + 1;
                self.cts_window_size = next_count;
                return Ok(TpRxFixedOutcome {
                    response: Some(make_cts(
                        self.destination,
                        self.source,
                        next_count,
                        seq + 1,
                        self.pgn,
                    )),
                    message: None,
                });
            }
        }

        Ok(TpRxFixedOutcome::default())
    }
}

// ─── TP Connection-Management byte codes ────────────────────────────────
pub mod tp_cm {
    pub const RTS: u8 = 0x10;
    pub const CTS: u8 = 0x11;
    pub const EOMA: u8 = 0x13;
    pub const BAM: u8 = 0x20;
    pub const ABORT: u8 = 0xFF;
}

// ─── Wire-format abort byte codes (ISO 11783-3) ────────────────────────
pub mod tp_abort {
    pub const ALREADY_IN_PROGRESS: u8 = 1;
    pub const NO_RESOURCES: u8 = 2;
    pub const TIMEOUT: u8 = 3;
    pub const CTS_WHILE_SENDING: u8 = 4;
    pub const MAX_RETRANSMITS: u8 = 5;
    pub const UNEXPECTED_DT: u8 = 6;
    pub const BAD_SEQUENCE: u8 = 7;
    pub const DUPLICATE_SEQUENCE: u8 = 8;
    pub const TOTAL_SIZE_TOO_BIG: u8 = 9;
}

/// Coarse state tag for the auxiliary [`TpTimerSession`] tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TpSessionState {
    #[default]
    Idle,
    WaitForCts,
    Sending,
    WaitForEndOfMsgAck,
    Complete,
    Aborted,
    TimedOut,
}

impl TpSessionState {
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(
            self,
            Self::Idle | Self::Complete | Self::Aborted | Self::TimedOut
        )
    }
}

/// Auxiliary per-session timer tracker used by the network layer to
/// drive higher-level timeouts and CTS keep-alives. Mirrors the C++
/// `TPTimerSession`.
#[derive(Debug, Clone)]
pub struct TpTimerSession {
    pub last_activity_ms: u32,
    pub timer_state: TpSessionState,
    pub abort_reason: u8,
    pub cts_keepalive_timer_ms: u32,
    pub source: Address,
    pub destination: Address,
    pub pgn: Pgn,
    pub port: u8,
    pub receiver_paused: bool,
}

impl Default for TpTimerSession {
    fn default() -> Self {
        Self {
            last_activity_ms: 0,
            timer_state: TpSessionState::Idle,
            abort_reason: 0,
            cts_keepalive_timer_ms: 0,
            source: super::constants::NULL_ADDRESS,
            destination: super::constants::NULL_ADDRESS,
            pgn: 0,
            port: 0,
            receiver_paused: false,
        }
    }
}

impl TpTimerSession {
    #[inline]
    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.timer_state.is_active()
    }
}

/// Transport Protocol engine.
pub struct TransportProtocol {
    sessions: TpSessions,
    timer_sessions: TpTimerSessions,
    max_receive_bytes: u32,
    max_sessions: usize,
    max_retransmits: u8,
    advertised_packets_per_cts: u8,
    stats: TransportStats,

    /// Fires once a session reaches [`SessionState::Complete`].
    pub on_complete: Event<TransportSession>,
    /// Fires when a session aborts (sender or receiver side).
    pub on_abort: Event<TransportAbortEvent>,
    /// Fires from the auxiliary timer-session machinery.
    pub on_session_timeout: Event<TpTimerSession>,
}

impl Default for TransportProtocol {
    fn default() -> Self {
        Self::new()
    }
}

