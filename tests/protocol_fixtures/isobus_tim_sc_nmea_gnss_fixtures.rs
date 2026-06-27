#[test]
fn fixture_isobus_tim_codecs_are_stable() {
    let pto = PtoState {
        engaged: true,
        cw_direction: false,
        speed: 540,
    };
    let pto_bytes = parse_named_hex_frame(ISOBUS_TIM_CODECS_HEX, "pto_engaged_ccw_540rpm");
    assert_eq!(pto.encode(), pto_bytes);
    for pgn in [PGN_FRONT_PTO, PGN_REAR_PTO] {
        assert_eq!(
            PtoState::decode(&Message::new(pgn, pto_bytes.to_vec(), 0x80)).unwrap(),
            pto
        );
    }
    let pto_max = PtoState {
        engaged: false,
        cw_direction: true,
        speed: u16::MAX,
    };
    let pto_max_bytes = parse_named_hex_frame(ISOBUS_TIM_CODECS_HEX, "pto_disengaged_cw_maxrpm");
    assert_eq!(pto_max.encode(), pto_max_bytes);
    for pgn in [PGN_FRONT_PTO, PGN_REAR_PTO] {
        assert_eq!(
            PtoState::decode(&Message::new(pgn, pto_max_bytes.to_vec(), 0x80)).unwrap(),
            pto_max
        );
    }
    for malformed in [
        "pto_short3",
        "pto_overlong9",
        "pto_bad_padding",
        "pto_bad_engaged_byte",
        "pto_bad_direction_byte",
    ] {
        assert!(
            PtoState::decode(&Message::new(
                PGN_FRONT_PTO,
                parse_named_hex_bytes(ISOBUS_TIM_CODECS_HEX, malformed),
                0x80
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let hitch = HitchState {
        motion_enabled: true,
        position: 7500,
    };
    let hitch_bytes = parse_named_hex_frame(ISOBUS_TIM_CODECS_HEX, "hitch_motion_enabled_75pct");
    assert_eq!(hitch.encode(), hitch_bytes);
    for pgn in [PGN_FRONT_HITCH, PGN_REAR_HITCH] {
        assert_eq!(
            HitchState::decode(&Message::new(pgn, hitch_bytes.to_vec(), 0x80)).unwrap(),
            hitch
        );
    }
    let hitch_min = HitchState {
        motion_enabled: false,
        position: 0,
    };
    let hitch_min_bytes = parse_named_hex_frame(ISOBUS_TIM_CODECS_HEX, "hitch_disabled_min");
    assert_eq!(hitch_min.encode(), hitch_min_bytes);
    for pgn in [PGN_FRONT_HITCH, PGN_REAR_HITCH] {
        assert_eq!(
            HitchState::decode(&Message::new(pgn, hitch_min_bytes.to_vec(), 0x80)).unwrap(),
            hitch_min
        );
    }
    let hitch_max = HitchState {
        motion_enabled: true,
        position: MAX_HITCH_POSITION,
    };
    let hitch_max_bytes = parse_named_hex_frame(ISOBUS_TIM_CODECS_HEX, "hitch_enabled_max");
    assert_eq!(hitch_max.try_encode().unwrap(), hitch_max_bytes);
    for pgn in [PGN_FRONT_HITCH, PGN_REAR_HITCH] {
        assert_eq!(
            HitchState::decode(&Message::new(pgn, hitch_max_bytes.to_vec(), 0x80)).unwrap(),
            hitch_max
        );
    }
    assert!(
        HitchState {
            motion_enabled: true,
            position: MAX_HITCH_POSITION + 1,
        }
        .try_encode()
        .is_err()
    );
    assert!(
        HitchState::decode(&Message::new(
            PGN_REAR_HITCH,
            parse_named_hex_bytes(ISOBUS_TIM_CODECS_HEX, "hitch_over_max"),
            0x80
        ))
        .is_none()
    );
    for malformed in [
        "hitch_short2",
        "hitch_overlong9",
        "hitch_bad_padding",
        "hitch_bad_motion_byte",
    ] {
        assert!(
            HitchState::decode(&Message::new(
                PGN_REAR_HITCH,
                parse_named_hex_bytes(ISOBUS_TIM_CODECS_HEX, malformed),
                0x80
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let aux = AuxValveCommand {
        index: 3,
        state: true,
        flow: 5000,
    };
    let aux_bytes = parse_named_hex_frame(ISOBUS_TIM_CODECS_HEX, "aux_valve_3_on_flow5000");
    assert_eq!(aux.encode(), aux_bytes);
    assert_eq!(
        AuxValveCommand::decode(&Message::new(PGN_AUX_VALVE_0_7, aux_bytes.to_vec(), 0x80))
            .unwrap(),
        aux
    );
    let aux_max = AuxValveCommand {
        index: MAX_AUX_VALVES - 1,
        state: false,
        flow: u16::MAX,
    };
    let aux_max_bytes = parse_named_hex_frame(ISOBUS_TIM_CODECS_HEX, "aux_valve_31_off_max_flow");
    assert_eq!(aux_max.try_encode().unwrap(), aux_max_bytes);
    assert_eq!(
        AuxValveCommand::decode(&Message::new(
            PGN_AUX_VALVE_24_31,
            aux_max_bytes.to_vec(),
            0x80
        ))
        .unwrap(),
        aux_max
    );
    for malformed in [
        "aux_valve_short3",
        "aux_valve_overlong9",
        "aux_valve_bad_padding",
        "aux_valve_invalid_index",
        "aux_valve_bad_state_byte",
    ] {
        assert!(
            AuxValveCommand::decode(&Message::new(
                PGN_AUX_VALVE_0_7,
                parse_named_hex_bytes(ISOBUS_TIM_CODECS_HEX, malformed),
                0x80
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    assert_eq!(
        [
            TimOption::FrontPtoDisengagementIsSupported.bit(),
            TimOption::RearHitchPositionIsSupported.bit(),
            TimOption::GuidanceCurvatureIsSupported.bit(),
        ],
        [0, 13, 21]
    );

    let options = parse_named_hex_bytes(
        ISOBUS_TIM_CODECS_HEX,
        "option_front_pto_rear_hitch_guidance",
    );
    let option_set = TimOptionSet::from_options(&[
        TimOption::FrontPtoDisengagementIsSupported,
        TimOption::RearHitchPositionIsSupported,
        TimOption::GuidanceCurvatureIsSupported,
    ]);
    assert_eq!(option_set.as_bytes().as_slice(), options.as_slice());
    let requested = TimOptionSet::from_options(&[
        TimOption::RearHitchPositionIsSupported,
        TimOption::GuidanceCurvatureIsSupported,
    ]);
    assert!(requested.is_subset_of(&option_set));
    let unavailable = TimOptionSet::from_options(&[TimOption::RearPtoSpeedCwIsSupported]);
    assert!(
        unavailable
            .missing_from(&option_set)
            .contains(TimOption::RearPtoSpeedCwIsSupported)
    );

    let mut authority = TimAuthority::new(option_set);
    assert_eq!(
        authority.ensure_command(TimCommand::RearHitchPosition),
        Err(TimValidationError::OptionNotRequested {
            option: TimOption::RearHitchPositionIsSupported
        })
    );
    authority.request(requested).unwrap();
    assert_eq!(
        authority.ensure_command(TimCommand::RearHitchPosition),
        Err(TimValidationError::AuthorityNotGranted {
            state: TimAuthorityState::Requested
        })
    );
    authority.grant().unwrap();
    assert!(
        authority
            .ensure_command(TimCommand::RearHitchPosition)
            .is_ok()
    );
    assert!(
        authority
            .ensure_command(TimCommand::GuidanceCurvature)
            .is_ok()
    );
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoSpeedCw),
        Err(TimValidationError::UnsupportedOption {
            option: TimOption::RearPtoSpeedCwIsSupported
        })
    );
    authority.set_interlocks(TimInterlocks::all_clear().with_road_transport_mode(true));
    assert_eq!(authority.state(), TimAuthorityState::Revoked);
    assert_eq!(
        authority.ensure_command(TimCommand::RearHitchPosition),
        Err(TimValidationError::InterlockActive {
            interlock: TimInterlock::RoadTransportMode
        })
    );
    authority.set_interlocks(TimInterlocks::all_clear());
    assert_eq!(
        authority.ensure_command(TimCommand::RearHitchPosition),
        Err(TimValidationError::AuthorityNotGranted {
            state: TimAuthorityState::Revoked
        })
    );
    authority.request(requested).unwrap();
    authority.grant().unwrap();
    assert!(
        authority
            .ensure_command(TimCommand::RearHitchPosition)
            .is_ok()
    );
}

#[test]
fn fixture_isobus_sc_master_client_status_vectors_are_stable() {
    let master_ready = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_ready");
    let master_ready_busy = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_ready_busy_flags");
    let master_pause = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_pause_after_playback");
    let client_ready = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_ready");
    let client_paused = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_paused_ready");
    let master_playback = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_playback_step7");
    let client_playback = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_step7");
    let master_playback_zero = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_playback_step0");
    let client_playback_zero = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_step0");
    let master_playback_reserved_step =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_playback_step_ff");
    let client_playback_reserved_step =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_step_ff");
    let client_playback_wrong_step =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_wrong_step0");
    let master_playback_max = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_playback_step49");
    let client_playback_max = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_step49");

    let master_cfg = SCMasterConfig::default()
        .with_status_interval(100)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(1_000_000);

    let mut malformed_master = SCMaster::new(master_cfg);
    malformed_master.add_step(sc_step(7)).unwrap();
    malformed_master.start().unwrap();
    for malformed in [
        "client_ready_wrong_code",
        "client_ready_short7",
        "client_ready_overlong9",
        "client_ready_step7",
        "client_recording_step7",
        "client_recording_completion_step7",
        "client_reserved_state_ready",
        "client_ready_bad_func_error",
        "client_ready_bad_tail",
    ] {
        malformed_master.handle_client_status(&Message::new(
            PGN_SC_CLIENT_STATUS,
            parse_named_hex_bytes(ISOBUS_SC_STATUS_HEX, malformed),
            0x20,
        ));
        assert!(
            malformed_master.is(SCState::Ready),
            "{malformed} must be ignored instead of prefix-decoded"
        );
    }
    malformed_master.handle_client_status(&Message::new(
        PGN_SC_MASTER_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(
        malformed_master.is(SCState::Ready),
        "SC master must ignore a client-status payload delivered under the wrong PGN"
    );
    for invalid_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        malformed_master.handle_client_status(&Message::new(
            PGN_SC_CLIENT_STATUS,
            client_ready.to_vec(),
            invalid_source,
        ));
        assert!(
            malformed_master.is(SCState::Ready),
            "SC master must ignore client status from invalid source 0x{invalid_source:02X}"
        );
    }

    let mut malformed_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    for malformed in [
        "master_ready_wrong_code",
        "master_ready_short7",
        "master_ready_overlong9",
        "master_ready_step7",
        "master_recording_step7",
        "master_recording_completion_step7",
        "master_reserved_state_ready",
        "master_ready_bad_busy_flags",
        "master_ready_bad_tail",
    ] {
        assert!(
            malformed_client
                .handle_master_status(&Message::new(
                    PGN_SC_MASTER_STATUS,
                    parse_named_hex_bytes(ISOBUS_SC_STATUS_HEX, malformed),
                    0x10,
                ))
                .is_none(),
            "{malformed} must not emit a client response"
        );
        assert!(malformed_client.is(SCState::Idle));
    }
    assert!(
        malformed_client
            .handle_master_status(&Message::new(
                PGN_SC_CLIENT_STATUS,
                master_ready.to_vec(),
                0x10,
            ))
            .is_none(),
        "SC client must ignore a master-status payload delivered under the wrong PGN"
    );
    for invalid_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert!(
            malformed_client
                .handle_master_status(&Message::new(
                    PGN_SC_MASTER_STATUS,
                    master_ready.to_vec(),
                    invalid_source,
                ))
                .is_none(),
            "SC client must ignore master status from invalid source 0x{invalid_source:02X}"
        );
        assert!(malformed_client.is(SCState::Idle));
    }
    let _ = malformed_client.handle_master_status(&Message::new(
        PGN_SC_MASTER_STATUS,
        master_ready.to_vec(),
        0x10,
    ));
    assert!(malformed_client.is(SCState::Ready));
    assert!(
        malformed_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_playback_reserved_step.to_vec(),
                0x10,
            ))
            .is_none(),
        "master PlayBack with 0xFF reserved step id must not emit a client response"
    );
    assert!(malformed_client.is(SCState::Ready));
    assert!(
        malformed_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                parse_named_hex_bytes(ISOBUS_SC_STATUS_HEX, "master_reserved_state_playback7"),
                0x10,
            ))
            .is_none(),
        "reserved master-state bytes must not be coerced into Idle or PlayBack"
    );
    assert!(malformed_client.is(SCState::Ready));

    let mut master = SCMaster::new(master_cfg);
    master.add_step(sc_step(7)).unwrap();
    master.start().unwrap();
    assert_eq!(master.update(100).unwrap(), master_ready);
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_SC_MASTER_STATUS,
            0x10,
            BROADCAST_ADDRESS,
            &master_ready,
        )
        .pgn(),
        PGN_SC_MASTER_STATUS
    );

    let mut busy_master = SCMaster::new(master_cfg);
    busy_master.add_step(sc_step(7)).unwrap();
    busy_master.start().unwrap();
    busy_master.set_busy_nv_memory(true);
    busy_master.set_busy_parsing_scd(true);
    assert_eq!(busy_master.update(100).unwrap(), master_ready_busy);

    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let ready_response = client
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_ready.to_vec(),
            0x10,
        ))
        .expect("ready transition emits client status");
    assert_eq!(ready_response, client_ready);
    assert!(client.is(SCState::Ready));

    master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(master.is(SCState::Active));
    assert_eq!(master.update(100).unwrap(), master_playback);

    let playback_response = client
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_playback.to_vec(),
            0x10,
        ))
        .expect("playback transition emits client status");
    assert_eq!(playback_response, client_playback);
    assert!(client.is(SCState::Active));

    master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_playback.to_vec(),
        0x20,
    ));
    assert!(master.is(SCState::Active));

    master.pause().unwrap();
    assert!(master.is(SCState::Paused));
    assert_eq!(master.update(100).unwrap(), master_pause);
    let paused_response = client
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_pause.to_vec(),
            0x10,
        ))
        .expect("pause transition emits client status");
    assert_eq!(paused_response, client_paused);
    assert!(client.is(SCState::Paused));

    master.resume().unwrap();
    assert!(master.is(SCState::Active));
    assert_eq!(master.update(100).unwrap(), master_playback);
    let resumed_response = client
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_playback.to_vec(),
            0x10,
        ))
        .expect("resume transition emits client status");
    assert_eq!(resumed_response, client_playback);
    assert!(client.is(SCState::Active));

    let mut guarded_master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(100),
    );
    guarded_master.add_step(sc_step(7)).unwrap();
    guarded_master.start().unwrap();
    guarded_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(guarded_master.is(SCState::Active));
    guarded_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_playback_wrong_step.to_vec(),
        0x20,
    ));
    guarded_master.update(150);
    assert!(guarded_master.is(SCState::Error));

    let mut reserved_step_master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(100),
    );
    reserved_step_master.add_step(sc_step(7)).unwrap();
    reserved_step_master.start().unwrap();
    reserved_step_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(reserved_step_master.is(SCState::Active));
    reserved_step_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_playback_reserved_step.to_vec(),
        0x20,
    ));
    reserved_step_master.update(150);
    assert!(reserved_step_master.is(SCState::Error));

    let mut reserved_state_master = SCMaster::new(
        SCMasterConfig::default()
            .with_status_interval(100)
            .with_ready_timeout(1_000_000)
            .with_active_timeout(100),
    );
    reserved_state_master.add_step(sc_step(7)).unwrap();
    reserved_state_master.start().unwrap();
    reserved_state_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(reserved_state_master.is(SCState::Active));
    reserved_state_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        parse_named_hex_bytes(ISOBUS_SC_STATUS_HEX, "client_reserved_state_playback7"),
        0x20,
    ));
    reserved_state_master.update(150);
    assert!(reserved_state_master.is(SCState::Error));

    let mut max_master = SCMaster::new(master_cfg);
    max_master
        .add_step(sc_step(SC_MAX_SEQUENCE_STEP_ID))
        .unwrap();
    max_master.start().unwrap();
    max_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(max_master.is(SCState::Active));
    assert_eq!(max_master.update(100).unwrap(), master_playback_max);

    let mut max_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let _ = max_client.handle_master_status(&Message::new(
        PGN_SC_MASTER_STATUS,
        master_ready.to_vec(),
        0x10,
    ));
    let max_response = max_client
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_playback_max.to_vec(),
            0x10,
        ))
        .expect("max step playback emits client status");
    assert_eq!(max_response, client_playback_max);

    let mut zero_master = SCMaster::new(master_cfg);
    zero_master.add_step(sc_step(0)).unwrap();
    zero_master.start().unwrap();
    zero_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(zero_master.is(SCState::Active));
    assert_eq!(zero_master.update(100).unwrap(), master_playback_zero);

    let mut zero_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let _ = zero_client.handle_master_status(&Message::new(
        PGN_SC_MASTER_STATUS,
        master_ready.to_vec(),
        0x10,
    ));
    let zero_response = zero_client
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_playback_zero.to_vec(),
            0x10,
        ))
        .expect("configured zero step playback emits client status");
    assert_eq!(zero_response, client_playback_zero);

    let two_client_cfg = SCMasterConfig::default()
        .with_status_interval(100)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(1_000_000)
        .with_required_client_count(2);
    let mut two_client_master = SCMaster::new(two_client_cfg);
    two_client_master.add_step(sc_step(7)).unwrap();
    two_client_master.start().unwrap();
    assert_eq!(two_client_master.update(100).unwrap(), master_ready);

    let mut client_a = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let mut client_b = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    assert_eq!(
        client_a
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_ready.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_ready
    );
    assert_eq!(
        client_b
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_ready.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_ready
    );

    two_client_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(
        two_client_master.is(SCState::Ready),
        "configured two-client sequence must not start after one Ready status"
    );
    two_client_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x21,
    ));
    assert!(two_client_master.is(SCState::Active));
    assert_eq!(two_client_master.update(100).unwrap(), master_playback);

    let client_a_playback = client_a
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_playback.to_vec(),
            0x10,
        ))
        .unwrap();
    let client_b_playback = client_b
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_playback.to_vec(),
            0x10,
        ))
        .unwrap();
    assert_eq!(client_a_playback, client_playback);
    assert_eq!(client_b_playback, client_playback);
    two_client_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_a_playback.to_vec(),
        0x20,
    ));
    two_client_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_b_playback.to_vec(),
        0x21,
    ));
    assert!(two_client_master.is(SCState::Active));
}

