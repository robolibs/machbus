//! Control Function Functionalities (`PGN 0xFC8E`) responder as a [`Plugin`].
//! Answers PGN-requests for the local functionality set.

use crate::isobus::functionalities::Functionalities;
use crate::j1939::requested_pgn;
use crate::net::pgn_defs::{PGN_CF_FUNCTIONALITIES, PGN_REQUEST};
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_REQUEST];

/// Control Function Functionalities responder plugin.
pub struct ControlFunctionalities {
    model: Functionalities,
}

impl ControlFunctionalities {
    /// Advertise the given functionality model.
    #[must_use]
    pub fn new(model: Functionalities) -> Self {
        Self { model }
    }

    /// Read the functionality model.
    #[must_use]
    pub fn model(&self) -> &Functionalities {
        &self.model
    }

    /// Mutate the functionality model.
    pub fn model_mut(&mut self) -> &mut Functionalities {
        &mut self.model
    }
}

impl Plugin for ControlFunctionalities {
    fn name(&self) -> &'static str {
        "control_functionalities"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_REQUEST) {
            return;
        }
        if msg.destination != BROADCAST_ADDRESS && msg.destination != ctx.address() {
            return;
        }
        if requested_pgn(msg) != Some(PGN_CF_FUNCTIONALITIES) {
            return;
        }
        ctx.send(
            PGN_CF_FUNCTIONALITIES,
            self.model.serialize(),
            BROADCAST_ADDRESS,
            Priority::Default,
        );
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
