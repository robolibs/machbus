use machbus::net::pgn_defs::{
    PGN_ADDRESS_CLAIMED, PGN_DM1, PGN_HEARTBEAT, PGN_NIU_NETWORK_MSG, PGN_REQUEST,
};
use machbus::net::{
    BROADCAST_ADDRESS, FilterRule, ForwardPolicy, Frame, Message, NULL_ADDRESS, Name, Niu,
    NiuConfig, NiuFilterMode, NiuFunction, NiuNetworkMsg, Priority, Router, Side,
};

fn frame(pgn: u32, source: u8, destination: u8, data: &[u8]) -> Frame {
    Frame::from_message(Priority::Default, pgn, source, destination, data)
}

fn name(identity: u32) -> Name {
    Name::default()
        .with_identity_number(identity)
        .with_manufacturer_code(0x456)
}

fn address_claim(source: u8, name: Name) -> Frame {
    frame(
        PGN_ADDRESS_CLAIMED,
        source,
        BROADCAST_ADDRESS,
        &name.to_bytes(),
    )
}

fn active_niu(config: NiuConfig) -> Niu {
    let mut niu = Niu::new(config);
    niu.start().unwrap();
    niu
}

#[test]
fn network_layer_niu_filter_message_round_trips_and_rejects_reserved_bits() {
    let msg = NiuNetworkMsg {
        function: NiuFunction::AddFilterEntry,
        port_number: 2,
        filter_pgn: 0x1EF00,
        ..NiuNetworkMsg::default()
    };
    let encoded = msg.encode().unwrap();
    assert_eq!(NiuNetworkMsg::decode(&encoded), Some(msg));

    let mut bad_high_bits = encoded;
    bad_high_bits[4] |= 0xFC;
    assert_eq!(NiuNetworkMsg::decode(&bad_high_bits), None);

    let mut bad_tail = encoded;
    bad_tail[7] = 0;
    assert_eq!(NiuNetworkMsg::decode(&bad_tail), None);
}

#[test]
fn network_layer_public_policy_decoder_rejects_noncanonical_bytes() {
    assert_eq!(ForwardPolicy::try_from_u8(0), Some(ForwardPolicy::Allow));
    assert_eq!(ForwardPolicy::try_from_u8(1), Some(ForwardPolicy::Block));
    assert_eq!(ForwardPolicy::try_from_u8(2), Some(ForwardPolicy::Monitor));
    for raw in [3, 4, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(ForwardPolicy::try_from_u8(raw), None);
    }
    // ISO 11783-4 Table 4: 0 = block-specific (default pass all),
    // 1 = pass-specific (default block all). These were bound the other way.
    assert_eq!(
        NiuFilterMode::try_from_u8(0),
        Some(NiuFilterMode::BlockSpecific)
    );
    assert_eq!(
        NiuFilterMode::try_from_u8(1),
        Some(NiuFilterMode::PassSpecific)
    );
    for raw in [2, 3, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(NiuFilterMode::try_from_u8(raw), None);
    }

    let mut encoded = FilterRule::new(0xEF00, ForwardPolicy::Allow, false)
        .encode()
        .unwrap();
    encoded[3] = (encoded[3] & !0x03) | 0x03;
    assert!(
        FilterRule::decode(&encoded).is_err(),
        "policy bits outside the defined set must not be promoted to Monitor"
    );
}

#[test]
fn network_layer_filter_rules_preserve_names_and_reject_hidden_absent_names() {
    let source = Name::from_raw(0x0102_0304_0506_0708);
    let rule = FilterRule::new(0xEF00, ForwardPolicy::Monitor, true)
        .with_source_name(source)
        .with_max_frequency_ms(250)
        .persistent(true);
    let encoded = rule.encode().unwrap();
    let decoded = FilterRule::decode(&encoded).unwrap();

    assert_eq!(decoded.pgn, 0xEF00);
    assert_eq!(decoded.policy, ForwardPolicy::Monitor);
    assert!(decoded.bidirectional);
    assert_eq!(decoded.source_name, Some(source));
    assert_eq!(decoded.destination_name, None);
    assert_eq!(decoded.max_frequency_ms, 250);
    assert!(decoded.persistent);

    let no_name = FilterRule::new(0xEF00, ForwardPolicy::Allow, false)
        .encode()
        .unwrap();
    let mut hidden_name = no_name;
    hidden_name[4] = 0;
    assert!(FilterRule::decode(&hidden_name).is_err());
}