#[test]
fn fixture_isobus_sc_multistep_and_abort_lifecycle_is_stable() {
    let master_ready = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_ready");
    let master_pause = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_pause_after_playback");
    let client_ready = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_ready");
    let client_paused = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_paused_ready");
    let master_playback_step8 =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_playback_step8");
    let client_playback_step8 =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_step8");
    let master_playback_step9 =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_playback_step9");
    let client_playback_step9 =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_step9");
    let master_playback_step7 =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_playback_step7");
    let client_playback_step7 =
        parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_playback_step7");
    let master_abort_step7 = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "master_abort_step7");
    let client_abort_step7 = parse_named_hex_frame(ISOBUS_SC_STATUS_HEX, "client_abort_step7");
    let timeout_master_cfg = SCMasterConfig::default()
        .with_status_interval(1_000_000)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(100);

    let master_cfg = SCMasterConfig::default()
        .with_status_interval(100)
        .with_ready_timeout(1_000_000)
        .with_active_timeout(1_000_000);
    let mut master = SCMaster::new(master_cfg);
    master.add_step(sc_step(8)).unwrap();
    master.add_step(sc_step(9)).unwrap();
    master.start().unwrap();
    assert_eq!(master.update(100).unwrap(), master_ready);

    let mut client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    assert_eq!(
        client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_ready.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_ready
    );
    master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert_eq!(master.update(100).unwrap(), master_playback_step8);
    assert_eq!(
        client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_playback_step8.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_playback_step8
    );
    master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_playback_step8.to_vec(),
        0x20,
    ));
    master.step_completed(8).unwrap();
    assert!(master.is(SCState::Active));
    assert_eq!(master.current_step().unwrap().step_id, 9);

    assert_eq!(master.update(100).unwrap(), master_playback_step9);
    assert_eq!(
        client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_playback_step9.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_playback_step9
    );
    master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_playback_step9.to_vec(),
        0x20,
    ));
    master.step_completed(9).unwrap();
    assert!(master.is(SCState::Complete));
    assert!(master.update(100).is_none());

    let mut paused_sequence_master = SCMaster::new(master_cfg);
    paused_sequence_master.add_step(sc_step(8)).unwrap();
    paused_sequence_master.add_step(sc_step(9)).unwrap();
    paused_sequence_master.start().unwrap();
    assert_eq!(paused_sequence_master.update(100).unwrap(), master_ready);

    let mut paused_sequence_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    assert_eq!(
        paused_sequence_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_ready.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_ready
    );
    paused_sequence_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert_eq!(
        paused_sequence_master.update(100).unwrap(),
        master_playback_step8
    );
    let paused_client_playback = paused_sequence_client
        .handle_master_status(&Message::new(
            PGN_SC_MASTER_STATUS,
            master_playback_step8.to_vec(),
            0x10,
        ))
        .unwrap();
    assert_eq!(paused_client_playback, client_playback_step8);
    paused_sequence_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        paused_client_playback.to_vec(),
        0x20,
    ));
    paused_sequence_master.pause().unwrap();
    assert_eq!(paused_sequence_master.update(100).unwrap(), master_pause);
    assert_eq!(
        paused_sequence_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_pause.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_paused
    );
    assert!(paused_sequence_client.is(SCState::Paused));
    paused_sequence_master.resume().unwrap();
    assert_eq!(
        paused_sequence_master.update(100).unwrap(),
        master_playback_step8
    );
    assert_eq!(
        paused_sequence_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_playback_step8.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_playback_step8
    );
    assert!(paused_sequence_client.is(SCState::Active));
    paused_sequence_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_playback_step8.to_vec(),
        0x20,
    ));
    paused_sequence_master.step_completed(8).unwrap();
    assert_eq!(
        paused_sequence_master.update(100).unwrap(),
        master_playback_step9
    );
    assert_eq!(
        paused_sequence_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_playback_step9.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_playback_step9
    );

    let mut aborting_master = SCMaster::new(master_cfg);
    aborting_master.add_step(sc_step(7)).unwrap();
    aborting_master.start().unwrap();
    aborting_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert_eq!(aborting_master.update(100).unwrap(), master_playback_step7);

    let mut aborting_client = SCClient::new(SCClientConfig::default().with_min_spacing(0));
    let _ = aborting_client.handle_master_status(&Message::new(
        PGN_SC_MASTER_STATUS,
        master_ready.to_vec(),
        0x10,
    ));
    assert_eq!(
        aborting_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_playback_step7.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_playback_step7
    );
    aborting_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_playback_step7.to_vec(),
        0x20,
    ));
    aborting_master.abort().unwrap();
    assert_eq!(aborting_master.update(0).unwrap(), master_abort_step7);
    assert_eq!(
        aborting_client
            .handle_master_status(&Message::new(
                PGN_SC_MASTER_STATUS,
                master_abort_step7.to_vec(),
                0x10,
            ))
            .unwrap(),
        client_abort_step7
    );
    assert!(aborting_client.is(SCState::Error));

    let mut active_timeout_master = SCMaster::new(timeout_master_cfg);
    active_timeout_master.add_step(sc_step(7)).unwrap();
    active_timeout_master.start().unwrap();
    active_timeout_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(active_timeout_master.is(SCState::Active));
    assert_eq!(
        active_timeout_master.update(150).unwrap(),
        master_abort_step7
    );
    assert!(active_timeout_master.is(SCState::Error));

    let mut busy_timeout_client = SCClient::new(
        SCClientConfig::default()
            .with_min_spacing(0)
            .with_busy_timeout(100),
    );
    let _ = busy_timeout_client.handle_master_status(&Message::new(
        PGN_SC_MASTER_STATUS,
        master_ready.to_vec(),
        0x10,
    ));
    let _ = busy_timeout_client.handle_master_status(&Message::new(
        PGN_SC_MASTER_STATUS,
        master_playback_step7.to_vec(),
        0x10,
    ));
    let _ = busy_timeout_client.set_busy(true);
    assert_eq!(busy_timeout_client.update(150).unwrap(), client_abort_step7);
    assert!(busy_timeout_client.is(SCState::Error));

    let mut client_abort_master = SCMaster::new(master_cfg);
    client_abort_master.add_step(sc_step(7)).unwrap();
    client_abort_master.start().unwrap();
    client_abort_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_ready.to_vec(),
        0x20,
    ));
    assert!(client_abort_master.is(SCState::Active));
    client_abort_master.handle_client_status(&Message::new(
        PGN_SC_CLIENT_STATUS,
        client_abort_step7.to_vec(),
        0x20,
    ));
    assert!(client_abort_master.is(SCState::Error));
    assert_eq!(client_abort_master.update(0).unwrap(), master_abort_step7);
}

