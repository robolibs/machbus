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

/// The ISB is a periodic message. A receiver that stops hearing it must not
/// keep treating the last "permit all operations" as current.
pub const ISB_REBROADCAST_MS: u32 = 100;

/// Three missed broadcasts and the transmitter is presumed gone.
pub const ISB_TIMEOUT_MS: u32 = 300;

/// The ISB rule an autonomy controller has to obey, in one place.
///
/// Both `AutoDrive` and `Guidance` own a stop latch and both must react to the
/// Auxiliary Shortcut Button the same way. They did not: the rules below landed
/// in `AutoDrive` and `Guidance` kept an older, narrower check, so cutting the
/// ISB cable left `Guidance` steering. Keeping the rule here means the next
/// controller cannot inherit half of it.
///
/// Two rules, both load-bearing:
/// - **Error is not permission.** Only an explicit *permit all operations*
///   clears the stop; every other state asserts it.
/// - **Silence is not permission.** A source seen once and then gone has taken
///   the operator's stop authority with it ([`ISB_TIMEOUT_MS`]).
///
/// Note that "every other state" currently includes *not available*, so a CF
/// padding a frame with 0xFF latches a stop. That is deliberate for now — it
/// matches what `AutoDrive` already shipped — but it is a design decision, not
/// an ISO 11783-7 requirement, and this is the one place to revisit it.
#[derive(Debug, Clone, Copy, Default)]
pub struct IsbGuard {
    last_seen_at: Option<Instant>,
    asserted: bool,
}

impl IsbGuard {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            last_seen_at: None,
            asserted: false,
        }
    }

    /// Record a decoded ISB state. Returns `true` while stop is asserted.
    pub const fn observe(&mut self, state: ShortcutButtonState, now: Instant) -> bool {
        self.last_seen_at = Some(now);
        self.asserted = !matches!(state, ShortcutButtonState::PermitAllImplementsToOperate);
        self.asserted
    }

    /// Age the guard. Returns `true` while stop is asserted, including when
    /// this call is what tripped it.
    pub fn tick(&mut self, now: Instant) -> bool {
        if let Some(seen) = self.last_seen_at
            && now.millis_since(seen) >= ISB_TIMEOUT_MS
        {
            self.asserted = true;
        }
        self.asserted
    }

    /// `true` while the operator is commanding stop, or a source that was seen
    /// has gone silent. A latched stop must not be cleared while this holds.
    #[must_use]
    pub const fn is_asserted(&self) -> bool {
        self.asserted
    }
}

/// Shortcut Button plugin.
#[derive(Default)]
pub struct ShortcutButton {
    last: Option<ShortcutButtonEvent>,
    /// When `last` arrived, so a stale ISB source stops reading as an all-clear.
    last_at: Option<Instant>,
    transition_count: u8,
    /// State this node broadcasts, re-sent on a cadence until changed.
    local_state: Option<ShortcutButtonState>,
    last_tx_at: Option<Instant>,
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

    /// Set the state this node broadcasts. It is then re-sent every
    /// [`ISB_REBROADCAST_MS`] until changed.
    ///
    /// The transition count advances only when the state actually changes. It
    /// used to advance on every transmission, so a conformant periodic re-send
    /// looked to receivers like an unbroken stream of new transitions.
    pub fn broadcast(&mut self, state: ShortcutButtonState) {
        if self.local_state != Some(state) {
            if self.local_state.is_some() {
                self.transition_count = self.transition_count.wrapping_add(1);
            }
            self.local_state = Some(state);
            // A state change goes out immediately rather than waiting.
            self.last_tx_at = None;
        }
    }

    /// The peer ISB state, or `None` when none has arrived or the transmitter
    /// has gone silent past [`ISB_TIMEOUT_MS`].
    ///
    /// Losing the ISB source is a fault, not an all-clear: `last()` keeps
    /// returning the cached record for display, but this accessor expires.
    #[must_use]
    pub fn current(&self, now: Instant) -> Option<ShortcutButtonEvent> {
        match (self.last, self.last_at) {
            (Some(e), Some(t)) if now.millis_since(t) < ISB_TIMEOUT_MS => Some(e),
            _ => None,
        }
    }

    /// `true` when a peer is actively commanding "stop all implement
    /// operations", or when a previously-seen ISB source has gone silent.
    #[must_use]
    pub fn is_stop_active(&self, now: Instant) -> bool {
        match self.current(now) {
            Some(e) => e.message.state == ShortcutButtonState::StopImplementOperations,
            // Seen once, then lost: treat as stop, not as permission.
            None => self.last.is_some(),
        }
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
        self.last_at = Some(ctx.now());
        ctx.emit(Event::ShortcutButton(event));
    }

    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let now = ctx.now();
        for payload in self.pending.drain(..) {
            ctx.send(
                PGN_SHORTCUT_BUTTON,
                payload,
                BROADCAST_ADDRESS,
                Priority::BelowNormal,
            );
        }

        if let Some(state) = self.local_state {
            let due = self
                .last_tx_at
                .is_none_or(|t| now.millis_since(t) >= ISB_REBROADCAST_MS);
            if due {
                ctx.send(
                    PGN_SHORTCUT_BUTTON,
                    encode_with_transition_count(state, self.transition_count).to_vec(),
                    BROADCAST_ADDRESS,
                    Priority::BelowNormal,
                );
                self.last_tx_at = Some(now);
            }
        }

        Some(now.add_millis(u64::from(ISB_REBROADCAST_MS)))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}
