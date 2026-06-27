//! The [`Plugin`] trait — a composable subsystem unit.
//!
//! A plugin is self-contained: it declares the PGNs it cares about
//! ([`Plugin::interests`]), reacts to received messages ([`Plugin::on_frame`]),
//! and does periodic work ([`Plugin::on_tick`]). It never touches IO or a clock
//! directly — the host [`Session`](super::Session) feeds it messages and the
//! current time, and collects its outbound frames and events through a
//! [`PluginCtx`]. This is the "C" (composition) half of the redesign and the
//! second fine-control surface: hold a plugin instance (via
//! [`Session::get`](super::Session::get)) and drive it directly.

use alloc::{collections::VecDeque, vec::Vec};
use core::any::Any;

use crate::net::{Address, Message, Name, Pgn, Priority};
use crate::session::sys::Event;
use crate::time::Instant;

/// A composable subsystem plugged into a [`Session`](super::Session).
///
/// Implementors must also be `'static` (so they can be type-identified for
/// [`Session::get`](super::Session::get)). The `as_any`/`as_any_mut` shims exist
/// only to enable that typed downcast and are trivial one-liners.
pub trait Plugin: Any {
    /// Stable identifier, for diagnostics/logging.
    fn name(&self) -> &'static str;

    /// PGNs this plugin wants delivered to [`Self::on_frame`]. Empty = none.
    fn interests(&self) -> &'static [Pgn] {
        &[]
    }

    /// Multi-frame (NMEA 2000 Fast Packet) PGNs this plugin consumes. The host
    /// registers these so the network layer reassembles them before dispatch.
    fn fast_packet_pgns(&self) -> &'static [Pgn] {
        &[]
    }

    /// A received [`Message`] whose PGN is in [`Self::interests`].
    fn on_frame(&mut self, msg: &Message, ctx: &mut PluginCtx<'_>) {
        let _ = (msg, ctx);
    }

    /// Periodic work (cadences, timeouts). Returns the next instant the plugin
    /// wants servicing, or `None` if idle.
    fn on_tick(&mut self, ctx: &mut PluginCtx<'_>) -> Option<Instant> {
        let _ = ctx;
        None
    }

    /// Upcast for typed component lookup. Implement as `self`.
    fn as_any(&self) -> &dyn Any;
    /// Mutable upcast for typed component lookup. Implement as `self`.
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// An outbound send request a plugin queues via [`PluginCtx::send`]. The host
/// flushes these through the network layer after the plugin returns.
pub(crate) struct SendCmd {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub dst: Address,
    pub prio: Priority,
}

/// A control-plane action a plugin requests via [`PluginCtx`]. The host applies
/// these against the network core after the plugin returns (they need access to
/// the internal control function, which a plugin cannot touch directly).
pub(crate) enum CtxAction {
    /// Adopt a new NAME for the local control function and re-claim.
    SetName(Name),
    /// (Re)start address claiming.
    RestartAddressClaim,
    /// Re-send address-claim responses (answer to RequestAddressClaim).
    SendAddressClaimResponses,
}

/// A plugin's keyhole to the host during a callback: read the current address
/// and time, queue outbound frames, and emit application events.
pub struct PluginCtx<'a> {
    address: Address,
    name: Name,
    now: Instant,
    sends: &'a mut Vec<SendCmd>,
    events: &'a mut VecDeque<Event>,
    actions: &'a mut Vec<CtxAction>,
}

impl<'a> PluginCtx<'a> {
    pub(crate) fn new(
        address: Address,
        name: Name,
        now: Instant,
        sends: &'a mut Vec<SendCmd>,
        events: &'a mut VecDeque<Event>,
        actions: &'a mut Vec<CtxAction>,
    ) -> Self {
        Self {
            address,
            name,
            now,
            sends,
            events,
            actions,
        }
    }

    /// The current monotonic time.
    #[must_use]
    pub fn now(&self) -> Instant {
        self.now
    }

    /// Our claimed source address (or `NULL_ADDRESS` before claim completes).
    #[must_use]
    pub fn address(&self) -> Address {
        self.address
    }

    /// The local control function's current NAME.
    #[must_use]
    pub fn name(&self) -> Name {
        self.name
    }

    /// Queue an application-layer message for transmission.
    pub fn send(&mut self, pgn: Pgn, data: impl Into<Vec<u8>>, dst: Address, prio: Priority) {
        self.sends.push(SendCmd {
            pgn,
            data: data.into(),
            dst,
            prio,
        });
    }

    /// Emit an application event onto the session's event queue.
    pub fn emit(&mut self, event: Event) {
        self.events.push_back(event);
    }

    /// Adopt a new NAME for the local control function and re-claim an address.
    pub fn set_name(&mut self, name: Name) {
        self.actions.push(CtxAction::SetName(name));
    }

    /// (Re)start address claiming for the local control function.
    pub fn restart_address_claim(&mut self) {
        self.actions.push(CtxAction::RestartAddressClaim);
    }

    /// Re-send address-claim responses (answer to a RequestAddressClaim).
    pub fn send_address_claim_responses(&mut self) {
        self.actions.push(CtxAction::SendAddressClaimResponses);
    }
}
