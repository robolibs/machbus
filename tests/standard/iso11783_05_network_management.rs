use machbus::j1939::pgn_request;
use machbus::net::pgn_defs::{PGN_ADDRESS_CLAIMED, PGN_REQUEST};
use machbus::net::pgn_defs::{PGN_COMMANDED_ADDRESS, PGN_NAME_MANAGEMENT};
use machbus::net::{
    ADDRESS_CLAIM_RTXD_MAX_MS, AddressClaimer, BROADCAST_ADDRESS, CfState, ClaimState, Frame,
    Identifier, InternalCf, IsoNet, MAX_ADDRESS, Message, NULL_ADDRESS, Name, NameFilter,
    NameFilterField, NameManagementMsg, NameManager, NameMgmtMode, NameNackReason, NetworkConfig,
    Priority,
};
use std::cell::RefCell;
use std::rc::Rc;
use wirebit::ShmLink;
use wirebit::topology::Topology;

fn name_with_identity(id: u32, self_configurable: bool) -> Name {
    Name::default()
        .with_identity_number(id)
        .with_function_code(0x80)
        .with_self_configurable(self_configurable)
}

fn name_management_message(payload: Vec<u8>, source: u8) -> Message {
    Message::new(PGN_NAME_MANAGEMENT, payload, source)
}

fn address_claim_frame(name: Name, source: u8) -> Frame {
    Frame::new(
        Identifier::encode(
            Priority::Default,
            PGN_ADDRESS_CLAIMED,
            source,
            BROADCAST_ADDRESS,
        ),
        name.to_bytes(),
        8,
    )
}

fn two_node_net_pair() -> (IsoNet<ShmLink>, IsoNet<ShmLink>, wirebit::topology::Built) {
    let mut topology = Topology::new();
    let producer = topology.add_node("producer");
    let observer = topology.add_node("observer");
    topology.can_bus("bus0").members(&[producer, observer]);
    let mut built = topology.build().unwrap();
    let bus = built.can_bus_mut("bus0").unwrap();
    let producer_endpoint = bus.take_endpoint("producer").unwrap();
    let observer_endpoint = bus.take_endpoint("observer").unwrap();

    let mut producer_net = IsoNet::new(NetworkConfig::default());
    let mut observer_net = IsoNet::new(NetworkConfig::default());
    producer_net.set_endpoint(0, producer_endpoint);
    observer_net.set_endpoint(0, observer_endpoint);
    (producer_net, observer_net, built)
}

fn pump_net_pair(
    producer: &mut IsoNet<ShmLink>,
    observer: &mut IsoNet<ShmLink>,
    topology: &mut wirebit::topology::Built,
) {
    for _ in 0..5 {
        topology.pump_all().unwrap();
        producer.update(1);
        observer.update(1);
        topology.pump_all().unwrap();
    }
}

fn pump_net_pair_until(
    producer: &mut IsoNet<ShmLink>,
    observer: &mut IsoNet<ShmLink>,
    topology: &mut wirebit::topology::Built,
    max_ticks: usize,
    elapsed_ms: u32,
    mut done: impl FnMut(&IsoNet<ShmLink>, &IsoNet<ShmLink>) -> bool,
) -> bool {
    for _ in 0..max_ticks {
        topology.pump_all().unwrap();
        producer.update(elapsed_ms);
        observer.update(elapsed_ms);
        topology.pump_all().unwrap();
        if done(producer, observer) {
            return true;
        }
    }
    false
}

#[test]
fn network_management_name_payload_has_exact_width() {
    let name = Name::from_raw(0x1122_3344_5566_7788);
    let bytes = name.to_bytes();
    assert_eq!(bytes.len(), 8);
    assert_eq!(Name::from_bytes(&bytes), Some(name));
    assert_eq!(Name::from_bytes(&bytes[..7]), None);
}

#[test]
fn network_management_request_address_claim_uses_canonical_request_payload() {
    let request = pgn_request::encode_request(PGN_ADDRESS_CLAIMED).unwrap();
    let message = Message::new(PGN_REQUEST, request.to_vec(), 0x80);

    assert_eq!(message.pgn, PGN_REQUEST);
    assert_eq!(
        pgn_request::requested_pgn(&message),
        Some(PGN_ADDRESS_CLAIMED)
    );
}

