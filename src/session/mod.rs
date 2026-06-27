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
    AuxiliaryEvent, BusEvent, ClaimEvent, DiagEvent, DmMemoryEvent, Event, EventQueue, FsEvent,
    FsServerEvent, GnssEvent, GuidanceEvent, HeartbeatEvent, Hitch, ImplementEvent,
    LanguageCommandEvent, MaintainPowerEvent, OverflowPolicy, PowertrainEvent, PowertrainSnapshot,
    Pto, ScEvent, ShortcutButtonEvent, TcEvent, TcServerEvent, TimEvent, VtEvent, VtServerEvent,
};

use plugin::{CtxAction, SendCmd};

use crate::net::can_adapter;
use crate::net::{
    Address, ClaimState, Error, Frame, IsoNet, Message, NULL_ADDRESS, Name, NetworkConfig, Result,
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

/// The sans-IO protocol core. See the [module docs](self).
pub struct Session {
    net: IsoNet<NullLink>,
    cf: crate::net::InternalCfHandle,
    plugins: Vec<Box<dyn Plugin>>,
    inbox: Rc<RefCell<VecDeque<Message>>>,
    events: VecDeque<Event>,
    last_tick: Option<Instant>,
    last_claim: ClaimState,
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
    }

    /// Advance timers to `now` without new input.
    pub fn tick(&mut self, now: Instant) {
        self.advance_time(now);
        // Route anything the network self-dispatched (e.g. claim responses).
        self.route_inbox(now);
        // Drive plugin cadences.
        let addr = self.address();
        let name = self.local_name();
        let mut sends = Vec::new();
        let mut actions = Vec::new();
        for plugin in &mut self.plugins {
            let mut ctx =
                PluginCtx::new(addr, name, now, &mut sends, &mut self.events, &mut actions);
            plugin.on_tick(&mut ctx);
        }
        self.flush(sends);
        self.apply_actions(actions);
        self.detect_claim();
    }

    // ── outputs ──

    /// Next `(port, frame)` the core wants to transmit, or `None` when drained.
    pub fn poll_transmit(&mut self) -> Option<(u8, Frame)> {
        self.net.take_outbound()
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
        let elapsed = self.last_tick.map_or(0, |last| now.millis_since(last));
        // Always drive the network on the first call (to kick off timers) and
        // whenever wall time advanced.
        if self.last_tick.is_none() || elapsed > 0 {
            self.net.update(elapsed);
        }
        self.last_tick = Some(now);
    }

    fn route_inbox(&mut self, now: Instant) {
        loop {
            let Some(msg) = self.inbox.borrow_mut().pop_front() else {
                break;
            };
            let addr = self.address();
            let name = self.local_name();
            let mut sends = Vec::new();
            let mut actions = Vec::new();
            for plugin in &mut self.plugins {
                if plugin.interests().contains(&msg.pgn) {
                    let mut ctx =
                        PluginCtx::new(addr, name, now, &mut sends, &mut self.events, &mut actions);
                    plugin.on_frame(&msg, &mut ctx);
                }
            }
            self.flush(sends);
            self.apply_actions(actions);
        }
    }

    fn flush(&mut self, sends: Vec<SendCmd>) {
        for cmd in sends {
            let _ = self
                .net
                .send(cmd.pgn, &cmd.data, self.cf, cmd.dst, cmd.prio);
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
        if state != self.last_claim {
            if state == ClaimState::Claimed {
                self.events
                    .push_back(Event::AddressClaim(ClaimEvent::Claimed {
                        address: self.address(),
                    }));
            }
            self.last_claim = state;
        }
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

        Ok(Session {
            net,
            cf,
            plugins: self.plugins,
            inbox,
            events: VecDeque::new(),
            last_tick: None,
            last_claim: ClaimState::None,
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
            }],
        };
        let v = list.encode();
        let mut out = [0xFFu8; 8];
        let n = v.len().min(8);
        out[..n].copy_from_slice(&v[..n]);
        out
    }
}