#[test]
fn fixture_nmea_fast_packet_two_frame_reassembly_is_stable() {
    let pgn = 0x1F805;
    let id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let frame0 = Frame::new(id, *FP_10B_FRAME0, 8);
    let frame1 = Frame::new(id, *FP_10B_FRAME1, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&frame0).is_none());
    let msg = rx.process_frame(&frame1).expect("second frame completes");

    assert_eq!(msg.pgn, pgn);
    assert_eq!(msg.source, 0x23);
    assert_eq!(msg.data, (0u8..10).collect::<Vec<_>>());
}

#[test]
fn fixture_nmea_fast_packet_min_payload_reassembles() {
    let pgn = 0x1F805;
    let id = Identifier::encode(Priority::Default, pgn, 0x23, 0xFF);
    let frame0 = Frame::new(id, *FP_9B_FRAME0, 8);
    let frame1 = Frame::new(id, *FP_9B_FRAME1, 8);

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&frame0).is_none());
    let msg = rx.process_frame(&frame1).expect("second frame completes");

    assert_eq!(msg.pgn, pgn);
    assert_eq!(msg.source, 0x23);
    assert_eq!(msg.data, (0u8..9).collect::<Vec<_>>());
    assert_eq!(rx.rx_session_count(), 0);
}

#[test]
fn fixture_nmea_fast_packet_max_payload_has_golden_edges() {
    let pgn = 0x1F805;
    let payload = incrementing_payload(223);
    let mut tx = FastPacketProtocol::new();
    let frames = tx
        .send(pgn, &payload, 0x23)
        .expect("223-byte fast packet must send");

    assert_eq!(frames.len(), 32);
    assert_eq!(frames[0].data, *FP_223B_FRAME0);
    assert_eq!(frames[31].data, *FP_223B_FRAME31);

    let mut rx = FastPacketProtocol::new();
    let mut completed = None;
    for frame in &frames {
        completed = rx.process_frame(frame).or(completed);
    }
    let msg = completed.expect("max-size fast packet completes");
    assert_eq!(msg.pgn, pgn);
    assert_eq!(msg.source, 0x23);
    assert_eq!(msg.data, payload);
}

