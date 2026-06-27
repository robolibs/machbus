//! ISO 11783 heartbeat as a [`Plugin`]. Wraps the pure
//! [`HeartbeatSender`]/[`HeartbeatTracker`] codecs: broadcasts `PGN_HEARTBEAT`
//! on cadence, tracks peer sequences, and reports missed peers.

use crate::j1939::heartbeat::hb_seq;
use crate::j1939::{HeartbeatSender, HeartbeatTracker};
use crate::net::pgn_defs::PGN_HEARTBEAT;
use crate::net::{Address, BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, HeartbeatEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_HEARTBEAT];

/// ISO 11783 heartbeat plugin.
pub struct Heartbeat {
    sender: HeartbeatSender,
    tracker: HeartbeatTracker,
    missed: Rc<RefCell<Vec<(Address, u32)>>>,
    last: Option<Instant>,
}

impl Heartbeat {
    /// Broadcast a heartbeat every `interval_ms` and use it as the peer-miss window.
    #[must_use]
    pub fn every(interval_ms: u32) -> Self {
        let mut tracker = HeartbeatTracker::new(interval_ms);
        let missed = Rc::new(RefCell::new(Vec::new()));
        let sink = missed.clone();
        tracker
            .on_heartbeat_missed
            .subscribe(move |event| sink.borrow_mut().push(*event));
        Self {
            sender: HeartbeatSender::new(interval_ms),
            tracker,
            missed,
            last: None,
        }
    }

    /// Track a peer for missed-heartbeat detection.
    pub fn track(&mut self, address: Address) {
        self.tracker.track(address);
    }

    /// Stop tracking a peer.
    pub fn untrack(&mut self, address: Address) {
        self.tracker.untrack(address);
    }

    /// Last observed sequence for a tracked peer.
    #[must_use]
    pub fn last_sequence(&self, address: Address) -> Option<u8> {
        self.tracker.last_sequence(address)
    }

    /// Missed-heartbeat count for a tracked peer.
    #[must_use]
    pub fn missed_count(&self, address: Address) -> u32 {
        self.tracker.missed_count(address)
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
        self.tracker.handle_message(msg);
        ctx.emit(Event::Heartbeat(HeartbeatEvent::Received {
            source: msg.source,
            sequence,
        }));
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last.map_or(0, |last| now.millis_since(last));
        self.last = Some(now);

        self.tracker.update(elapsed);
        for (source, missed_count) in self.missed.borrow_mut().drain(..) {
            ctx.emit(Event::Heartbeat(HeartbeatEvent::Missed {
                source,
                missed_count,
            }));
        }

        if self.sender.update(elapsed) {
            let sequence = self.sender.next_sequence();
            ctx.send(
                PGN_HEARTBEAT,
                vec![sequence],
                BROADCAST_ADDRESS,
                Priority::Default,
            );
            ctx.emit(Event::Heartbeat(HeartbeatEvent::Sent { sequence }));
        }
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
