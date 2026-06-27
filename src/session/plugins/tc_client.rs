//! Task Controller client as a [`Plugin`]. Wraps the pump-style
//! [`TaskControllerClient`]: routes `PGN_TC_TO_ECU`, ships FSM-driven outbound
//! frames on tick, and surfaces state changes as [`TcEvent`].

use crate::isobus::tc::{DDOP, TCClientConfig, TCClientOutbound, TCState, TaskControllerClient};
use crate::net::pgn_defs::PGN_TC_TO_ECU;
use crate::net::{Address, BROADCAST_ADDRESS, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, TcEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_TC_TO_ECU];

/// Task Controller client plugin.
pub struct TcClient {
    client: TaskControllerClient,
    events: Rc<RefCell<Vec<TcEvent>>>,
    last_tick: Option<Instant>,
}

impl TcClient {
    /// Create with a config and the DDOP to upload.
    #[must_use]
    pub fn new(config: TCClientConfig, ddop: DDOP) -> Self {
        let mut client = TaskControllerClient::new(config);
        client.set_ddop(ddop);
        let events = Rc::new(RefCell::new(Vec::new()));
        let sink = events.clone();
        client
            .on_state_change
            .subscribe(move |&s| sink.borrow_mut().push(TcEvent::StateChanged(s)));
        Self {
            client,
            events,
            last_tick: None,
        }
    }

    /// Begin connecting to the TC server (uploads the DDOP).
    ///
    /// # Errors
    /// Propagates client connect errors (e.g. invalid DDOP).
    pub fn connect(&mut self) -> Result<()> {
        self.client.connect()
    }

    /// Disconnect from the TC server.
    ///
    /// # Errors
    /// Propagates client disconnect errors.
    pub fn disconnect(&mut self) -> Result<()> {
        self.client.disconnect()
    }

    /// Reupload a new DDOP while connected.
    ///
    /// # Errors
    /// Propagates client reupload errors.
    pub fn reupload_ddop(&mut self, ddop: DDOP) -> Result<()> {
        self.client.reupload_ddop(ddop)
    }

    /// Current FSM state.
    #[must_use]
    pub fn state(&self) -> TCState {
        self.client.state()
    }

    /// Whether the DDOP has been activated by the server.
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self.state(), TCState::Connected)
    }

    /// TC server address (post-discovery).
    #[must_use]
    pub fn tc_address(&self) -> Address {
        self.client.tc_address()
    }

    /// Direct access to the underlying client.
    pub fn client_mut(&mut self) -> &mut TaskControllerClient {
        &mut self.client
    }

    fn ship(out: TCClientOutbound, ctx: &mut PluginCtx<'_>) {
        ctx.send(
            out.pgn,
            out.data,
            out.dest.unwrap_or(BROADCAST_ADDRESS),
            Priority::Default,
        );
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::Tc(event));
        }
    }
}

impl Plugin for TcClient {
    fn name(&self) -> &'static str {
        "tc_client"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_TC_TO_ECU) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        let outbound: Vec<_> = self.client.handle_tc_message(msg).into_iter().collect();
        for out in outbound {
            Self::ship(out, ctx);
        }
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        let outbound: Vec<_> = self.client.update(elapsed).into_iter().collect();
        for out in outbound {
            Self::ship(out, ctx);
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
