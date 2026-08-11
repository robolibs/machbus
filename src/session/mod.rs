//! `Session` — the sans-IO protocol core (Layer 1 of the facade redesign).
//!
//! A `Session` is a pure state machine: you [`feed`](Session::feed) it received
//! frames stamped with the current [`Instant`], advance its timers with
//! [`tick`](Session::tick), and drain its outputs with
//! [`poll_transmit`](Session::poll_transmit) and
//! [`poll_event`](Session::poll_event). It owns no CAN interface and reads no
//! clock — a driver (Layer 2, added later) bridges it to real IO. This is what
//! makes the core deterministically testable and `no_std`-ready.
//!
//! Subsystems are composed as [`Plugin`]s via the builder, and can be driven
//! directly for fine control via [`Session::get`] / [`Session::get_mut`].
//!
//! ```no_run
//! use machbus::session::{Session, plugins::Diagnostics};
//! use machbus::time::Instant;
//! use machbus::prelude::*;
//!
//! let mut s = Session::builder(Name::default(), 0x80)
//!     .plug(Diagnostics::every(1000))
//!     .build()
//!     .unwrap();
//! s.start().unwrap();
//!
//! let mut now = Instant::ZERO;
//! // ... in your loop: feed received frames, tick, drain outputs ...
//! s.tick(now);
//! while let Some((port, frame)) = s.poll_transmit() { /* bus.send(port, frame) */ }
//! while let Some(event) = s.poll_event() { /* handle(event) */ }
//! now = now.add_millis(10);
//! ```

pub mod driver;
pub mod events;
mod plugin;
pub mod plugins;
pub mod presets;
/// Internal protocol/event layer shared by the session facade.
///
/// Not a public facade — `session` is the only public entry point. This module
/// holds the unified [`Event`] enum, the per-subsystem `*Event` types, and the
/// reusable decode helpers the plugins build on.
pub(crate) mod sys;

pub use driver::{Controls, Driver, EndpointTransport, Subscription, Transport};
pub use events::SubsystemEvent;
pub use plugin::{Plugin, PluginCtx};

// The public event surface — `session` is the single facade, so the unified
// event enum and every subsystem event type are re-exported here.
pub use sys::{
    AutodriveRefusal, AutomationStatus, AuxiliaryEvent, BusEvent, ClaimEvent, DiagEvent,
    DmMemoryEvent, DriveCommand, Event, EventQueue, FsEvent, FsServerEvent, GnssEvent,
    GuidanceEvent, HeartbeatEvent, Hitch, ImplementEvent, LanguageCommandEvent, MaintainPowerEvent,
    OverflowPolicy, PowertrainEvent, PowertrainSnapshot, Pto, SafeStopTrigger, ScEvent,
    ShortcutButtonEvent, StopLatch, TcEvent, TcServerEvent, TimEvent, VtEvent, VtServerEvent,
};

use plugin::{CtxAction, SendCmd};

use crate::net::can_adapter;
use crate::net::{
    Address, ClaimState, Error, Frame, IsoNet, Message, NULL_ADDRESS, Name, NetworkConfig, Pgn,
    Result,
};
use crate::time::Instant;
use alloc::{boxed::Box, collections::VecDeque, rc::Rc, vec::Vec};
use core::{any::TypeId, cell::RefCell};

/// A CAN adapter link that is never driven. A [`Session`] runs the network core
/// in capture mode with no endpoints attached, so these methods are unreachable
/// in practice — they exist only to satisfy the `IsoNet<L: Link>` bound.
pub struct NullLink;

impl can_adapter::Link for NullLink {
    fn send(&mut self, _frame: &can_adapter::Frame) -> can_adapter::Result<()> {
        Ok(())
    }
    fn recv(&mut self) -> can_adapter::Result<can_adapter::Frame> {
        Err(can_adapter::Error::Empty)
    }
    fn can_send(&self) -> bool {
        false
    }
    fn can_recv(&self) -> bool {
        false
    }
    fn name(&self) -> &str {
        "null"
    }
}

/// How many times [`Session::dispatch_events`] re-runs to deliver events a
/// plugin emitted while reacting to another event.
const MAX_EVENT_DISPATCH_ROUNDS: usize = 8;

/// The sans-IO protocol core. See the [module docs](self).
pub struct Session {
    net: IsoNet<NullLink>,
    cf: crate::net::InternalCfHandle,
    plugins: Vec<Box<dyn Plugin>>,
    inbox: Rc<RefCell<VecDeque<Message>>>,
    /// Source addresses seen violating our claim, queued by the network layer.
    violations: Rc<RefCell<Vec<Address>>>,
    events: VecDeque<Event>,
    /// Events observed but not yet shown to the plugins. Everything the session
    /// produces lands here first so a subsystem can react before the
    /// application sees it; [`Session::dispatch_events`] drains it into
    /// `events`.
    pending: VecDeque<Event>,
    last_tick: Option<Instant>,
    last_claim: ClaimState,
    /// Address held while `last_claim` was `Claimed`, so a later loss can
    /// report which address went away.
    last_claimed_address: Address,
    /// Earliest wake-up any plugin asked for on the last [`Session::tick`].
    next_deadline: Option<Instant>,
}

impl Session {
    /// Start building a session for control function `name` preferring `address`.
    #[must_use]
    pub fn builder(name: Name, preferred: Address) -> SessionBuilder {
        SessionBuilder::new(name, preferred)
    }

    /// Begin address claiming. Drive it forward with [`Self::tick`].
    pub fn start(&mut self) -> Result<()> {
        self.net.start_address_claiming()
    }

    /// Our current source address (`NULL_ADDRESS` until claim completes).
    #[must_use]
    pub fn address(&self) -> Address {
        self.net
            .internal_cf(self.cf)
            .map_or(NULL_ADDRESS, crate::net::InternalCf::address)
    }

    /// Current address-claim state.
    #[must_use]
    pub fn claim_state(&self) -> ClaimState {
        self.net
            .internal_cf(self.cf)
            .map_or(ClaimState::None, crate::net::InternalCf::claim_state)
    }

    /// Whether we have claimed an address.
    #[must_use]
    pub fn is_claimed(&self) -> bool {
        self.claim_state() == ClaimState::Claimed
    }

    // ── inputs ──

    /// Feed one received frame on `port`, stamped `now`.
    pub fn feed(&mut self, port: u8, frame: &Frame, now: Instant) {
        self.advance_time(now);
        self.net.feed(frame, port);
        self.route_inbox(now);
        self.detect_claim();
        self.dispatch_events(now);
    }

    /// Advance timers to `now` without new input.
    pub fn tick(&mut self, now: Instant) {
        self.advance_time(now);
        // Route anything the network self-dispatched (e.g. claim responses).
        self.route_inbox(now);
        // Drive plugin cadences.
        let addr = self.address();
        let name = self.local_name();
        let claimed = self.is_claimed();
        let mut sends = Vec::new();
        let mut actions = Vec::new();
        let mut deadline: Option<Instant> = None;
        for plugin in &mut self.plugins {
            let mut ctx = PluginCtx::new(
                addr,
                name,
                now,
                claimed,
                &mut sends,
                &mut self.pending,
                &mut actions,
            );
            if let Some(at) = plugin.on_tick(&mut ctx) {
                deadline = Some(deadline.map_or(at, |cur: Instant| cur.min(at)));
            }
        }
        self.next_deadline = deadline;
        self.flush(sends);
        self.apply_actions(actions);
        self.raise_address_violation_dtcs();
        self.detect_claim();
        self.dispatch_events(now);
    }

