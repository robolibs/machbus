//! Two-node `session`-facade harness for the standard conformance suite.
//!
//! The facade-level conformance checks build two [`Session`] nodes on one
//! virtual `wirebit` CAN bus, drive them through their [`Driver`]s, and pump
//! frames across the bus between polls. This replaces the old `Stack`-based
//! two-node harness; the per-standard files keep their pure-codec checks and use
//! this module for the integration round-trips.

#![allow(dead_code)]

use machbus::Instant;
use machbus::net::{Address, Name, Result};
use machbus::session::{Controls, Driver, EndpointTransport, Event, Plugin, Session};
use wirebit::ShmLink;
use wirebit::topology::{Built, Topology};

type Drv = Driver<EndpointTransport<ShmLink>>;

/// Build a self-configurable NAME with the given identity and function code.
pub fn make_name(identity: u32, function: u8) -> Name {
    Name::default()
        .with_identity_number(identity)
        .with_function_code(function)
        .with_self_configurable(true)
}

/// Two `session` nodes sharing one virtual bus.
pub struct TwoNode {
    built: Built,
    pub a: Controls,
    da: Drv,
    pub ea: Vec<Event>,
    pub b: Controls,
    db: Drv,
    pub eb: Vec<Event>,
    now: Instant,
}

impl TwoNode {
    /// Build two nodes, each with its plugin set, and start their claims.
    pub fn new(
        name_a: Name,
        addr_a: Address,
        plugins_a: Vec<Box<dyn Plugin>>,
        name_b: Name,
        addr_b: Address,
        plugins_b: Vec<Box<dyn Plugin>>,
    ) -> Result<Self> {
        let mut topo = Topology::new();
        let n1 = topo.add_node("a");
        let n2 = topo.add_node("b");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().expect("topology builds");
        let bus = built.can_bus_mut("bus0").expect("bus0 exists");
        let ep_a = bus.take_endpoint("a").expect("endpoint a");
        let ep_b = bus.take_endpoint("b").expect("endpoint b");

        let (a, da) = Session::builder(name_a, addr_a)
            .plug_group(plugins_a)
            .spawn(EndpointTransport::new(0, ep_a))?;
        let (b, db) = Session::builder(name_b, addr_b)
            .plug_group(plugins_b)
            .spawn(EndpointTransport::new(0, ep_b))?;
        a.start()?;
        b.start()?;
        Ok(Self {
            built,
            a,
            da,
            ea: Vec::new(),
            b,
            db,
            eb: Vec::new(),
            now: Instant::ZERO,
        })
    }

    /// Advance the clock by `dt_ms`, poll both drivers, pump the bus, poll again.
    pub fn step(&mut self, dt_ms: u64) -> Result<()> {
        self.now = self.now.add_millis(dt_ms);
        while let Some(e) = self.da.poll_at(self.now)? {
            self.ea.push(e);
        }
        while let Some(e) = self.db.poll_at(self.now)? {
            self.eb.push(e);
        }
        self.built.pump_all().expect("pump");
        while let Some(e) = self.da.poll_at(self.now)? {
            self.ea.push(e);
        }
        while let Some(e) = self.db.poll_at(self.now)? {
            self.eb.push(e);
        }
        Ok(())
    }

    /// Run `steps` iterations of `dt_ms`.
    pub fn run(&mut self, steps: usize, dt_ms: u64) -> Result<()> {
        for _ in 0..steps {
            self.step(dt_ms)?;
        }
        Ok(())
    }

    /// Step until both nodes have claimed (bounded), returning success.
    pub fn run_until_claimed(&mut self) -> Result<bool> {
        for _ in 0..80 {
            self.step(50)?;
            if self.a.is_claimed() && self.b.is_claimed() {
                return Ok(true);
            }
        }
        Ok(self.a.is_claimed() && self.b.is_claimed())
    }

    /// All events collected on node A so far.
    pub fn events_a(&self) -> &[Event] {
        &self.ea
    }
    /// All events collected on node B so far.
    pub fn events_b(&self) -> &[Event] {
        &self.eb
    }
}

// ─────────────────────────────────────────────────────────────────────
// Facade-level conformance round-trips — one per major subsystem.
//
// Each test builds two `session` nodes (each with the relevant plugin set),
// drives the claim handshake, then runs the bus and asserts on collected
// events and/or plugin state. These replace the deleted `Stack`-based
// two-node conformance checks (iso11783_07/08/09/10/12/14, heartbeat).
// ─────────────────────────────────────────────────────────────────────

