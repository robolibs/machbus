//! Playground example for trying out machbus features.
//!
//! Run with `make run` (or `cargo run --example main`).
//!
//! Modify this file freely as new modules land — it's the quickest way
//! to exercise the public API without writing a full integration test.

use std::cell::RefCell;
use std::rc::Rc;

use machbus::net::{
    AddressClaimer, BROADCAST_ADDRESS, BusLoad, ClaimState, DataSpan, DegradedAction, Error,
    ErrorCode, Event, ExtendedTransportProtocol, FastPacketProtocol, Frame, FreshnessRequirement,
    Identifier, InternalCf, IsoNet, Message, NULL_ADDRESS, Name, NameFilter, NameFilterField,
    NetworkConfig, Niu, NiuConfig, NiuFilterMode, PartnerCf, Pgn, Priority, ProcessingFlags,
    SafeState, SafetyConfig, SafetyPolicy, Scheduler, Side, StateMachine, Timeout, Timer,
    TransportProtocol, TransportSession, WorkingSetManager, bitfield, pgn,
    pgn_defs::{PGN_ADDRESS_CLAIMED, PGN_DM1, PGN_HEARTBEAT, PGN_REQUEST},
};

fn main() {
    println!("=== machbus playground ===\n");

    section("phase 1 — primitives");
    println!("  PGN_REQUEST   = 0x{PGN_REQUEST:04X} ({PGN_REQUEST})");
    println!("  Priority::High = {:?}", Priority::High);
    println!("  NULL          = 0x{NULL_ADDRESS:02X}, BROADCAST = 0x{BROADCAST_ADDRESS:02X}");

    let mut buf = [0u8; 4];
    bitfield::pack_u32_le(&mut buf, 0xDEAD_BEEF);
    println!("  pack_u32_le   -> {buf:02X?}");

    let span = DataSpan::from(&[0x10u8, 0x20, 0x30, 0x40][..]);
    println!(
        "  DataSpan oob  = 0x{:08X} (defensive)",
        span.get_u32_le(99)
    );

    section("phase 2 — pgn helpers");
    println!(
        "  pgn::pdu_format(REQUEST)     = 0x{:02X}",
        pgn::pgn_pdu_format(PGN_REQUEST)
    );
    println!(
        "  pgn::is_pdu2(REQUEST)        = {}  (PDU1: dest-specific)",
        pgn::pgn_is_pdu2(PGN_REQUEST)
    );
    println!(
        "  pgn::is_pdu2(DM1)            = {}  (PDU2: broadcast)",
        pgn::pgn_is_pdu2(PGN_DM1)
    );
    println!(
        "  pgn::is_valid(0x40000)       = {}  (over 18 bits)",
        pgn::pgn_is_valid(0x40000)
    );
    if let Some(info) = pgn::pgn_lookup(PGN_HEARTBEAT) {
        println!(
            "  pgn::lookup(HEARTBEAT)       = name={:?}, len={}, prio={}, broadcast={}",
            info.name, info.data_length, info.default_priority, info.is_broadcast
        );
    }

    section("phase 2 — identifier");
    let id = Identifier::encode(Priority::Default, PGN_REQUEST, 0x80, 0x42);
    println!(
        "  encode(REQUEST, src=0x80, dst=0x42) -> raw=0x{:08X}",
        id.raw
    );
    println!(
        "  decode: prio={:?}, pgn=0x{:04X}, src=0x{:02X}, dst=0x{:02X}, broadcast={}",
        id.priority(),
        id.pgn(),
        id.source(),
        id.destination(),
        id.is_broadcast()
    );

    let id2 = Identifier::encode(Priority::Default, PGN_DM1, 0x80, 0x42);
    println!(
        "  PDU2 (DM1): destination forced to 0x{:02X} (broadcast)",
        id2.destination()
    );

    section("phase 2 — frame & wirebit interop");
    let frame = Frame::from_message(
        Priority::High,
        PGN_HEARTBEAT,
        0x80,
        BROADCAST_ADDRESS,
        &[0xDE, 0xAD, 0xBE, 0xEF],
    );
    println!("  frame.payload()    = {:02X?}", frame.payload());
    println!("  frame.length       = {}", frame.length);
    println!("  frame.priority()   = {:?}", frame.priority());

    let cf = frame.to_can_frame();
    println!(
        "  to wirebit::CanFrame: id=0x{:08X}, ext={}",
        cf.id(),
        cf.is_extended()
    );

    let restored = Frame::from_can_frame(&cf).expect("ext frame round-trips");
    println!(
        "  from wirebit::CanFrame: pgn=0x{:04X}, src=0x{:02X}",
        restored.pgn(),
        restored.source()
    );

    section("phase 2 — message");
    let mut msg = Message {
        pgn: PGN_ADDRESS_CLAIMED,
        source: 0x80,
        ..Default::default()
    };
    msg.set_u32_le(0, 0xCAFE_F00D);
    msg.set_u32_le(4, 0x1234_5678);
    println!(
        "  pgn=0x{:04X}, size={}, broadcast={}",
        msg.pgn,
        msg.size(),
        msg.is_broadcast()
    );
    println!("  data            = {:02X?}", msg.data);
    println!("  get_u32_le(0)   = 0x{:08X}", msg.get_u32_le(0));
    println!("  get_u64_le(0)   = 0x{:016X}", msg.get_u64_le(0));
    println!("  get_bits(0, 12) = 0x{:03X}", msg.get_bits(0, 12));

    section("phase 2 — name & address-claim arbitration");
    let n_low = Name::default()
        .with_identity_number(0x12345)
        .with_manufacturer_code(0x100)
        .with_function_code(0x80);
    let n_high = Name::default()
        .with_identity_number(0x99999)
        .with_manufacturer_code(0x500)
        .with_function_code(0x80);
    println!("  low.raw  = 0x{:016X}", n_low.raw);
    println!("  high.raw = 0x{:016X}", n_high.raw);
    println!(
        "  low <  high? {}  (lower NAME wins arbitration)",
        n_low < n_high
    );
    println!("  low.identity_number = 0x{:05X}", n_low.identity_number());
    println!("  low.bytes (LE)      = {:02X?}", n_low.to_bytes());

    let restored = Name::from_bytes(&n_low.to_bytes()).expect("8 bytes");
    println!("  round-trip equal?   {}", restored == n_low);

    section("phase 2 — full encode → frame → decode round-trip");
    let req_payload = [0x00u8, 0xEE, 0x00]; // request the Address Claimed PGN
    let frame = Frame::from_message(Priority::High, PGN_REQUEST, 0x80, 0x42, &req_payload);
    println!(
        "  TX  pgn=0x{:04X}, src=0x{:02X}, dst=0x{:02X}, payload={:02X?}",
        frame.pgn(),
        frame.source(),
        frame.destination(),
        frame.payload()
    );
    let cf = frame.to_can_frame();
    let rx = Frame::from_can_frame(&cf).unwrap();
    let _: Pgn = rx.pgn(); // type sanity
    println!(
        "  RX  pgn=0x{:04X}, src=0x{:02X}, dst=0x{:02X}, payload={:02X?}",
        rx.pgn(),
        rx.source(),
        rx.destination(),
        rx.payload()
    );

    section("phase 1 — error helpers");
    let _: Error = Error::invalid_pgn(0xEA00);
    let _: Error = Error::timeout();
    let _: ErrorCode = ErrorCode::TransportAborted;
    println!(
        "  Error::invalid_pgn(0xEA00) -> {}",
        Error::invalid_pgn(0xEA00)
    );
    println!("  Error::timeout()           -> {}", Error::timeout());

    section("phase 3 — event");
    let mut on_message: Event<u32> = Event::new();
    let log = Rc::new(RefCell::new(Vec::<u32>::new()));
    let l = log.clone();
    let t1 = on_message.subscribe(move |&v| l.borrow_mut().push(v));
    on_message.subscribe(|&v| println!("  handler-2 saw {v}"));
    on_message.emit(&42);
    on_message.emit(&99);
    println!("  log = {:?}", log.borrow());
    println!("  active listeners = {}", on_message.count());
    on_message.unsubscribe(t1);
    println!("  after unsub: count = {}", on_message.count());

    section("phase 3 — timer / timeout");
    let mut t = Timer::new(100, true);
    t.start();
    let mut fires = 0;
    for _ in 0..10 {
        if t.update(50) {
            fires += 1;
        }
    }
    println!("  Timer(100ms, auto) fired {fires} times in 10×50ms");

    let mut to = Timeout::new(150);
    to.start();
    let timed_out = to.update(100) || to.update(60);
    println!(
        "  Timeout(150ms): fired={timed_out}, active={}",
        to.active()
    );

    section("phase 3 — state machine");
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Phase {
        Boot,
        Claim,
        Run,
    }
    let mut sm = StateMachine::new(Phase::Boot);
    sm.on_transition
        .subscribe(|t| println!("  transition {:?} -> {:?}", t.0, t.1));
    sm.transition(Phase::Claim);
    sm.transition(Phase::Run);
    sm.transition(Phase::Run); // no-op, no event
    println!("  final state = {:?}", sm.state());

    section("phase 3 — bus load");
    let mut bl = BusLoad::new();
    for _ in 0..10 {
        bl.add_frame(8);
    }
    bl.update(100);
    println!(
        "  10×8B in 100ms → load = {:.2}% on 250 kbit/s ISOBUS",
        bl.load_percent()
    );

    section("phase 3 — scheduler");
    let mut sched = Scheduler::new();
    let count = Rc::new(RefCell::new(0u32));
    let c = count.clone();
    sched.add("heartbeat", 100, 0, move || {
        *c.borrow_mut() += 1;
        true
    });
    for _ in 0..5 {
        sched.update(100);
    }
    println!("  heartbeat task fired {} times in 5×100ms", count.borrow());

    section("phase 3 — processing flags");
    let mut pf = ProcessingFlags::new();
    let triggered = Rc::new(RefCell::new(Vec::<u8>::new()));
    for i in [0u8, 5, 10] {
        let t = triggered.clone();
        pf.register_flag(i, move || t.borrow_mut().push(i));
    }
    pf.set(0);
    pf.set(10);
    println!("  pending mask = 0x{:08X}", pf.pending());
    pf.process();
    println!("  fired flags  = {:?}", triggered.borrow());

    section("phase 3 — safety policy");
    let mut policy = SafetyPolicy::new(SafetyConfig::default());
    policy.require_freshness(
        FreshnessRequirement::new("guidance")
            .max_age_ms(100)
            .escalation_ms(500)
            .action(DegradedAction::Disable),
    );
    let states = Rc::new(RefCell::new(Vec::<(SafeState, SafeState)>::new()));
    let s = states.clone();
    policy
        .on_state_change
        .subscribe(move |t| s.borrow_mut().push(*t));

    println!("  initial state = {:?}", policy.state());
    policy.update(150); // source missing → Degraded
    println!(
        "  after 150ms (no source): {:?}, action={:?}",
        policy.state(),
        policy.current_action()
    );
    policy.report_alive("guidance");
    policy.update(10); // fresh again → Normal
    println!("  after report_alive:    {:?}", policy.state());
    println!("  state-change log: {:?}", states.borrow());

    section("phase 4 — address claim contention canary");
    fn make_name(identity: u32, self_config: bool) -> Name {
        Name::default()
            .with_identity_number(identity)
            .with_function_code(0x80)
            .with_self_configurable(self_config)
    }
    let low = make_name(0x100, true);
    let high = make_name(0x999, true);
    println!("  low.raw  = 0x{:016X}", low.raw);
    println!("  high.raw = 0x{:016X}", high.raw);

    let mut cf_low = InternalCf::new(low, 0, 0x80);
    let mut cf_high = InternalCf::new(high, 0, 0x80);
    let mut clm_low = AddressClaimer::new(0);
    let mut clm_high = AddressClaimer::new(50); // 50ms RTxD

    cf_low
        .on_address_claimed
        .subscribe(|&a| println!("  [low]  claimed 0x{a:02X}"));
    cf_high
        .on_address_claimed
        .subscribe(|&a| println!("  [high] claimed 0x{a:02X}"));
    cf_high
        .on_address_lost
        .subscribe(|_| println!("  [high] lost arbitration, will re-claim after RTxD"));

    let _ = clm_low.start(&mut cf_low);
    let _ = clm_high.start(&mut cf_high);
    println!("  both CFs started; conflict at 0x80");

    // Each sees the other's claim.
    let _ = clm_low.handle_claim(&mut cf_low, 0x80, high);
    let _ = clm_high.handle_claim(&mut cf_high, 0x80, low);

    // Tick until both finish.
    for _ in 0..6 {
        let _ = clm_low.update(&mut cf_low, 60);
        let _ = clm_high.update(&mut cf_high, 60);
    }

    println!(
        "  [low]  state={:?}  addr=0x{:02X}",
        cf_low.claim_state(),
        cf_low.address()
    );
    println!(
        "  [high] state={:?}  addr=0x{:02X}",
        cf_high.claim_state(),
        cf_high.address()
    );
    assert_eq!(cf_low.claim_state(), ClaimState::Claimed);
    assert_eq!(cf_high.claim_state(), ClaimState::Claimed);
    assert_eq!(cf_low.address(), 0x80);
    assert_eq!(cf_high.address(), 0x81);

    section("phase 4 — partner CF with NAME filter");
    let partner = PartnerCf::new(
        0,
        vec![
            NameFilter::new(NameFilterField::FunctionCode, 0x80),
            NameFilter::new(NameFilterField::IdentityNumber, 0x100),
        ],
    );
    println!("  filters: {:?}", partner.filters());
    println!(
        "  matches low  (identity=0x{:05X}): {}",
        0x100,
        partner.matches_name(&low)
    );
    println!(
        "  matches high (identity=0x{:05X}): {}",
        0x999,
        partner.matches_name(&high)
    );

    section("phase 4 — working set (100ms member spacing)");
    let mut ws = WorkingSetManager::new();
    ws.add_member(make_name(0x10, false));
    ws.add_member(make_name(0x20, false));
    let master_payload = ws.start_broadcast();
    println!(
        "  master payload (size={}): {:02X?}",
        master_payload[0], master_payload
    );
    let mut step = 0;
    while ws.is_broadcasting() {
        if let Some((m, _)) = ws.update(110) {
            step += 1;
            println!("  member {step}: identity=0x{:05X}", m.identity_number());
        }
    }

    section("phase 5 — TP CMDT round-trip (RTS / CTS / DT × N / EoMA)");
    {
        let payload: Vec<u8> = (0..40u32).map(|n| n as u8).collect();
        let mut tx = TransportProtocol::new();
        let mut rx = TransportProtocol::new();
        let received = Rc::new(RefCell::new(None::<TransportSession>));
        let r = received.clone();
        rx.on_complete
            .subscribe(move |s| *r.borrow_mut() = Some(s.clone()));

        let rts = tx
            .send(0xEF00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        println!(
            "  TX: RTS for {} bytes (≈{} packets)",
            payload.len(),
            payload.len().div_ceil(7)
        );
        let cts_resp = rx.process_frame(&rts, 0);
        let cts = cts_resp[0];
        println!("  RX: CTS num={} next_seq={}", cts.data[1], cts.data[2]);

        let _ = tx.process_frame(&cts, 0);
        let dt_frames = tx.get_pending_data_frames();
        println!("  TX: {} DT frames queued", dt_frames.len());

        let mut eoma_seen = None;
        for dt in &dt_frames {
            for resp in rx.process_frame(dt, 0) {
                if resp.data[0] == machbus::net::tp::tp_cm::EOMA {
                    eoma_seen = Some(resp);
                }
            }
        }
        let _ = tx.process_frame(&eoma_seen.unwrap(), 0);
        let got = received.borrow().clone().unwrap();
        println!(
            "  RX delivered {} bytes, source=0x{:02X}, equal? {}",
            got.data.len(),
            got.source_address,
            got.data == payload
        );
    }

    section("phase 5 — ETP round-trip (>1785 bytes, DPO + CTS windowing)");
    {
        let payload: Vec<u8> = (0..2500u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let mut rx = ExtendedTransportProtocol::new();
        let received = Rc::new(RefCell::new(None::<TransportSession>));
        let r = received.clone();
        rx.on_complete
            .subscribe(move |s| *r.borrow_mut() = Some(s.clone()));

        let rts = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        println!(
            "  TX: ETP RTS for {} bytes ({} packets, ~{} windows of 16)",
            payload.len(),
            payload.len().div_ceil(7),
            payload.len().div_ceil(7).div_ceil(16)
        );
        let mut to_tx = rx.process_frame(&rts, 0);

        let mut turns = 0;
        for turn in 0..50 {
            for f in to_tx.drain(..) {
                let _ = tx.process_frame(&f, 0);
            }
            let dt = tx.get_pending_data_frames();
            if dt.is_empty() {
                break;
            }
            for f in &dt {
                to_tx.extend(rx.process_frame(f, 0));
            }
            turns = turn + 1;
            if received.borrow().is_some() {
                break;
            }
        }
        let got = received.borrow().clone().unwrap();
        println!(
            "  Converged in {turns} turns, delivered {} bytes, equal? {}",
            got.data.len(),
            got.data == payload
        );
    }

    section("phase 5 — NMEA2000 fast packet");
    {
        let payload: Vec<u8> = (0..50u32).map(|n| n as u8).collect();
        let mut tx = FastPacketProtocol::new();
        let mut rx = FastPacketProtocol::new();
        let frames = tx.send(0xF010, &payload, 0x10).unwrap();
        println!("  TX: {} frames for {} bytes", frames.len(), payload.len());

        let mut got = None;
        for f in &frames {
            if let Some(m) = rx.process_frame(f) {
                got = Some(m);
            }
        }
        let msg = got.unwrap();
        println!(
            "  RX delivered {} bytes (equal? {})",
            msg.data.len(),
            msg.data == payload
        );
    }

    section("phase 6 — full stack: two IsoNets on a simulated bus");
    {
        use wirebit::topology::Topology;

        let mut topo = Topology::new();
        let n1 = topo.add_node("n1");
        let n2 = topo.add_node("n2");
        topo.can_bus("bus0").members(&[n1, n2]);
        let mut built = topo.build().unwrap();
        let bus = built.can_bus_mut("bus0").unwrap();
        let ep_a = bus.take_endpoint("n1").unwrap();
        let ep_b = bus.take_endpoint("n2").unwrap();

        let mut net_a: IsoNet<wirebit::ShmLink> = IsoNet::new(NetworkConfig::default());
        let mut net_b: IsoNet<wirebit::ShmLink> = IsoNet::new(NetworkConfig::default());
        net_a.set_endpoint(0, ep_a);
        net_b.set_endpoint(0, ep_b);

        let h_a = net_a
            .create_internal(
                Name::default()
                    .with_identity_number(0x100)
                    .with_function_code(0x80)
                    .with_self_configurable(true),
                0,
                0x80,
            )
            .unwrap();
        let h_b = net_b
            .create_internal(
                Name::default()
                    .with_identity_number(0x999)
                    .with_function_code(0x80)
                    .with_self_configurable(true),
                0,
                0x81,
            )
            .unwrap();

        net_a.start_address_claiming().unwrap();
        net_b.start_address_claiming().unwrap();

        // Pump the bus until both have claimed.
        let mut ticks = 0;
        for tick in 0..50 {
            net_a.update(100);
            net_b.update(100);
            built.pump_all().unwrap();
            net_a.update(0);
            net_b.update(0);
            built.pump_all().unwrap();
            if net_a.internal_cf(h_a).unwrap().claim_state() == ClaimState::Claimed
                && net_b.internal_cf(h_b).unwrap().claim_state() == ClaimState::Claimed
            {
                ticks = tick + 1;
                break;
            }
        }
        println!("  both claimed in {ticks} ticks");
        println!(
            "  net_a address = 0x{:02X} (state={:?})",
            net_a.internal_cf(h_a).unwrap().address(),
            net_a.internal_cf(h_a).unwrap().claim_state()
        );
        println!(
            "  net_b address = 0x{:02X} (state={:?})",
            net_b.internal_cf(h_b).unwrap().address(),
            net_b.internal_cf(h_b).unwrap().claim_state()
        );

        // Send a 50-byte payload from A → B (triggers TP CMDT).
        let payload: Vec<u8> = (0..50u32).map(|n| n as u8).collect();
        let received = Rc::new(RefCell::new(Vec::<Message>::new()));
        let r = received.clone();
        net_b
            .register_pgn_callback(0xEF11, move |m| r.borrow_mut().push(m.clone()))
            .unwrap();

        let dst = net_b.internal_cf(h_b).unwrap().address();
        net_a
            .send(0xEF11, &payload, h_a, dst, Priority::Lowest)
            .unwrap();
        println!("  TX: 50-byte payload via TP CMDT to 0x{dst:02X}");

        for _ in 0..50 {
            net_a.update(50);
            net_b.update(50);
            built.pump_all().unwrap();
            net_a.update(0);
            net_b.update(0);
            built.pump_all().unwrap();
            if !received.borrow().is_empty() {
                break;
            }
        }
        let msgs = received.borrow();
        assert_eq!(msgs.len(), 1);
        println!(
            "  RX: {} byte payload reassembled (equal? {})",
            msgs[0].data.len(),
            msgs[0].data == payload
        );
        println!("  bus load on net_a port 0: {:.2}%", net_a.bus_load(0));
    }

    section("phase 7 — NIU bridge: forward + filter + rate-limit");
    {
        let mut niu = Niu::new(
            NiuConfig::default()
                .name("demo-niu")
                .mode(NiuFilterMode::BlockAll), // block-all baseline
        );
        niu.set_filter_mode(NiuFilterMode::BlockAll);
        niu.allow_pgn(PGN_HEARTBEAT, true);
        niu.allow_pgn_rate_limited(PGN_DM1, 100, true);
        niu.block_pgn(PGN_REQUEST, true);
        niu.start().unwrap();

        println!(
            "  filters: {} rules, mode={:?}",
            niu.filters().len(),
            niu.filter_mode()
        );

        // Heartbeat: allowed, forwards.
        let hb = Frame::from_message(
            Priority::Default,
            PGN_HEARTBEAT,
            0x10,
            BROADCAST_ADDRESS,
            &[1; 8],
        );
        let r1 = niu.process_frame(hb, Side::Tractor, 0);
        println!("  heartbeat from tractor → {}", r1.is_some());

        // Request PGN: explicitly blocked.
        let req = Frame::from_message(Priority::Default, PGN_REQUEST, 0x10, 0x42, &[0; 3]);
        let r2 = niu.process_frame(req, Side::Tractor, 0);
        println!("  request    from tractor → {}", r2.is_some());

        // DM1 with 100ms rate limit: first passes, second within window blocks.
        let dm1 = Frame::from_message(Priority::Default, PGN_DM1, 0x10, BROADCAST_ADDRESS, &[0; 8]);
        let r3 = niu.process_frame(dm1, Side::Tractor, 50);
        let r4 = niu.process_frame(dm1, Side::Tractor, 100);
        let r5 = niu.process_frame(dm1, Side::Tractor, 200);
        println!("  DM1 t=50  → {} (first allowed)", r3.is_some());
        println!(
            "  DM1 t=100 → {} (within 100ms window, blocked)",
            r4.is_some()
        );
        println!("  DM1 t=200 → {} (window elapsed, allowed)", r5.is_some());

        println!("  forwarded={}, blocked={}", niu.forwarded(), niu.blocked());
    }

    section("phase 8 — IOP parser (byte walker)");
    {
        // Synthesize a conformant ISO 11783-6 IOP buffer (no per-object
        // length prefix) with a Container followed by a NumberVariable.
        let mut buf = Vec::new();
        buf.extend_from_slice(&[0x01, 0x00]); // id = 1
        buf.push(3); // type = Container
        buf.extend_from_slice(&[100, 0, 200, 0, 0]); // w=100, h=200, hidden=0
        buf.extend_from_slice(&[0, 0]); // num_objects=0, num_macros=0
        buf.extend_from_slice(&[0x02, 0x00]); // id = 2
        buf.push(21); // type = NumberVariable
        buf.extend_from_slice(&[0x0D, 0xF0, 0xFE, 0xCA]); // value LE = 0xCAFEF00D

        let valid = machbus::net::validate(&buf);
        let objs = machbus::net::parse_iop_data(&buf).unwrap();
        let version = machbus::net::hash_to_version(&buf);

        println!("  validate(buf)            = {valid}");
        println!("  parse_iop_data → {} objects:", objs.len());
        for o in &objs {
            println!(
                "    id=0x{:04X} type={:>2} body_len={}",
                o.id,
                o.type_byte,
                o.body.len()
            );
        }
        println!("  hash_to_version          = {version}  (FNV-1a → 7 chars in A..=P)");
    }

    section("phase 9 — j1939 messages");
    {
        use machbus::j1939::heartbeat::hb_seq;
        use machbus::j1939::{
            Dtc, Eec1, Fmi, HeartbeatSender, MaintainPowerData, MaintainPowerRequirement,
            ShortcutButtonState, TimeDate,
        };

        // Engine status round-trip.
        let eec1 = Eec1 {
            engine_torque_percent: 50.0,
            driver_demand_percent: 75.0,
            actual_engine_percent: 45.0,
            engine_speed_rpm: 1500.0,
            starter_mode: 1,
            source_address: 0x10,
        };
        let bytes = eec1.encode();
        let decoded = Eec1::decode(&bytes).unwrap();
        println!(
            "  EEC1: {} rpm round-trip OK ({:.1} rpm)",
            eec1.engine_speed_rpm, decoded.engine_speed_rpm
        );

        // DTC round-trip.
        let dtc = Dtc {
            spn: 0x1_2345,
            fmi: Fmi::AbnormalRateChange,
            occurrence_count: 7,
        };
        let dtc_bytes = dtc.encode();
        let dtc_decoded = Dtc::decode(&dtc_bytes).unwrap();
        println!(
            "  DTC: spn=0x{:05X} fmi={:?} oc={}",
            dtc_decoded.spn, dtc_decoded.fmi, dtc_decoded.occurrence_count
        );

        // Heartbeat sender ISO 11783-7 §8.3 walk.
        let mut hb = HeartbeatSender::default();
        let seq: Vec<u8> = (0..5).map(|_| hb.next_sequence()).collect();
        println!(
            "  Heartbeat sender: {:?} (INIT={}, then 0,1,2,3)",
            seq,
            hb_seq::INIT
        );

        // TimeDate round-trip.
        let td = TimeDate {
            seconds: Some(45),
            minutes: Some(30),
            hours: Some(14),
            day: Some(15),
            month: Some(6),
            year: Some(2024),
            ..Default::default()
        };
        let td_bytes = td.encode();
        let td_decoded =
            TimeDate::decode(&machbus::net::Message::new(0xFEE6, td_bytes.to_vec(), 0x10)).unwrap();
        println!(
            "  Time/Date: {}-{:02}-{:02} {:02}:{:02}:{:02}",
            td_decoded.year.unwrap(),
            td_decoded.month.unwrap(),
            td_decoded.day.unwrap(),
            td_decoded.hours.unwrap(),
            td_decoded.minutes.unwrap(),
            td_decoded.seconds.unwrap()
        );

        // Maintain Power codec.
        let mpd = MaintainPowerData {
            maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
            ..Default::default()
        };
        let _ = mpd.encode();
        println!("  MaintainPowerData: ECU_PWR={:?}", mpd.maintain_ecu_power);

        // ShortcutButton trivial encode/decode.
        let sb_payload =
            machbus::j1939::shortcut_button::encode(ShortcutButtonState::StopImplementOperations);
        println!(
            "  ShortcutButton::StopImplementOperations → counter=0x{:02X} state=0x{:02X}",
            sb_payload[6], sb_payload[7]
        );
    }

    section("done");
    println!("  Phases 1–9 functional. net/ + j1939/ ported (14 + 31 modules).");
}

fn section(name: &str) {
    println!("\n[{name}]");
}