#[test]
fn fixture_nmea_fast_packet_rejects_invalid_pgn_before_identifier_normalization() {
    let mut tx = FastPacketProtocol::new();
    let err = tx
        .send(0x4_0000, &(0..9u8).collect::<Vec<_>>(), 0x23)
        .expect_err("Fast Packet must reject PGNs above the 18-bit wire range");
    assert_eq!(err.code, ErrorCode::InvalidPgn);
    assert!(tx.stats().is_empty());
}

#[test]
fn fixture_nmea_fast_packet_rejects_invalid_source_addresses() {
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let id = Identifier::encode(Priority::Default, 0x1F805, source, BROADCAST_ADDRESS);
        let first = Frame::new(id, *FP_10B_FRAME0, 8);
        let mut rx = FastPacketProtocol::new();

        assert!(rx.process_frame(&first).is_none());
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 1);
    }
}

#[test]
fn fixture_nmea_fast_packet_sats_in_view_public_pgn_stream_reassembles() {
    let payload = parse_named_hex_bytes(NMEA_GNSS_SATS_IN_VIEW_FAST_PACKET_HEX, "payload");
    let expected_frames = [
        parse_named_hex_frame(NMEA_GNSS_SATS_IN_VIEW_FAST_PACKET_HEX, "frame0"),
        parse_named_hex_frame(NMEA_GNSS_SATS_IN_VIEW_FAST_PACKET_HEX, "frame1"),
        parse_named_hex_frame(NMEA_GNSS_SATS_IN_VIEW_FAST_PACKET_HEX, "frame2"),
    ];

    let mut tx = FastPacketProtocol::new();
    let frames = tx
        .send(PGN_GNSS_SATELLITES_IN_VIEW, &payload, 0x23)
        .expect("PGN 129540 Satellites in View should use NMEA fast packet");
    assert_eq!(frames.len(), expected_frames.len());
    for (frame, expected_data) in frames.iter().zip(expected_frames) {
        assert_eq!(frame.pgn(), PGN_GNSS_SATELLITES_IN_VIEW);
        assert_eq!(frame.source(), 0x23);
        assert_eq!(frame.destination(), BROADCAST_ADDRESS);
        assert_eq!(frame.data, expected_data);
    }

    let mut rx = FastPacketProtocol::new();
    let mut completed = None;
    for frame in &frames {
        completed = rx.process_frame(frame).or(completed);
    }
    let msg = completed.expect("last fast-packet frame should complete PGN 129540");
    assert_eq!(msg.pgn, PGN_GNSS_SATELLITES_IN_VIEW);
    assert_eq!(msg.source, 0x23);
    assert_eq!(msg.destination, BROADCAST_ADDRESS);
    assert_eq!(msg.data, payload);
}

