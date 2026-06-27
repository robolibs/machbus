//! ISO 11783-14 Sequence Control *client* as a [`Plugin`]. Wraps [`SCClient`]:
//! consumes master status (`PGN_SC_MASTER_STATUS`), emits client status, and
//! surfaces sequence requests as [`ScEvent`].

use crate::isobus::sc::{SCClient, SCClientConfig, SCState};
use crate::net::pgn_defs::{PGN_SC_CLIENT_STATUS, PGN_SC_MASTER_STATUS};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, ScEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_SC_MASTER_STATUS];

/// Sequence Control client plugin.
pub struct ScClient {
    client: SCClient,
    events: Rc<RefCell<Vec<ScEvent>>>,
    pending: Vec<[u8; 8]>,
    last_tick: Option<Instant>,
}

impl ScClient {
    /// Create with the given client config.
    #[must_use]
    pub fn new(config: SCClientConfig) -> Self {
        let mut client = SCClient::new(config);
        let events = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut client, &events);
        Self {
            client,
            events,
            pending: Vec::new(),
            last_tick: None,
        }
    }

    /// Current client state.
    #[must_use]
    pub fn state(&self) -> SCState {
        self.client.state()
    }

    /// Whether the client is busy.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.client.is_busy()
    }

    /// Set/clear busy; a client status is queued when the spacing window allows.
    pub fn set_busy(&mut self, busy: bool) {
        if let Some(payload) = self.client.set_busy(busy) {
            self.pending.push(payload);
        }
    }

    /// Acknowledge a requested step as complete.
    ///
    /// # Errors
    /// Propagates client errors.
    pub fn report_step_complete(&mut self, step_id: u16) -> Result<()> {
        if let Some(payload) = self.client.report_step_complete(step_id)? {
            self.pending.push(payload);
        }
        Ok(())
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::Sc(event));
        }
    }
}

impl Plugin for ScClient {
    fn name(&self) -> &'static str {
        "sc_client"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_SC_MASTER_STATUS) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        if let Some(payload) = self.client.handle_master_status(msg) {
            ctx.send(
                PGN_SC_CLIENT_STATUS,
                payload.to_vec(),
                BROADCAST_ADDRESS,
                Priority::Default,
            );
        }
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        for payload in self.pending.drain(..) {
            ctx.send(
                PGN_SC_CLIENT_STATUS,
                payload.to_vec(),
                BROADCAST_ADDRESS,
                Priority::Default,
            );
        }
        if let Some(payload) = self.client.update(elapsed) {
            ctx.send(
                PGN_SC_CLIENT_STATUS,
                payload.to_vec(),
                BROADCAST_ADDRESS,
                Priority::Default,
            );
        }
        self.drain_events(ctx);
        None
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

fn wire_events(client: &mut SCClient, sink: &Rc<RefCell<Vec<ScEvent>>>) {
    let s = sink.clone();
    client.on_state_change.subscribe(move |&(from, to)| {
        s.borrow_mut()
            .push(ScEvent::ClientStateChanged { from, to })
    });
    let s = sink.clone();
    client
        .on_sequence_start
        .subscribe(move |_| s.borrow_mut().push(ScEvent::ClientSequenceStart));
    let s = sink.clone();
    client
        .on_step_request
        .subscribe(move |&step_id| s.borrow_mut().push(ScEvent::ClientStepRequest { step_id }));
    let s = sink.clone();
    client
        .on_pause
        .subscribe(move |_| s.borrow_mut().push(ScEvent::ClientPause));
    let s = sink.clone();
    client
        .on_resume
        .subscribe(move |_| s.borrow_mut().push(ScEvent::ClientResume));
    let s = sink.clone();
    client
        .on_abort
        .subscribe(move |_| s.borrow_mut().push(ScEvent::ClientAbort));
}