use machbus::geo::Wgs;
use machbus::isobus::implement::{HitchStatus, PtoStatus, WheelBasedSpeedDist};
use machbus::isobus::tc::{
    DDOP, DeviceElement, DeviceElementType, DeviceObject, TCClientConfig, TCServerConfig,
    TCServerState, TCState,
};
use machbus::isobus::vt::{
    DataMaskBody, ObjectPool, VTClientConfig, VTServerConfig, VTServerState, VTState, WorkingSet,
    WorkingSetBody, create_data_mask, create_working_set,
};
use machbus::j1939::diagnostic::{Dtc, Fmi};
use machbus::nmea::{GNSSPosition, NMEAConfig};
use machbus::session::plugins::{
    Diagnostics, Gnss, Guidance, Heartbeat, Implement, TcClient, TcServer, VtClient, VtServer,
};
use machbus::session::{
    DiagEvent, GnssEvent, GuidanceEvent, HeartbeatEvent, Hitch, ImplementEvent, Pto, TcServerEvent,
    VtEvent,
};

fn boxed<P: Plugin + 'static>(p: P) -> Box<dyn Plugin> {
    Box::new(p) as Box<dyn Plugin>
}

/// 1. ISO 11783-12 Diagnostics: A raises a DTC, B observes the DM1.
#[test]
fn diagnostics_dm1_crosses_to_peer() {
    let mut bus = TwoNode::new(
        make_name(0x100, 0x80),
        0x80,
        vec![boxed(Diagnostics::every(1000))],
        make_name(0x200, 0x80),
        0x81,
        vec![boxed(Diagnostics::every(1000))],
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("claim"));

    bus.a.with_mut::<Diagnostics, _>(|d| {
        d.raise(Dtc {
            spn: 1234,
            fmi: Fmi::BelowNormal,
            occurrence_count: 1,
        });
    });

    // DM1 is broadcast on the diagnostics cadence (1 s); run a few seconds.
    bus.run(8, 1000).expect("run");

    let saw_dm1 = bus.events_b().iter().any(|e| {
        matches!(
            e,
            Event::Diag(DiagEvent::Dm1Received { active, .. }) if !active.is_empty()
        )
    });
    assert!(saw_dm1, "node B should observe a DM1 with an active DTC");
}

/// 2. ISO 11783-7 Implement: A broadcasts hitch/PTO/speed status, B's
///    `Implement` plugin caches them and emits `ImplementEvent`s.
#[test]
fn implement_status_broadcast_reaches_peer() {
    let mut bus = TwoNode::new(
        make_name(0x101, 0x80),
        0x80,
        vec![boxed(Implement::new())],
        make_name(0x201, 0x80),
        0x81,
        vec![boxed(Implement::new())],
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("claim"));

    bus.a.with_mut::<Implement, _>(|imp| {
        imp.broadcast_hitch_status(
            Hitch::Front,
            HitchStatus {
                position_percent: 50,
                ..HitchStatus::default()
            },
        );
        imp.broadcast_pto_status(
            Pto::Rear,
            PtoStatus {
                shaft_speed_rpm: 540.0,
                ..PtoStatus::default()
            },
        );
        imp.broadcast_wheel_speed(WheelBasedSpeedDist {
            speed_mps: 2.0,
            distance_m: 100.0,
            ..WheelBasedSpeedDist::default()
        });
    });

    bus.run(6, 100).expect("run");

    let events = bus.events_b();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Imp(ImplementEvent::HitchStatus { .. }))),
        "peer should observe hitch status"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Imp(ImplementEvent::PtoStatus { .. }))),
        "peer should observe PTO status"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Imp(ImplementEvent::WheelSpeed(_)))),
        "peer should observe wheel speed"
    );

    // Peer cache stays current via fine control.
    let cached = bus
        .b
        .with::<Implement, _>(|imp| imp.last_wheel_speed())
        .flatten();
    assert!(
        cached.is_some_and(|w| (w.speed_mps - 2.0).abs() < 0.01),
        "peer Implement cache should hold the broadcast wheel speed"
    );
}