    /// Turn queued address violations into active DTCs so they appear in the
    /// next DM1, per ISO 11783-5 §4.4.4.3.
    fn raise_address_violation_dtcs(&mut self) {
        let pending: Vec<Address> = core::mem::take(&mut *self.violations.borrow_mut());
        if pending.is_empty() {
            return;
        }
        for source in pending {
            let dtc = crate::j1939::diagnostic::Dtc::address_violation(source);
            if let Some(diagnostics) = self.get_mut::<plugins::Diagnostics>() {
                diagnostics.raise(dtc);
            } else {
                // No diagnostics plugged: still surface it, so the violation is
                // not silently lost.
                self.pending
                    .push_back(Event::Diag(sys::DiagEvent::Raised(dtc)));
            }
        }
    }

    /// Earliest instant a plugged subsystem asked to be ticked again, as of the
    /// last [`Self::tick`].
    ///
    /// This is **advisory**: a host loop may sleep until this instant to avoid
    /// spinning, but ticking more often is always safe and ticking later only
    /// makes cadences late. `None` means no subsystem has pending work.
    #[must_use]
    pub fn next_deadline(&self) -> Option<Instant> {
        self.next_deadline
    }

    // ── outputs ──

    /// Next `(port, frame)` the core wants to transmit, or `None` when drained.
    pub fn poll_transmit(&mut self) -> Option<(u8, Frame)> {
        self.net.take_outbound()
    }

    /// Queue an event from outside the plugin set (the driver uses this for
    /// bus-level conditions it observes on the transport, such as bus-off).
    pub(crate) fn push_event(&mut self, event: Event) {
        self.pending.push_back(event);
    }

    /// Show every newly observed event to the plugins, then hand it to the
    /// application queue.
    ///
    /// A plugin may emit while reacting, so this runs in rounds; the cap stops
    /// two plugins echoing each other forever. Whatever is still queued when the
    /// cap is reached is delivered to the application undispatched rather than
    /// dropped — losing a safety event is worse than delivering it late.
    fn dispatch_events(&mut self, now: Instant) {
        let addr = self.address();
        let name = self.local_name();
        let claimed = self.is_claimed();
        for _ in 0..MAX_EVENT_DISPATCH_ROUNDS {
            if self.pending.is_empty() {
                break;
            }
            let round: Vec<Event> = self.pending.drain(..).collect();
            let mut sends = Vec::new();
            let mut actions = Vec::new();
            for event in round {
                for plugin in &mut self.plugins {
                    let mut ctx = PluginCtx::new(
                        addr,
                        name,
                        now,
                        claimed,
                        &mut sends,
                        &mut self.pending,
                        &mut actions,
                    );
                    plugin.on_event(&event, &mut ctx);
                }
                self.events.push_back(event);
            }
            self.flush(sends);
            self.apply_actions(actions);
        }
        while let Some(event) = self.pending.pop_front() {
            self.events.push_back(event);
        }
    }

    /// Next application event, or `None` when drained.
    pub fn poll_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }

    /// Drain just one subsystem's events, leaving the rest queued.
    ///
    /// `session.drain::<VtEvent>()` returns every queued VT event and preserves
    /// ordering of the events left behind. This is the typed per-subsystem
    /// stream — use it when you only care about one concern instead of matching
    /// the full [`Event`] enum.
    pub fn drain<E: SubsystemEvent + Clone>(&mut self) -> Vec<E> {
        let mut matched = Vec::new();
        let mut rest = VecDeque::with_capacity(self.events.len());
        while let Some(event) = self.events.pop_front() {
            if let Some(typed) = E::try_ref(&event) {
                matched.push(typed.clone());
            } else {
                rest.push_back(event);
            }
        }
        self.events = rest;
        matched
    }

    /// Raw escape hatch: queue an application message from the session's own
    /// control function. Buffers like any other send (drain via
    /// [`Self::poll_transmit`]).
    ///
    /// # Errors
    /// Propagates [`IsoNet::send`] errors (e.g. not yet claimed, invalid PGN).
    pub fn send_raw(
        &mut self,
        pgn: crate::net::Pgn,
        data: &[u8],
        dst: Address,
        priority: crate::net::Priority,
    ) -> Result<()> {
        self.net.send(pgn, data, self.cf, dst, priority)
    }

    // ── fine control: own a subsystem component ──

    /// Borrow a plugged subsystem by type, e.g. `session.get::<Diagnostics>()`.
    #[must_use]
    pub fn get<P: Plugin>(&self) -> Option<&P> {
        self.plugins
            .iter()
            .find_map(|p| p.as_any().downcast_ref::<P>())
    }

    /// Mutably borrow a plugged subsystem by type for fine control.
    pub fn get_mut<P: Plugin>(&mut self) -> Option<&mut P> {
        self.plugins
            .iter_mut()
            .find_map(|p| p.as_any_mut().downcast_mut::<P>())
    }

    // ── internals ──

    fn local_name(&self) -> Name {
        self.net
            .internal_cf(self.cf)
            .map_or_else(Name::default, crate::net::InternalCf::name)
    }

    fn advance_time(&mut self, now: Instant) {
        let Some(last) = self.last_tick else {
            // First call: kick off timers without consuming any time.
            self.net.update(0);
            self.last_tick = Some(now);
            return;
        };

        // A non-monotonic clock would otherwise saturate to a 0 ms delta and
        // stall every timer silently. Surface it and resynchronise instead.
        if now.as_micros() < last.as_micros() {
            self.pending
                .push_back(Event::Bus(BusEvent::ClockWentBackwards {
                    by_micros: last.as_micros() - now.as_micros(),
                }));
            self.last_tick = Some(now);
            return;
        }

        let elapsed = now.millis_since(last);
        if elapsed > 0 {
            self.net.update(elapsed);
            // Advance only by the milliseconds actually consumed so the
            // sub-millisecond remainder survives into the next call. Setting
            // `last_tick = now` here would discard it, and a pump faster than
            // 1 kHz would then never accumulate a whole millisecond.
            self.last_tick = Some(last.add_millis(u64::from(elapsed)));
        }
    }

    fn route_inbox(&mut self, now: Instant) {
        loop {
            let Some(msg) = self.inbox.borrow_mut().pop_front() else {
                break;
            };
            let addr = self.address();
            let name = self.local_name();
            let claimed = self.is_claimed();
            let mut sends = Vec::new();
            let mut actions = Vec::new();
            for plugin in &mut self.plugins {
                if plugin.interests().contains(&msg.pgn) {
                    let mut ctx = PluginCtx::new(
                        addr,
                        name,
                        now,
                        claimed,
                        &mut sends,
                        &mut self.pending,
                        &mut actions,
                    );
                    plugin.on_frame(&msg, &mut ctx);
                }
            }
            self.flush(sends);
            self.apply_actions(actions);
        }
    }

    fn flush(&mut self, sends: Vec<SendCmd>) {
        for cmd in sends {
            // A send refused because the CF has not claimed an address used to
            // vanish here. Safety-relevant subsystems queue commands through
            // this path, so a silent drop lets an application believe it is
            // steering while nothing reaches the bus.
            if self
                .net
                .send(cmd.pgn, &cmd.data, self.cf, cmd.dst, cmd.prio)
                .is_err()
            {
                self.pending.push_back(Event::Bus(BusEvent::SendFailed {
                    pgn: cmd.pgn,
                    dst: cmd.dst,
                }));
            }
        }
    }

    fn apply_actions(&mut self, actions: Vec<CtxAction>) {
        for action in actions {
            match action {
                CtxAction::SetName(name) => {
                    if let Some(cf) = self.net.internal_cf_mut(self.cf) {
                        cf.set_name(name);
                    }
                    let _ = self.net.start_address_claiming();
                }
                CtxAction::RestartAddressClaim => {
                    let _ = self.net.start_address_claiming();
                }
                CtxAction::SendAddressClaimResponses => {
                    let _ = self.net.send_address_claim_responses();
                }
            }
        }
    }

    fn detect_claim(&mut self) {
        let state = self.claim_state();
        if state == self.last_claim {
            return;
        }

        match state {
            ClaimState::Claimed => {
                self.pending
                    .push_back(Event::AddressClaim(ClaimEvent::Claimed {
                        address: self.address(),
                    }));
            }
            // Leaving Claimed means arbitration was lost or the CF went off
            // the bus. Either way anything queued from here on is dropped by
            // `flush`, so the application has to know.
            _ if self.last_claim == ClaimState::Claimed => {
                self.pending.push_back(Event::AddressClaim(match state {
                    ClaimState::None => ClaimEvent::Disconnected,
                    _ => ClaimEvent::Lost {
                        previous_address: self.last_claimed_address,
                    },
                }));
            }
            _ => {}
        }

        if state == ClaimState::Claimed {
            self.last_claimed_address = self.address();
        }
        self.last_claim = state;
    }
}

