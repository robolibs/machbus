//! Language Command (`PGN 0xFE0F`) as a [`Plugin`]. Broadcasts local language /
//! unit preferences and caches peer preferences.

use crate::j1939::LanguageData;
use crate::net::pgn_defs::PGN_LANGUAGE_COMMAND;
use crate::net::{BROADCAST_ADDRESS, Message, Pgn, Priority};
use crate::session::plugin::{Plugin, PluginCtx};
use crate::session::sys::{Event, LanguageCommandEvent};
use crate::time::Instant;
use core::any::Any;

const INTERESTS: &[Pgn] = &[PGN_LANGUAGE_COMMAND];

/// Language Command plugin.
pub struct LanguageCommand {
    local: LanguageData,
    last: Option<LanguageCommandEvent>,
    pending: bool,
}

impl LanguageCommand {
    /// Create with the local language/unit preference.
    #[must_use]
    pub fn new(local: LanguageData) -> Self {
        Self {
            local,
            last: None,
            pending: false,
        }
    }

    /// Current local preference.
    #[must_use]
    pub fn local(&self) -> LanguageData {
        self.local
    }

    /// Replace the local preference.
    pub fn set_local(&mut self, local: LanguageData) {
        self.local = local;
    }

    /// Last peer Language Command observed.
    #[must_use]
    pub fn last(&self) -> Option<LanguageCommandEvent> {
        self.last
    }

    /// Queue a broadcast of the current local preference (flushed on tick).
    pub fn broadcast(&mut self) {
        self.pending = true;
    }

    /// Set the local preference and queue a broadcast.
    pub fn broadcast_data(&mut self, data: LanguageData) {
        self.local = data;
        self.pending = true;
    }
}

impl Plugin for LanguageCommand {
    fn name(&self) -> &'static str {
        "language_command"
    }

    fn interests(&self) -> &'static [Pgn] {
        INTERESTS
    }

    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        if !msg.has_usable_envelope_for_pgn(PGN_LANGUAGE_COMMAND) {
            return;
        }
        let Some(data) = LanguageData::decode(msg) else {
            return;
        };
        let event = LanguageCommandEvent {
            source: msg.source,
            data,
        };
        self.last = Some(event);
        ctx.emit(Event::LanguageCommand(event));
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        if self.pending {
            self.pending = false;
            ctx.send(
                PGN_LANGUAGE_COMMAND,
                self.local.encode().to_vec(),
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
