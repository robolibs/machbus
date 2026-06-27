//! Task Controller *server* as a [`Plugin`]. Wraps [`TaskControllerServer`]:
//! routes `PGN_ECU_TO_TC` and `PGN_WORKING_SET_MASTER`, broadcasts status on
//! tick, and surfaces client/peer-control activity as [`TcServerEvent`].

use crate::isobus::tc::{PeerControlAssignment, TCOutbound, TCServerConfig, TaskControllerServer};
use crate::net::pgn_defs::{PGN_ECU_TO_TC, PGN_TC_TO_ECU, PGN_WORKING_SET_MASTER};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority, Result};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, TcServerEvent};
use crate::time::Instant;
use alloc::rc::Rc;
use core::{any::Any, cell::RefCell};

const INTERESTS: &[Pgn] = &[PGN_ECU_TO_TC, PGN_WORKING_SET_MASTER];

/// Task Controller (server role) plugin.
pub struct TcServer {
    server: TaskControllerServer,
    events: Rc<RefCell<Vec<TcServerEvent>>>,
    last_tick: Option<Instant>,
}

impl TcServer {
    /// Create with the given server config.
    ///
    /// # Errors
    /// Returns an error if the config fails validation.
    pub fn new(config: TCServerConfig) -> Result<Self> {
        config.validate()?;
        let mut server = TaskControllerServer::new(config);
        let events = Rc::new(RefCell::new(Vec::new()));
        wire_events(&mut server, &events);
        Ok(Self {
            server,
            events,
            last_tick: None,
        })
    }

    /// Direct access to the underlying server.
    pub fn server_mut(&mut self) -> &mut TaskControllerServer {
        &mut self.server
    }

    fn ship(out: TCOutbound, ctx: &mut PluginCtx<'_>) {
        ctx.send(
            out.pgn,
            out.data,
            out.dest.unwrap_or(BROADCAST_ADDRESS),
            Priority::Default,
        );
    }

    fn drain_events(&mut self, ctx: &mut PluginCtx<'_>) {
        for event in self.events.borrow_mut().drain(..) {
            ctx.emit(Event::TcServer(event));
        }
    }
}

impl Plugin for TcServer {
    fn name(&self) -> &'static str {
        "tc_server"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return;
        }
        let outbound: Vec<_> = if msg.pgn == PGN_WORKING_SET_MASTER {
            self.server
                .handle_working_set_master(msg)
                .into_iter()
                .collect()
        } else if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        } else {
            self.server.handle_client_message(msg).into_iter().collect()
        };
        for out in outbound {
            Self::ship(out, ctx);
        }
        self.drain_events(ctx);
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        self.last_tick = Some(now);

        let measurements: Vec<_> = self
            .server
            .update_measurements(elapsed)
            .into_iter()
            .collect();
        for out in measurements {
            Self::ship(out, ctx);
        }
        if let Some(payload) = self.server.update(elapsed) {
            ctx.send(
                PGN_TC_TO_ECU,
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

fn wire_events(server: &mut TaskControllerServer, sink: &Rc<RefCell<Vec<TcServerEvent>>>) {
    let s = sink.clone();
    server
        .on_state_change
        .subscribe(move |&st| s.borrow_mut().push(TcServerEvent::StateChanged(st)));
    let s = sink.clone();
    server
        .on_client_version_received
        .subscribe(move |&(address, version)| {
            s.borrow_mut()
                .push(TcServerEvent::ClientVersionReceived { address, version });
        });
    let s = sink.clone();
    server
        .on_peer_control_assignment_received
        .subscribe(move |a: &PeerControlAssignment| {
            s.borrow_mut().push(TcServerEvent::PeerControlAssignment {
                source: a.source_address,
                destination: a.destination_address,
                source_element: a.source_element,
                source_ddi: a.source_ddi,
                destination_element: a.destination_element,
                destination_ddi: a.destination_ddi,
            });
        });
}