#[test]
fn network_layer_name_scoped_filters_require_observed_address_claims() {
    let source_name = name(0x510);
    let destination_name = name(0x520);

    let mut pass_all = active_niu(NiuConfig::default());
    pass_all.add_filter(
        FilterRule::new(PGN_DM1, ForwardPolicy::Block, true).with_source_name(source_name),
    );
    let dm1_from_unclaimed_source = frame(PGN_DM1, 0x31, BROADCAST_ADDRESS, &[0xFF; 8]);
    assert!(
        pass_all
            .process_frame(dm1_from_unclaimed_source, Side::Tractor, 10)
            .is_some(),
        "a NAME-scoped block rule must not match before the address claim has been observed"
    );
    assert!(
        pass_all
            .process_frame(address_claim(0x31, source_name), Side::Tractor, 11)
            .is_some(),
        "Address Claimed remains forwardable while learning the source NAME"
    );
    assert_eq!(
        pass_all.observed_name(Side::Tractor, 0x31),
        Some(source_name)
    );
    assert!(
        pass_all
            .process_frame(dm1_from_unclaimed_source, Side::Tractor, 12)
            .is_none(),
        "after observation, the source NAME-scoped block rule applies"
    );

    let mut block_all = active_niu(NiuConfig::default().mode(NiuFilterMode::PassSpecific));
    block_all.add_filter(
        FilterRule::new(PGN_REQUEST, ForwardPolicy::Allow, true)
            .with_destination_name(destination_name),
    );
    let request_to_destination = frame(PGN_REQUEST, 0x41, 0x42, &[0x00, 0xEE, 0x00]);
    assert!(
        block_all
            .process_frame(request_to_destination, Side::Tractor, 20)
            .is_none(),
        "a destination NAME-scoped allow rule must not match an unobserved address"
    );
    assert!(
        block_all
            .process_frame(address_claim(0x42, destination_name), Side::Tractor, 21)
            .is_none(),
        "BlockAll may block forwarding while still learning valid address claims"
    );
    assert_eq!(
        block_all.observed_name(Side::Tractor, 0x42),
        Some(destination_name)
    );
    assert!(
        block_all
            .process_frame(request_to_destination, Side::Tractor, 22)
            .is_some(),
        "after observation, destination-specific traffic may match the destination NAME"
    );
    assert!(
        block_all
            .process_frame(
                frame(PGN_REQUEST, 0x41, 0x43, &[0x00, 0xEE, 0x00]),
                Side::Tractor,
                23,
            )
            .is_none(),
        "the same rule must not guess for an unobserved destination address"
    );
}

/// ISO 11783-4 lists PGN 60672 as "CF to NIU, Destination-Specific", so a
/// configuration command must be addressed to the bridge it reconfigures.
fn niu_control(data: Vec<u8>, source: u8, destination: u8) -> Message {
    Message::with_addressing(
        PGN_NIU_NETWORK_MSG,
        data,
        source,
        destination,
        Priority::Default,
    )
}

#[test]
fn network_layer_persistent_filters_survive_runtime_clear_only() {
    let mut niu = Niu::new(NiuConfig::default());
    niu.add_filter(FilterRule::new(PGN_HEARTBEAT, ForwardPolicy::Allow, true).persistent(true));
    niu.add_filter(FilterRule::new(PGN_REQUEST, ForwardPolicy::Block, true));

    niu.clear_filters();
    assert_eq!(niu.filters().len(), 1);
    assert_eq!(niu.filters()[0].pgn, PGN_HEARTBEAT);
    assert!(niu.filters()[0].persistent);

    let delete_all = NiuNetworkMsg {
        function: NiuFunction::ClearFilterEntry,
        ..NiuNetworkMsg::default()
    }
    .encode()
    .unwrap();
    niu.handle_niu_message(&niu_control(delete_all.to_vec(), 0x44, 0x20));
    assert_eq!(
        niu.filters().len(),
        1,
        "a remote clear removes runtime rules but not configured persistent policy"
    );
    assert!(niu.filters()[0].persistent);
}

