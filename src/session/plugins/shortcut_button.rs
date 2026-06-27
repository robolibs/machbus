//! Shortcut Button / ISB (`PGN 0xFD02`) as a [`Plugin`]. Caches peer state and
//! broadcasts local state with an auto-incrementing transition count.

use crate::j1939::shortcut_button::{
    ShortcutButtonState, decode_message, encode_with_transition_count,
};
use crate::net::pgn_defs::PGN_SHORTCUT_BUTTON;
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, ShortcutButtonEvent};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_SHORTCUT_BUTTON];

/// Shortcut Button plugin.
#[derive(Default)]
pub struct ShortcutButton {
    last: Option<ShortcutButtonEvent>,
    transition_count: u8,
    pending: Vec<Vec<u8>>,
}

impl ShortcutButton {
    /// Create a Shortcut Button plugin.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Last peer Shortcut Button status observed.
    #[must_use]
    pub fn last(&self) -> Option<ShortcutButtonEvent> {
        self.last
    }

    /// Queue a broadcast of `state` with the next local transition count.
    pub fn broadcast(&mut self, state: ShortcutButtonState) {
        let count = self.transition_count;
        self.pending
            .push(encode_with_transition_count(state, count).to_vec());
        self.transition_count = count.wrapping_add(1);
    }

    /// Queue a broadcast with an explicit transition count.
    pub fn broadcast_with_transition_count(&mut self, state: ShortcutButtonState, count: u8) {
        self.pending
            .push(encode_with_transition_count(state, count).to_vec());
    }
}

impl Plugin for ShortcutButton {
    fn name(&self) -> &'static str {
        "shortcut_button"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_SHORTCUT_BUTTON) {
            return;
        }
        let Some(decoded) = decode_message(msg) else {
            return;
        };
        let event = ShortcutButtonEvent {
            source: msg.source,
            message: decoded,
        };
        self.last = Some(event);
        ctx.emit(Event::ShortcutButton(event));
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        for payload in self.pending.drain(..) {
            ctx.send(
                PGN_SHORTCUT_BUTTON,
                payload,
                BROADCAST_ADDRESS,
                Priority::Default,
            );
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