#[test]
fn fixture_nmea0183_serial_gnss_mixed_log_is_stable() {
    let mut parser = SerialGNSS::new();
    let positions: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
    let cogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let sogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let positions_log = positions.clone();
    let cogs_log = cogs.clone();
    let sogs_log = sogs.clone();
    parser
        .on_position
        .subscribe(move |position| positions_log.borrow_mut().push(*position));
    parser
        .on_cog
        .subscribe(move |cog| cogs_log.borrow_mut().push(*cog));
    parser
        .on_sog
        .subscribe(move |sog| sogs_log.borrow_mut().push(*sog));

    let bytes = NMEA0183_SERIAL_GNSS_MIXED_LOG.as_bytes();
    for chunk in bytes.chunks(17) {
        parser.feed_bytes(chunk);
    }

    let latest = parser
        .latest_position()
        .expect("fixture log must produce a GNSS fix");
    assert!((latest.wgs.latitude - 48.117_3).abs() < 0.000_001);
    assert!((latest.wgs.longitude - 11.516_666_666_7).abs() < 0.000_001);
    assert!((latest.wgs.altitude - 545.4).abs() < 0.001);
    assert_eq!(latest.fix_type, GNSSFixType::GNSSFix);
    assert_eq!(latest.satellites_used, 8);
    assert!((latest.hdop.unwrap() - 1.3).abs() < 0.000_001);
    assert!((latest.pdop.unwrap() - 2.5).abs() < 0.000_001);
    assert!((latest.vdop.unwrap() - 2.1).abs() < 0.000_001);
    assert!((latest.speed_mps.unwrap() - (10.2 / 3.6)).abs() < 0.000_001);
    assert!((latest.cog_rad.unwrap() - 54.7_f64.to_radians()).abs() < 0.000_001);
    assert_eq!(latest.timestamp_us, 45_319_000_000);
    assert_eq!(parser.latest_satellites_in_view(), Some(11));
    assert_eq!(
        parser.latest_utc_datetime(),
        Some(NmeaUtcDateTime {
            year: 2002,
            month: 7,
            day: 4,
            hour: 20,
            minute: 15,
            second: 30,
            microsecond: 0,
            local_zone_hours: 0,
            local_zone_minutes: 0,
        })
    );

    // The invalid-checksum GGA in the middle advertises 99 satellites and
    // bogus geometry. It must not overwrite the valid fix.
    assert_eq!(positions.borrow().len(), 3);
    assert!((positions.borrow()[0].wgs.latitude - 49.274_166_666_7).abs() < 0.000_001);
    assert_eq!(positions.borrow()[0].satellites_used, 0);
    assert_eq!(positions.borrow()[1].satellites_used, 8);
    assert_eq!(positions.borrow()[2].satellites_used, 8);

    // RMC and VTG both emit course/speed updates; the final cache must reflect
    // VTG because it appears last in the fixture.
    assert_eq!(cogs.borrow().len(), 2);
    assert_eq!(sogs.borrow().len(), 2);
}

