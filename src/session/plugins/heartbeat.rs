//! ISO 11783 heartbeat as a [`Plugin`]. Wraps the pure
//! [`HeartbeatSender`]/[`HeartbeatTracker`] codecs: broadcasts `PGN_HEARTBEAT`
//! on cadence, tracks peer sequences, and reports missed peers.

use crate::j1939::heartbeat::hb_seq;
use crate::j1939::{HB_COMM_ERROR_TIMEOUT_MS, HbReceiverState, HeartbeatReceiver, HeartbeatSender};
use crate::net::pgn_defs::PGN_HEARTBEAT;
use crate::net::{Address, BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, HeartbeatEvent};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_HEARTBEAT];

/// ISO 11783 heartbeat plugin.
///
/// Receive-side validation is the conformant [`HeartbeatReceiver`] (§8.3.3
/// sequence rules, §8.3.4 300 ms comm-error window, 8-count recovery), one per
/// peer. The plugin previously used the weaker tracker, which stored the last
/// sequence without checking it and used the *transmit* interval as the miss
/// window — so the crate's own `every(1000)` idiom gave a 3000 ms peer-loss
/// window against the 300 ms the standard requires.
pub struct Heartbeat {
    sender: HeartbeatSender,
    /// Per-peer receiver state machines, discovered on first heartbeat.
    receivers: Vec<(Address, HeartbeatReceiver, HbReceiverState)>,
    last: Option<Instant>,
}

impl Heartbeat {
    /// Broadcast a heartbeat every `interval_ms` and use it as the peer-miss window.
    #[must_use]
    pub fn every(interval_ms: u32) -> Self {
        Self {
            sender: HeartbeatSender::new(interval_ms),
            receivers: Vec::new(),
            last: None,
        }
    }

    /// Track a peer explicitly. Peers are also discovered on their first
    /// heartbeat, so this is only needed to detect a peer that never speaks.
    pub fn track(&mut self, address: Address) {
        self.receiver_for(address);
    }

    /// Stop tracking a peer.
    pub fn untrack(&mut self, address: Address) {
        self.receivers.retain(|(a, _, _)| *a != address);
    }

    /// Last sequence byte accepted from a peer.
    #[must_use]
    pub fn last_sequence(&self, address: Address) -> Option<u8> {
        self.receivers
            .iter()
            .find(|(a, _, _)| *a == address)
            .and_then(|(_, r, _)| r.last_sequence())
    }

    /// How many comm-error windows a peer has entered.
    #[must_use]
    pub fn missed_count(&self, address: Address) -> u32 {
        self.receivers
            .iter()
            .find(|(a, _, _)| *a == address)
            .map_or(0, |(_, r, _)| {
                u32::from(r.state() == HbReceiverState::CommError)
            })
    }

    /// Conformance state of a tracked peer.
    #[must_use]
    pub fn peer_state(&self, address: Address) -> Option<HbReceiverState> {
        self.receivers
            .iter()
            .find(|(a, _, _)| *a == address)
            .map(|(_, r, _)| r.state())
    }

    /// `true` when any tracked peer is in a heartbeat error state — the signal
    /// the autonomy path treats as loss of communication.
    #[must_use]
    pub fn any_peer_faulted(&self) -> bool {
        self.receivers
            .iter()
            .any(|(_, r, _)| r.state() != HbReceiverState::Normal)
    }

    fn receiver_for(
        &mut self,
        address: Address,
    ) -> &mut (Address, HeartbeatReceiver, HbReceiverState) {
        if !self.receivers.iter().any(|(a, _, _)| *a == address) {
            self.receivers
                .push((address, HeartbeatReceiver::new(), HbReceiverState::Normal));
        }
        self.receivers
            .iter_mut()
            .find(|(a, _, _)| *a == address)
            .expect("just inserted")
    }

    /// Schedule an error heartbeat for the next due broadcast.
    pub fn signal_error(&mut self) {
        self.sender.signal_error();
    }

    /// Schedule a shutdown heartbeat for the next due broadcast.
    pub fn signal_shutdown(&mut self) {
        self.sender.signal_shutdown();
    }
}

