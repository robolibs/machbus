#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::{PGN_ADDRESS_CLAIMED, PGN_DM1, PGN_HEARTBEAT, PGN_REQUEST};
    use crate::net::types::Priority;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn make_frame(pgn: Pgn, src: Address, dst: Address) -> Frame {
        Frame::from_message(Priority::Default, pgn, src, dst, &[1, 2, 3])
    }

    #[test]
    fn niu_profiles_state_behaviours_and_honest_support() {
        assert_eq!(NIU_PROFILES.len(), 4);
        // Simple router: forwards/filters, no translation.
        let r = niu_profile(NiuProfile::SimpleRouter);
        assert!(r.forwarding && r.filtering);
        assert!(!r.address_translation && !r.runtime_reconfiguration);
        assert_eq!(r.status, NiuProfileStatus::Implemented);
        // Bridge adds translation.
        assert!(niu_profile(NiuProfile::Bridge).address_translation);
        // Managed gateway adds runtime reconfig and config persistence
        // (NiuConfig::save / load_from), but is a PartialHelper: the control
        // plane and gateway repackaging are not implemented.
        let g = niu_profile(NiuProfile::ManagedGateway);
        assert!(g.runtime_reconfiguration);
        assert!(g.persistence);
        assert_eq!(g.status, NiuProfileStatus::PartialHelper);
        // Only the managed gateway claims persistence.
        assert_eq!(NIU_PROFILES.iter().filter(|p| p.persistence).count(), 1);
    }

    #[test]
    fn niu_config_round_trips_through_persisted_string() {
        let cfg = NiuConfig::default()
            .name("Gateway A")
            .global_default(false)
            .loop_guard_window_ms(1234)
            .loop_guard_capacity(7);
        let restored = NiuConfig::from_persisted_string(&cfg.to_persisted_string());
        assert_eq!(restored.name, "Gateway A");
        assert!(!restored.forward_global_by_default);
        assert_eq!(restored.loop_guard_window_ms, 1234);
        assert_eq!(restored.loop_guard_max_recent_forwards, 7);
        assert_eq!(restored.filter_mode, cfg.filter_mode);
    }

    #[test]
    fn niu_config_saves_and_loads_from_disk() {
        let path = std::env::temp_dir().join("machbus_niu_cfg_test.txt");
        let path = path.to_string_lossy().to_string();
        let cfg = NiuConfig::default()
            .name("Persisted NIU")
            .persistence(&path);
        assert!(cfg.save().unwrap(), "save writes when a file is configured");
        let loaded = NiuConfig::load_from(&path).unwrap();
        assert_eq!(loaded.name, "Persisted NIU");
        assert_eq!(loaded.persistence_file.as_deref(), Some(path.as_str()));
        let _ = std::fs::remove_file(&path);
        // A config with no persistence file is a no-op save.
        assert!(!NiuConfig::default().save().unwrap());
    }

    fn make_address_claim(src: Address, name: Name) -> Frame {
        Frame::from_message(
            Priority::Default,
            PGN_ADDRESS_CLAIMED,
            src,
            BROADCAST_ADDRESS,
            &name.to_bytes(),
        )
    }

    #[test]
    fn defaults_to_pass_all() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.start().unwrap();
        let f = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        assert!(niu.process_frame(f, Side::Tractor, 0).is_some());
        assert_eq!(niu.forwarded(), 1);
    }

    #[test]
    fn block_all_default_drops_unlisted() {
        let mut niu = Niu::new(NiuConfig::default().mode(NiuFilterMode::BlockAll));
        niu.set_filter_mode(NiuFilterMode::BlockAll);
        niu.start().unwrap();
        let f = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        assert!(niu.process_frame(f, Side::Tractor, 0).is_none());
        assert_eq!(niu.blocked(), 1);
    }

    #[test]
    fn block_all_with_explicit_allow_passes_listed() {
        let mut niu = Niu::new(NiuConfig::default().mode(NiuFilterMode::BlockAll));
        niu.set_filter_mode(NiuFilterMode::BlockAll);
        niu.allow_pgn(PGN_HEARTBEAT, true);
        niu.start().unwrap();

        let f1 = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        let f2 = make_frame(PGN_DM1, 0x10, 0xFF);
        assert!(niu.process_frame(f1, Side::Tractor, 0).is_some());
        assert!(niu.process_frame(f2, Side::Tractor, 0).is_none());
        assert_eq!(niu.forwarded(), 1);
        assert_eq!(niu.blocked(), 1);
    }

    #[test]
    fn pass_all_with_explicit_block_drops_listed() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.block_pgn(PGN_DM1, true);
        niu.start().unwrap();
        let f = make_frame(PGN_DM1, 0x10, 0xFF);
        assert!(niu.process_frame(f, Side::Tractor, 0).is_none());
        assert_eq!(niu.blocked(), 1);
    }

    #[test]
    fn inactive_niu_drops_everything() {
        let mut niu = Niu::new(NiuConfig::default());
        // not started
        let f = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        assert!(niu.process_frame(f, Side::Tractor, 0).is_none());
        // and counters do not move when inactive
        assert_eq!(niu.forwarded(), 0);
        assert_eq!(niu.blocked(), 0);
    }

    #[test]
    fn monitor_policy_forwards_and_fires_event() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.monitor_pgn(PGN_HEARTBEAT, true);
        niu.start().unwrap();
        let count = Rc::new(RefCell::new(0u32));
        let c = count.clone();
        niu.on_monitored.subscribe(move |_| *c.borrow_mut() += 1);

        let f = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        assert!(niu.process_frame(f, Side::Tractor, 0).is_some());
        assert_eq!(*count.borrow(), 1);
        assert_eq!(niu.forwarded(), 1);
    }

    #[test]
    fn rate_limiter_blocks_within_window() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.allow_pgn_rate_limited(PGN_HEARTBEAT, 100, true);
        niu.start().unwrap();

        let f = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        assert!(niu.process_frame(f, Side::Tractor, 0).is_some()); // first ok
        assert!(niu.process_frame(f, Side::Tractor, 50).is_none()); // 50 ms later, blocked
        assert!(niu.process_frame(f, Side::Tractor, 150).is_some()); // 150 ms, allowed again
        assert_eq!(niu.forwarded(), 2);
        assert_eq!(niu.blocked(), 1);
    }

    #[test]
    fn rate_limited_drops_are_counted_separately_from_policy_blocks() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.allow_pgn_rate_limited(PGN_HEARTBEAT, 100, true);
        niu.block_pgn(PGN_DM1, true);
        niu.start().unwrap();

        let hb = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        assert!(niu.process_frame(hb, Side::Tractor, 0).is_some());
        // Second heartbeat within the window: a rate-limited drop.
        assert!(niu.process_frame(hb, Side::Tractor, 50).is_none());
        // A policy-blocked PGN: a block, but NOT rate-limited.
        let blocked = make_frame(PGN_DM1, 0x10, 0xFF);
        assert!(niu.process_frame(blocked, Side::Tractor, 60).is_none());

        assert_eq!(niu.rate_limited(), 1);
        // blocked() is the total of all drops (rate-limited + policy block).
        assert_eq!(niu.blocked(), 2);
        assert_eq!(niu.forwarded(), 1);
    }

    #[test]
    fn loop_guard_blocks_echoed_frames_from_opposite_side() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.start().unwrap();
        let window = niu.config().loop_guard_window_ms;
        let f = make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS);

        assert!(niu.process_frame(f, Side::Tractor, 100).is_some());
        assert!(
            niu.process_frame(f, Side::Tractor, 101).is_some(),
            "same-side periodic messages are not loop echoes"
        );
        assert!(
            niu.process_frame(f, Side::Implement, 102).is_none(),
            "the same raw frame returning from the just-forwarded side is a loop echo"
        );
        assert_eq!(niu.blocked(), 1);

        assert!(
            niu.process_frame(f, Side::Implement, 101 + window + 1)
                .is_some(),
            "loop guard entries expire after the configured window"
        );
    }

    #[test]
    fn loop_guard_can_be_disabled_for_explicit_lab_setups() {
        let mut niu = Niu::new(NiuConfig::default().loop_guard_window_ms(0));
        niu.start().unwrap();
        let f = make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS);

        assert!(niu.process_frame(f, Side::Tractor, 0).is_some());
        assert!(niu.process_frame(f, Side::Implement, 1).is_some());
        assert_eq!(niu.blocked(), 0);
    }

    #[test]
    fn loop_guard_capacity_is_bounded_and_configurable_under_storms() {
        let mut niu = Niu::new(NiuConfig::default().loop_guard_capacity(2));
        niu.start().unwrap();

        let first = make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS);
        let second = make_frame(PGN_HEARTBEAT, 0x11, BROADCAST_ADDRESS);
        let third = make_frame(PGN_HEARTBEAT, 0x12, BROADCAST_ADDRESS);

        assert!(niu.process_frame(first, Side::Tractor, 100).is_some());
        assert!(niu.process_frame(second, Side::Tractor, 101).is_some());
        assert!(niu.process_frame(third, Side::Tractor, 102).is_some());
        assert_eq!(
            niu.recent_forwards.len(),
            2,
            "loop-guard history must stay within the configured cap"
        );

        assert!(
            niu.process_frame(second, Side::Implement, 103).is_none(),
            "recent echoed frames inside the bounded window are blocked"
        );
        assert!(
            niu.process_frame(third, Side::Implement, 104).is_none(),
            "newest echoed frames are retained during a storm"
        );
    }

    #[test]
    fn loop_guard_capacity_zero_disables_recent_frame_storage() {
        let mut niu = Niu::new(NiuConfig::default().loop_guard_capacity(0));
        niu.start().unwrap();
        let f = make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS);

        assert!(niu.process_frame(f, Side::Tractor, 0).is_some());
        assert!(niu.recent_forwards.is_empty());
        assert!(
            niu.process_frame(f, Side::Implement, 1).is_some(),
            "capacity zero is an explicit lab-mode opt-out from loop storage"
        );
    }

    #[test]
    fn loop_guard_blocks_echoes_in_two_niu_ring_topology() {
        let mut ab = Niu::new(NiuConfig::default().loop_guard_capacity(8));
        let mut bc = Niu::new(NiuConfig::default().loop_guard_capacity(8));
        ab.start().unwrap();
        bc.start().unwrap();
        let f = make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS);

        let on_b_from_a = ab
            .process_frame(f, Side::Tractor, 100)
            .expect("A->B forward");
        let on_c_from_b = bc
            .process_frame(on_b_from_a, Side::Tractor, 101)
            .expect("B->C forward");

        assert!(
            bc.process_frame(on_c_from_b, Side::Implement, 102)
                .is_none(),
            "the downstream NIU blocks an immediate C->B echo"
        );
        assert!(
            ab.process_frame(on_b_from_a, Side::Implement, 103)
                .is_none(),
            "the upstream NIU still blocks the B->A echo in a ring"
        );
        assert_eq!(ab.blocked(), 1);
        assert_eq!(bc.blocked(), 1);
    }

    #[test]
    fn unidirectional_rule_only_applies_tractor_side() {
        let mut niu = Niu::new(NiuConfig::default().mode(NiuFilterMode::BlockAll));
        niu.set_filter_mode(NiuFilterMode::BlockAll);
        niu.allow_pgn(PGN_HEARTBEAT, false); // tractor → implement only
        niu.start().unwrap();

        let f = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        assert!(niu.process_frame(f, Side::Tractor, 0).is_some());
        // Implement-side: rule does not apply ⇒ default mode = BlockAll → blocked.
        assert!(niu.process_frame(f, Side::Implement, 0).is_none());
    }

    #[test]
    fn source_name_filter_matches_after_address_claim_observation() {
        let name = Name::default()
            .with_identity_number(0x1234)
            .with_manufacturer_code(0x456);
        let mut niu = Niu::new(NiuConfig::default());
        niu.add_filter(FilterRule::new(PGN_DM1, ForwardPolicy::Block, true).with_source_name(name));
        niu.start().unwrap();

        let dm1 = make_frame(PGN_DM1, 0x10, BROADCAST_ADDRESS);
        assert!(
            niu.process_frame(dm1, Side::Tractor, 0).is_some(),
            "before address-claim observation, a NAME-scoped rule must not guess"
        );

        assert!(
            niu.process_frame(make_address_claim(0x10, name), Side::Tractor, 1)
                .is_some()
        );
        assert_eq!(niu.observed_name(Side::Tractor, 0x10), Some(name));

        assert!(
            niu.process_frame(dm1, Side::Tractor, 2).is_none(),
            "after address-claim observation, the source NAME block applies"
        );
        assert!(
            niu.process_frame(
                make_frame(PGN_DM1, 0x11, BROADCAST_ADDRESS),
                Side::Tractor,
                3
            )
            .is_some()
        );
    }

    #[test]
    fn block_all_source_name_allow_rule_uses_observed_claim() {
        let name = Name::default()
            .with_identity_number(0x3456)
            .with_manufacturer_code(0x234);
        let mut niu = Niu::new(NiuConfig::default().mode(NiuFilterMode::BlockAll));
        niu.set_filter_mode(NiuFilterMode::BlockAll);
        niu.add_filter(FilterRule::new(PGN_DM1, ForwardPolicy::Allow, true).with_source_name(name));
        niu.start().unwrap();

        let dm1 = make_frame(PGN_DM1, 0x20, BROADCAST_ADDRESS);
        assert!(niu.process_frame(dm1, Side::Tractor, 0).is_none());
        assert!(
            niu.process_frame(make_address_claim(0x20, name), Side::Tractor, 1)
                .is_none()
        );
        assert_eq!(niu.observed_name(Side::Tractor, 0x20), Some(name));

        assert!(niu.process_frame(dm1, Side::Tractor, 2).is_some());
        assert!(
            niu.process_frame(
                make_frame(PGN_DM1, 0x21, BROADCAST_ADDRESS),
                Side::Tractor,
                3
            )
            .is_none()
        );
    }

    #[test]
    fn destination_name_filter_matches_destination_specific_frames() {
        let destination_name = Name::default()
            .with_identity_number(0x6789)
            .with_manufacturer_code(0x321);
        let mut niu = Niu::new(NiuConfig::default().mode(NiuFilterMode::BlockAll));
        niu.set_filter_mode(NiuFilterMode::BlockAll);
        niu.add_filter(
            FilterRule::new(PGN_REQUEST, ForwardPolicy::Allow, true)
                .with_destination_name(destination_name),
        );
        niu.start().unwrap();

        assert!(
            niu.process_frame(make_address_claim(0x42, destination_name), Side::Tractor, 0)
                .is_none()
        );
        assert_eq!(
            niu.observed_name(Side::Tractor, 0x42),
            Some(destination_name)
        );

        let request_to_known_name = Frame::from_message(
            Priority::Default,
            PGN_REQUEST,
            0x10,
            0x42,
            &[
                PGN_ADDRESS_CLAIMED as u8,
                (PGN_ADDRESS_CLAIMED >> 8) as u8,
                0x00,
            ],
        );
        assert!(
            niu.process_frame(request_to_known_name, Side::Tractor, 1)
                .is_some()
        );

        let request_to_unknown_address = Frame::from_message(
            Priority::Default,
            PGN_REQUEST,
            0x10,
            0x43,
            &[
                PGN_ADDRESS_CLAIMED as u8,
                (PGN_ADDRESS_CLAIMED >> 8) as u8,
                0x00,
            ],
        );
        assert!(
            niu.process_frame(request_to_unknown_address, Side::Tractor, 2)
                .is_none()
        );
    }

    #[test]
    fn filter_rule_round_trips_through_encode_decode() {
        let rule = FilterRule::new(PGN_REQUEST, ForwardPolicy::Monitor, false)
            .with_source_name(Name::default().with_identity_number(0x1234))
            .with_max_frequency_ms(500)
            .persistent(true);
        let bytes = rule.encode().unwrap();
        assert_eq!(bytes.len(), 22);
        let decoded = FilterRule::decode(&bytes).unwrap();
        assert_eq!(decoded.pgn, rule.pgn);
        assert_eq!(decoded.policy, rule.policy);
        assert_eq!(decoded.bidirectional, rule.bidirectional);
        assert_eq!(decoded.persistent, rule.persistent);
        assert_eq!(decoded.source_name, rule.source_name);
        assert_eq!(decoded.destination_name, None);
        assert_eq!(decoded.max_frequency_ms, rule.max_frequency_ms);
    }

    #[test]
    fn filter_rule_encode_rejects_unencodable_pgn_and_rate() {
        let invalid_pgn = FilterRule::new(0x40000, ForwardPolicy::Allow, true);
        let err = invalid_pgn.encode().unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));

        let invalid_rate = FilterRule::new(PGN_REQUEST, ForwardPolicy::Allow, true)
            .with_max_frequency_ms(u32::from(u16::MAX) + 1);
        let err = invalid_rate.encode().unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(err.message.contains("max_frequency_ms"));
    }

    #[test]
    fn filter_rule_decode_rejects_malformed_payloads() {
        let rule = FilterRule::new(PGN_REQUEST, ForwardPolicy::Monitor, true)
            .persistent(true)
            .with_max_frequency_ms(500);
        let valid = rule.encode().unwrap();

        assert!(FilterRule::decode(&valid[..21]).is_err());

        let mut overlong = valid.clone();
        overlong.push(0xFF);
        assert!(FilterRule::decode(&overlong).is_err());

        let mut bad_pgn_high_bits = valid.clone();
        bad_pgn_high_bits[2] |= 0x04;
        assert!(FilterRule::decode(&bad_pgn_high_bits).is_err());

        let mut bad_policy = valid.clone();
        bad_policy[3] = (bad_policy[3] & !0x03) | 0x03;
        assert!(FilterRule::decode(&bad_policy).is_err());

        let mut bad_reserved_flags = valid.clone();
        bad_reserved_flags[3] |= 0x40;
        assert!(FilterRule::decode(&bad_reserved_flags).is_err());

        let mut hidden_source = valid.clone();
        hidden_source[4] = 0x00;
        assert!(FilterRule::decode(&hidden_source).is_err());

        let mut hidden_dest = valid;
        hidden_dest[12] = 0x00;
        assert!(FilterRule::decode(&hidden_dest).is_err());
    }

    #[test]
    fn niu_filter_snapshot_is_deterministic_and_excludes_rate_runtime_state() {
        let mut niu = Niu::new(NiuConfig::default());
        let source = Name::default().with_identity_number(0x1234);
        niu.add_filter(
            FilterRule::new(PGN_DM1, ForwardPolicy::Block, true).with_source_name(source),
        );
        niu.add_filter(
            FilterRule::new(PGN_HEARTBEAT, ForwardPolicy::Allow, true)
                .with_max_frequency_ms(100)
                .persistent(true),
        );

        let before = niu.filter_snapshot();
        let mut sorted_pgns: Vec<_> = before.iter().map(|snapshot| snapshot.pgn).collect();
        sorted_pgns.sort_unstable();
        assert_eq!(
            before
                .iter()
                .map(|snapshot| snapshot.pgn)
                .collect::<Vec<_>>(),
            sorted_pgns
        );
        assert!(before.iter().any(|snapshot| {
            snapshot.pgn == PGN_HEARTBEAT
                && snapshot.policy == ForwardPolicy::Allow
                && snapshot.max_frequency_ms == 100
                && snapshot.persistent
        }));

        niu.start().unwrap();
        assert!(
            niu.process_frame(make_frame(PGN_HEARTBEAT, 0x10, 0xFF), Side::Tractor, 10)
                .is_some()
        );
        assert!(niu.filters()[1].last_forward_time_ms.is_some());

        let after = niu.filter_snapshot();
        assert_eq!(after, before);
    }

    #[test]
    fn niu_policy_snapshot_captures_config_and_excludes_runtime_state() {
        let mut niu = Niu::new(
            NiuConfig::default()
                .name("audit-niu")
                .global_default(false)
                .specific_default(true)
                .loop_guard_window_ms(750)
                .persistence("niu-policy.rules"),
        );
        niu.add_filter(
            FilterRule::new(PGN_HEARTBEAT, ForwardPolicy::Allow, true)
                .with_max_frequency_ms(100)
                .persistent(true),
        );

        let before = niu.policy_snapshot();
        assert_eq!(before.name, "audit-niu");
        assert_eq!(before.filter_mode, NiuFilterMode::PassAll);
        assert!(!before.forward_global_by_default);
        assert!(before.forward_specific_by_default);
        assert_eq!(before.loop_guard_window_ms, 750);
        assert_eq!(
            before.loop_guard_max_recent_forwards,
            DEFAULT_LOOP_GUARD_MAX_RECENT_FORWARDS
        );
        assert_eq!(before.persistence_file.as_deref(), Some("niu-policy.rules"));
        assert_eq!(before.filters.len(), 1);
        assert_eq!(before.filters[0].pgn, PGN_HEARTBEAT);
        assert!(before.filters[0].persistent);

        niu.start().unwrap();
        assert!(
            niu.process_frame(
                make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS),
                Side::Tractor,
                0
            )
            .is_some()
        );
        assert!(
            niu.process_frame(
                make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS),
                Side::Tractor,
                50
            )
            .is_none(),
            "runtime rate limiter state must not appear in the policy dump"
        );
        assert!(
            niu.process_frame(
                make_address_claim(0x11, Name::default().with_identity_number(0x5678)),
                Side::Tractor,
                60,
            )
            .is_none(),
            "learned NAME state is runtime state, not policy"
        );
        assert_eq!(niu.policy_snapshot(), before);
    }

    #[test]
    fn niu_msg_round_trips_through_encode_decode() {
        let msg = NiuNetworkMsg {
            function: NiuFunction::PortStatsResponse,
            port_number: 1,
            msgs_forwarded: 0xCAFE,
            msgs_blocked: 0xBEEF,
            ..Default::default()
        };
        let bytes = msg.encode().unwrap();
        let decoded = NiuNetworkMsg::decode(&bytes).unwrap();
        assert_eq!(decoded.function, NiuFunction::PortStatsResponse);
        assert_eq!(decoded.port_number, 1);
        assert_eq!(decoded.msgs_forwarded, 0xCAFE);
        assert_eq!(decoded.msgs_blocked, 0xBEEF);
    }

    #[test]
    fn niu_msg_encode_rejects_invalid_filter_pgn_and_saturates_stats() {
        let invalid = NiuNetworkMsg {
            function: NiuFunction::AddFilterEntry,
            filter_pgn: 0x40000,
            ..Default::default()
        };
        let err = invalid.encode().unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));

        let stats = NiuNetworkMsg {
            function: NiuFunction::PortStatsResponse,
            msgs_forwarded: u32::MAX,
            msgs_blocked: u32::from(u16::MAX) + 1,
            ..Default::default()
        };
        let decoded = NiuNetworkMsg::decode(&stats.encode().unwrap()).unwrap();
        assert_eq!(decoded.msgs_forwarded, u32::from(u16::MAX));
        assert_eq!(decoded.msgs_blocked, u32::from(u16::MAX));
    }

    #[test]
    fn niu_msg_try_decode_rejects_malformed_payloads() {
        assert!(NiuNetworkMsg::try_decode(&[0x04, 0x01]).is_none());
        assert!(
            NiuNetworkMsg::try_decode(&[0x04, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF])
                .is_none()
        );
        assert!(
            NiuNetworkMsg::try_decode(&[0x99, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_none()
        );
        assert!(
            NiuNetworkMsg::try_decode(&[0x02, 0x01, 0x00, 0xEF, 0x04, 0xFF, 0xFF, 0xFF]).is_none()
        );
        assert!(
            NiuNetworkMsg::try_decode(&[0x02, 0x01, 0x00, 0xEF, 0x00, 0x00, 0xFF, 0xFF]).is_none()
        );
        assert!(
            NiuNetworkMsg::try_decode(&[0x06, 0x01, 0x02, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_none()
        );
        assert!(
            NiuNetworkMsg::try_decode(&[0x06, 0x01, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]).is_none()
        );
        assert!(
            NiuNetworkMsg::try_decode(&[0x0D, 0x02, 0x34, 0x12, 0xCD, 0xAB, 0x00, 0xFF]).is_none()
        );
    }

    #[test]
    fn handle_add_filter_message_grows_filter_db() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.start().unwrap();
        let mut payload = NiuNetworkMsg {
            function: NiuFunction::AddFilterEntry,
            filter_pgn: PGN_HEARTBEAT,
            ..Default::default()
        }
        .encode()
        .unwrap()
        .to_vec();
        // Pad to make a valid Message (the function only inspects the first 5 bytes here).
        payload.resize(8, 0xFF);
        let msg = Message::new(PGN_NIU_NETWORK_MSG, payload.clone(), 0x10);
        niu.handle_niu_message(&Message::new(PGN_HEARTBEAT, payload.clone(), 0x10));
        assert!(
            niu.filters().is_empty(),
            "control payloads on non-NIU PGNs must be ignored"
        );

        niu.handle_niu_message(&msg);
        assert_eq!(niu.filters().len(), 1);
        assert_eq!(niu.filters()[0].pgn, PGN_HEARTBEAT);
    }

    #[test]
    fn handle_niu_message_ignores_malformed_payloads() {
        let mut niu = Niu::new(NiuConfig::default());
        niu.start().unwrap();
        let captured: Rc<RefCell<Vec<(NiuNetworkMsg, Address)>>> =
            Rc::new(RefCell::new(Vec::new()));
        let captured_events = captured.clone();
        niu.on_niu_message
            .subscribe(move |event| captured_events.borrow_mut().push(*event));

        for payload in [
            vec![0x02, 0x01, 0x00, 0xEF],
            vec![0x02, 0x01, 0x00, 0xEF, 0x04, 0xFF, 0xFF, 0xFF],
            vec![0x02, 0x01, 0x00, 0xEF, 0x00, 0x00, 0xFF, 0xFF],
            vec![0x06, 0x01, 0x02, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            vec![0x99, 0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        ] {
            niu.handle_niu_message(&Message::new(PGN_NIU_NETWORK_MSG, payload, 0x10));
        }

        assert!(niu.filters().is_empty());
        assert!(captured.borrow().is_empty());
        assert_eq!(niu.filter_mode(), NiuFilterMode::PassAll);
    }

    #[test]
    fn router_translates_source_address_for_known_entries() {
        let mut router = Router::new(NiuConfig::default());
        router.niu_mut().start().unwrap();
        // Same NAME has tractor address 0x10, implement address 0x20.
        let name = Name::default().with_identity_number(0x100);
        router.add_translation(name, 0x10, 0x20).unwrap();

        // Broadcast frame from tractor side, source 0x10 → translated to 0x20.
        let f = make_frame(PGN_HEARTBEAT, 0x10, 0xFF);
        let out = router
            .process_frame(f, Side::Tractor, 0)
            .expect("broadcast forwards");
        assert_eq!(out.source(), 0x20);
        assert_eq!(out.pgn(), PGN_HEARTBEAT);
    }

    #[test]
    fn router_blocks_destination_specific_without_dest_translation() {
        let mut router = Router::new(NiuConfig::default());
        router.niu_mut().start().unwrap();
        let name = Name::default().with_identity_number(0x100);
        router.add_translation(name, 0x10, 0x20).unwrap();

        // Destination-specific frame with unknown destination.
        let f = make_frame(PGN_REQUEST, 0x10, 0x42);
        assert!(router.process_frame(f, Side::Tractor, 0).is_none());
        assert_eq!(router.niu().blocked(), 1);
    }

    #[test]
    fn router_translates_both_source_and_destination() {
        let mut router = Router::new(NiuConfig::default());
        router.niu_mut().start().unwrap();
        let n1 = Name::default().with_identity_number(0x100);
        let n2 = Name::default().with_identity_number(0x200);
        router.add_translation(n1, 0x10, 0x20).unwrap();
        router.add_translation(n2, 0x42, 0x52).unwrap();

        let f = make_frame(PGN_REQUEST, 0x10, 0x42);
        let out = router.process_frame(f, Side::Tractor, 0).expect("forwards");
        assert_eq!(out.source(), 0x20);
        assert_eq!(out.destination(), 0x52);
    }

    #[test]
    fn router_loop_guard_remembers_translated_output_frame() {
        let mut router = Router::new(NiuConfig::default());
        router.niu_mut().start().unwrap();
        let name = Name::default().with_identity_number(0x100);
        router.add_translation(name, 0x10, 0x20).unwrap();

        let f = make_frame(PGN_HEARTBEAT, 0x10, BROADCAST_ADDRESS);
        let out = router
            .process_frame(f, Side::Tractor, 100)
            .expect("forwards");
        assert_eq!(out.source(), 0x20);

        assert!(
            router.process_frame(out, Side::Implement, 101).is_none(),
            "the translated frame returning from the destination side is blocked as a loop echo"
        );
        assert_eq!(router.niu().blocked(), 1);
    }

    #[test]
    fn translation_db_entries_replace_on_duplicate_name() {
        let mut db = AddressTranslationDb::new();
        let name = Name::default().with_identity_number(0x100);
        db.add(name, 0x10, 0x20).unwrap();
        db.add(name, 0x11, 0x21).unwrap();
        assert_eq!(db.entries().len(), 1);
        assert_eq!(db.translate(0x11, Side::Tractor), Some(0x21));
    }

    #[test]
    fn translation_db_rejects_side_local_address_conflicts() {
        let mut db = AddressTranslationDb::new();
        let n1 = Name::default().with_identity_number(0x100);
        let n2 = Name::default().with_identity_number(0x200);
        let n3 = Name::default().with_identity_number(0x300);
        db.add(n1, 0x10, 0x20).unwrap();

        let tractor_conflict = db.add(n2, 0x10, 0x21).unwrap_err();
        assert_eq!(tractor_conflict.code, ErrorCode::AddressConflict);
        assert!(
            tractor_conflict
                .message
                .contains("tractor-side address 0x10")
        );

        let implement_conflict = db.add(n3, 0x11, 0x20).unwrap_err();
        assert_eq!(implement_conflict.code, ErrorCode::AddressConflict);
        assert!(
            implement_conflict
                .message
                .contains("implement-side address 0x20")
        );

        assert_eq!(db.entries().len(), 1);
        assert_eq!(db.translate(0x10, Side::Tractor), Some(0x20));
    }

    #[test]
    fn translation_db_rejects_reserved_addresses() {
        let mut db = AddressTranslationDb::new();
        let name = Name::default().with_identity_number(0x100);

        for bad_addr in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let err = db.add(name, bad_addr, 0x20).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidAddress);

            let err = db.add(name, 0x10, bad_addr).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidAddress);

            assert!(!db.is_address_available(bad_addr, Side::Tractor));
            assert!(!db.is_address_available(bad_addr, Side::Implement));
        }
        assert!(db.entries().is_empty());
    }

    #[test]
    fn router_add_translation_propagates_conflict_errors() {
        let mut router = Router::new(NiuConfig::default());
        let n1 = Name::default().with_identity_number(0x100);
        let n2 = Name::default().with_identity_number(0x200);

        router.add_translation(n1, 0x10, 0x20).unwrap();
        let err = router.add_translation(n2, 0x10, 0x21).unwrap_err();
        assert_eq!(err.code, ErrorCode::AddressConflict);
        assert_eq!(router.translation_db().entries().len(), 1);
    }

    #[test]
    fn router_policy_snapshot_combines_filters_and_translations() {
        let mut router = Router::new(
            NiuConfig::default()
                .name("audit-router")
                .mode(NiuFilterMode::BlockAll)
                .global_default(false)
                .specific_default(false)
                .loop_guard_window_ms(900)
                .persistence("router-policy.rules"),
        );
        let n2 = Name::default().with_identity_number(0x200);
        let n1 = Name::default().with_identity_number(0x100);
        router.niu_mut().block_pgn(PGN_DM1, true);
        router.niu_mut().allow_pgn(PGN_HEARTBEAT, true);
        router.add_translation(n2, 0x20, 0x30).unwrap();
        router.add_translation(n1, 0x10, 0x21).unwrap();

        let snapshot = router.policy_snapshot();
        assert_eq!(snapshot.niu.name, "audit-router");
        assert_eq!(snapshot.niu.filter_mode, NiuFilterMode::BlockAll);
        assert!(!snapshot.niu.forward_global_by_default);
        assert!(!snapshot.niu.forward_specific_by_default);
        assert_eq!(snapshot.niu.loop_guard_window_ms, 900);
        assert_eq!(
            snapshot.niu.loop_guard_max_recent_forwards,
            DEFAULT_LOOP_GUARD_MAX_RECENT_FORWARDS
        );
        assert_eq!(
            snapshot.niu.persistence_file.as_deref(),
            Some("router-policy.rules")
        );
        assert_eq!(snapshot.niu.filters, snapshot.filters);
        assert_eq!(snapshot.filters.len(), 2);
        assert_eq!(snapshot.filters[0].pgn, PGN_HEARTBEAT);
        assert_eq!(snapshot.filters[1].pgn, PGN_DM1);
        assert_eq!(snapshot.translations.len(), 2);
        assert_eq!(snapshot.translations[0].name, n1);
        assert_eq!(snapshot.translations[1].name, n2);
    }

    #[test]
    fn translation_db_snapshot_is_active_and_deterministic() {
        let mut db = AddressTranslationDb::new();
        let n3 = Name::default().with_identity_number(0x300);
        let n1 = Name::default().with_identity_number(0x100);
        let n2 = Name::default().with_identity_number(0x200);

        db.add(n3, 0x30, 0x40).unwrap();
        db.add(n1, 0x10, 0x20).unwrap();
        db.add(n2, 0x21, 0x31).unwrap();
        db.remove(n3);

        let snapshot = db.snapshot();
        assert_eq!(snapshot.len(), 2);
        assert_eq!(snapshot[0].name, n1);
        assert_eq!(snapshot[1].name, n2);
        assert_eq!(snapshot[0].tractor_address, 0x10);
        assert_eq!(snapshot[1].tractor_address, 0x21);
    }

    #[test]
    fn address_table_basic_learn_and_lookup() {
        let mut table = AddressTable::new();
        assert!(table.is_empty());
        table.learn(0x10, Side::Tractor);
        table.learn(0x42, Side::Implement);
        assert_eq!(table.lookup(0x10), Some(Side::Tractor));
        assert_eq!(table.lookup(0x42), Some(Side::Implement));
        assert_eq!(table.lookup(0xFF), None);
        assert_eq!(table.len(), 2);
    }
}