#[test]
fn fixture_nmea0183_serial_gnss_garbage_soak_is_stable() {
    let mut parser = SerialGNSS::new();
    let positions: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
    let cogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let sogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let positions_log = positions.clone();
    let cogs_log = cogs.clone();
    let sogs_log = sogs.clone();
    parser
        .on_position
        .subscribe(move |position| positions_log.borrow_mut().push(*position));
    parser
        .on_cog
        .subscribe(move |cog| cogs_log.borrow_mut().push(*cog));
    parser
        .on_sog
        .subscribe(move |sog| sogs_log.borrow_mut().push(*sog));

    let bytes = NMEA0183_SERIAL_GNSS_GARBAGE_SOAK_LOG.as_bytes();
    let chunk_pattern = [1_usize, 2, 3, 5, 8, 13, 21];
    let mut offset = 0;
    for chunk_len in chunk_pattern.into_iter().cycle() {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + chunk_len).min(bytes.len());
        parser.feed_bytes(&bytes[offset..end]);
        offset = end;
    }

    let latest = parser
        .latest_position()
        .expect("fixture log must recover from garbage and produce a GNSS fix");
    assert!((latest.wgs.latitude - 48.117_3).abs() < 0.000_001);
    assert!((latest.wgs.longitude - 11.516_666_666_7).abs() < 0.000_001);
    assert!((latest.wgs.altitude - 545.4).abs() < 0.001);
    assert_eq!(latest.fix_type, GNSSFixType::GNSSFix);
    assert_eq!(latest.satellites_used, 8);
    assert!((latest.hdop.unwrap() - 0.9).abs() < 0.000_001);
    assert!((latest.speed_mps.unwrap() - (10.2 / 3.6)).abs() < 0.000_001);
    assert!((latest.cog_rad.unwrap() - 54.7_f64.to_radians()).abs() < 0.000_001);

    // The malformed-checksum GGA, invalid-coordinate GGA/RMC, oversized line,
    // and unsupported sentence must not emit positions or corrupt the cache.
    assert_eq!(positions.borrow().len(), 2);
    assert_eq!(positions.borrow()[0].satellites_used, 8);
    assert_eq!(positions.borrow()[1].satellites_used, 8);
    assert_eq!(cogs.borrow().len(), 2);
    assert_eq!(sogs.borrow().len(), 2);
}

#[test]
fn fixture_nmea_product_info_fast_packet_stream_reaches_management_layer() {
    let expected = N2KProductInfo {
        nmea2000_version: 0x0901,
        product_code: 0x1234,
        model_id: "Machbus".to_string(),
        software_version: "0.0.2".to_string(),
        model_version: "RustPort".to_string(),
        serial_code: "SN-MACHBUS-0001".to_string(),
        certification_level: 2,
        load_equivalency: 1,
    };
    assert_eq!(
        expected.encode().unwrap().as_slice(),
        N2K_PRODUCT_INFO_MACHBUS_PAYLOAD
    );
    assert!(
        N2KProductInfo::decode(&parse_named_hex_bytes(
            N2K_MANAGEMENT_MALFORMED_HEX,
            "product_info_nonprintable_model_id",
        ))
        .is_err(),
        "Product Info rejects non-printable fixed string bytes before event routing"
    );

    let mut tx = FastPacketProtocol::new();
    let frames = tx
        .send(PGN_PRODUCT_INFO, N2K_PRODUCT_INFO_MACHBUS_PAYLOAD, 0x23)
        .expect("Product Info is a valid fast-packet payload");
    assert_eq!(frames.len(), 20);
    assert_eq!(frames[0].pgn(), PGN_PRODUCT_INFO);
    assert_eq!(frames[0].source(), 0x23);
    assert_eq!(frames[0].destination(), BROADCAST_ADDRESS);
    assert_eq!(frames[0].data, *N2K_PRODUCT_INFO_MACHBUS_FRAME0);
    assert_eq!(frames[19].data, *N2K_PRODUCT_INFO_MACHBUS_FRAME19);

    let mut rx = FastPacketProtocol::new();
    let mut reassembled = None;
    for frame in &frames {
        reassembled = rx.process_frame(frame).or(reassembled);
    }
    let msg = reassembled.expect("Product Info fast-packet stream reassembles");
    assert_eq!(msg.pgn, PGN_PRODUCT_INFO);
    assert_eq!(msg.source, 0x23);
    assert_eq!(msg.data.as_slice(), N2K_PRODUCT_INFO_MACHBUS_PAYLOAD);

    let mut management = N2KManagement::default();
    management
        .request_product_info(0x23)
        .expect("request starts pending state");
    assert!(management.has_pending_request_to(0x23));

    let received: Rc<RefCell<Vec<(N2KProductInfo, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let received_log = received.clone();
    management
        .on_product_info_received
        .subscribe(move |info| received_log.borrow_mut().push(info.clone()));

    management.handle_message(&msg);

    assert!(!management.has_pending_request_to(0x23));
    let received = received.borrow();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0], (expected, 0x23));
}