/// Builder for a [`Session`]. Compose subsystems with [`SessionBuilder::plug`].
pub struct SessionBuilder {
    name: Name,
    preferred: Address,
    plugins: Vec<Box<dyn Plugin>>,
    config: NetworkConfig,
}

impl SessionBuilder {
    fn new(name: Name, preferred: Address) -> Self {
        Self {
            name,
            preferred,
            plugins: Vec::new(),
            config: NetworkConfig::default(),
        }
    }

    /// Override the network configuration (ports, timeouts, …).
    #[must_use]
    pub fn network_config(mut self, config: NetworkConfig) -> Self {
        self.config = config;
        self
    }

    /// Add a subsystem plugin. Two plugins of the same type cause
    /// [`SessionBuilder::build`] to fail (one instance per type, by design).
    #[must_use]
    pub fn plug<P: Plugin>(mut self, plugin: P) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Add a curated group of plugins (a persona/role preset).
    #[must_use]
    pub fn plug_group(mut self, group: impl IntoIterator<Item = Box<dyn Plugin>>) -> Self {
        self.plugins.extend(group);
        self
    }

    /// Finalize the session.
    ///
    /// # Errors
    /// Returns an error if two plugins share the same type, or if the internal
    /// control function cannot be created.
    pub fn build(self) -> Result<Session> {
        let mut seen: Vec<TypeId> = Vec::new();
        for plugin in &self.plugins {
            let tid = plugin.as_any().type_id();
            if seen.contains(&tid) {
                return Err(Error::invalid_state(
                    "duplicate plugin type: one instance per plugin type is allowed",
                ));
            }
            seen.push(tid);
        }

        // Two authors of one command PGN from a single source address means a
        // safe stop commanded by one is overwritten by the other. Rejecting the
        // combination is the only way to make that unassemblable; checking only
        // for duplicate types let `Guidance` and `AutoDrive` both drive 0xAD00.
        let mut claimed: Vec<(Pgn, &'static str)> = Vec::new();
        for plugin in &self.plugins {
            for &pgn in plugin.transmits() {
                if let Some((_, other)) = claimed.iter().find(|(p, _)| *p == pgn) {
                    return Err(Error::invalid_state(alloc::format!(
                        "plugins '{}' and '{}' both transmit PGN 0x{pgn:04X}: \
                         one command PGN may have only one author",
                        other,
                        plugin.name(),
                    )));
                }
                claimed.push((pgn, plugin.name()));
            }
        }

        let mut net = IsoNet::<NullLink>::new(self.config);
        net.set_capture_outbound(true);
        let cf = net.create_internal(self.name, 0, self.preferred)?;

        // Register any multi-frame (Fast Packet) PGNs plugins consume, so the
        // network layer reassembles them before dispatch.
        for plugin in &self.plugins {
            for &pgn in plugin.fast_packet_pgns() {
                net.register_fast_packet_pgn(pgn)?;
            }
        }

        let inbox: Rc<RefCell<VecDeque<Message>>> = Rc::new(RefCell::new(VecDeque::new()));
        {
            let q = inbox.clone();
            net.on_message
                .subscribe(move |m| q.borrow_mut().push_back(m.clone()));
        }

        // ISO 11783-5 §4.4.4.3 requires an address violation to activate a DTC
        // (SPN 2000 + SA, FMI 31). The network layer detected violations and
        // emitted an event that nothing consumed, and `Dtc::address_violation`
        // had no callers at all.
        let violations: Rc<RefCell<Vec<Address>>> = Rc::new(RefCell::new(Vec::new()));
        {
            let q = violations.clone();
            net.on_address_violation
                .subscribe(move |sa| q.borrow_mut().push(*sa));
        }

        Ok(Session {
            net,
            cf,
            plugins: self.plugins,
            inbox,
            violations,
            events: VecDeque::new(),
            pending: VecDeque::new(),
            last_tick: None,
            last_claim: ClaimState::None,
            last_claimed_address: crate::net::NULL_ADDRESS,
            next_deadline: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::plugins::Diagnostics;
    use super::*;
    use crate::j1939::diagnostic::{Dtc, Fmi};
    use crate::net::pgn_defs::PGN_DM1;

    fn test_name(identity: u32) -> Name {
        Name::default()
            .with_identity_number(identity)
            .with_function_code(0x80)
            .with_self_configurable(true)
    }

    /// F0.3 — `flush` used to swallow the send error, so an application could
    /// command steering before the address claim completed and observe nothing
    /// at all: no error, no event, and no frame on the bus.
    #[test]
    fn send_refused_before_claim_is_reported() {
        use super::plugins::Guidance;
        use crate::net::pgn_defs::PGN_GUIDANCE_SYSTEM_CMD;

        let mut session = Session::builder(test_name(23), 0x80)
            .plug(Guidance::new())
            .build()
            .unwrap();
        // Deliberately not claimed: no `start()`, no ticks to completion.
        session
            .get_mut::<Guidance>()
            .unwrap()
            .command_curvature(20.0);
        session.tick(Instant::from_millis(10));

        assert!(
            !session.is_claimed(),
            "precondition: the CF has not claimed an address"
        );
        // The controller must not queue a frame it knows cannot reach the bus.
        // It used to queue one, have it refused, and still clear `dirty` and
        // advance the cadence — so the first real command was delayed by up to
        // MAX_TX_INTERVAL_MS after the claim finally completed.
        let refused = std::iter::from_fn(|| session.poll_event()).any(|e| {
            matches!(
                e,
                Event::Bus(BusEvent::SendFailed { pgn, .. }) if pgn == PGN_GUIDANCE_SYSTEM_CMD
            )
        });
        assert!(
            !refused,
            "a guidance command must not be queued before the claim completes"
        );

        // The raw escape hatch has no such gate, and must refuse rather than
        // silently drop: this is the path an application drives directly.
        assert!(
            session
                .send_raw(
                    PGN_GUIDANCE_SYSTEM_CMD,
                    &[0u8; 8],
                    crate::net::BROADCAST_ADDRESS,
                    crate::net::Priority::Normal,
                )
                .is_err(),
            "a raw send without an address must report the refusal to its caller"
        );
    }

    /// P1.8 — the fix-quality signal was decoded and reached no consumer, so a
    /// receiver reporting no usable fix left the autonomy path free to steer.
    /// Here nothing has yet reported a method, which is exactly the state a
    /// machine must not drive in.
    #[test]
    fn an_unusable_gnss_fix_stops_the_autonomy_path() {
        use super::plugins::{AutoDrive, Gnss};
        use crate::geo::Wgs;
        use crate::net::pgn_defs::PGN_GNSS_POSITION_RAPID;
        use crate::net::{BROADCAST_ADDRESS, Frame, Identifier, Priority};
        use crate::nmea::{GNSSPosition, NMEAConfig, NMEAInterface};
        use crate::session::sys::GnssEvent;

        let mut session = Session::builder(test_name(29), 0x80)
            .plug(AutoDrive::new())
            .plug(Gnss::new(NMEAConfig::default().with_all(true)))
            .build()
            .unwrap();
        session.start().unwrap();
        claim(&mut session);

        let pos = GNSSPosition {
            wgs: Wgs::new(52.0, 5.0, 0.0),
            ..Default::default()
        };
        let frame = Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_GNSS_POSITION_RAPID,
                0x1C,
                BROADCAST_ADDRESS,
            ),
            {
                let mut d = [0xFFu8; 8];
                d.copy_from_slice(&NMEAInterface::build_position(&pos));
                d
            },
            8,
        );

        let mut now = Instant::from_millis(10_000);
        session.feed(0, &frame, now);
        while session.poll_event().is_some() {}
        assert!(
            session
                .get::<Gnss>()
                .is_some_and(|g| !g.is_position_stale()),
            "precondition: a fresh position is not stale"
        );

        // No 129029 has reported a method, so the cached fix is `NoFix`.
        assert!(
            session.get::<Gnss>().is_some_and(Gnss::is_fix_degraded),
            "a position with no reported method is not steerable"
        );
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::FixDegraded),
            "fix quality must reach the autonomy path, not just the event queue"
        );

        // And the receiver going quiet is reported in its own right.
        let mut saw_stale = false;
        for _ in 0..40 {
            now = now.add_millis(100);
            session.tick(now);
            while let Some(ev) = session.poll_event() {
                if matches!(ev, Event::Gnss(GnssEvent::PositionStale { .. })) {
                    saw_stale = true;
                }
            }
        }
        assert!(saw_stale, "a silent receiver must be reported as stale");
        assert!(session.get::<Gnss>().is_some_and(Gnss::is_position_stale));
    }