#[test]
fn network_management_request_address_claim_response_follows_claim_state() {
    let mut cf = InternalCf::new(name_with_identity(0x701, true), 0, 0x80);
    let mut claimer = AddressClaimer::new(10);

    assert!(
        claimer.handle_request_for_claim(&mut cf).is_empty(),
        "a CF that has not attempted a claim must stay silent"
    );
    assert_eq!(cf.claim_state(), ClaimState::None);
    assert_eq!(cf.cf().state, CfState::Offline);

    let _ = claimer.start(&mut cf);
    let contest_window_reply = claimer.handle_request_for_claim(&mut cf);
    assert_eq!(contest_window_reply.len(), 1);
    assert_eq!(contest_window_reply[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(contest_window_reply[0].source(), 0x80);
    assert_eq!(contest_window_reply[0].payload(), &cf.name().to_bytes());
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);

    let _ = claimer.update(&mut cf, 300);
    assert_eq!(cf.claim_state(), ClaimState::Claimed);
    assert_eq!(cf.cf().state, CfState::Online);
    let claimed_reply = claimer.handle_request_for_claim(&mut cf);
    assert_eq!(claimed_reply.len(), 1);
    assert_eq!(claimed_reply[0].source(), 0x80);
    assert_eq!(claimed_reply[0].payload(), &cf.name().to_bytes());

    let cannot_claim = claimer.handle_duplicate_name(&mut cf);
    assert_eq!(cannot_claim.len(), 1);
    assert_eq!(cannot_claim[0].source(), NULL_ADDRESS);
    assert_eq!(cf.claim_state(), ClaimState::Failed);
    assert_eq!(cf.address(), NULL_ADDRESS);

    let failed_reply = claimer.handle_request_for_claim(&mut cf);
    assert_eq!(failed_reply.len(), 1);
    assert_eq!(failed_reply[0].source(), NULL_ADDRESS);
    assert_eq!(failed_reply[0].payload(), &cf.name().to_bytes());
}

#[test]
fn network_management_address_claim_contest_is_name_ordered_and_state_safe() {
    let local_name = name_with_identity(0x010, true);
    let mut cf = InternalCf::new(local_name, 0, 0x80);
    let mut claimer = AddressClaimer::new(0);
    let lost_events = Rc::new(RefCell::new(0usize));
    let lost_log = lost_events.clone();
    cf.on_address_lost
        .subscribe(move |_| *lost_log.borrow_mut() += 1);

    let _ = claimer.start(&mut cf);
    let unrelated = claimer.handle_claim(&mut cf, 0x42, name_with_identity(0x001, true));
    assert!(
        unrelated.is_empty(),
        "claims for unrelated addresses must not mutate local claim state"
    );
    assert_eq!(cf.address(), 0x80);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(*lost_events.borrow(), 0);

    let worse_name = name_with_identity(0x999, true);
    let defend = claimer.handle_claim(&mut cf, 0x80, worse_name);
    assert_eq!(defend.len(), 1);
    assert_eq!(defend[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(defend[0].source(), 0x80);
    assert_eq!(defend[0].payload(), &local_name.to_bytes());
    assert_eq!(cf.address(), 0x80);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(*lost_events.borrow(), 0);

    let better_name = name_with_identity(0x001, true);
    let reclaim = claimer.handle_claim(&mut cf, 0x80, better_name);
    assert_eq!(reclaim.len(), 1);
    assert_eq!(reclaim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(reclaim[0].source(), 0x81);
    assert_eq!(reclaim[0].payload(), &local_name.to_bytes());
    assert_eq!(cf.address(), 0x81);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(*lost_events.borrow(), 1);
}

#[test]
fn network_management_address_claim_loser_reclaims_after_delay_without_going_online_early() {
    let mut cf = InternalCf::new(name_with_identity(0x999, true), 0, 0x80);
    let mut claimer = AddressClaimer::new(50);

    let start_frames = claimer.start(&mut cf);
    assert_eq!(start_frames.len(), 2);
    assert_eq!(start_frames[0].pgn(), PGN_REQUEST);
    assert_eq!(start_frames[1].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);

    let winner = name_with_identity(0x100, true);
    let immediate = claimer.handle_claim(&mut cf, 0x80, winner);
    assert!(
        immediate.is_empty(),
        "RTxD-delayed losers must not immediately emit another claim"
    );
    assert_eq!(cf.cf().state, CfState::Offline);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(cf.address(), 0x80);

    assert!(
        claimer.update(&mut cf, 49).is_empty(),
        "reclaim must remain silent until the configured delay elapses"
    );
    assert_eq!(cf.address(), 0x80);
    assert_eq!(cf.cf().state, CfState::Offline);

    let reclaim = claimer.update(&mut cf, 1);
    assert_eq!(reclaim.len(), 1);
    assert_eq!(reclaim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(reclaim[0].source(), 0x81);
    assert_eq!(cf.address(), 0x81);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);

    let _ = claimer.update(&mut cf, 300);
    assert_eq!(cf.claim_state(), ClaimState::Claimed);
    assert_eq!(cf.cf().state, CfState::Online);
}

#[test]
fn network_management_delayed_reclaim_does_not_answer_with_yielded_address() {
    let mut cf = InternalCf::new(name_with_identity(0x991, true), 0, 0x80);
    let mut claimer = AddressClaimer::new(25);

    let _ = claimer.start(&mut cf);
    let winner = name_with_identity(0x100, true);
    assert!(
        claimer.handle_claim(&mut cf, 0x80, winner).is_empty(),
        "losing to a higher-priority NAME with RTxD must queue a delayed reclaim"
    );
    assert_eq!(cf.cf().state, CfState::Offline);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(
        claimer.handle_request_for_claim(&mut cf),
        Vec::<Frame>::new(),
        "a CF that yielded its contested address must not answer requests with that yielded source while waiting for RTxD"
    );

    assert!(
        claimer.update(&mut cf, 24).is_empty(),
        "the delayed reclaim must remain quiet before RTxD expires"
    );
    assert!(
        claimer.handle_request_for_claim(&mut cf).is_empty(),
        "requests during the quiet RTxD interval must not re-advertise the yielded address"
    );

    let reclaim = claimer.update(&mut cf, 1);
    assert_eq!(reclaim.len(), 1);
    assert_eq!(reclaim[0].source(), 0x81);
    assert_eq!(cf.address(), 0x81);
    let reply_after_reclaim = claimer.handle_request_for_claim(&mut cf);
    assert_eq!(reply_after_reclaim.len(), 1);
    assert_eq!(
        reply_after_reclaim[0].source(),
        0x81,
        "after the delayed reclaim is transmitted, requests may report the new claimed source"
    );
}

#[test]
fn network_management_reclaim_random_delay_is_clamped_to_standard_window() {
    let mut cf = InternalCf::new(name_with_identity(0xAA0, true), 0, 0x80);
    let mut claimer = AddressClaimer::new(ADDRESS_CLAIM_RTXD_MAX_MS + 500);

    let _ = claimer.start(&mut cf);
    let better_name = name_with_identity(0x001, true);
    assert!(
        claimer.handle_claim(&mut cf, 0x80, better_name).is_empty(),
        "a delayed reclaim must not be emitted from inside the contest handler"
    );
    assert_eq!(cf.address(), 0x80);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(cf.cf().state, CfState::Offline);

    assert!(
        claimer
            .update(&mut cf, ADDRESS_CLAIM_RTXD_MAX_MS - 1)
            .is_empty(),
        "the clamped random-delay window must remain quiet before its final millisecond"
    );
    assert_eq!(cf.address(), 0x80);

    let reclaim = claimer.update(&mut cf, 1);
    assert_eq!(reclaim.len(), 1);
    assert_eq!(reclaim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(reclaim[0].source(), 0x81);
    assert_eq!(reclaim[0].payload(), &cf.name().to_bytes());
    assert_eq!(cf.address(), 0x81);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
}

#[test]
fn network_management_pending_reclaim_rechecks_newly_occupied_address_before_transmit() {
    let mut cf = InternalCf::new(name_with_identity(0xBB0, true), 0, 0x80);
    let mut claimer = AddressClaimer::new(10);

    let _ = claimer.start(&mut cf);
    assert!(
        claimer
            .handle_claim(&mut cf, 0x80, name_with_identity(0x001, true))
            .is_empty(),
        "RTxD-delayed loser must queue rather than immediately transmit"
    );
    assert_eq!(cf.address(), 0x80);

    assert!(
        claimer
            .handle_claim(&mut cf, 0x81, name_with_identity(0x081, true))
            .is_empty(),
        "a peer claim for the queued next address is observed but not directly contested yet"
    );

    let reclaim = claimer.update(&mut cf, 10);
    assert_eq!(reclaim.len(), 1);
    assert_eq!(reclaim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(
        reclaim[0].source(),
        0x82,
        "delayed reclaim must skip an address learned as occupied during RTxD"
    );
    assert_eq!(cf.address(), 0x82);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(cf.cf().state, CfState::Offline);
}

#[test]
fn network_management_pending_reclaim_fails_if_all_later_addresses_become_occupied() {
    let mut cf = InternalCf::new(name_with_identity(0xCC0, true), 0, 0x80);
    let mut claimer = AddressClaimer::new(10);

    let _ = claimer.start(&mut cf);
    let _ = claimer.handle_claim(&mut cf, 0x80, name_with_identity(0x001, true));

    for address in 0..=MAX_ADDRESS {
        if address != 0x80 {
            let _ =
                claimer.handle_claim(&mut cf, address, name_with_identity(address as u32, true));
        }
    }

    let cannot_claim = claimer.update(&mut cf, 10);
    assert_eq!(cannot_claim.len(), 1);
    assert_eq!(cannot_claim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(cannot_claim[0].source(), NULL_ADDRESS);
    assert_eq!(cannot_claim[0].payload(), &cf.name().to_bytes());
    assert_eq!(cf.address(), NULL_ADDRESS);
    assert_eq!(cf.claim_state(), ClaimState::Failed);
    assert_eq!(cf.cf().state, CfState::Offline);
}

#[test]
fn network_management_cannot_claim_paths_use_null_source_and_do_not_retry() {
    let mut fixed = InternalCf::new(name_with_identity(0x999, false), 0, 0x80);
    let mut claimer = AddressClaimer::new(0);
    let _ = claimer.start(&mut fixed);

    let cannot_claim = claimer.handle_claim(&mut fixed, 0x80, name_with_identity(0x100, false));
    assert_eq!(cannot_claim.len(), 1);
    assert_eq!(cannot_claim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(cannot_claim[0].source(), NULL_ADDRESS);
    assert_eq!(cannot_claim[0].payload(), &fixed.name().to_bytes());
    assert_eq!(fixed.claim_state(), ClaimState::Failed);
    assert_eq!(fixed.address(), NULL_ADDRESS);
    assert!(
        claimer.update(&mut fixed, 1_000).is_empty(),
        "failed non-self-configurable CFs must not retry by timer"
    );
    assert!(
        claimer
            .handle_claim(&mut fixed, 0x80, name_with_identity(0x101, false))
            .is_empty(),
        "failed CFs must not emit repeated cannot-claim frames for later claims"
    );
    assert_eq!(fixed.claim_state(), ClaimState::Failed);
    assert_eq!(fixed.address(), NULL_ADDRESS);

    let duplicate_name = name_with_identity(0x444, true);
    let mut duplicate = InternalCf::new(duplicate_name, 0, 0x90);
    let mut duplicate_claimer = AddressClaimer::new(25);
    let _ = duplicate_claimer.start(&mut duplicate);
    let cannot_resolve = duplicate_claimer.handle_duplicate_name(&mut duplicate);
    assert_eq!(cannot_resolve.len(), 1);
    assert_eq!(cannot_resolve[0].source(), NULL_ADDRESS);
    assert_eq!(duplicate.claim_state(), ClaimState::Failed);
    assert!(
        duplicate_claimer
            .handle_duplicate_name(&mut duplicate)
            .is_empty(),
        "duplicate NAME failure must emit a single cannot-claim frame"
    );
}

#[test]
fn network_management_duplicate_name_cancels_pending_reclaim_without_retry() {
    let local_name = name_with_identity(0x445, true);
    let mut cf = InternalCf::new(local_name, 0, 0x80);
    let mut claimer = AddressClaimer::new(25);

    let _ = claimer.start(&mut cf);
    assert!(
        claimer
            .handle_claim(&mut cf, 0x80, name_with_identity(0x100, true))
            .is_empty(),
        "losing arbitration with RTxD must queue a delayed reclaim"
    );
    assert_eq!(cf.address(), 0x80);
    assert_eq!(cf.claim_state(), ClaimState::WaitForContest);
    assert_eq!(cf.cf().state, CfState::Offline);

    let cannot_claim = claimer.handle_duplicate_name(&mut cf);
    assert_eq!(cannot_claim.len(), 1);
    assert_eq!(cannot_claim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(cannot_claim[0].source(), NULL_ADDRESS);
    assert_eq!(cannot_claim[0].payload(), &local_name.to_bytes());
    assert_eq!(cf.address(), NULL_ADDRESS);
    assert_eq!(cf.claim_state(), ClaimState::Failed);
    assert_eq!(cf.cf().state, CfState::Offline);

    assert!(
        claimer.update(&mut cf, 25).is_empty(),
        "duplicate NAME Cannot Claim must cancel any delayed reclaim"
    );
    assert_eq!(cf.address(), NULL_ADDRESS);
    assert_eq!(cf.claim_state(), ClaimState::Failed);
    assert!(
        claimer.handle_duplicate_name(&mut cf).is_empty(),
        "duplicate NAME failure must not emit repeated Cannot Claim frames"
    );
}

#[test]
fn network_management_rejects_unclaimable_local_preferred_addresses_before_claiming() {
    for preferred in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let local_name = name_with_identity(0x880 + u32::from(preferred), true);
        let mut cf = InternalCf::new(local_name, 0, preferred);
        let mut claimer = AddressClaimer::new(50);

        let frames = claimer.start(&mut cf);
        assert_eq!(
            frames.len(),
            1,
            "an unclaimable local preferred source address must not send a request+claim pair"
        );
        assert_eq!(frames[0].pgn(), PGN_ADDRESS_CLAIMED);
        assert_eq!(
            frames[0].source(),
            NULL_ADDRESS,
            "invalid local preferred addresses must degrade to Cannot Claim"
        );
        assert_eq!(frames[0].payload(), &local_name.to_bytes());
        assert_eq!(cf.claim_state(), ClaimState::Failed);
        assert_eq!(cf.cf().state, CfState::Offline);
        assert_eq!(cf.address(), NULL_ADDRESS);
        assert!(
            claimer.update(&mut cf, 1_000).is_empty(),
            "invalid preferred-address startup must not retry by timer"
        );

        let request_reply = claimer.handle_request_for_claim(&mut cf);
        assert_eq!(request_reply.len(), 1);
        assert_eq!(request_reply[0].source(), NULL_ADDRESS);
        assert_eq!(request_reply[0].payload(), &local_name.to_bytes());
    }
}

#[test]
fn network_management_saturated_self_configurable_loser_does_not_reuse_observed_addresses() {
    let mut cf = InternalCf::new(name_with_identity(0xFFFF, true), 0, 0x80);
    let mut claimer = AddressClaimer::new(0);
    let _ = claimer.start(&mut cf);

    for addr in 0..=MAX_ADDRESS {
        if addr != 0x80 {
            let _ = claimer.handle_claim(&mut cf, addr, name_with_identity(addr as u32, true));
        }
    }

    let frames = claimer.handle_claim(&mut cf, 0x80, name_with_identity(1, true));
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].source(), NULL_ADDRESS);
    assert_eq!(cf.claim_state(), ClaimState::Failed);
    assert_eq!(cf.address(), NULL_ADDRESS);
}

#[test]
fn network_management_restart_claiming_after_claimed_reuses_same_name_and_address() {
    let (mut local, mut observer, mut topology) = two_node_net_pair();
    let local_name = name_with_identity(0xD01, true);
    let local_cf = local.create_internal(local_name, 0, 0x80).unwrap();

    local.start_address_claiming().unwrap();
    assert!(
        pump_net_pair_until(&mut local, &mut observer, &mut topology, 50, 10, |a, _| {
            a.internal_cf(local_cf).unwrap().claim_state() == ClaimState::Claimed
        }),
        "initial address claim did not settle"
    );
    assert_eq!(local.internal_cf(local_cf).unwrap().address(), 0x80);
    assert_eq!(local.internal_cf(local_cf).unwrap().name(), local_name);
    assert!(local.internal_cf(local_cf).unwrap().cf().is_online());

    local.start_address_claiming().unwrap();
    let restarted = local.internal_cf(local_cf).unwrap();
    assert_eq!(restarted.claim_state(), ClaimState::WaitForContest);
    assert_eq!(
        restarted.address(),
        0x80,
        "restarting address claim must not temporarily fall back to NULL or a different source"
    );
    assert_eq!(restarted.name(), local_name);

    assert!(
        pump_net_pair_until(&mut local, &mut observer, &mut topology, 50, 10, |a, _| {
            a.internal_cf(local_cf).unwrap().claim_state() == ClaimState::Claimed
        }),
        "restarted address claim did not settle"
    );
    let final_cf = local.internal_cf(local_cf).unwrap();
    assert_eq!(final_cf.address(), 0x80);
    assert_eq!(final_cf.name(), local_name);
    assert!(final_cf.cf().is_online());
}

#[test]
fn network_management_claimed_cf_loses_to_later_lower_name_and_reclaims_next_address() {
    let (mut contender, mut local, mut topology) = two_node_net_pair();
    let local_name = name_with_identity(0xD20, true);
    let local_cf = local.create_internal(local_name, 0, 0x80).unwrap();

    local.start_address_claiming().unwrap();
    assert!(
        pump_net_pair_until(&mut contender, &mut local, &mut topology, 50, 10, |_, b| {
            b.internal_cf(local_cf).unwrap().claim_state() == ClaimState::Claimed
        }),
        "local CF did not claim its preferred address"
    );
    assert_eq!(local.internal_cf(local_cf).unwrap().address(), 0x80);
    assert!(local.internal_cf(local_cf).unwrap().cf().is_online());

    contender
        .send_frame(
            &address_claim_frame(name_with_identity(0x001, true), 0x80),
            0,
        )
        .unwrap();
    assert!(
        pump_net_pair_until(&mut contender, &mut local, &mut topology, 10, 1, |_, b| {
            b.internal_cf(local_cf).unwrap().address() == 0x81
        }),
        "a claimed CF that loses to a lower NAME must immediately reclaim a different address when RTxD is zero"
    );
    let reclaiming = local.internal_cf(local_cf).unwrap();
    assert_eq!(reclaiming.claim_state(), ClaimState::WaitForContest);
    assert_eq!(reclaiming.address(), 0x81);
    assert_eq!(reclaiming.name(), local_name);
    assert!(
        !reclaiming.cf().is_online(),
        "the reclaimed address must not be considered online until its contest window elapses"
    );

    assert!(
        pump_net_pair_until(&mut contender, &mut local, &mut topology, 50, 10, |_, b| {
            b.internal_cf(local_cf).unwrap().claim_state() == ClaimState::Claimed
        }),
        "reclaimed address did not settle after the contest window"
    );
    let claimed = local.internal_cf(local_cf).unwrap();
    assert_eq!(claimed.address(), 0x81);
    assert_eq!(claimed.name(), local_name);
    assert!(claimed.cf().is_online());
}

#[test]
fn network_management_name_management_public_decoders_reject_noncanonical_bytes() {
    let valid_modes = [
        NameMgmtMode::SetPending,
        NameMgmtMode::RequestPendingResponse,
        NameMgmtMode::RequestCurrentResponse,
        NameMgmtMode::Acknowledge,
        NameMgmtMode::NegativeAcknowledge,
        NameMgmtMode::RequestPending,
        NameMgmtMode::RequestCurrent,
        NameMgmtMode::AdoptPending,
        NameMgmtMode::RequestAddressClaim,
    ];
    for mode in valid_modes {
        assert_eq!(NameMgmtMode::try_from_u8(mode.as_u8()), Some(mode));
        assert_eq!(NameMgmtMode::from_u8(mode.as_u8()), Some(mode));
    }
    for raw in [9, 10, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(NameMgmtMode::try_from_u8(raw), None);
    }

    let valid_reasons = [
        NameNackReason::Security,
        NameNackReason::InvalidItems,
        NameNackReason::Conflict,
        NameNackReason::Checksum,
        NameNackReason::PendingNotSet,
        NameNackReason::Other,
    ];
    for reason in valid_reasons {
        assert_eq!(NameNackReason::try_from_u8(reason.as_u8()), Some(reason));
        assert_eq!(NameNackReason::from_u8(reason.as_u8()), Some(reason));
    }
    for raw in [6, 7, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(NameNackReason::try_from_u8(raw), None);
    }
}

#[test]
fn network_management_name_management_payloads_are_canonical_and_do_not_loop_responses() {
    let current = name_with_identity(0x321, true);
    let valid = NameManagementMsg::for_name(NameMgmtMode::RequestCurrent, current).encode();
    assert!(NameManagementMsg::decode(&valid).is_some());
    assert!(NameManagementMsg::decode(&valid[..16]).is_none());

    let mut bad_padding = valid.clone();
    bad_padding[9] = 0;
    assert!(
        NameManagementMsg::decode(&bad_padding).is_none(),
        "non-NACK messages must keep the reserved tail canonical"
    );

    let mut bad_nack =
        NameManagementMsg::for_name(NameMgmtMode::NegativeAcknowledge, current).encode();
    bad_nack[9] = 0xFE;
    assert!(
        NameManagementMsg::decode(&bad_nack).is_none(),
        "unknown NACK reasons must not decode"
    );

    let mut manager = NameManager::new();
    for response_mode in [
        NameMgmtMode::Acknowledge,
        NameMgmtMode::NegativeAcknowledge,
        NameMgmtMode::RequestCurrentResponse,
        NameMgmtMode::RequestPendingResponse,
        NameMgmtMode::RequestAddressClaim,
    ] {
        let mut msg = NameManagementMsg::for_name(response_mode, current);
        if response_mode == NameMgmtMode::NegativeAcknowledge {
            msg.nack_reason = NameNackReason::Other;
        }
        assert!(
            manager
                .handle_name_management(&name_management_message(msg.encode(), 0x42), current)
                .is_none(),
            "response-like NAME-management modes must not create reply loops"
        );
    }
}

#[test]
fn network_management_malformed_name_management_payloads_do_not_fire_events() {
    let current = name_with_identity(0x324, true);
    let pending = current.with_function_code(0x84);
    let mut manager = NameManager::new();
    let events: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let event_log = events.clone();
    manager
        .on_name_management
        .subscribe(move |(_, source)| event_log.borrow_mut().push(*source));

    let mut bad_mode = NameManagementMsg::for_name(NameMgmtMode::SetPending, pending).encode();
    bad_mode[0] = 0x09;
    assert!(
        manager
            .handle_name_management(&name_management_message(bad_mode, 0x42), current)
            .is_none(),
        "unknown NAME-management modes must be rejected before dispatch"
    );

    let mut bad_non_nack_tail =
        NameManagementMsg::for_name(NameMgmtMode::SetPending, pending).encode();
    bad_non_nack_tail[9] = 0x00;
    assert!(
        manager
            .handle_name_management(&name_management_message(bad_non_nack_tail, 0x42), current)
            .is_none(),
        "non-NACK NAME-management payloads must keep the reserved tail canonical"
    );

    let mut bad_nack_reason =
        NameManagementMsg::for_name(NameMgmtMode::NegativeAcknowledge, current).encode();
    bad_nack_reason[9] = 0x7F;
    assert!(
        manager
            .handle_name_management(&name_management_message(bad_nack_reason, 0x42), current)
            .is_none(),
        "reserved NACK reasons must not be surfaced as valid events"
    );

    assert!(
        events.borrow().is_empty(),
        "malformed NAME-management payloads must not emit observer events"
    );
    assert!(
        !manager.has_pending(),
        "malformed NAME-management payloads must not mutate pending NAME state"
    );

    let reply = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::RequestCurrent, current).encode(),
                0x42,
            ),
            current,
        )
        .expect("a canonical RequestCurrent payload should still dispatch");
    assert_eq!(reply.msg.mode, NameMgmtMode::RequestCurrentResponse);
    assert_eq!(*events.borrow(), vec![0x42]);
}

#[test]
fn network_management_request_address_claim_is_targeted_and_not_a_reply_loop() {
    let current = name_with_identity(0x322, true);
    let wrong_target = name_with_identity(0x323, true);
    let mut manager = NameManager::new();
    let requests: Rc<RefCell<Vec<Name>>> = Rc::new(RefCell::new(Vec::new()));
    let request_log = requests.clone();
    manager
        .on_request_address_claim
        .subscribe(move |name| request_log.borrow_mut().push(*name));

    let wrong = manager.handle_name_management(
        &name_management_message(
            NameManagementMsg::for_name(NameMgmtMode::RequestAddressClaim, wrong_target).encode(),
            0x42,
        ),
        current,
    );
    assert!(
        wrong.is_none(),
        "RequestAddressClaim must not generate a NAME-management reply"
    );
    assert!(
        requests.borrow().is_empty(),
        "wrong-target RequestAddressClaim must not trigger local address claiming"
    );

    let targeted = manager.handle_name_management(
        &name_management_message(
            NameManagementMsg::for_name(NameMgmtMode::RequestAddressClaim, current).encode(),
            0x42,
        ),
        current,
    );
    assert!(
        targeted.is_none(),
        "targeted RequestAddressClaim is a claim trigger, not a PGN 0x9300 response"
    );
    assert_eq!(*requests.borrow(), vec![current]);
    assert!(
        !manager.has_pending(),
        "RequestAddressClaim must not change pending NAME state"
    );
}

#[test]
fn network_management_set_pending_adopt_cycle_preserves_identity_and_reports_failures() {
    let current = name_with_identity(0x456, true);
    let pending = current.with_function_code(0x81);
    let mut manager = NameManager::new();

    let reply = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::SetPending, pending).encode(),
                0x42,
            ),
            current,
        )
        .expect("SetPending should acknowledge a compatible pending NAME");
    assert_eq!(reply.destination, 0x42);
    assert_eq!(reply.msg.mode, NameMgmtMode::Acknowledge);
    assert_eq!(manager.pending_name(), Some(pending));

    let pending_reply = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::RequestPending, Name::default()).encode(),
                0x42,
            ),
            current,
        )
        .expect("RequestPending should return the pending NAME");
    assert_eq!(pending_reply.msg.mode, NameMgmtMode::RequestPendingResponse);
    assert_eq!(pending_reply.msg.name(), pending);

    let adopt_reply = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::AdoptPending, Name::default()).encode(),
                0x42,
            ),
            current,
        )
        .expect("AdoptPending should acknowledge when a pending NAME exists");
    assert_eq!(adopt_reply.msg.mode, NameMgmtMode::Acknowledge);
    assert!(!manager.has_pending());

    let bad_identity = name_with_identity(0x999, true);
    let bad_reply = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::SetPending, bad_identity).encode(),
                0x42,
            ),
            current,
        )
        .expect("invalid SetPending should produce a NACK");
    assert_eq!(bad_reply.msg.mode, NameMgmtMode::NegativeAcknowledge);
    assert_eq!(bad_reply.msg.nack_reason, NameNackReason::InvalidItems);
    assert!(!manager.has_pending());
}