/// 3. NMEA / GNSS: A broadcasts a position, B sees `GnssEvent::Position`.
#[test]
fn gnss_position_broadcast_reaches_peer() {
    let mut bus = TwoNode::new(
        make_name(0x102, 0x80),
        0x80,
        vec![boxed(Gnss::new(NMEAConfig::default()))],
        make_name(0x202, 0x80),
        0x81,
        vec![boxed(Gnss::new(NMEAConfig::default()))],
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("claim"));

    let pos = GNSSPosition {
        wgs: Wgs::new(52.0, 5.0, 0.0),
        ..GNSSPosition::default()
    };
    bus.a.with_mut::<Gnss, _>(|g| g.broadcast_position(&pos));

    bus.run(6, 100).expect("run");

    assert!(
        bus.events_b()
            .iter()
            .any(|e| matches!(e, Event::Gnss(GnssEvent::Position(_)))),
        "peer should observe a GNSS position"
    );
    let cached = bus.b.with::<Gnss, _>(|g| g.latest_position()).flatten();
    assert!(
        cached.is_some(),
        "peer Gnss cache should hold a position fix"
    );
}

/// 4. ISO 11783-6 VT: A = server, B = client. The client targets A and
///    progresses past `Disconnected`; the server registers a connecting client.
#[test]
fn vt_client_connects_to_server() {
    let server_addr: Address = 0x80;
    let server = VtServer::new(VTServerConfig::default()).expect("vt server config");
    // A minimal, serializable object pool (one Working Set + one Data Mask).
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([10u16]))
        .with_object(create_data_mask(10, &DataMaskBody::default()));
    let client = VtClient::new(VTClientConfig::default(), pool, WorkingSet::default());

    let mut bus = TwoNode::new(
        make_name(0x103, 0x80),
        server_addr,
        vec![boxed(server)],
        make_name(0x203, 0x80),
        0x81,
        vec![boxed(client)],
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("claim"));

    // Start the server FSM and arm the client connect toward A's address.
    bus.a
        .with_mut::<VtServer, _>(|s| s.start())
        .expect("plugin present")
        .expect("server start");
    let target = bus.a.address();
    bus.b.with_mut::<VtClient, _>(|c| c.connect_to(target));

    bus.run(40, 100).expect("run");

    // Client left the idle state (handshake under way).
    let client_state = bus.b.with::<VtClient, _>(VtClient::state).expect("client");
    assert_ne!(
        client_state,
        VTState::Disconnected,
        "VT client should advance past Disconnected once connecting to the server"
    );
    let client_progressed = bus
        .events_b()
        .iter()
        .any(|e| matches!(e, Event::Vt(VtEvent::StateChanged(_))));
    assert!(client_progressed, "VT client should emit a state change");

    // Server moved off Disconnected (it is broadcasting status / handling the client).
    let server_state = bus.a.with::<VtServer, _>(VtServer::state).expect("server");
    assert_ne!(
        server_state,
        VTServerState::Disconnected,
        "VT server should be running after start"
    );
}

/// 5. ISO 11783-10 TC: A = server, B = client. The server registers the
///    client's version (DDOP handshake start); the client advances past idle.
#[test]
fn tc_client_handshakes_with_server() {
    let server = TcServer::new(TCServerConfig::default().with_booms(1).with_sections(1))
        .expect("tc server config");
    let ddop = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("Implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_designator("Root"),
        );
    let client = TcClient::new(TCClientConfig::default(), ddop);

    let mut bus = TwoNode::new(
        make_name(0x104, 0x80),
        0x80,
        vec![boxed(server)],
        make_name(0x204, 0x80),
        0x81,
        vec![boxed(client)],
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("claim"));

    bus.a
        .with_mut::<TcServer, _>(|s| s.server_mut().start())
        .expect("plugin present")
        .expect("server start");
    bus.b
        .with_mut::<TcClient, _>(TcClient::connect)
        .expect("plugin present")
        .expect("client connect");

    bus.run(60, 100).expect("run");

    // Client advanced off Disconnected (discovery / version exchange).
    let client_state = bus.b.with::<TcClient, _>(TcClient::state).expect("client");
    assert_ne!(
        client_state,
        TCState::Disconnected,
        "TC client should advance past Disconnected during the handshake"
    );

    // Server received the client's version (DDOP handshake reached the server).
    let saw_version = bus.events_a().iter().any(|e| {
        matches!(
            e,
            Event::TcServer(TcServerEvent::ClientVersionReceived { .. })
        )
    });
    assert!(
        saw_version,
        "TC server should observe the client's version during the handshake"
    );
    let server_state = bus
        .a
        .with_mut::<TcServer, _>(|s| s.server_mut().state())
        .expect("server");
    assert_ne!(
        server_state,
        TCServerState::Disconnected,
        "TC server should be active after start"
    );
}

