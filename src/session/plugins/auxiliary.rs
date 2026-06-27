//! ISO 11783 AUX-O / AUX-N input status as a [`Plugin`]. Caches peer
//! auxiliary-function state and broadcasts local state on command.

use crate::isobus::auxiliary::{AuxNFunction, AuxOFunction};
use crate::net::pgn_defs::{PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2};
use crate::net::{Address, BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{AuxiliaryEvent, Event};
use crate::time::Instant;
use alloc::collections::BTreeMap;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2];

/// AUX-O / AUX-N plugin.
#[derive(Default)]
pub struct Auxiliary {
    last_aux_o: BTreeMap<(Address, u8), AuxOFunction>,
    last_aux_n: BTreeMap<(Address, u8), AuxNFunction>,
    pending: Vec<(Pgn, Vec<u8>)>,
}

impl Auxiliary {
    /// Create an auxiliary plugin.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Last accepted AUX-O status for `(source, function_number)`.
    #[must_use]
    pub fn last_aux_o(&self, source: Address, function_number: u8) -> Option<AuxOFunction> {
        self.last_aux_o.get(&(source, function_number)).copied()
    }

    /// Last accepted AUX-N status for `(source, function_number)`.
    #[must_use]
    pub fn last_aux_n(&self, source: Address, function_number: u8) -> Option<AuxNFunction> {
        self.last_aux_n.get(&(source, function_number)).copied()
    }

    /// Queue a local AUX-O broadcast (flushed on tick).
    pub fn broadcast_aux_o(&mut self, function: AuxOFunction) {
        self.pending
            .push((PGN_AUX_INPUT_STATUS, function.encode().to_vec()));
    }

    /// Queue a local AUX-N type-2 broadcast (flushed on tick).
    pub fn broadcast_aux_n(&mut self, function: AuxNFunction) {
        self.pending
            .push((PGN_AUX_INPUT_TYPE2, function.encode().to_vec()));
    }
}

impl Plugin for Auxiliary {
    fn name(&self) -> &'static str {
        "auxiliary"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return;
        }
        let event = match msg.pgn {
            PGN_AUX_INPUT_STATUS => AuxOFunction::decode(msg).map(|function| {
                self.last_aux_o
                    .insert((msg.source, function.function_number), function);
                AuxiliaryEvent::AuxO {
                    source: msg.source,
                    function,
                }
            }),
            PGN_AUX_INPUT_TYPE2 => AuxNFunction::decode(msg).map(|function| {
                self.last_aux_n
                    .insert((msg.source, function.function_number), function);
                AuxiliaryEvent::AuxN {
                    source: msg.source,
                    function,
                }
            }),
            _ => None,
        };
        if let Some(event) = event {
            ctx.emit(Event::Auxiliary(event));
        }
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        for (pgn, data) in self.pending.drain(..) {
            ctx.send(pgn, data, BROADCAST_ADDRESS, Priority::Default);
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