#[test]
fn network_management_pending_name_queries_and_adoption_are_state_safe() {
    let current = name_with_identity(0x457, true);
    let pending = current.with_function_code(0x85);
    let mut manager = NameManager::new();
    let adopted_events: Rc<RefCell<Vec<Name>>> = Rc::new(RefCell::new(Vec::new()));
    let adopted_log = adopted_events.clone();
    manager
        .on_name_changed
        .subscribe(move |name| adopted_log.borrow_mut().push(*name));

    let missing_pending = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::RequestPending, Name::default()).encode(),
                0x42,
            ),
            current,
        )
        .expect("RequestPending without a pending NAME should return a NACK");
    assert_eq!(missing_pending.destination, 0x42);
    assert_eq!(missing_pending.msg.mode, NameMgmtMode::NegativeAcknowledge);
    assert_eq!(
        missing_pending.msg.nack_reason,
        NameNackReason::PendingNotSet
    );
    assert!(!manager.has_pending());
    assert!(adopted_events.borrow().is_empty());

    let missing_adopt = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::AdoptPending, Name::default()).encode(),
                0x42,
            ),
            current,
        )
        .expect("AdoptPending without a pending NAME should return a NACK");
    assert_eq!(missing_adopt.msg.mode, NameMgmtMode::NegativeAcknowledge);
    assert_eq!(missing_adopt.msg.nack_reason, NameNackReason::PendingNotSet);
    assert!(!manager.has_pending());
    assert!(adopted_events.borrow().is_empty());

    manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::SetPending, pending).encode(),
                0x42,
            ),
            current,
        )
        .expect("SetPending should acknowledge a compatible NAME");
    assert_eq!(manager.pending_name(), Some(pending));

    let pending_reply = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::RequestPending, Name::default()).encode(),
                0x42,
            ),
            current,
        )
        .expect("RequestPending should report but not consume the pending NAME");
    assert_eq!(pending_reply.msg.mode, NameMgmtMode::RequestPendingResponse);
    assert_eq!(pending_reply.msg.name(), pending);
    assert_eq!(manager.pending_name(), Some(pending));
    assert!(adopted_events.borrow().is_empty());

    let adopt_reply = manager
        .handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::AdoptPending, Name::default()).encode(),
                0x42,
            ),
            current,
        )
        .expect("AdoptPending should acknowledge and consume an existing pending NAME");
    assert_eq!(adopt_reply.msg.mode, NameMgmtMode::Acknowledge);
    assert!(!manager.has_pending());
    assert_eq!(*adopted_events.borrow(), vec![pending]);
}

