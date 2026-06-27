//! Group Function responder (carried on `PGN_ACKNOWLEDGMENT`) as a [`Plugin`].
//! Answers request/command Group Functions per a deterministic support policy.

use crate::isobus::group_function::{GroupFunctionMsg, GroupFunctionResponder};
use crate::net::pgn_defs::PGN_ACKNOWLEDGMENT;
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_ACKNOWLEDGMENT];

/// Group Function responder plugin.
pub struct GroupFunction {
    responder: GroupFunctionResponder,
}

impl GroupFunction {
    /// Answer Group Functions from the given responder/policy.
    #[must_use]
    pub fn new(responder: GroupFunctionResponder) -> Self {
        Self { responder }
    }

    /// Read the responder policy.
    #[must_use]
    pub fn responder(&self) -> &GroupFunctionResponder {
        &self.responder
    }

    /// Mutate the responder policy.
    pub fn responder_mut(&mut self) -> &mut GroupFunctionResponder {
        &mut self.responder
    }
}

impl Plugin for GroupFunction {
    fn name(&self) -> &'static str {
        "group_function"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_ACKNOWLEDGMENT) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        let Some(decoded) = GroupFunctionMsg::decode(&msg.data) else {
            return;
        };
        let Some(response) = self.responder.response_for(&decoded) else {
            return;
        };
        if let Ok(payload) = response.encode() {
            ctx.send(
                PGN_ACKNOWLEDGMENT,
                payload.to_vec(),
                msg.source,
                Priority::Default,
            );
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
