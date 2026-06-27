//! Request2 (`PGN 0xC900`) responder as a [`Plugin`]. Answers PGN-Request2 from
//! a registry, selecting direct vs Transfer replies via the pure responder.

use crate::j1939::Request2Responder;
use crate::net::pgn_defs::PGN_REQUEST2;
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_REQUEST2];

/// Request2 responder plugin.
pub struct Request2 {
    responder: Request2Responder,
}

impl Request2 {
    /// Answer Request2 from the given responder/registry.
    #[must_use]
    pub fn new(responder: Request2Responder) -> Self {
        Self { responder }
    }

    /// Read the responder registry.
    #[must_use]
    pub fn responder(&self) -> &Request2Responder {
        &self.responder
    }

    /// Mutate the responder registry.
    pub fn responder_mut(&mut self) -> &mut Request2Responder {
        &mut self.responder
    }
}

impl Plugin for Request2 {
    fn name(&self) -> &'static str {
        "request2"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_REQUEST2) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        if let Some(reply) = self.responder.handle_message(msg) {
            ctx.send(reply.pgn, reply.data, reply.destination, Priority::Default);
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