#[test]
fn network_management_ignores_invalid_sources_before_name_or_commanded_address_mutation() {
    let current = name_with_identity(0x556, true);

    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let mut manager = NameManager::new();
        let pending = current.with_function_code(0x82);
        let reply = manager.handle_name_management(
            &name_management_message(
                NameManagementMsg::for_name(NameMgmtMode::SetPending, pending).encode(),
                source,
            ),
            current,
        );
        assert!(
            reply.is_none(),
            "NAME-management frames from unusable source addresses must not be answered"
        );
        assert!(
            !manager.has_pending(),
            "invalid-source SetPending must not mutate pending NAME state"
        );

        let mut commanded = current.to_bytes().to_vec();
        commanded.push(0x44);
        assert_eq!(
            manager.handle_commanded_address(
                &Message::new(PGN_COMMANDED_ADDRESS, commanded, source),
                current,
            ),
            None,
            "invalid-source Commanded Address must not be accepted"
        );
    }

    let mut manager = NameManager::new();
    let pending = current.with_function_code(0x82);
    let null_destination = Message::with_addressing(
        PGN_NAME_MANAGEMENT,
        NameManagementMsg::for_name(NameMgmtMode::SetPending, pending).encode(),
        0x42,
        NULL_ADDRESS,
        Priority::Default,
    );
    assert!(
        manager
            .handle_name_management(&null_destination, current)
            .is_none(),
        "null-destination NAME-management metadata must not be answered"
    );
    assert!(!manager.has_pending());

    let mut commanded = current.to_bytes().to_vec();
    commanded.push(0x44);
    let commanded_null_destination = Message::with_addressing(
        PGN_COMMANDED_ADDRESS,
        commanded,
        0x42,
        NULL_ADDRESS,
        Priority::Default,
    );
    assert_eq!(
        manager.handle_commanded_address(&commanded_null_destination, current),
        None,
        "null-destination Commanded Address metadata must not be accepted"
    );
}