    /// J2/J3 — the fix *quality* signal only exists in PGN 129029, which
    /// broadcasts at 1 Hz. PGN 129025 arrives at 10 Hz with coordinates and
    /// nothing else, and the decoder carries the previous quality forward, so
    /// a stale `RTKFixed` was re-asserted ten times a second and kept feeding
    /// the position watchdog. DD209 integrity was decoded and then dropped
    /// entirely, so RTK Fixed + Caution looked exactly like a healthy fix.
    #[test]
    fn stale_or_flagged_gnss_quality_stops_the_autonomy_path() {
        use super::plugins::{AutoDrive, Gnss};
        use crate::geo::Wgs;
        use crate::net::fast_packet::FastPacketProtocol;
        use crate::net::pgn_defs::{PGN_GNSS_POSITION_DATA, PGN_GNSS_POSITION_RAPID};
        use crate::net::{BROADCAST_ADDRESS, Frame, Identifier, Priority};
        use crate::nmea::{GNSSPosition, NMEAConfig, NMEAInterface};

        // 129029 is a 43-byte fast packet, so it reaches the plugin as a
        // reassembled message built from a run of single frames.
        fn detail_frames(integrity_byte: u8) -> Vec<Frame> {
            let mut fp = FastPacketProtocol::new();
            fp.send(PGN_GNSS_POSITION_DATA, &detail_frame(integrity_byte), 0x1C)
                .expect("a 43-byte fast packet encodes")
        }

        fn detail_frame(integrity_byte: u8) -> Vec<u8> {
            let mut detail = vec![0xFFu8; 43];
            let lat_raw = (52.0_f64 * 1e16) as i64;
            let lon_raw = (5.0_f64 * 1e16) as i64;
            detail[7..15].copy_from_slice(&lat_raw.to_le_bytes());
            detail[15..23].copy_from_slice(&lon_raw.to_le_bytes());
            detail[23..31].copy_from_slice(&0i64.to_le_bytes());
            detail[31] = 0x40; // RTK Fixed
            detail[32] = integrity_byte;
            detail[33] = 12;
            detail[34..36].copy_from_slice(&100u16.to_le_bytes());
            detail[36..38].copy_from_slice(&150u16.to_le_bytes());
            detail[42] = 0;
            detail
        }

        fn rapid_frame() -> Frame {
            let pos = GNSSPosition {
                wgs: Wgs::new(52.0, 5.0, 0.0),
                ..Default::default()
            };
            Frame::new(
                Identifier::encode(
                    Priority::Default,
                    PGN_GNSS_POSITION_RAPID,
                    0x1C,
                    BROADCAST_ADDRESS,
                ),
                NMEAInterface::build_position(&pos),
                8,
            )
        }

        let build = || {
            // 129029 only reaches a plugin once fast-packet reassembly is on.
            let mut session = Session::builder(test_name(41), 0x80)
                .network_config(crate::net::NetworkConfig::default().fast_packet(true))
                .plug(AutoDrive::new())
                .plug(
                    Gnss::new(NMEAConfig::default().with_all(true)).with_fix_quality_stale_ms(3000),
                )
                .build()
                .unwrap();
            session.start().unwrap();
            session
        };

        // A healthy RTK Fixed with integrity Safe is steerable.
        let mut session = build();
        claim(&mut session);
        let mut now = Instant::from_millis(10_000);
        for frame in detail_frames(0xFD) {
            session.feed(0, &frame, now);
        }
        while session.poll_event().is_some() {}
        let healthy = session
            .get::<Gnss>()
            .and_then(super::plugins::Gnss::latest_position)
            .expect("the detailed fix reaches the plugin");
        assert_eq!(healthy.fix_type, crate::nmea::GNSSFixType::RTKFixed);
        assert_eq!(healthy.integrity, crate::nmea::GNSSIntegrity::Safe);
        assert!(
            session.get::<Gnss>().is_some_and(|g| !g.is_fix_degraded()),
            "RTK Fixed reported Safe must be steerable"
        );

        // The same fix reported Caution must not be.
        let mut flagged = build();
        claim(&mut flagged);
        for frame in detail_frames(0xFE) {
            flagged.feed(0, &frame, now);
        }
        while flagged.poll_event().is_some() {}
        let cautioned = flagged
            .get::<Gnss>()
            .and_then(super::plugins::Gnss::latest_position)
            .expect("the detailed fix reaches the plugin");
        assert_eq!(cautioned.integrity, crate::nmea::GNSSIntegrity::Caution);
        assert!(
            flagged.get::<Gnss>().is_some_and(Gnss::is_fix_degraded),
            "DD209 Caution must degrade the fix even at RTK Fixed"
        );
        assert_eq!(
            flagged
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::FixDegraded),
        );