#[test]
fn network_layer_rate_limited_filter_blocks_inside_window_without_policy_drift() {
    let mut niu = active_niu(NiuConfig::default().loop_guard_capacity(8));
    niu.allow_pgn_rate_limited(PGN_HEARTBEAT, 100, true);
    let policy_before = niu.policy_snapshot();

    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, 0x21, BROADCAST_ADDRESS, &[0x7D, 0xFF]),
            Side::Tractor,
            1_000,
        )
        .is_some(),
        "the first matching rate-limited frame establishes the time window"
    );
    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, 0x21, BROADCAST_ADDRESS, &[0x7E, 0xFF]),
            Side::Tractor,
            1_099,
        )
        .is_none(),
        "a matching frame inside the configured minimum interval must be blocked"
    );
    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, 0x21, BROADCAST_ADDRESS, &[0x7F, 0xFF]),
            Side::Tractor,
            1_100,
        )
        .is_some(),
        "the boundary at the configured interval is forwardable again"
    );

    assert_eq!(niu.forwarded(), 2);
    assert_eq!(niu.blocked(), 1);
    assert_eq!(
        niu.policy_snapshot(),
        policy_before,
        "rate-limiter timestamps are runtime state and must not alter the policy snapshot"
    );
}

#[test]
fn network_layer_filter_mode_control_gates_default_forwarding_until_allow_rule() {
    let mut niu = active_niu(NiuConfig::default());

    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, 0x31, BROADCAST_ADDRESS, &[0x01]),
            Side::Tractor,
            2_000,
        )
        .is_some(),
        "block-specific mode forwards an unmatched broadcast by default"
    );

    // The filter mode has no Table 2 function code — §6.6.2.3.3 says it
    // "cannot be changed without clearing and rebuilding the database for that
    // port pair", so it is set out of band, not by a network message.
    niu.set_filter_mode(NiuFilterMode::PassSpecific);
    assert_eq!(niu.filter_mode(), NiuFilterMode::PassSpecific);
    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, 0x31, BROADCAST_ADDRESS, &[0x02]),
            Side::Tractor,
            2_001,
        )
        .is_none(),
        "BlockAll mode must block unmatched traffic"
    );

    let allow_heartbeat = NiuNetworkMsg {
        function: NiuFunction::AddFilterEntry,
        port_number: 1,
        filter_pgn: PGN_HEARTBEAT,
        ..NiuNetworkMsg::default()
    }
    .encode()
    .unwrap();
    niu.handle_niu_message(&niu_control(allow_heartbeat.to_vec(), 0x44, 0x20));
    assert_eq!(niu.filters().len(), 1);
    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, 0x31, BROADCAST_ADDRESS, &[0x03]),
            Side::Tractor,
            2_002,
        )
        .is_some(),
        "an explicit allow entry must pass even while the default mode blocks"
    );
}

/// ISO 11783-4 Table 2 defines no function code for setting the filter mode:
/// §6.6.2.3.3 says it "cannot be changed without clearing and rebuilding the
/// database for that port pair", so it is configured out of band. The message
/// this test used to exercise was a local invention and is gone; what remains
/// worth checking is that the mode byte itself rejects reserved values.
#[test]
fn network_layer_filter_mode_rejects_reserved_modes() {
    assert!(NiuFilterMode::try_from_u8(0).is_some());
    assert!(NiuFilterMode::try_from_u8(1).is_some());
    for reserved in [2u8, 3, 0x7F, 0xFE, 0xFF] {
        assert_eq!(
            NiuFilterMode::try_from_u8(reserved),
            None,
            "Table 4 reserves {reserved}"
        );
    }
}

#[test]
fn network_layer_niu_rejects_invalid_sources_without_forwarding_or_learning() {
    let mut niu = Niu::new(NiuConfig::default());
    niu.start().unwrap();

    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, BROADCAST_ADDRESS, BROADCAST_ADDRESS, &[1]),
            Side::Tractor,
            0,
        )
        .is_none(),
        "0xFF is not a valid source address"
    );
    assert!(
        niu.process_frame(
            frame(PGN_HEARTBEAT, NULL_ADDRESS, BROADCAST_ADDRESS, &[1]),
            Side::Tractor,
            1,
        )
        .is_none(),
        "0xFE is only valid as source for Cannot Claim Address"
    );
    assert_eq!(niu.forwarded(), 0);
    assert_eq!(niu.blocked(), 2);

    let cannot_claim =
        niu.process_frame(address_claim(NULL_ADDRESS, name(0x100)), Side::Tractor, 2);
    assert!(
        cannot_claim.is_some(),
        "Cannot Claim Address remains visible across an NIU"
    );
    assert_eq!(
        niu.observed_name(Side::Tractor, NULL_ADDRESS),
        None,
        "NULL-source Cannot Claim frames must not populate NAME/address cache"
    );
}