#[test]
fn fixture_nmea_gnss_position_cog_sog_and_heading_vectors_are_stable() {
    let position_bytes = parse_hex_bytes(NMEA_GNSS_POSITION_RAPID_52N_5E_HEX.trim());
    let cog_sog_bytes = parse_hex_bytes(NMEA_GNSS_COG_SOG_EAST_5_5MPS_HEX.trim());
    let heading_bytes = parse_hex_bytes(NMEA_HEADING_TRACK_1RAD_DEV_NEG0_1_VAR0_25_HEX.trim());
    let magnetic_variation_bytes = parse_named_hex_frame(
        NMEA_MAGNETIC_VARIATION_HEX,
        "magnetic_variation_manual_age14_var0_125",
    );
    let magnetic_variation_unavailable = parse_named_hex_frame(
        NMEA_MAGNETIC_VARIATION_HEX,
        "magnetic_variation_unavailable",
    );
    let bad_magnetic_variation_source = parse_named_hex_frame(
        NMEA_MAGNETIC_VARIATION_HEX,
        "malformed_magnetic_variation_bad_source",
    );
    let bad_magnetic_variation_tail = parse_named_hex_frame(
        NMEA_MAGNETIC_VARIATION_HEX,
        "malformed_magnetic_variation_bad_tail",
    );
    let system_time_bytes =
        parse_named_hex_frame(NMEA_SYSTEM_TIME_HEX, "glonass_sid42_day12345_seconds3600_5");
    let bad_system_time_source = parse_named_hex_frame(NMEA_SYSTEM_TIME_HEX, "bad_source6");

    let pos = GNSSPosition {
        wgs: Wgs::new(52.0, 5.0, 0.0),
        ..Default::default()
    };
    assert_eq!(
        NMEAInterface::build_position(&pos).as_slice(),
        position_bytes.as_slice()
    );
    assert_eq!(
        NMEAInterface::build_cog_sog(std::f64::consts::FRAC_PI_2, 5.5).as_slice(),
        cog_sog_bytes.as_slice()
    );
    assert_eq!(
        NMEAInterface::build_heading(1.0, -0.1, 0.25).as_slice(),
        heading_bytes.as_slice()
    );
    assert_eq!(
        NMEAInterface::build_magnetic_variation(0.125, 14).as_slice(),
        magnetic_variation_bytes.as_slice()
    );
    assert_eq!(
        NMEAInterface::build_system_time(&SystemTimeData {
            sid: 0x2A,
            source: TimeSource::GLONASS,
            days_since_epoch: 12_345,
            seconds_since_midnight: 3_600.5,
        })
        .as_slice(),
        system_time_bytes.as_slice()
    );

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let headings: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let headings_log = headings.clone();
    iface
        .on_heading
        .subscribe(move |heading| headings_log.borrow_mut().push(*heading));
    let magnetic_variations: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let magnetic_variations_log = magnetic_variations.clone();
    iface
        .on_magnetic_variation
        .subscribe(move |variation| magnetic_variations_log.borrow_mut().push(*variation));
    let system_times: Rc<RefCell<Vec<SystemTimeData>>> = Rc::new(RefCell::new(Vec::new()));
    let system_times_log = system_times.clone();
    iface
        .on_system_time
        .subscribe(move |time| system_times_log.borrow_mut().push(*time));

    for invalid_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        iface.handle_message(&Message::new(
            PGN_GNSS_POSITION_RAPID,
            position_bytes.clone(),
            invalid_source,
        ));
        assert!(
            iface.latest_position().is_none(),
            "NMEA interface must ignore position from invalid source 0x{invalid_source:02X}"
        );
    }

    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_RAPID,
        position_bytes.clone(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_GNSS_COG_SOG_RAPID,
        cog_sog_bytes.clone(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_HEADING_TRACK,
        heading_bytes.clone(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        magnetic_variation_bytes.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        magnetic_variation_unavailable.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        bad_magnetic_variation_source.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_MAGNETIC_VARIATION,
        bad_magnetic_variation_tail.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_SYSTEM_TIME,
        system_time_bytes.to_vec(),
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_SYSTEM_TIME,
        bad_system_time_source.to_vec(),
        0x23,
    ));

    let cached = iface.latest_position().expect("position should decode");
    assert!((cached.wgs.latitude - 52.0).abs() < 1e-6);
    assert!((cached.wgs.longitude - 5.0).abs() < 1e-6);
    assert!((cached.cog_rad.unwrap() - std::f64::consts::FRAC_PI_2).abs() < 0.001);
    assert!((cached.speed_mps.unwrap() - 5.5).abs() < 0.01);
    assert!((cached.heading_rad.unwrap() - 1.0).abs() < 0.0001);
    assert_eq!(headings.borrow().len(), 1);
    assert!((headings.borrow()[0] - 1.0).abs() < 0.0001);
    assert_eq!(magnetic_variations.borrow().len(), 1);
    assert!((magnetic_variations.borrow()[0] - 0.125).abs() < 0.0001);
    assert_eq!(system_times.borrow().len(), 1);
    assert_eq!(system_times.borrow()[0].source, TimeSource::GLONASS);
    assert_eq!(system_times.borrow()[0].days_since_epoch, 12_345);
    assert!((system_times.borrow()[0].seconds_since_midnight - 3_600.5).abs() < 0.0001);
}