impl Plugin for Heartbeat {
    fn name(&self) -> &'static str {
        "heartbeat"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_HEARTBEAT) {
            return;
        }
        let Some(&sequence) = msg.data.first() else {
            return;
        };
        let valid_width = msg.data.len() == 1
            || (msg.data.len() == 8 && msg.data[1..].iter().all(|&b| b == 0xFF));
        if !valid_width || sequence == hb_seq::RESERVED_LOW || sequence == hb_seq::RESERVED_HIGH {
            return;
        }
        let source = msg.source;

        // 254/255 are self-reported conditions, not sequence values.
        match sequence {
            hb_seq::SENDER_ERROR => {
                ctx.emit(Event::Heartbeat(HeartbeatEvent::SenderError { source }));
            }
            hb_seq::SHUTDOWN => {
                ctx.emit(Event::Heartbeat(HeartbeatEvent::GracefulShutdown {
                    source,
                }));
            }
            _ => {}
        }

        let entry = self.receiver_for(source);
        let before = entry.1.state();
        entry.1.process(sequence);
        let after = entry.1.state();
        entry.2 = after;

        if before != after {
            match after {
                HbReceiverState::SequenceError => {
                    ctx.emit(Event::Heartbeat(HeartbeatEvent::SequenceError {
                        source,
                        sequence,
                    }));
                }
                HbReceiverState::CommError => {
                    ctx.emit(Event::Heartbeat(HeartbeatEvent::CommError { source }));
                }
                HbReceiverState::Normal => {
                    ctx.emit(Event::Heartbeat(HeartbeatEvent::Recovered { source }));
                }
                // Already reported above from the sequence byte itself; the
                // receiver now also holds the state, so the peer stops reading
                // as healthy.
                HbReceiverState::TransmissionError | HbReceiverState::GracefulShutdown => {}
            }
        }

        ctx.emit(Event::Heartbeat(HeartbeatEvent::Received {
            source,
            sequence,
        }));
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = crate::time::advance_millis(&mut self.last, now);

        // The comm-error window is fixed by ISO 11783-7 §8.3.4 at 300 ms; it is
        // not the transmit interval.
        for (source, receiver, cached) in &mut self.receivers {
            let before = receiver.state();
            receiver.update(elapsed);
            let after = receiver.state();
            *cached = after;
            if before != after && after == HbReceiverState::CommError {
                ctx.emit(Event::Heartbeat(HeartbeatEvent::CommError {
                    source: *source,
                }));
                ctx.emit(Event::Heartbeat(HeartbeatEvent::Missed {
                    source: *source,
                    missed_count: 1,
                }));
            }
        }

        if self.sender.update(elapsed) {
            let sequence = self.sender.next_sequence();
            ctx.send(
                PGN_HEARTBEAT,
                vec![sequence],
                BROADCAST_ADDRESS,
                Priority::BelowNormal,
            );
            ctx.emit(Event::Heartbeat(HeartbeatEvent::Sent { sequence }));
        }
        Some(now.add_millis(u64::from(HB_COMM_ERROR_TIMEOUT_MS)))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Frame, Identifier, Name};
    use crate::session::Session;

    fn claimed(interval_ms: u32) -> Session {
        let name = Name::default()
            .with_identity_number(0x321)
            .with_function_code(0x80)
            .with_self_configurable(true);
        let mut s = Session::builder(name, 0x80)
            .plug(Heartbeat::every(interval_ms))
            .build()
            .unwrap();
        s.start().unwrap();
        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(100);
            s.tick(now);
            while s.poll_transmit().is_some() {}
            if s.is_claimed() {
                break;
            }
        }
        s
    }

    fn beat(s: &mut Session, sequence: u8, at: Instant) {
        let frame = Frame::new(
            Identifier::encode(
                Priority::BelowNormal,
                PGN_HEARTBEAT,
                0x42,
                BROADCAST_ADDRESS,
            ),
            [sequence, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            8,
        );
        s.feed(0, &frame, at);
    }

    /// S1.6 — the plugin used `HeartbeatTracker`, which stored the sequence and
    /// validated nothing: a peer jumping by 50 read as perfectly healthy. The
    /// §8.3.3 rule is that an increment of more than 3 is a sequence error.
    #[test]
    fn peer_sequence_jump_is_a_sequence_error() {
        let mut s = claimed(100);
        let base = 5_000u64;
        beat(&mut s, 10, Instant::from_millis(base));
        while s.poll_event().is_some() {}

        beat(&mut s, 60, Instant::from_millis(base + 50));

        let flagged = std::iter::from_fn(|| s.poll_event()).any(|e| {
            matches!(
                e,
                Event::Heartbeat(HeartbeatEvent::SequenceError { source, .. }) if source == 0x42
            )
        });
        assert!(flagged, "a jump of 50 must be reported as a sequence error");
        assert_eq!(
            s.get::<Heartbeat>().unwrap().peer_state(0x42),
            Some(HbReceiverState::SequenceError)
        );
        assert!(s.get::<Heartbeat>().unwrap().any_peer_faulted());
    }

    /// S1.6 — the miss window is the §8.3.4 300 ms constant, not the transmit
    /// interval. With `every(1000)` the old code waited 3000 ms.
    #[test]
    fn comm_error_uses_the_standard_window_not_the_transmit_interval() {
        let mut s = claimed(1000);
        let base = 5_000u64;
        beat(&mut s, 1, Instant::from_millis(base));
        while s.poll_event().is_some() {}

        s.tick(Instant::from_millis(
            base + u64::from(HB_COMM_ERROR_TIMEOUT_MS) + 10,
        ));

        let flagged = std::iter::from_fn(|| s.poll_event()).any(|e| {
            matches!(
                e,
                Event::Heartbeat(HeartbeatEvent::CommError { source }) if source == 0x42
            )
        });
        assert!(
            flagged,
            "a silent peer must be flagged at 300 ms even when we transmit every 1000 ms"
        );
    }

    /// Sequences 254 and 255 are self-reported conditions, not counter values.
    #[test]
    fn sender_error_and_shutdown_are_distinguished() {
        let mut s = claimed(100);
        let base = 5_000u64;

        beat(&mut s, hb_seq::SENDER_ERROR, Instant::from_millis(base));
        assert!(std::iter::from_fn(|| s.poll_event()).any(|e| matches!(
            e,
            Event::Heartbeat(HeartbeatEvent::SenderError { source }) if source == 0x42
        )));

        beat(&mut s, hb_seq::SHUTDOWN, Instant::from_millis(base + 10));
        assert!(std::iter::from_fn(|| s.poll_event()).any(|e| matches!(
            e,
            Event::Heartbeat(HeartbeatEvent::GracefulShutdown { source }) if source == 0x42
        )));
    }
}