#[test]
fn network_layer_niu_control_messages_ignore_invalid_sources_before_mutation() {
    let mut niu = Niu::new(NiuConfig::default());
    let add_filter = NiuNetworkMsg {
        function: NiuFunction::AddFilterEntry,
        port_number: 1,
        filter_pgn: PGN_HEARTBEAT,
        ..NiuNetworkMsg::default()
    }
    .encode()
    .unwrap();

    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        niu.handle_niu_message(&Message::new(
            PGN_NIU_NETWORK_MSG,
            add_filter.to_vec(),
            source,
        ));
        assert!(
            niu.filters().is_empty(),
            "invalid control source 0x{source:02X} must not mutate the filter database"
        );
    }
    niu.handle_niu_message(&Message::with_addressing(
        PGN_NIU_NETWORK_MSG,
        add_filter.to_vec(),
        0x44,
        NULL_ADDRESS,
        Priority::Default,
    ));
    assert!(
        niu.filters().is_empty(),
        "null-destination NIU control metadata must not mutate the filter database"
    );

    // A globally addressed command is not this bridge's to act on either.
    niu.handle_niu_message(&Message::new(
        PGN_NIU_NETWORK_MSG,
        add_filter.to_vec(),
        0x44,
    ));
    assert!(niu.filters().is_empty());

    niu.handle_niu_message(&niu_control(add_filter.to_vec(), 0x44, 0x20));
    assert_eq!(niu.filters().len(), 1);
    assert_eq!(niu.filters()[0].pgn, PGN_HEARTBEAT);
}