#[test]
fn network_management_address_claim_cannot_claim_does_not_create_online_partner() {
    let (mut producer, mut observer, mut topology) = two_node_net_pair();
    let partner_name = name_with_identity(0x558, true);
    let partner = observer
        .create_partner(
            0,
            vec![NameFilter::new(NameFilterField::IdentityNumber, 0x558)],
        )
        .unwrap();

    producer
        .send_frame(&address_claim_frame(partner_name, 0x80), 0)
        .unwrap();
    pump_net_pair(&mut producer, &mut observer, &mut topology);
    let partner_cf = observer.partner_cf(partner).unwrap();
    assert_eq!(partner_cf.address(), 0x80);
    assert!(partner_cf.cf().is_online());

    producer
        .send_frame(&address_claim_frame(partner_name, NULL_ADDRESS), 0)
        .unwrap();
    pump_net_pair(&mut producer, &mut observer, &mut topology);
    let partner_cf = observer.partner_cf(partner).unwrap();
    assert_eq!(
        partner_cf.address(),
        NULL_ADDRESS,
        "Cannot Claim Address must not be learned as an online partner address"
    );
    assert!(
        !partner_cf.cf().is_online(),
        "Cannot Claim Address reports absence from the claimable address space"
    );

    producer
        .send_frame(&address_claim_frame(partner_name, BROADCAST_ADDRESS), 0)
        .unwrap();
    pump_net_pair(&mut producer, &mut observer, &mut topology);
    let partner_cf = observer.partner_cf(partner).unwrap();
    assert_eq!(
        partner_cf.address(),
        NULL_ADDRESS,
        "broadcast-source Address Claimed frames must not resurrect partner state"
    );
    assert!(!partner_cf.cf().is_online());
}