        // Back to the healthy session: 129029 stops, 129025 keeps arriving.
        // The position watchdog stays fed, so only a quality watchdog catches
        // this — the whole point of the finding.
        for _ in 0..40 {
            now = now.add_millis(100);
            session.feed(0, &rapid_frame(), now);
            session.tick(now);
            while session.poll_event().is_some() {}
        }
        assert!(
            session
                .get::<Gnss>()
                .is_some_and(|g| !g.is_position_stale()),
            "129025 at 10 Hz keeps the position watchdog fed"
        );
        assert!(
            session
                .get::<Gnss>()
                .is_some_and(Gnss::is_fix_quality_stale),
            "a fix method nobody has re-confirmed for 4 s is stale"
        );
        assert!(
            session.get::<Gnss>().is_some_and(Gnss::is_fix_degraded),
            "stale quality must degrade the fix"
        );
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::FixDegraded),
            "stale fix quality must reach the autonomy path"
        );
    }

    /// A3 / G9 — the C3 guard was tested by building a `GnssHazards` on the
    /// side, poking its private field and assigning the struct into the
    /// plugin. That proves `clear_stop` reads a field; it never exercises
    /// `on_event`, so deleting `self.gnss.observe(event)` from either
    /// controller kept it green and silently reverted C3. This drives the
    /// whole path through `Session` and asserts the machine-visible outcome.
    #[test]
    fn clear_stop_is_refused_through_the_session_while_the_receiver_is_stale() {
        use super::plugins::{AutoDrive, Gnss, Guidance};
        use crate::net::fast_packet::FastPacketProtocol;
        use crate::net::pgn_defs::PGN_GNSS_POSITION_DATA;
        use crate::nmea::NMEAConfig;

        // A healthy RTK Fixed with DD209 Safe, so the fix is steerable and the
        // watchdog that fires first is the position one.
        fn healthy_fix() -> Vec<crate::net::Frame> {
            let mut detail = vec![0xFFu8; 43];
            detail[7..15].copy_from_slice(&((52.0_f64 * 1e16) as i64).to_le_bytes());
            detail[15..23].copy_from_slice(&((5.0_f64 * 1e16) as i64).to_le_bytes());
            detail[23..31].copy_from_slice(&0i64.to_le_bytes());
            detail[31] = 0x40; // RTK Fixed
            detail[32] = 0xFD; // reserved ones + DD209 Safe
            detail[33] = 12;
            detail[34..36].copy_from_slice(&100u16.to_le_bytes());
            detail[36..38].copy_from_slice(&150u16.to_le_bytes());
            detail[42] = 0;
            FastPacketProtocol::new()
                .send(PGN_GNSS_POSITION_DATA, &detail, 0x1C)
                .expect("a 43-byte fast packet encodes")
        }

        let gnss = || Gnss::new(NMEAConfig::default().with_all(true));
        let net_config = || crate::net::NetworkConfig::default().fast_packet(true);

        // AutoDrive first.
        let mut session = Session::builder(test_name(42), 0x80)
            .network_config(net_config())
            .plug(AutoDrive::new())
            .plug(gnss())
            .build()
            .unwrap();
        session.start().unwrap();
        claim(&mut session);

        let mut now = Instant::from_millis(10_000);
        for frame in healthy_fix() {
            session.feed(0, &frame, now);
        }
        while session.poll_event().is_some() {}
        assert!(
            session
                .get::<AutoDrive>()
                .is_some_and(|a| a.stop_reason().is_none()),
            "precondition: a healthy RTK Fixed is steerable"
        );

        // The cable is cut. Nothing else arrives. The position watchdog
        // (1.5 s) fires before the quality watchdog (3 s).
        for _ in 0..18 {
            now = now.add_millis(100);
            session.tick(now);
            while session.poll_event().is_some() {}
        }
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::PositionStale),
            "a receiver that stops reporting must stop the autonomy path"
        );

        // The operator presses "clear stop". The receiver is still silent, and
        // because the trigger is edge-emitted no second PositionStale is
        // coming — so clearing here would disarm the net permanently.
        session.get_mut::<AutoDrive>().unwrap().clear_stop();
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::PositionStale),
            "clear_stop must be refused while the receiver is still stale"
        );

        // And the refusal is machine-visible, not just internal state.
        now = now.add_millis(100);
        session.tick(now);
        assert!(
            !session
                .get::<AutoDrive>()
                .is_some_and(super::plugins::AutoDrive::is_engaged),
            "a refused clear must leave the controller disengaged"
        );

        // A position arriving is what actually resolves it.
        now = now.add_millis(100);
        for frame in healthy_fix() {
            session.feed(0, &frame, now);
        }
        while session.poll_event().is_some() {}
        session.get_mut::<AutoDrive>().unwrap().clear_stop();
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            None,
            "once the receiver reports again the operator can clear"
        );

        // Guidance carries the identical guard and had no test at all.
        let mut guided = Session::builder(test_name(43), 0x80)
            .network_config(net_config())
            .plug(Guidance::new())
            .plug(gnss())
            .build()
            .unwrap();
        guided.start().unwrap();
        claim(&mut guided);

        let mut now = Instant::from_millis(10_000);
        for frame in healthy_fix() {
            guided.feed(0, &frame, now);
        }
        while guided.poll_event().is_some() {}
        for _ in 0..18 {
            now = now.add_millis(100);
            guided.tick(now);
            while guided.poll_event().is_some() {}
        }
        assert_eq!(
            guided
                .get::<Guidance>()
                .and_then(super::plugins::Guidance::stop_reason),
            Some(SafeStopTrigger::PositionStale)
        );
        guided.get_mut::<Guidance>().unwrap().clear_stop();
        assert_eq!(
            guided
                .get::<Guidance>()
                .and_then(super::plugins::Guidance::stop_reason),
            Some(SafeStopTrigger::PositionStale),
            "Guidance::clear_stop must be refused on the same terms"
        );
    }

    /// B5 — round 5 filed the DD209 change as converting a previously-working
    /// receiver into a "permanent stop ... with no operator recovery". Half of
    /// that is right and half is not, and the difference matters:
    ///
    /// - Under the reserved-as-ones rule an all-0xFF integrity byte *is*
    ///   `0xFC | 3` = Unsafe. There is no distinct "not reported" encoding for
    ///   a 2-bit field whose reserved neighbours are required to be ones, so a
    ///   receiver sending 0xFF is asserting Unsafe and refusing to steer on it
    ///   is correct, not a regression. Changing the mapping would need the
    ///   DD209 table (G2), and would be wrong on this evidence.
    /// - The stop is **not** permanent. As soon as the receiver reports any
    ///   non-degraded integrity the plugin emits `FixRestored`, the hazard
    ///   clears, and the operator's `clear_stop()` is accepted.
    ///
    /// What is genuinely true is that a receiver which *always* sends 0xFF can
    /// never be autosteered — which is the intended reading of "this fix is
    /// unsafe", and is the release-note item rather than a code change.
    #[test]
    fn an_unsafe_dd209_stops_autonomy_and_a_safe_one_lets_the_operator_clear() {
        use super::plugins::{AutoDrive, Gnss};
        use crate::net::fast_packet::FastPacketProtocol;
        use crate::net::pgn_defs::PGN_GNSS_POSITION_DATA;
        use crate::nmea::{GNSSIntegrity, NMEAConfig};

        fn fix(integrity_byte: u8) -> Vec<crate::net::Frame> {
            let mut detail = vec![0xFFu8; 43];
            detail[7..15].copy_from_slice(&((52.0_f64 * 1e16) as i64).to_le_bytes());
            detail[15..23].copy_from_slice(&((5.0_f64 * 1e16) as i64).to_le_bytes());
            detail[23..31].copy_from_slice(&0i64.to_le_bytes());
            detail[31] = 0x40; // RTK Fixed
            detail[32] = integrity_byte;
            detail[33] = 12;
            detail[34..36].copy_from_slice(&100u16.to_le_bytes());
            detail[36..38].copy_from_slice(&150u16.to_le_bytes());
            detail[42] = 0;
            FastPacketProtocol::new()
                .send(PGN_GNSS_POSITION_DATA, &detail, 0x1C)
                .expect("a 43-byte fast packet encodes")
        }

        let mut session = Session::builder(test_name(44), 0x80)
            .network_config(crate::net::NetworkConfig::default().fast_packet(true))
            .plug(AutoDrive::new())
            .plug(Gnss::new(NMEAConfig::default().with_all(true)))
            .build()
            .unwrap();
        session.start().unwrap();
        claim(&mut session);

        // An all-0xFF fill decodes as Unsafe, and stops the machine.
        let now = Instant::from_millis(10_000);
        for frame in fix(0xFF) {
            session.feed(0, &frame, now);
        }
        while session.poll_event().is_some() {}
        assert_eq!(
            session
                .get::<Gnss>()
                .and_then(super::plugins::Gnss::latest_position)
                .map(|p| p.integrity),
            Some(GNSSIntegrity::Unsafe),
            "0xFF is 0xFC | 3 under the reserved-as-ones rule"
        );
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::FixDegraded)
        );

        // Clearing is refused while the receiver still says Unsafe.
        session.get_mut::<AutoDrive>().unwrap().clear_stop();
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::FixDegraded),
            "an Unsafe fix is not something the operator can dismiss"
        );

        // The receiver reports Safe. That is what makes the stop clearable —
        // the lockout is not permanent.
        let now = now.add_millis(100);
        for frame in fix(0xFD) {
            session.feed(0, &frame, now);
        }
        while session.poll_event().is_some() {}
        session.get_mut::<AutoDrive>().unwrap().clear_stop();
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            None,
            "once the receiver reports Safe the operator can clear the stop"
        );
    }

    /// B2 / G3 — the §8 heartbeat receiver used to require an all-0xFF tail,
    /// and it returned *before* `receiver_for(source)`. A peer whose padding
    /// differed was therefore never registered as a tracked peer at all: the
    /// §8.3.4 loss-of-communication window never ran for it, and when it died
    /// `SafeStopTrigger::HeartbeatError` was never produced. Fail-open on a
    /// watchdog, which is the shape this rule exists to prevent.
    #[test]
    fn a_zero_padded_heartbeat_peer_is_still_supervised() {
        use super::plugins::{AutoDrive, Heartbeat};
        use crate::j1939::HB_COMM_ERROR_TIMEOUT_MS;
        use crate::net::pgn_defs::PGN_HEARTBEAT;
        use crate::net::{BROADCAST_ADDRESS, Frame, Identifier, Priority};

        // Byte 1 is the sequence; bytes 2-8 are undefined and zero-filled here,
        // which is how plenty of stacks build a frame.
        let beat = |sequence: u8| {
            let mut data = [0x00u8; 8];
            data[0] = sequence;
            Frame::new(
                Identifier::encode(Priority::Default, PGN_HEARTBEAT, 0x33, BROADCAST_ADDRESS),
                data,
                8,
            )
        };

        let mut session = Session::builder(test_name(45), 0x80)
            .plug(AutoDrive::new())
            .plug(Heartbeat::every(100))
            .build()
            .unwrap();
        session.start().unwrap();
        claim(&mut session);

        let mut now = Instant::from_millis(10_000);
        for sequence in 1..=3u8 {
            session.feed(0, &beat(sequence), now);
            while session.poll_event().is_some() {}
            now = now.add_millis(100);
        }
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            None,
            "precondition: a healthy peer does not stop anything"
        );

        // The peer dies. Being supervised is exactly what makes that a stop.
        for _ in 0..((HB_COMM_ERROR_TIMEOUT_MS / 100) + 4) {
            now = now.add_millis(100);
            session.tick(now);
            while session.poll_event().is_some() {}
        }
        assert_eq!(
            session
                .get::<AutoDrive>()
                .and_then(super::plugins::AutoDrive::stop_reason),
            Some(SafeStopTrigger::HeartbeatError),
            "a zero-padded peer must be tracked, so its death stops the machine"
        );
    }

    /// P1.9 — `build()` rejected only duplicate types, so nothing stopped a
    /// caller plugging both controllers. Both author PGN 0xAD00 from the same
    /// source address, so a safe stop commanded by one is overwritten by the
    /// other on the next tick and the steering ECU sees intent-to-steer
    /// chatter that makes autosteer engage and drop repeatedly.
    #[test]
    fn two_authors_of_one_command_pgn_cannot_be_assembled() {
        use super::plugins::{AutoDrive, Guidance};

        let outcome = Session::builder(test_name(25), 0x80)
            .plug(Guidance::new())
            .plug(AutoDrive::new())
            .build();
        let Err(err) = outcome else {
            panic!("two authors of 0xAD00 must be refused");
        };
        let text = alloc::format!("{err}");
        assert!(
            text.contains("AD00") && text.contains("one author"),
            "the error must name the conflicting PGN, got: {text}"
        );

        // Either alone is fine.
        assert!(
            Session::builder(test_name(26), 0x80)
                .plug(Guidance::new())
                .build()
                .is_ok()
        );
        assert!(
            Session::builder(test_name(27), 0x80)
                .plug(AutoDrive::new())
                .build()
                .is_ok()
        );
    }

    /// P1.5 — the measured consequence of clearing `dirty` on a refused send was
    /// that the first Guidance System Command reached the bus 2000 ms after
    /// power-up. It must arrive as soon as the claim allows.
    #[test]
    fn first_guidance_command_follows_the_claim_promptly() {
        use super::plugins::Guidance;
        use crate::net::pgn_defs::PGN_GUIDANCE_SYSTEM_CMD;

        let mut session = Session::builder(test_name(24), 0x80)
            .plug(Guidance::new())
            .build()
            .unwrap();
        session.start().unwrap();
        session
            .get_mut::<Guidance>()
            .unwrap()
            .command_curvature(5.0);

        let mut now = Instant::ZERO;
        let mut claimed_at: Option<Instant> = None;
        let mut first_cmd_at: Option<Instant> = None;
        for _ in 0..200 {
            now = now.add_millis(10);
            session.tick(now);
            while let Some((_, frame)) = session.poll_transmit() {
                if frame.id.pgn() == PGN_GUIDANCE_SYSTEM_CMD && first_cmd_at.is_none() {
                    first_cmd_at = Some(now);
                }
            }
            if claimed_at.is_none() && session.is_claimed() {
                claimed_at = Some(now);
            }
            if first_cmd_at.is_some() {
                break;
            }
        }

        let claimed_at = claimed_at.expect("the CF should claim with no contention");
        let first = first_cmd_at.expect("a guidance command should reach the bus");
        // One guidance cadence period (Guidance::MIN_TX_INTERVAL_MS).
        let delay = first.millis_since(claimed_at);
        assert!(
            delay <= 100,
            "first command must follow the claim within one cadence, took {delay} ms"
        );
    }

    /// F0.1 — a pump faster than 1 kHz used to freeze every timer: each call
    /// saw a 0 ms delta and `last_tick` was still advanced to `now`, so the
    /// sub-millisecond remainder was discarded and never accumulated.
    #[test]
    fn sub_millisecond_pump_still_advances_timers() {
        let mut session = Session::builder(test_name(20), 0x80).build().unwrap();
        session.start().unwrap();

        // 100 µs steps: every individual step is a 0 ms delta.
        let mut now = Instant::ZERO;
        for _ in 0..20_000 {
            now = now.add_micros(100);
            session.tick(now);
            if session.is_claimed() {
                break;
            }
        }

        assert!(
            session.is_claimed(),
            "address claiming must complete under a >1 kHz pump (2 s simulated)"
        );
    }

    /// P3.1 — the round-1 residue fix landed only in `Session::advance_time`;
    /// every plugin watchdog still did `elapsed = now - last; last = now` and
    /// froze under a fast pump. A frozen heartbeat watchdog reports a dead
    /// safety-critical ECU as alive.
    #[test]
    fn plugin_watchdogs_survive_a_sub_millisecond_pump() {
        use super::plugins::Heartbeat;
        use crate::net::pgn_defs::PGN_HEARTBEAT;
        use crate::net::{BROADCAST_ADDRESS, Frame, Identifier, Priority};
        use crate::session::sys::HeartbeatEvent;

        let mut session = Session::builder(test_name(28), 0x80)
            .plug(Heartbeat::every(100))
            .build()
            .unwrap();
        session.start().unwrap();

        // Claim, then let one peer heartbeat arrive so it is tracked.
        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(100);
            session.tick(now);
            while session.poll_transmit().is_some() {}
            if session.is_claimed() {
                break;
            }
        }
        assert!(session.is_claimed());

        let hb = Frame::new(
            Identifier::encode(Priority::Default, PGN_HEARTBEAT, 0x26, BROADCAST_ADDRESS),
            [0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            8,
        );
        session.feed(0, &hb, now);
        while session.poll_event().is_some() {}

        // The peer goes silent while the host pumps at 10 kHz. Every individual
        // step is a 0 ms delta, so a truncating watchdog never times out.
        for _ in 0..10_000 {
            now = now.add_micros(100);
            session.tick(now);
            while session.poll_transmit().is_some() {}
        }

        let saw_comm_error = core::iter::from_fn(|| session.poll_event())
            .any(|e| matches!(e, Event::Heartbeat(HeartbeatEvent::CommError { .. })));
        assert!(
            saw_comm_error,
            "the §8.3.4 300 ms window must expire under a >1 kHz pump (1 s simulated)"
        );
    }

    /// F0.1 — the residue must survive across calls, not be rounded away.
    #[test]
    fn sub_millisecond_residue_accumulates_into_whole_milliseconds() {
        let mut session = Session::builder(test_name(21), 0x80).build().unwrap();
        session.start().unwrap();

        // The first tick only establishes the baseline; the ten 100 µs steps
        // after it are the one millisecond under test.
        let mut now = Instant::ZERO;
        session.tick(now);
        for _ in 0..10 {
            now = now.add_micros(100);
            session.tick(now);
        }

        assert_eq!(
            session.last_tick,
            Some(Instant::ZERO.add_millis(1)),
            "ten 100 µs steps must consume exactly 1 ms, leaving no residue"
        );
    }

    /// F0.1 — a backwards clock previously saturated to a 0 ms delta and
    /// stalled every watchdog silently.
    #[test]
    fn backwards_clock_is_reported_and_resynchronises() {
        let mut session = Session::builder(test_name(22), 0x80).build().unwrap();
        session.start().unwrap();
        session.tick(Instant::from_millis(1_000));
        while session.poll_event().is_some() {}

        session.tick(Instant::from_millis(400));

        let reported = std::iter::from_fn(|| session.poll_event()).any(|e| {
            matches!(
                e,
                Event::Bus(BusEvent::ClockWentBackwards { by_micros }) if by_micros == 600_000
            )
        });
        assert!(reported, "a backwards clock must surface as a bus event");
        assert_eq!(
            session.last_tick,
            Some(Instant::from_millis(400)),
            "timers resynchronise to the new instant rather than stalling"
        );
    }

    fn claim(session: &mut Session) {
        session.start().unwrap();
        let mut now = Instant::ZERO;
        for _ in 0..40 {
            now = now.add_millis(100);
            session.tick(now);
            if session.is_claimed() {
                break;
            }
        }
        assert!(
            session.is_claimed(),
            "session should claim with no contention"
        );
    }

    #[test]
    fn builder_rejects_duplicate_plugin_types() {
        let result = Session::builder(test_name(1), 0x80)
            .plug(Diagnostics::every(1000))
            .plug(Diagnostics::every(500))
            .build();
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.to_string().contains("duplicate plugin type"));
    }

    #[test]
    fn session_claims_address_and_emits_event() {
        let mut s = Session::builder(test_name(2), 0x80)
            .plug(Diagnostics::every(1000))
            .build()
            .unwrap();
        claim(&mut s);

        let claimed = std::iter::from_fn(|| s.poll_event())
            .any(|e| matches!(e, Event::AddressClaim(ClaimEvent::Claimed { .. })));
        assert!(claimed, "a Claimed event must be emitted");
    }

    #[test]
    fn diagnostics_plugin_broadcasts_dm1_through_session() {
        let mut s = Session::builder(test_name(3), 0x80)
            .plug(Diagnostics::every(1000))
            .build()
            .unwrap();
        claim(&mut s);
        while s.poll_transmit().is_some() {} // drain claim frames

        // Fine control: own the plugin and raise a DTC.
        s.get_mut::<Diagnostics>()
            .expect("diagnostics plugged")
            .raise(Dtc {
                spn: 1234,
                fmi: Fmi::BelowNormal,
                occurrence_count: 1,
                conversion_method: false,
            });

        // Advance past the broadcast interval; expect a DM1 frame on the wire.
        let mut now = Instant::from_millis(5_000);
        now = now.add_millis(1_100);
        s.tick(now);

        let dm1 = std::iter::from_fn(|| s.poll_transmit()).find(|(_, f)| f.pgn() == PGN_DM1);
        assert!(
            dm1.is_some(),
            "Diagnostics plugin must broadcast a DM1 frame"
        );
    }

    #[test]
    fn diagnostics_plugin_emits_event_on_peer_dm1() {
        let mut s = Session::builder(test_name(4), 0x80)
            .plug(Diagnostics::every(1000))
            .build()
            .unwrap();
        claim(&mut s);

        // Synthesize a peer DM1 broadcast and feed it in.
        let payload = encode_one_dtc_dm1(2000, Fmi::ConditionExists);
        let frame = Frame::new(
            crate::net::Identifier::encode(
                crate::net::Priority::Default,
                PGN_DM1,
                0x20,
                crate::net::BROADCAST_ADDRESS,
            ),
            payload,
            8,
        );
        s.feed(0, &frame, Instant::from_millis(6_000));

        let got = std::iter::from_fn(|| s.poll_event())
            .any(|e| matches!(e, Event::Diag(crate::session::sys::DiagEvent::Dm1Received { source, .. }) if source == 0x20));
        assert!(got, "peer DM1 must surface as DiagEvent::Dm1Received");
    }

    #[test]
    fn gnss_plugin_decodes_inbound_cog_sog() {
        use super::plugins::Gnss;
        use crate::net::pgn_defs::PGN_GNSS_COG_SOG_RAPID;
        use crate::nmea::{NMEAConfig, NMEAInterface};

        let mut s = Session::builder(test_name(5), 0x80)
            .plug(Gnss::new(NMEAConfig::default().with_all(true)))
            .build()
            .unwrap();

        // GNSS decoding does not require a claimed address.
        let payload = NMEAInterface::build_cog_sog(1.0, 5.0);
        let frame = Frame::new(
            crate::net::Identifier::encode(
                crate::net::Priority::Default,
                PGN_GNSS_COG_SOG_RAPID,
                0x20,
                crate::net::BROADCAST_ADDRESS,
            ),
            payload,
            8,
        );
        s.feed(0, &frame, Instant::from_millis(100));

        let got_cog = std::iter::from_fn(|| s.poll_event())
            .any(|e| matches!(e, Event::Gnss(crate::session::sys::GnssEvent::Cog(_))));
        assert!(got_cog, "inbound COG/SOG must surface as a GnssEvent");
    }

    #[test]
    fn gnss_plugin_broadcasts_on_command() {
        use super::plugins::Gnss;
        use crate::net::pgn_defs::PGN_GNSS_COG_SOG_RAPID;
        use crate::nmea::NMEAConfig;

        let mut s = Session::builder(test_name(6), 0x80)
            .plug(Gnss::new(NMEAConfig::default().with_all(true)))
            .build()
            .unwrap();
        claim(&mut s);
        while s.poll_transmit().is_some() {}

        s.get_mut::<Gnss>()
            .expect("gnss plugged")
            .broadcast_cog_sog(0.5, 3.0);

        let now = Instant::from_millis(7_000);
        s.tick(now);

        let sent = std::iter::from_fn(|| s.poll_transmit())
            .any(|(_, f)| f.pgn() == PGN_GNSS_COG_SOG_RAPID);
        assert!(
            sent,
            "broadcast_cog_sog must put a frame on the wire on tick"
        );
    }

    #[test]
    fn heartbeat_plugin_broadcasts_after_claim() {
        use super::plugins::Heartbeat;
        use crate::net::pgn_defs::PGN_HEARTBEAT;

        let mut s = Session::builder(test_name(7), 0x80)
            .plug(Heartbeat::every(100))
            .build()
            .unwrap();
        claim(&mut s);
        // A few more ticks past the claim so the sender fires while claimed.
        let mut now = Instant::from_millis(5_000);
        for _ in 0..4 {
            now = now.add_millis(100);
            s.tick(now);
        }
        let sent = std::iter::from_fn(|| s.poll_transmit()).any(|(_, f)| f.pgn() == PGN_HEARTBEAT);
        assert!(
            sent,
            "heartbeat plugin must broadcast PGN_HEARTBEAT once claimed"
        );
    }

    #[test]
    fn implement_plugin_decodes_hitch_command() {
        use super::plugins::Implement;
        use crate::isobus::implement::tractor_commands::{HitchCommand, HitchCommandMsg};
        use crate::net::pgn_defs::PGN_FRONT_HITCH_CMD;
        use crate::session::sys::{Hitch, ImplementEvent};

        let mut s = Session::builder(test_name(8), 0x80)
            .plug(Implement::new())
            .build()
            .unwrap();

        let cmd = HitchCommandMsg {
            command: HitchCommand::Position,
            target_position: 100,
            rate: 5,
        };
        let mut payload = [0xFFu8; 8];
        let encoded = cmd.encode();
        let n = encoded.len().min(8);
        payload[..n].copy_from_slice(&encoded[..n]);
        let frame = Frame::new(
            crate::net::Identifier::encode(
                crate::net::Priority::Default,
                PGN_FRONT_HITCH_CMD,
                0x20,
                crate::net::BROADCAST_ADDRESS,
            ),
            payload,
            8,
        );
        s.feed(0, &frame, Instant::from_millis(100));

        let got = std::iter::from_fn(|| s.poll_event()).any(|e| {
            matches!(
                e,
                Event::Imp(ImplementEvent::HitchCommand {
                    hitch: Hitch::Front,
                    ..
                })
            )
        });
        assert!(got, "front-hitch command must surface as an ImplementEvent");
    }

    #[test]
    fn all_subsystem_plugins_compose_and_claim() {
        // Compose a broad mix of ported plugins and confirm the session builds,
        // claims, and ticks without panicking — a wiring smoke test for the
        // whole plugin set.
        use super::plugins::{
            Auxiliary, ControlFunctionalities, DmMemory, Heartbeat, Implement, LanguageCommand,
            MaintainPower, NameManagement, Powertrain, ShortcutButton, Tim,
        };
        use crate::isobus::functionalities::Functionalities;
        use crate::isobus::tim::{TimAuthority, TimOptionSet};
        use crate::j1939::{LanguageData, PowerRole};

        let mut s = Session::builder(test_name(20), 0x80)
            .plug(Heartbeat::every(100))
            .plug(MaintainPower::new(PowerRole::Tecu))
            .plug(ShortcutButton::new())
            .plug(LanguageCommand::new(LanguageData::default()))
            .plug(Powertrain::new())
            .plug(DmMemory::new(None))
            .plug(Auxiliary::new())
            .plug(ControlFunctionalities::new(Functionalities::default()))
            .plug(NameManagement::new())
            .plug(Implement::new())
            .plug(Tim::new(TimAuthority::new(TimOptionSet::empty())))
            .build()
            .unwrap();
        claim(&mut s);
        assert!(s.is_claimed());
        // Fine-control access works across plugin types.
        assert!(s.get::<Heartbeat>().is_some());
        assert!(s.get::<Tim>().is_some());
        assert!(s.get::<Powertrain>().is_some());
    }

    #[test]
    fn tractor_preset_group_plugs_expected_subsystems() {
        use super::plugins::{Diagnostics, Implement, Powertrain};

        let mut s = Session::builder(test_name(30), 0xF0)
            .plug_group(super::presets::tractor())
            .build()
            .unwrap();
        claim(&mut s);
        assert!(s.is_claimed());
        assert!(s.get::<Diagnostics>().is_some());
        assert!(s.get::<Implement>().is_some());
        assert!(s.get::<Powertrain>().is_some());
    }

    #[test]
    fn typed_drain_returns_only_that_subsystems_events() {
        let mut s = Session::builder(test_name(22), 0x80)
            .plug(Diagnostics::every(1000))
            .build()
            .unwrap();
        claim(&mut s); // accumulates a Claimed event without draining
        let claims = s.drain::<ClaimEvent>();
        assert!(
            claims
                .iter()
                .any(|c| matches!(c, ClaimEvent::Claimed { .. })),
            "typed drain must return the queued ClaimEvent"
        );
        // The drained type is gone; a re-drain is empty.
        assert!(s.drain::<ClaimEvent>().is_empty());
    }

    #[test]
    fn vt_server_plugin_builds_and_ticks() {
        use super::plugins::VtServer;
        use crate::isobus::vt::VTServerConfig;

        let mut s = Session::builder(test_name(21), 0x80)
            .plug(VtServer::new(VTServerConfig::default()).unwrap())
            .build()
            .unwrap();
        claim(&mut s);
        s.get_mut::<VtServer>().unwrap().start().unwrap();
        let now = Instant::from_millis(9_000);
        s.tick(now);
        // Server should be running and emit its VT status broadcast over time.
        assert!(s.get::<VtServer>().is_some());
    }

    fn encode_one_dtc_dm1(spn: u32, fmi: Fmi) -> [u8; 8] {
        let list = crate::j1939::diagnostic::DmDtcList {
            lamps: crate::j1939::diagnostic::DiagnosticLamps::default(),
            dtcs: vec![Dtc {
                spn,
                fmi,
                occurrence_count: 1,
                conversion_method: false,
            }],
        };
        let v = list.encode();
        let mut out = [0xFFu8; 8];
        let n = v.len().min(8);
        out[..n].copy_from_slice(&v[..n]);
        out
    }

    /// 6B — ISO 11783-5 §4.4.4.3 requires an address violation to activate a
    /// DTC (SPN 2000 + SA, FMI 31). The network layer detected violations and
    /// emitted an event nothing consumed; `Dtc::address_violation` had no
    /// callers anywhere in the crate.
    #[test]
    fn address_violation_activates_a_dtc() {
        use super::plugins::Diagnostics;
        use crate::net::pgn_defs::PGN_TIME_DATE;
        use crate::net::{Frame, Identifier, Priority};

        let mut session = Session::builder(test_name(24), 0x80)
            .plug(Diagnostics::every(1000))
            .build()
            .unwrap();
        claim(&mut session);
        let our_address = session.address();

        // Another CF transmits from the address we hold.
        let intruder = Frame::new(
            Identifier::encode(
                Priority::Default,
                PGN_TIME_DATE,
                our_address,
                crate::net::BROADCAST_ADDRESS,
            ),
            [0xFF; 8],
            8,
        );
        session.feed(0, &intruder, Instant::from_millis(9_000));
        session.tick(Instant::from_millis(9_010));

        let expected = crate::j1939::diagnostic::Dtc::address_violation(our_address);
        let active = session.get::<Diagnostics>().unwrap().active();
        assert!(
            active.iter().any(|d| d.spn == expected.spn),
            "the violation must become an active DTC so it reaches the next DM1"
        );
    }
}