#[test]
fn fixture_nmea_gnss_detail_dops_and_attitude_vectors_are_stable() {
    let position_detail = parse_hex_bytes(NMEA_GNSS_POSITION_DATA_52N_5E_ALT12_345_RTK_HEX.trim());
    let dops_bytes = parse_hex_bytes(NMEA_GNSS_DOPS_AUTO_3D_HDOP0_85_VDOP1_10_TDOP0_50_HEX.trim());
    let dops_bad_reserved = parse_hex_bytes(NMEA_GNSS_DOPS_BAD_RESERVED_BITS_HEX.trim());
    let dops_bad_reserved_mode = parse_hex_bytes(NMEA_GNSS_DOPS_BAD_RESERVED_MODE_HEX.trim());
    let attitude_bytes = parse_hex_bytes(NMEA_ATTITUDE_YAW1_PITCH_NEG0_1_ROLL0_25_HEX.trim());
    let position_detail_one_ref = parse_named_hex_bytes(
        NMEA_GNSS_POSITION_DATA_REFERENCE_STATION_LENGTHS_HEX,
        "one_reference_station",
    );
    let position_detail_overlong = parse_named_hex_bytes(
        NMEA_GNSS_POSITION_DATA_REFERENCE_STATION_LENGTHS_HEX,
        "malformed_unavailable_count_overlong",
    );
    let position_detail_truncated = parse_named_hex_bytes(
        NMEA_GNSS_POSITION_DATA_REFERENCE_STATION_LENGTHS_HEX,
        "malformed_one_reference_truncated",
    );

    assert_eq!(position_detail.len(), 43);
    assert_eq!(position_detail_one_ref.len(), 47);
    assert_eq!(dops_bytes.len(), 8);
    assert_eq!(attitude_bytes.len(), 8);

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let detail_log: Rc<RefCell<Vec<machbus::nmea::GNSSPosition>>> =
        Rc::new(RefCell::new(Vec::new()));
    let detail_seen = detail_log.clone();
    iface
        .on_position
        .subscribe(move |pos| detail_seen.borrow_mut().push(*pos));
    let dops_log: Rc<RefCell<Vec<machbus::nmea::GNSSDOPData>>> = Rc::new(RefCell::new(Vec::new()));
    let dops_seen = dops_log.clone();
    iface
        .on_gnss_dops
        .subscribe(move |dops| dops_seen.borrow_mut().push(*dops));
    let attitude_log: Rc<RefCell<Vec<(f64, f64, f64)>>> = Rc::new(RefCell::new(Vec::new()));
    let attitude_seen = attitude_log.clone();
    iface
        .on_attitude
        .subscribe(move |att| attitude_seen.borrow_mut().push(*att));

    iface.handle_message(&Message::new(PGN_GNSS_POSITION_DATA, position_detail, 0x23));
    let detailed = iface
        .latest_position()
        .expect("position detail should decode");
    assert!((detailed.wgs.latitude - 52.0).abs() < 1e-12);
    assert!((detailed.wgs.longitude - 5.0).abs() < 1e-12);
    assert!((detailed.altitude_m.unwrap() - 12.345).abs() < 0.000001);
    assert_eq!(detailed.fix_type, GNSSFixType::RTKFixed);
    assert_eq!(detailed.satellites_used, 12);
    assert!((detailed.hdop.unwrap() - 0.85).abs() < 0.000001);
    assert!((detailed.pdop.unwrap() - 1.32).abs() < 0.000001);
    assert_eq!(detail_log.borrow().len(), 1);

    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DATA,
        position_detail_overlong,
        0x23,
    ));
    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DATA,
        position_detail_truncated,
        0x23,
    ));
    assert_eq!(
        detail_log.borrow().len(),
        1,
        "GNSS detail count/length mismatches must be ignored"
    );

    iface.handle_message(&Message::new(
        PGN_GNSS_POSITION_DATA,
        position_detail_one_ref,
        0x23,
    ));
    assert_eq!(detail_log.borrow().len(), 2);

    iface.handle_message(&Message::new(PGN_GNSS_DOPS, dops_bytes, 0x23));
    let dops = dops_log.borrow();
    assert_eq!(dops.len(), 1);
    assert_eq!(dops[0].desired_mode, GNSSDOPMode::Auto);
    assert_eq!(dops[0].actual_mode, GNSSDOPMode::Mode3D);
    assert!((dops[0].hdop - 0.85).abs() < 0.000001);
    assert!((dops[0].vdop - 1.10).abs() < 0.000001);
    assert!((dops[0].tdop - 0.50).abs() < 0.000001);
    drop(dops);

    let dop_cached = iface.latest_position().expect("DOPs should update cache");
    assert!((dop_cached.hdop.unwrap() - 0.85).abs() < 0.000001);
    assert!((dop_cached.vdop.unwrap() - 1.10).abs() < 0.000001);
    assert!((dop_cached.pdop.unwrap() - 1.32).abs() < 0.000001);

    iface.handle_message(&Message::new(PGN_GNSS_DOPS, dops_bad_reserved, 0x23));
    iface.handle_message(&Message::new(PGN_GNSS_DOPS, dops_bad_reserved_mode, 0x23));
    assert_eq!(
        dops_log.borrow().len(),
        1,
        "GNSS DOPs reserved bits/modes must be ignored before event emission"
    );
    let dop_cached = iface
        .latest_position()
        .expect("bad DOPs reserved bits should preserve cache");
    assert!((dop_cached.hdop.unwrap() - 0.85).abs() < 0.000001);
    assert!((dop_cached.vdop.unwrap() - 1.10).abs() < 0.000001);
    assert!((dop_cached.pdop.unwrap() - 1.32).abs() < 0.000001);

    iface.handle_message(&Message::new(PGN_ATTITUDE, attitude_bytes, 0x23));
    let attitudes = attitude_log.borrow();
    assert_eq!(attitudes.len(), 1);
    assert!((attitudes[0].0 - 1.0).abs() < 0.0001);
    assert!((attitudes[0].1 + 0.1).abs() < 0.0001);
    assert!((attitudes[0].2 - 0.25).abs() < 0.0001);
    drop(attitudes);

    let attitude_cached = iface
        .latest_position()
        .expect("attitude should update latest position");
    assert!((attitude_cached.heading_rad.unwrap() - 1.0).abs() < 0.0001);
    assert!((attitude_cached.pitch_rad.unwrap() + 0.1).abs() < 0.0001);
    assert!((attitude_cached.roll_rad.unwrap() - 0.25).abs() < 0.0001);
}

#[test]
fn fixture_nmea_gnss_position_data_fast_packet_reaches_interface() {
    let position_detail = parse_hex_bytes(NMEA_GNSS_POSITION_DATA_52N_5E_ALT12_345_RTK_HEX.trim());
    assert_eq!(position_detail.len(), 43);

    let expected_frames = [
        parse_named_hex_frame(NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX, "frame0"),
        parse_named_hex_frame(NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX, "frame1"),
        parse_named_hex_frame(NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX, "frame2"),
        parse_named_hex_frame(NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX, "frame3"),
        parse_named_hex_frame(NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX, "frame4"),
        parse_named_hex_frame(NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX, "frame5"),
        parse_named_hex_frame(NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX, "frame6"),
    ];

    let mut tx = FastPacketProtocol::new();
    let frames = tx
        .send(PGN_GNSS_POSITION_DATA, &position_detail, 0x23)
        .expect("GNSS Position Data should use NMEA fast packet");
    assert_eq!(frames.len(), expected_frames.len());
    for (frame, expected) in frames.iter().zip(expected_frames.iter()) {
        assert_eq!(frame.pgn(), PGN_GNSS_POSITION_DATA);
        assert_eq!(frame.source(), 0x23);
        assert_eq!(frame.destination(), BROADCAST_ADDRESS);
        assert_eq!(frame.data, *expected);
    }

    let mut rx = FastPacketProtocol::new();
    let id = Identifier::encode(
        Priority::Default,
        PGN_GNSS_POSITION_DATA,
        0x23,
        BROADCAST_ADDRESS,
    );
    let mut completed = None;
    for payload in expected_frames {
        completed = rx.process_frame(&Frame::new(id, payload, 8)).or(completed);
    }
    let msg = completed.expect("last fast-packet frame should complete GNSS Position Data");
    assert_eq!(msg.pgn, PGN_GNSS_POSITION_DATA);
    assert_eq!(msg.source, 0x23);
    assert_eq!(msg.data, position_detail);

    let mut iface = NMEAInterface::new(NMEAConfig::default().with_all(true));
    let positions: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
    let positions_log = positions.clone();
    iface
        .on_position
        .subscribe(move |pos| positions_log.borrow_mut().push(*pos));

    iface.handle_message(&msg);

    let detailed = iface
        .latest_position()
        .expect("reassembled GNSS Position Data should update cache");
    assert!((detailed.wgs.latitude - 52.0).abs() < 1e-12);
    assert!((detailed.wgs.longitude - 5.0).abs() < 1e-12);
    assert!((detailed.altitude_m.unwrap() - 12.345).abs() < 0.000001);
    assert_eq!(detailed.fix_type, GNSSFixType::RTKFixed);
    assert_eq!(detailed.satellites_used, 12);
    assert!((detailed.hdop.unwrap() - 0.85).abs() < 0.000001);
    assert!((detailed.pdop.unwrap() - 1.32).abs() < 0.000001);
    assert_eq!(positions.borrow().len(), 1);
}