#[test]
fn network_management_rejects_wrong_pgn_envelopes_before_mutation_or_events() {
    let current = name_with_identity(0x557, true);
    let pending = current.with_function_code(0x83);
    let mut manager = NameManager::new();

    let name_events: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let name_event_log = name_events.clone();
    manager
        .on_name_management
        .subscribe(move |(_, source)| name_event_log.borrow_mut().push(*source));

    let wrong_name_pgn = Message::new(
        PGN_REQUEST,
        NameManagementMsg::for_name(NameMgmtMode::SetPending, pending).encode(),
        0x42,
    );
    assert!(
        manager
            .handle_name_management(&wrong_name_pgn, current)
            .is_none(),
        "NAME-management payloads must not be accepted under a different PGN"
    );
    assert!(!manager.has_pending());
    assert!(name_events.borrow().is_empty());

    let commanded_events: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let commanded_event_log = commanded_events.clone();
    manager
        .on_commanded_address
        .subscribe(move |address| commanded_event_log.borrow_mut().push(*address));

    let mut commanded = current.to_bytes().to_vec();
    commanded.push(0x44);
    assert_eq!(
        manager.handle_commanded_address(&Message::new(PGN_REQUEST, commanded, 0x42), current),
        None,
        "Commanded Address payloads must not be accepted under a different PGN"
    );
    assert!(commanded_events.borrow().is_empty());
}