/// 6. ISO 11783-7 Heartbeat: both nodes broadcast heartbeats on cadence;
///    each tracks the other and observes `HeartbeatEvent::Received`.
#[test]
fn heartbeat_round_trip_between_peers() {
    let mut bus = TwoNode::new(
        make_name(0x105, 0x80),
        0x80,
        vec![boxed(Heartbeat::every(100))],
        make_name(0x205, 0x80),
        0x81,
        vec![boxed(Heartbeat::every(100))],
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("claim"));

    let addr_a = bus.a.address();
    let addr_b = bus.b.address();
    bus.a.with_mut::<Heartbeat, _>(|h| h.track(addr_b));
    bus.b.with_mut::<Heartbeat, _>(|h| h.track(addr_a));

    bus.run(20, 100).expect("run");

    let a_sent = bus
        .events_a()
        .iter()
        .any(|e| matches!(e, Event::Heartbeat(HeartbeatEvent::Sent { .. })));
    assert!(a_sent, "node A should broadcast heartbeats");

    let b_received = bus.events_b().iter().any(|e| {
        matches!(e, Event::Heartbeat(HeartbeatEvent::Received { source, .. }) if *source == addr_a)
    });
    assert!(b_received, "node B should receive node A's heartbeats");

    let a_received = bus.events_a().iter().any(|e| {
        matches!(e, Event::Heartbeat(HeartbeatEvent::Received { source, .. }) if *source == addr_b)
    });
    assert!(a_received, "node A should receive node B's heartbeats");

    // Tracker observed a sequence for the peer.
    let seq = bus
        .b
        .with::<Heartbeat, _>(|h| h.last_sequence(addr_a))
        .flatten();
    assert!(
        seq.is_some(),
        "peer tracker should hold a heartbeat sequence"
    );
}

#[test]
fn two_nodes_claim_distinct_addresses_on_the_session_facade() {
    let mut bus = TwoNode::new(
        make_name(0x100, 0x80),
        0x80,
        Vec::new(),
        make_name(0x999, 0x80),
        0x81,
        Vec::new(),
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("drive claim"));
    assert!(bus.a.is_claimed());
    assert!(bus.b.is_claimed());
    assert_ne!(bus.a.address(), bus.b.address());
}

/// 7. ISO 11783-7 automatic guidance (autosteer): node A is the guidance
///    controller (commands curvature); node B plays the steering ECU and
///    broadcasts Agricultural Guidance Machine Info, which A decodes.
#[test]
fn guidance_machine_info_reaches_the_controller() {
    use machbus::isobus::implement::guidance::{GenericSaeBs02SlotValue, GuidanceMachineInfo};
    use machbus::net::Priority;
    use machbus::net::pgn_defs::PGN_GUIDANCE_MACHINE_INFO;

    let mut bus = TwoNode::new(
        make_name(0x110, 0x80),
        0x80,
        vec![boxed(Guidance::new())],
        make_name(0x210, 0x80),
        0x81,
        Vec::new(),
    )
    .expect("build two-node bus");
    assert!(bus.run_until_claimed().expect("claim"));

    // A commands the steering system to follow a curvature (PGN 0xAD00 goes out).
    bus.a.with_mut::<Guidance, _>(|g| g.command_curvature(2.5));

    // B acts as the steering ECU and broadcasts machine info (PGN 0xAC00).
    let info = GuidanceMachineInfo {
        estimated_curvature: 1.25,
        steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
        ..Default::default()
    };
    bus.b
        .send_raw(
            PGN_GUIDANCE_MACHINE_INFO,
            &info.encode(),
            0xFF,
            Priority::Default,
        )
        .expect("send machine info");

    bus.run(6, 50).expect("run");

    let got = bus.events_a().iter().any(|e| {
        matches!(
            e,
            Event::Guidance(GuidanceEvent::MachineInfo {
                steering_ready: true,
                ..
            })
        )
    });
    assert!(got, "controller should receive guidance machine info");
    assert!(
        bus.a
            .with::<Guidance, _>(|g| g.estimated_curvature().is_some())
            .unwrap_or(false),
        "controller should cache the steering ECU's estimated curvature"
    );
}
