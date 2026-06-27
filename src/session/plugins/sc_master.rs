//! ISO 11783-14 Sequence Control *master* as a [`Plugin`]. Wraps [`SCMaster`]:
//! consumes client status (`PGN_SC_CLIENT_STATUS`), broadcasts master status on
//! tick, and surfaces sequence progress as [`ScEvent`].

use crate::isobus::sc::{SCMaster, SCMasterConfig, SCState, SequenceStep};
use crate::net::pgn_defs::{PGN_SC_CLIENT_STATUS, PGN_SC_MASTER_STATUS};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, ScEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_SC_CLIENT_STATUS];

/// Sequence Control master plugin.
pub struct ScMaster {
    master: SCMaster,
    events: Rc<RefCell<Vec<ScEvent>>>,
    last_tick: Option<Instant>,
}

impl ScMaster {
    /// Create with the given master config.
    #[must_use]
    pub fn new(config: SCMasterConfig) -> Self {
        let mut master = SCMaster::new(config);
        let events = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut master, &events);
        Self {
            master,
            events,
            last_tick: None,
        }
    }

    /// Current master state.
    #[must_use]
    pub fn state(&self) -> SCState {
        self.master.state()
    }

    /// Register a sequence step before starting.
    ///
    /// # Errors
    /// Propagates master errors.
    pub fn add_step(&mut self, step: SequenceStep) -> Result<()> {
        self.master.add_step(step)
    }

    /// Start the sequence (status emitted on the next tick).
    ///
    /// # Errors
    /// Propagates master errors.
    pub fn start(&mut self) -> Result<()> {
        self.master.start()
    }

    /// Pause the active step.
    ///
    /// # Errors
    /// Propagates master errors.
    pub fn pause(&mut self) -> Result<()> {
        self.master.pause()
    }

    /// Resume from pause.
    ///
    /// # Errors
    /// Propagates master errors.
    pub fn resume(&mut self) -> Result<()> {
        self.master.resume()
    }

    /// Abort the sequence.
    ///
    /// # Errors
    /// Propagates master errors.
    pub fn abort(&mut self) -> Result<()> {
        self.master.abort()
    }

    /// Mark a step complete.
    ///
    /// # Errors
    /// Propagates master errors.
    pub fn step_completed(&mut self, step_id: u16) -> Result<()> {
        self.master.step_completed(step_id)
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::Sc(event));
        }
    }
}

impl Plugin for ScMaster {
    fn name(&self) -> &'static str {
        "sc_master"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_SC_CLIENT_STATUS) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        self.master.handle_client_status(msg);
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        if let Some(payload) = self.master.update(elapsed) {
            ctx.send(
                PGN_SC_MASTER_STATUS,
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

fn wire_events(master: &mut SCMaster, sink: &Rc<RefCell<Vec<ScEvent>>>) {
    let s = sink.clone();
    master.on_state_change.subscribe(move |&(from, to)| {
        s.borrow_mut()
            .push(ScEvent::MasterStateChanged { from, to })
    });
    let s = sink.clone();
    master
        .on_step_started
        .subscribe(move |&step_id| s.borrow_mut().push(ScEvent::MasterStepStarted { step_id }));
    let s = sink.clone();
    master.on_step_completed.subscribe(move |&step_id| {
        s.borrow_mut()
            .push(ScEvent::MasterStepCompleted { step_id })
    });
    let s = sink.clone();
    master
        .on_sequence_complete
        .subscribe(move |_| s.borrow_mut().push(ScEvent::MasterSequenceComplete));
    let s = sink.clone();
    master
        .on_timeout
        .subscribe(move |&reason| s.borrow_mut().push(ScEvent::MasterTimeout { reason }));
    let s = sink.clone();
    master.on_client_status.subscribe(move |&(source, state)| {
        s.borrow_mut()
            .push(ScEvent::MasterClientStatus { source, state });
    });
}