#[test]
fn network_management_commanded_address_accepts_only_exact_target_and_claimable_address() {
    let current = name_with_identity(0x555, true);
    let mut manager = NameManager::new();
    let mut payload = current.to_bytes().to_vec();
    payload.push(0x44);

    assert_eq!(
        manager.handle_commanded_address(
            &Message::new(PGN_COMMANDED_ADDRESS, payload.clone(), 0x42),
            current,
        ),
        Some(0x44)
    );

    let mut wrong_target = name_with_identity(0x556, true).to_bytes().to_vec();
    wrong_target.push(0x44);
    assert_eq!(
        manager.handle_commanded_address(
            &Message::new(PGN_COMMANDED_ADDRESS, wrong_target, 0x42),
            current,
        ),
        None
    );

    let mut invalid_address = current.to_bytes().to_vec();
    invalid_address.push(NULL_ADDRESS);
    assert_eq!(
        manager.handle_commanded_address(
            &Message::new(PGN_COMMANDED_ADDRESS, invalid_address, 0x42),
            current,
        ),
        None
    );

    let mut overlong = current.to_bytes().to_vec();
    overlong.extend([0x44, 0xFF]);
    assert_eq!(
        manager.handle_commanded_address(
            &Message::new(PGN_COMMANDED_ADDRESS, overlong, BROADCAST_ADDRESS),
            current,
        ),
        None
    );
}

