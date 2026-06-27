//! Virtual Terminal *server* as a [`Plugin`]. Wraps [`VTServer`]: routes
//! `PGN_ECU_TO_VT` from clients, broadcasts `VT_STATUS` on tick (all outbound on
//! `PGN_VT_TO_ECU`), and surfaces client/key/value activity as [`VtServerEvent`].

use crate::isobus::vt::{OutboundFrame, VTServer, VTServerConfig, VTServerState};
use crate::net::pgn_defs::{PGN_ECU_TO_VT, PGN_VT_TO_ECU};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, VtServerEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_ECU_TO_VT];

/// Virtual Terminal (server role) plugin.
pub struct VtServer {
    server: VTServer,
    events: Rc<RefCell<Vec<VtServerEvent>>>,
    last_tick: Option<Instant>,
}

impl VtServer {
    /// Create with the given server config.
    ///
    /// # Errors
    /// Returns an error if the config fails validation.
    pub fn new(config: VTServerConfig) -> Result<Self> {
        config.validate()?;
        let mut server = VTServer::new(config);
        let events = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut server, &events);
        Ok(Self {
            server,
            events,
            last_tick: None,
        })
    }

    /// Start the server (`Disconnected` → `WaitForConnect`).
    ///
    /// # Errors
    /// Propagates server start errors.
    pub fn start(&mut self) -> Result<()> {
        self.server.start()
    }

    /// Stop the server.
    ///
    /// # Errors
    /// Propagates server stop errors.
    pub fn stop(&mut self) -> Result<()> {
        self.server.stop()
    }

    /// Current server state.
    #[must_use]
    pub fn state(&self) -> VTServerState {
        self.server.state()
    }

    /// Direct access to the underlying server.
    pub fn server_mut(&mut self) -> &mut VTServer {
        &mut self.server
    }

    fn ship(out: OutboundFrame, ctx: &mut PluginCtx<'_>) {
        ctx.send(
            PGN_VT_TO_ECU,
            out.data,
            out.dest.unwrap_or(BROADCAST_ADDRESS),
            Priority::Default,
        );
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::VtServer(event));
        }
    }
}

impl Plugin for VtServer {
    fn name(&self) -> &'static str {
        "vt_server"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_ECU_TO_VT) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        let outbound: Vec<_> = self.server.handle_ecu_message(msg).into_iter().collect();
        for out in outbound {
            Self::ship(out, ctx);
        }
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        if let Some(payload) = self.server.update(elapsed) {
            ctx.send(
                PGN_VT_TO_ECU,
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

fn wire_events(server: &mut VTServer, sink: &Rc<RefCell<Vec<VtServerEvent>>>) {
    let s = sink.clone();
    server
        .on_state_change
        .subscribe(move |&st| s.borrow_mut().push(VtServerEvent::StateChanged(st)));
    let s = sink.clone();
    server
        .on_client_connected
        .subscribe(move |&addr| s.borrow_mut().push(VtServerEvent::ClientConnected(addr)));
    let s = sink.clone();
    server.on_client_disconnected.subscribe(move |&addr| {
        s.borrow_mut().push(VtServerEvent::ClientDisconnected(addr));
    });
    let s = sink.clone();
    server.on_active_ws_changed.subscribe(move |&(from, to)| {
        s.borrow_mut()
            .push(VtServerEvent::ActiveWorkingSetChanged { from, to });
    });
    let s = sink.clone();
    server.on_soft_key_activation.subscribe(move |&(id, key)| {
        s.borrow_mut().push(VtServerEvent::SoftKey {
            id,
            key_number: key,
        });
    });
    let s = sink.clone();
    server.on_button_activation.subscribe(move |&(id, key)| {
        s.borrow_mut().push(VtServerEvent::Button {
            id,
            key_number: key,
        });
    });
    let s = sink.clone();
    server
        .on_numeric_value_change
        .subscribe(move |&(id, value)| {
            s.borrow_mut()
                .push(VtServerEvent::NumericValueChanged { id, value });
        });
    let s = sink.clone();
    server.on_string_value_change.subscribe(move |(id, value)| {
        s.borrow_mut().push(VtServerEvent::StringValueChanged {
            id: *id,
            value: value.clone(),
        });
    });
    let s = sink.clone();
    server
        .on_input_object_selected
        .subscribe(move |&(id, selected, edit_active)| {
            s.borrow_mut().push(VtServerEvent::InputObjectSelected {
                id,
                selected,
                edit_active,
            });
        });
}