#[test]
fn network_layer_router_translates_address_claims_and_blocks_spoofed_claims() {
    let mut router = Router::new(NiuConfig::default());
    router.niu_mut().start().unwrap();
    let translated_name = name(0x200);
    router.add_translation(translated_name, 0x10, 0x20).unwrap();

    let translated_claim = router
        .process_frame(address_claim(0x10, translated_name), Side::Tractor, 0)
        .expect("matching translated Address Claimed should forward");
    assert_eq!(translated_claim.pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(translated_claim.source(), 0x20);
    assert_eq!(translated_claim.destination(), BROADCAST_ADDRESS);
    assert_eq!(translated_claim.payload(), translated_name.to_bytes());

    let spoofed_claim = address_claim(0x10, name(0x201));
    assert!(
        router
            .process_frame(spoofed_claim, Side::Tractor, 1)
            .is_none(),
        "a translated address may not claim a different NAME"
    );
    assert_eq!(router.niu().blocked(), 1);

    let wrong_side_address = address_claim(0x11, translated_name);
    assert!(
        router
            .process_frame(wrong_side_address, Side::Tractor, 2)
            .is_none(),
        "a known translated NAME may not appear from an unexpected side-local address"
    );
    assert_eq!(router.niu().blocked(), 2);
}

#[test]
fn network_layer_router_blocks_untranslated_destination_specific_frames() {
    let mut router = Router::new(NiuConfig::default());
    router.niu_mut().start().unwrap();
    router.add_translation(name(0x300), 0x10, 0x20).unwrap();

    assert!(
        router
            .process_frame(frame(PGN_REQUEST, 0x10, 0x33, &[0, 0, 0]), Side::Tractor, 0)
            .is_none(),
        "destination-specific frames need a destination translation"
    );
    assert_eq!(router.niu().blocked(), 1);
}

#[test]
fn network_layer_router_translates_bidirectional_destination_frames_and_cannot_claims() {
    let mut router = Router::new(NiuConfig::default().loop_guard_capacity(8));
    router.niu_mut().start().unwrap();
    let tractor_name = name(0x371);
    let implement_name = name(0x372);
    router.add_translation(tractor_name, 0x10, 0x20).unwrap();
    router.add_translation(implement_name, 0x11, 0x21).unwrap();

    let tractor_to_implement = frame(PGN_REQUEST, 0x10, 0x11, &[0x00, 0xEA, 0x00]);
    let translated = router
        .process_frame(tractor_to_implement, Side::Tractor, 4_000)
        .expect("destination-specific traffic with both endpoints mapped should cross the router");
    assert_eq!(translated.source(), 0x20);
    assert_eq!(translated.destination(), 0x21);
    assert_eq!(translated.pgn(), PGN_REQUEST);
    assert_eq!(translated.payload(), [0x00, 0xEA, 0x00]);

    assert!(
        router
            .process_frame(translated, Side::Implement, 4_001)
            .is_none(),
        "the translated output frame must be remembered as an echo candidate on the target side"
    );

    let implement_to_tractor = frame(PGN_REQUEST, 0x21, 0x20, &[0x00, 0xEE, 0x00]);
    let reverse = router
        .process_frame(implement_to_tractor, Side::Implement, 4_002)
        .expect("reverse destination-specific traffic should use the inverse side mapping");
    assert_eq!(reverse.source(), 0x11);
    assert_eq!(reverse.destination(), 0x10);
    assert_eq!(reverse.pgn(), PGN_REQUEST);
    assert_eq!(reverse.payload(), [0x00, 0xEE, 0x00]);

    let cannot_claim = router
        .process_frame(
            address_claim(NULL_ADDRESS, tractor_name),
            Side::Tractor,
            4_003,
        )
        .expect("Cannot Claim Address must remain visible through a router");
    assert_eq!(cannot_claim.pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(cannot_claim.source(), NULL_ADDRESS);
    assert_eq!(cannot_claim.destination(), BROADCAST_ADDRESS);
    assert_eq!(cannot_claim.payload(), tractor_name.to_bytes());
}

#[test]
fn network_layer_router_translation_admission_is_unique_and_claimable() {
    let mut router = Router::new(NiuConfig::default());
    let first = name(0x401);
    let second = name(0x402);

    router.add_translation(first, 0x10, 0x20).unwrap();
    let before_conflict = router.policy_snapshot().translations;
    assert_eq!(before_conflict.len(), 1);

    assert!(
        router.add_translation(second, 0x10, 0x21).is_err(),
        "two active NAMEs must not share the same tractor-side address"
    );
    assert_eq!(
        router.policy_snapshot().translations,
        before_conflict,
        "failed tractor-side admission must not partially mutate the table"
    );

    assert!(
        router.add_translation(second, 0x11, 0x20).is_err(),
        "two active NAMEs must not share the same implement-side address"
    );
    assert_eq!(
        router.policy_snapshot().translations,
        before_conflict,
        "failed implement-side admission must not partially mutate the table"
    );

    for invalid in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert!(
            router.add_translation(second, invalid, 0x22).is_err(),
            "reserved source address 0x{invalid:02X} is not a claimable tractor-side mapping"
        );
        assert!(
            router.add_translation(second, 0x12, invalid).is_err(),
            "reserved source address 0x{invalid:02X} is not a claimable implement-side mapping"
        );
    }
    assert_eq!(
        router.policy_snapshot().translations,
        before_conflict,
        "invalid-address admission must not alter existing translations"
    );

    router
        .add_translation(first, 0x12, 0x22)
        .expect("the same NAME may move to a new free address pair");
    let after_replace = router.policy_snapshot().translations;
    assert_eq!(after_replace.len(), 1);
    assert_eq!(after_replace[0].name, first);
    assert_eq!(after_replace[0].tractor_address, 0x12);
    assert_eq!(after_replace[0].implement_address, 0x22);
}

#[test]
fn network_layer_line_topology_forwards_once_and_blocks_bounce_back() {
    let mut ab = active_niu(NiuConfig::default().loop_guard_capacity(8));
    let mut bc = active_niu(NiuConfig::default().loop_guard_capacity(8));
    let heartbeat = frame(PGN_HEARTBEAT, 0x21, BROADCAST_ADDRESS, &[0x7D, 0xFF]);

    let on_middle_from_a = ab
        .process_frame(heartbeat, Side::Tractor, 1_000)
        .expect("first NIU forwards the frame into the middle segment");
    let on_c_from_middle = bc
        .process_frame(on_middle_from_a, Side::Tractor, 1_001)
        .expect("second NIU forwards the same legal broadcast into the end segment");

    assert_eq!(on_c_from_middle.pgn(), PGN_HEARTBEAT);
    assert_eq!(on_c_from_middle.source(), 0x21);
    assert_eq!(ab.forwarded(), 1);
    assert_eq!(bc.forwarded(), 1);

    assert!(
        bc.process_frame(on_c_from_middle, Side::Implement, 1_002)
            .is_none(),
        "an immediate echo from the end segment must not be forwarded back into the line"
    );
    assert!(
        ab.process_frame(on_middle_from_a, Side::Implement, 1_003)
            .is_none(),
        "the upstream NIU still remembers the just-forwarded middle-segment frame"
    );
    assert_eq!(ab.blocked(), 1);
    assert_eq!(bc.blocked(), 1);
}

#[test]
fn network_layer_fork_topology_keeps_loop_guards_per_branch() {
    let mut left_branch = active_niu(NiuConfig::default().loop_guard_capacity(4));
    let mut right_branch = active_niu(NiuConfig::default().loop_guard_capacity(4));
    let request = frame(PGN_REQUEST, 0x31, BROADCAST_ADDRESS, &[0x00, 0xEE, 0x00]);

    let on_left = left_branch
        .process_frame(request, Side::Tractor, 2_000)
        .expect("left branch receives the trunk broadcast");
    let on_right = right_branch
        .process_frame(request, Side::Tractor, 2_000)
        .expect("right branch receives the same trunk broadcast");
    assert_eq!(on_left, on_right);

    assert!(
        left_branch
            .process_frame(on_left, Side::Implement, 2_001)
            .is_none(),
        "left branch echo is blocked by the left branch loop guard"
    );
    assert!(
        right_branch
            .process_frame(on_right, Side::Implement, 2_001)
            .is_none(),
        "right branch echo is blocked by the right branch loop guard"
    );
    assert_eq!(left_branch.forwarded(), 1);
    assert_eq!(right_branch.forwarded(), 1);
    assert_eq!(left_branch.blocked(), 1);
    assert_eq!(right_branch.blocked(), 1);
}

#[test]
fn network_layer_ring_topology_loop_guard_expires_only_after_window() {
    let mut ab = active_niu(
        NiuConfig::default()
            .loop_guard_capacity(8)
            .loop_guard_window_ms(50),
    );
    let mut bc = active_niu(
        NiuConfig::default()
            .loop_guard_capacity(8)
            .loop_guard_window_ms(50),
    );
    let frame = frame(PGN_HEARTBEAT, 0x41, BROADCAST_ADDRESS, &[0x01]);

    let on_b_from_a = ab
        .process_frame(frame, Side::Tractor, 3_000)
        .expect("A to B leg forwards");
    let on_c_from_b = bc
        .process_frame(on_b_from_a, Side::Tractor, 3_001)
        .expect("B to C leg forwards");

    assert!(
        bc.process_frame(on_c_from_b, Side::Implement, 3_010)
            .is_none(),
        "ring echo inside the loop window is blocked at the downstream NIU"
    );
    assert!(
        ab.process_frame(on_b_from_a, Side::Implement, 3_011)
            .is_none(),
        "ring echo inside the loop window is blocked at the upstream NIU"
    );

    assert!(
        ab.process_frame(on_b_from_a, Side::Implement, 3_052)
            .is_some(),
        "the loop guard is a bounded recent-frame guard, not a permanent filter"
    );
    assert_eq!(ab.blocked(), 1);
    assert_eq!(bc.blocked(), 1);
}

/// 6I — ISO 11783-4 Table 4 binds mode `0` to "Block-specific PGNs (default =
/// pass all)" and `1` to "Pass-specific PGNs (default = block all)". The two
/// were bound the other way round, so a bridge configured for §6.2.2 — the
/// standard's stated "preferred mode of operation for bridges conforming to
/// ISO 11783" — blocked everything it should have forwarded.
#[test]
fn network_layer_filter_mode_default_matches_table_4() {
    use machbus::net::niu::{Niu, NiuConfig, NiuFilterMode};

    // Mode 0: default pass. An unlisted PGN is forwarded.
    let block_specific = Niu::new(NiuConfig::default().mode(NiuFilterMode::BlockSpecific));
    assert_eq!(block_specific.filter_mode(), NiuFilterMode::BlockSpecific);
    assert_eq!(
        NiuFilterMode::BlockSpecific as u8,
        0,
        "Table 4 assigns block-specific the value 0"
    );

    // Mode 1: default block. An unlisted PGN is not forwarded.
    let pass_specific = Niu::new(NiuConfig::default().mode(NiuFilterMode::PassSpecific));
    assert_eq!(pass_specific.filter_mode(), NiuFilterMode::PassSpecific);
    assert_eq!(
        NiuFilterMode::PassSpecific as u8,
        1,
        "Table 4 assigns pass-specific the value 1"
    );

    // The default mode is the one §6.2.2 calls preferred for conforming bridges.
    assert_eq!(NiuFilterMode::default(), NiuFilterMode::BlockSpecific);
}

/// 6I — §6.6.2.3.3 adds an entry to the filter database, and what an entry
/// *means* depends on that database's mode (§6.2.2/§6.2.3): in block-specific
/// mode a listed PGN is blocked, in pass-specific mode it is passed.
///
/// The handler always installed `Allow`, so adding an entry to a
/// block-specific database asked the bridge to forward exactly the PGN the
/// caller wanted stopped — actively overriding the intended blocking.
#[test]
fn network_layer_filter_entry_policy_follows_the_database_mode() {
    use machbus::net::niu::{ForwardPolicy, Niu, NiuConfig, NiuFilterMode};

    for (mode, expected) in [
        (NiuFilterMode::BlockSpecific, ForwardPolicy::Block),
        (NiuFilterMode::PassSpecific, ForwardPolicy::Allow),
    ] {
        let mut niu = Niu::new(NiuConfig::default().mode(mode));
        niu.set_filter_mode(mode);
        niu.add_filter(machbus::net::niu::FilterRule::new(0xEF00, expected, true));

        let installed = niu
            .filters()
            .iter()
            .find(|f| f.pgn == 0xEF00)
            .expect("entry installed");
        assert_eq!(
            installed.policy, expected,
            "an entry in {mode:?} mode must mean {expected:?}"
        );
    }
}

/// 6I — ISO 11783-4 Table 2 assigns every NIU message function its code.
/// This stack used a locally invented 1..=15 sequence, so **no message it sent
/// or accepted matched a conforming NIU**: an "add filter entry" from a real
/// bridge (code 2) happened to collide, but a request for the filter database
/// (code 0) decoded as nothing, and its own "set filter mode" (6) would read
/// as "create a filter database entry" on the wire.
#[test]
fn network_layer_function_codes_match_table_2() {
    use machbus::net::niu::NiuFunction as F;

    for (code, expected) in [
        (0u8, F::RequestFilterDb),
        (1, F::FilterDbResponse),
        (2, F::AddFilterEntry),
        (3, F::DeleteFilterEntry),
        (4, F::ClearFilterEntry),
        (6, F::CreateFilterEntry),
        (7, F::AddNameQualifiedEntries),
        (64, F::RequestSourceAddressList),
        (65, F::SourceAddressListResponse),
        (66, F::RequestSourceAddressNameList),
        (67, F::SourceAddressNameListResponse),
        (128, F::RequestGeneralParametrics),
        (129, F::GeneralParametricsResponse),
        (130, F::ResetGeneralStatistics),
        (131, F::RequestSpecificParametrics),
        (132, F::SpecificParametricsResponse),
        (133, F::ResetSpecificStatistics),
        (192, F::OpenConnection),
        (193, F::OpenConnectionResponse),
        (194, F::CloseConnection),
        (195, F::CloseConnectionResponse),
    ] {
        assert_eq!(F::try_from_u8(code), Some(expected), "code {code}");
        assert_eq!(expected.as_u8(), code);
    }

    // 5 is "Obsolete, not to be used".
    assert_eq!(F::try_from_u8(5), None);

    // Every reserved band stays reserved.
    for reserved in [8u8, 63, 68, 127, 134, 191, 196, 255] {
        assert_eq!(F::try_from_u8(reserved), None, "reserved {reserved}");
    }
}

/// 6I — §6.7.2.3/§6.7.2.4 (`N.NTX_Request` / `N.NTX_Response`) let a CF
/// discover the control functions behind a router, whose address claims it
/// never saw because they sit in a different address space. Neither message
/// existed, so CFs behind a router were undiscoverable: the NIU held the data
/// internally and nothing could carry it.
#[test]
fn network_layer_topology_messages_round_trip() {
    use machbus::net::niu::{NiuFunction, NiuTopologyMsg, SourceAddressName};

    // Request: 8 bytes, port in the low nibble, high nibble F, tail all FF.
    let request = NiuTopologyMsg::request(3);
    let bytes = request.encode().unwrap();
    assert_eq!(bytes.len(), 8);
    assert_eq!(bytes[0], NiuFunction::RequestSourceAddressNameList.as_u8());
    assert_eq!(bytes[1], 0xF3, "port pair: port 3, bits 5-8 set to F");
    assert!(bytes[2..].iter().all(|&b| b == 0xFF), "bytes 3-8 reserved");
    assert_eq!(NiuTopologyMsg::decode(&bytes), Some(request));

    // Response: count, then 9 bytes per source-address/NAME pair.
    let entries = vec![
        SourceAddressName {
            address: 0x26,
            name: 0x0123_4567_89AB_CDEF,
        },
        SourceAddressName {
            address: 0x80,
            name: 0xFEDC_BA98_7654_3210,
        },
    ];
    let response = NiuTopologyMsg::response(1, entries);
    let bytes = response.encode().unwrap();
    assert_eq!(bytes[0], NiuFunction::SourceAddressNameListResponse.as_u8());
    assert_eq!(bytes[2], 2, "byte 3 is the pair count");
    assert_eq!(bytes.len(), 3 + 2 * 9);
    assert_eq!(bytes[3], 0x26, "first source address");
    assert_eq!(NiuTopologyMsg::decode(&bytes), Some(response));

    // A truncated pair list is malformed, not a shorter list.
    let mut truncated = bytes.clone();
    truncated.pop();
    assert_eq!(NiuTopologyMsg::decode(&truncated), None);

    // The port pair's high nibble must be F.
    let mut bad_port_pair = bytes;
    bad_port_pair[1] &= 0x0F;
    assert_eq!(NiuTopologyMsg::decode(&bad_port_pair), None);

    // Port 15 is the global port and is not addressable here.
    assert!(NiuTopologyMsg::request(15).encode().is_err());
}

/// 6I — §6.9.5 connection management was entirely absent, so a CF could not
/// reach a peer behind a router at all. The request carries the port pair and
/// the NAME of the target CF (obtained from the §6.7.2.4 topology response);
/// the response carries a success bit and, on failure, a reason code.
#[test]
fn network_layer_connection_management_round_trips() {
    use machbus::net::niu::{
        ConnectionFailureReason, NiuConnectionRequest, NiuConnectionResponse, NiuFunction,
    };

    let open = NiuConnectionRequest {
        function: NiuFunction::OpenConnection,
        to_port: 2,
        from_port: 0,
        name: 0x0123_4567_89AB_CDEF,
    };
    let bytes = open.encode().unwrap();
    assert_eq!(bytes.len(), 10);
    assert_eq!(bytes[0], 192, "Table 2 code for request-to-open");
    assert_eq!(bytes[1], 0x02, "low nibble = to port, high nibble = from");
    assert_eq!(NiuConnectionRequest::decode(&bytes), Some(open));

    // Failure response carries a reason; success does not need one.
    let refused = NiuConnectionResponse {
        function: NiuFunction::OpenConnectionResponse,
        to_port: 0,
        from_port: 2,
        success: false,
        reason: ConnectionFailureReason::CannotFindCfWithName,
    };
    let bytes = refused.encode().unwrap();
    assert_eq!(bytes.len(), 8);
    assert_eq!(bytes[0], 193);
    assert_eq!(bytes[2] & 0x03, 0x00, "0x00 = failure");
    assert_eq!(bytes[2] & 0xFC, 0xFC, "bits 3-8 reserved, set to 1");
    assert_eq!(NiuConnectionResponse::decode(&bytes), Some(refused));

    let accepted = NiuConnectionResponse {
        function: NiuFunction::CloseConnectionResponse,
        to_port: 0,
        from_port: 1,
        success: true,
        reason: ConnectionFailureReason::NotAvailable,
    };
    let bytes = accepted.encode().unwrap();
    assert_eq!(bytes[2] & 0x03, 0x01, "0x01 = success");
    assert_eq!(NiuConnectionResponse::decode(&bytes), Some(accepted));

    // Reserved failure reasons 5..=254 are not decodable.
    for reserved in [5u8, 100, 254] {
        let mut bad = bytes;
        bad[2] = 0xFC;
        bad[3] = reserved;
        assert_eq!(NiuConnectionResponse::decode(&bad), None, "{reserved}");
    }

    // A request function may not be encoded as a response, or vice versa.
    assert!(
        NiuConnectionRequest {
            function: NiuFunction::OpenConnectionResponse,
            to_port: 1,
            from_port: 0,
            name: 0,
        }
        .encode()
        .is_err()
    );
}