#[test]
fn commanded_address_to_occupied_address_is_refused_with_current_claim() {
    // ISO 11783-5: a Commanded Address the target cannot take (the address is
    // held by another CF) must not move the target; it keeps its current
    // address and re-announces that claim.
    let mut topo = Topology::new();
    let internal = topo.add_node("internal");
    let external = topo.add_node("external");
    topo.can_bus("bus0").members(&[internal, external]);
    let mut built = topo.build().unwrap();
    let bus = built.can_bus_mut("bus0").unwrap();
    let internal_ep = bus.take_endpoint("internal").unwrap();
    let external_ep = bus.take_endpoint("external").unwrap();

    let internal_name = Name::default()
        .with_self_configurable(true)
        .with_function_code(0x80)
        .with_identity_number(0x343)
        .with_manufacturer_code(69);
    let external_name = Name::default()
        .with_self_configurable(true)
        .with_function_code(0x81)
        .with_identity_number(0xF8)
        .with_manufacturer_code(69);

    let mut internal_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    let mut external_net: IsoNet<ShmLink> = IsoNet::new(NetworkConfig::default());
    internal_net.set_endpoint(0, internal_ep);
    external_net.set_endpoint(0, external_ep);

    let internal_handle = internal_net
        .create_internal(internal_name, 0, 0x43)
        .unwrap();
    let external_handle = external_net
        .create_internal(external_name, 0, 0x80)
        .unwrap();
    // internal_net observes the external CF so it knows 0x80 is occupied.
    let internal_observes_external = internal_net
        .create_partner(
            0,
            vec![NameFilter::new(
                NameFilterField::IdentityNumber,
                external_name.identity_number(),
            )],
        )
        .unwrap();

    internal_net.start_address_claiming().unwrap();
    external_net.start_address_claiming().unwrap();
    for _ in 0..30 {
        internal_net.update(50);
        external_net.update(50);
        built.pump_all().unwrap();
        internal_net.update(0);
        external_net.update(0);
        built.pump_all().unwrap();
        let internal_claimed = internal_net
            .internal_cf(internal_handle)
            .unwrap()
            .claim_state()
            == ClaimState::Claimed;
        let observed = internal_net
            .partner_cf(internal_observes_external)
            .map(|p| p.address())
            == Some(0x80);
        if internal_claimed && observed {
            break;
        }
    }
    assert_eq!(
        internal_net.internal_cf(internal_handle).unwrap().address(),
        0x43
    );
    assert_eq!(
        internal_net
            .partner_cf(internal_observes_external)
            .unwrap()
            .address(),
        0x80,
        "internal must have observed the external CF occupying 0x80"
    );

    // Command the internal CF to 0x80 — already held by the external CF.
    let mut commanded = internal_name.to_bytes().to_vec();
    commanded.push(0x80);
    external_net
        .send(
            PGN_COMMANDED_ADDRESS,
            &commanded,
            external_handle,
            BROADCAST_ADDRESS,
            Priority::Lowest,
        )
        .unwrap();
    for _ in 0..30 {
        internal_net.update(50);
        external_net.update(50);
        built.pump_all().unwrap();
        internal_net.update(0);
        external_net.update(0);
        built.pump_all().unwrap();
    }

    // The internal CF must have kept its original address (not moved to 0x80).
    let internal_cf = internal_net.internal_cf(internal_handle).unwrap();
    assert_eq!(
        internal_cf.address(),
        0x43,
        "commanded address to an occupied address must be refused"
    );
    assert_eq!(internal_cf.claim_state(), ClaimState::Claimed);
    assert!(internal_cf.cf().is_online());
}
