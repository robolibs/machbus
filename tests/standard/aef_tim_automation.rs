use machbus::isobus::{
    AuxValveCommand, HitchState, PtoState, TIM_OPTION_DEFINED_MASK, TimAuthority,
    TimAuthorityState, TimCommand, TimInterlock, TimInterlocks, TimOption, TimOptionSet,
    TimValidationError,
};
use machbus::net::pgn_defs::{
    PGN_AUX_VALVE_0_7, PGN_FRONT_HITCH, PGN_FRONT_PTO, PGN_REAR_HITCH, PGN_REAR_PTO,
};
use machbus::net::{BROADCAST_ADDRESS, Message, NULL_ADDRESS, Priority};
use std::collections::BTreeSet;

#[test]
fn tim_authority_requires_request_grant_and_clear_interlocks_before_command() {
    let options = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
    let mut authority = TimAuthority::new(options);

    assert_eq!(
        authority.ensure_command(TimCommand::RearHitchPosition),
        Err(TimValidationError::OptionNotRequested {
            option: TimOption::RearHitchPositionIsSupported,
        })
    );

    authority.request(options).unwrap();
    assert_eq!(authority.state(), TimAuthorityState::Requested);
    assert_eq!(
        authority.ensure_command(TimCommand::RearHitchPosition),
        Err(TimValidationError::AuthorityNotGranted {
            state: TimAuthorityState::Requested,
        })
    );

    authority.grant().unwrap();
    assert_eq!(authority.state(), TimAuthorityState::Granted);
    assert!(
        authority
            .ensure_command(TimCommand::RearHitchPosition)
            .is_ok()
    );
}

#[test]
fn tim_interlock_revokes_granted_authority_and_blocks_command() {
    let options = TimOptionSet::from_options(&[TimOption::RearPtoEngagementCwIsSupported]);
    let mut authority = TimAuthority::new(options);
    authority.request(options).unwrap();
    authority.grant().unwrap();

    authority.set_interlocks(TimInterlocks::all_clear().with_external_stop(true));
    assert_eq!(authority.state(), TimAuthorityState::Revoked);
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoEngageCw),
        Err(TimValidationError::InterlockActive {
            interlock: TimInterlock::ExternalStop,
        })
    );
}

#[test]
fn tim_revoked_authority_requires_fresh_request_before_commanding_again() {
    let options = TimOptionSet::from_options(&[TimOption::RearPtoDisengagementIsSupported]);
    let mut authority = TimAuthority::new(options);
    authority.request(options).unwrap();
    authority.grant().unwrap();
    assert_eq!(authority.state(), TimAuthorityState::Granted);

    authority.set_interlocks(TimInterlocks::all_clear().with_road_transport_mode(true));
    assert_eq!(authority.state(), TimAuthorityState::Revoked);
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoDisengage),
        Err(TimValidationError::InterlockActive {
            interlock: TimInterlock::RoadTransportMode,
        })
    );

    authority.set_interlocks(TimInterlocks::all_clear());
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoDisengage),
        Err(TimValidationError::AuthorityNotGranted {
            state: TimAuthorityState::Revoked,
        }),
        "clearing a safety interlock must not silently restore command authority"
    );
    assert_eq!(
        authority.grant(),
        Err(TimValidationError::AuthorityNotRequested {
            state: TimAuthorityState::Revoked,
        }),
        "a revoked grant must not be re-granted without a new request cycle"
    );

    authority.request(options).unwrap();
    authority.grant().unwrap();
    assert!(
        authority
            .ensure_command(TimCommand::RearPtoDisengage)
            .is_ok()
    );
}

#[test]
fn tim_denied_or_manually_revoked_authority_blocks_commands_until_new_request_cycle() {
    let options = TimOptionSet::from_options(&[
        TimOption::RearPtoDisengagementIsSupported,
        TimOption::RearPtoEngagementCwIsSupported,
    ]);
    let mut authority = TimAuthority::new(options);
    authority.request(options).unwrap();

    authority.deny();
    assert_eq!(authority.state(), TimAuthorityState::Denied);
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoDisengage),
        Err(TimValidationError::AuthorityNotGranted {
            state: TimAuthorityState::Denied,
        }),
        "denied TIM authority must not allow a safe command to slip through"
    );
    assert_eq!(
        authority.grant(),
        Err(TimValidationError::AuthorityNotRequested {
            state: TimAuthorityState::Denied,
        }),
        "denied TIM authority must require a fresh request before grant"
    );

    authority.request(options).unwrap();
    authority.grant().unwrap();
    assert!(
        authority
            .ensure_command(TimCommand::RearPtoDisengage)
            .is_ok()
    );

    authority.revoke();
    assert_eq!(authority.state(), TimAuthorityState::Revoked);
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoEngageCw),
        Err(TimValidationError::AuthorityNotGranted {
            state: TimAuthorityState::Revoked,
        }),
        "manually revoked TIM authority must block a still-requested command"
    );
    assert_eq!(
        authority.grant(),
        Err(TimValidationError::AuthorityNotRequested {
            state: TimAuthorityState::Revoked,
        }),
        "manual revoke must not be re-granted without a fresh request"
    );

    authority.request(options).unwrap();
    authority.grant().unwrap();
    assert!(
        authority
            .ensure_command(TimCommand::RearPtoEngageCw)
            .is_ok()
    );
}

#[test]
fn tim_authority_rerequest_narrows_pending_and_granted_commands() {
    let broad = TimOptionSet::from_options(&[
        TimOption::RearPtoDisengagementIsSupported,
        TimOption::RearPtoEngagementCwIsSupported,
    ]);
    let narrow = TimOptionSet::from_options(&[TimOption::RearPtoDisengagementIsSupported]);
    let mut authority = TimAuthority::new(broad);

    authority.request(broad).unwrap();
    authority.grant().unwrap();
    assert!(
        authority
            .ensure_command(TimCommand::RearPtoDisengage)
            .is_ok()
    );
    assert!(
        authority
            .ensure_command(TimCommand::RearPtoEngageCw)
            .is_ok()
    );

    authority.request(narrow).unwrap();
    assert_eq!(
        authority.state(),
        TimAuthorityState::Requested,
        "a new authority request must return to pending state before commands are allowed"
    );
    assert_eq!(authority.requested_options(), narrow);
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoEngageCw),
        Err(TimValidationError::OptionNotRequested {
            option: TimOption::RearPtoEngagementCwIsSupported,
        }),
        "commands removed by a narrower request must stop being authorized immediately"
    );
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoDisengage),
        Err(TimValidationError::AuthorityNotGranted {
            state: TimAuthorityState::Requested,
        }),
        "commands retained by a re-request still need a fresh grant"
    );

    authority.grant().unwrap();
    assert!(
        authority
            .ensure_command(TimCommand::RearPtoDisengage)
            .is_ok()
    );
    assert_eq!(
        authority.ensure_command(TimCommand::RearPtoEngageCw),
        Err(TimValidationError::OptionNotRequested {
            option: TimOption::RearPtoEngagementCwIsSupported,
        })
    );
}

#[test]
fn tim_empty_authority_request_does_not_create_command_grant() {
    let available = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
    let mut authority = TimAuthority::new(available);

    assert_eq!(
        authority.request(TimOptionSet::empty()),
        Err(TimValidationError::EmptyOptionRequest),
        "an empty TIM authority request must not create a grantable automation session"
    );
    assert_eq!(authority.state(), TimAuthorityState::Idle);
    assert_eq!(authority.requested_options(), TimOptionSet::empty());
    assert_eq!(
        authority.grant(),
        Err(TimValidationError::AuthorityNotRequested {
            state: TimAuthorityState::Idle,
        })
    );
    assert_eq!(
        authority.ensure_command(TimCommand::RearHitchPosition),
        Err(TimValidationError::OptionNotRequested {
            option: TimOption::RearHitchPositionIsSupported,
        })
    );
}

#[test]
fn tim_command_option_mapping_covers_each_defined_authority_bit() {
    let command_options = [
        (
            TimCommand::FrontPtoDisengage,
            TimOption::FrontPtoDisengagementIsSupported,
        ),
        (
            TimCommand::FrontPtoEngageCcw,
            TimOption::FrontPtoEngagementCcwIsSupported,
        ),
        (
            TimCommand::FrontPtoEngageCw,
            TimOption::FrontPtoEngagementCwIsSupported,
        ),
        (
            TimCommand::FrontPtoSpeedCcw,
            TimOption::FrontPtoSpeedCcwIsSupported,
        ),
        (
            TimCommand::FrontPtoSpeedCw,
            TimOption::FrontPtoSpeedCwIsSupported,
        ),
        (
            TimCommand::RearPtoDisengage,
            TimOption::RearPtoDisengagementIsSupported,
        ),
        (
            TimCommand::RearPtoEngageCcw,
            TimOption::RearPtoEngagementCcwIsSupported,
        ),
        (
            TimCommand::RearPtoEngageCw,
            TimOption::RearPtoEngagementCwIsSupported,
        ),
        (
            TimCommand::RearPtoSpeedCcw,
            TimOption::RearPtoSpeedCcwIsSupported,
        ),
        (
            TimCommand::RearPtoSpeedCw,
            TimOption::RearPtoSpeedCwIsSupported,
        ),
        (
            TimCommand::FrontHitchMotion,
            TimOption::FrontHitchMotionIsSupported,
        ),
        (
            TimCommand::FrontHitchPosition,
            TimOption::FrontHitchPositionIsSupported,
        ),
        (
            TimCommand::RearHitchMotion,
            TimOption::RearHitchMotionIsSupported,
        ),
        (
            TimCommand::RearHitchPosition,
            TimOption::RearHitchPositionIsSupported,
        ),
        (
            TimCommand::VehicleSpeedForward,
            TimOption::VehicleSpeedInForwardDirectionIsSupported,
        ),
        (
            TimCommand::VehicleSpeedReverse,
            TimOption::VehicleSpeedInReverseDirectionIsSupported,
        ),
        (
            TimCommand::VehicleSpeedStartMotion,
            TimOption::VehicleSpeedStartMotionIsSupported,
        ),
        (
            TimCommand::VehicleSpeedStopMotion,
            TimOption::VehicleSpeedStopMotionIsSupported,
        ),
        (
            TimCommand::VehicleSpeedForwardSetByServer,
            TimOption::VehicleSpeedForwardSetByServerIsSupported,
        ),
        (
            TimCommand::VehicleSpeedReverseSetByServer,
            TimOption::VehicleSpeedReverseSetByServerIsSupported,
        ),
        (
            TimCommand::VehicleSpeedChangeDirection,
            TimOption::VehicleSpeedChangeDirectionIsSupported,
        ),
        (
            TimCommand::GuidanceCurvature,
            TimOption::GuidanceCurvatureIsSupported,
        ),
    ];
    let defined_option_count = TIM_OPTION_DEFINED_MASK
        .iter()
        .map(|byte| byte.count_ones())
        .sum::<u32>() as usize;
    assert_eq!(command_options.len(), defined_option_count);

    let mut all_options = TimOptionSet::empty();
    let mut seen_bits = BTreeSet::new();
    for (command, option) in command_options {
        assert_eq!(command.required_option(), option);
        assert!(
            seen_bits.insert(option.bit()),
            "TIM command-option mapping must not duplicate authority bit {}",
            option.bit()
        );
        all_options.set(option, true);
    }
    assert_eq!(
        all_options.as_bytes(),
        TIM_OPTION_DEFINED_MASK,
        "every currently defined TIM authority bit must be reachable by exactly one command"
    );
    assert!(!all_options.has_reserved_bits());
}

#[test]
fn tim_option_sets_reject_reserved_bits_before_authority_negotiation() {
    assert_eq!(TIM_OPTION_DEFINED_MASK, [0xFF, 0xFF, 0x3F]);

    let defined = [0x21, 0x10, 0x20];
    let parsed = TimOptionSet::try_from_bytes(defined).unwrap();
    assert!(!parsed.has_reserved_bits());
    assert!(parsed.contains(TimOption::FrontPtoDisengagementIsSupported));
    assert!(parsed.contains(TimOption::RearHitchMotionIsSupported));
    assert!(parsed.contains(TimOption::GuidanceCurvatureIsSupported));

    let reserved = [0x00, 0x00, 0x40];
    assert_eq!(
        TimOptionSet::try_from_bytes(reserved),
        Err(TimValidationError::ReservedOptionBits { bytes: reserved })
    );

    let mut authority = TimAuthority::new(TimOptionSet::from_bytes(reserved));
    assert_eq!(
        authority.request(TimOptionSet::empty()),
        Err(TimValidationError::ReservedOptionBits { bytes: reserved }),
        "reserved advertised TIM options must not be accepted as authority input"
    );

    let available = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
    let mut authority = TimAuthority::new(available);
    assert_eq!(
        authority.request(TimOptionSet::from_bytes(reserved)),
        Err(TimValidationError::ReservedOptionBits { bytes: reserved }),
        "reserved requested TIM options must not pass subset negotiation"
    );
    assert_eq!(authority.state(), TimAuthorityState::Idle);
}

#[test]
fn tim_unsupported_option_requests_do_not_mutate_authority_state() {
    let available = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
    let supported = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
    let unsupported = TimOptionSet::from_options(&[TimOption::GuidanceCurvatureIsSupported]);
    let mut authority = TimAuthority::new(available);

    assert_eq!(
        authority.request(unsupported),
        Err(TimValidationError::UnsupportedOptions {
            requested: unsupported,
            available,
        })
    );
    assert_eq!(authority.state(), TimAuthorityState::Idle);
    assert_eq!(authority.requested_options(), TimOptionSet::empty());

    authority.request(supported).unwrap();
    authority.grant().unwrap();
    assert_eq!(authority.state(), TimAuthorityState::Granted);
    assert!(
        authority
            .ensure_command(TimCommand::RearHitchPosition)
            .is_ok()
    );

    assert_eq!(
        authority.request(unsupported),
        Err(TimValidationError::UnsupportedOptions {
            requested: unsupported,
            available,
        }),
        "a rejected authority expansion must not replace the active grant"
    );
    assert_eq!(authority.state(), TimAuthorityState::Granted);
    assert_eq!(authority.requested_options(), supported);
    assert!(
        authority
            .ensure_command(TimCommand::RearHitchPosition)
            .is_ok()
    );
}

#[test]
fn tim_interlock_matrix_blocks_grant_without_advancing_state() {
    let options = TimOptionSet::from_options(&[TimOption::VehicleSpeedStopMotionIsSupported]);
    let blocking_cases = [
        (
            TimInterlocks::all_clear().with_operator_present(false),
            TimInterlock::OperatorNotPresent,
        ),
        (
            TimInterlocks::all_clear().with_road_transport_mode(true),
            TimInterlock::RoadTransportMode,
        ),
        (
            TimInterlocks::all_clear().with_external_stop(true),
            TimInterlock::ExternalStop,
        ),
        (
            TimInterlocks::all_clear().with_implement_ready(false),
            TimInterlock::ImplementNotReady,
        ),
    ];

    for (interlocks, expected) in blocking_cases {
        let mut authority = TimAuthority::new(options);
        authority.set_interlocks(interlocks);
        authority.request(options).unwrap();

        assert_eq!(
            authority.grant(),
            Err(TimValidationError::InterlockActive {
                interlock: expected,
            })
        );
        assert_eq!(
            authority.state(),
            TimAuthorityState::Requested,
            "blocked grant for {expected:?} must not advance to Granted"
        );
        assert_eq!(
            authority.ensure_command(TimCommand::VehicleSpeedStopMotion),
            Err(TimValidationError::InterlockActive {
                interlock: expected,
            })
        );

        authority.set_interlocks(TimInterlocks::all_clear());
        authority.grant().unwrap();
        assert!(
            authority
                .ensure_command(TimCommand::VehicleSpeedStopMotion)
                .is_ok()
        );
    }
}

#[test]
fn tim_status_and_command_decoders_reject_invalid_sources_before_cache_use() {
    let pto = PtoState {
        engaged: true,
        cw_direction: true,
        speed: 1_000,
    };
    let hitch = HitchState {
        motion_enabled: true,
        position: 7_500,
    };
    let aux = AuxValveCommand {
        index: 7,
        state: true,
        flow: 4_200,
    };

    assert_eq!(
        PtoState::decode(&Message::new(PGN_FRONT_PTO, pto.encode().to_vec(), 0x44)),
        Some(pto)
    );
    assert_eq!(
        HitchState::decode(&Message::new(
            PGN_FRONT_HITCH,
            hitch.encode().to_vec(),
            0x44,
        )),
        Some(hitch)
    );
    assert_eq!(
        PtoState::decode(&Message::new(PGN_REAR_PTO, pto.encode().to_vec(), 0x44)),
        Some(pto)
    );
    assert_eq!(
        HitchState::decode(&Message::new(PGN_REAR_HITCH, hitch.encode().to_vec(), 0x44)),
        Some(hitch)
    );
    assert_eq!(
        AuxValveCommand::decode(&Message::new(
            PGN_AUX_VALVE_0_7,
            aux.encode().to_vec(),
            0x44
        )),
        Some(aux)
    );

    for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert_eq!(
            PtoState::decode(&Message::new(
                PGN_FRONT_PTO,
                pto.encode().to_vec(),
                bad_source,
            )),
            None,
            "TIM PTO status from invalid source 0x{bad_source:02X} must not decode"
        );
        assert_eq!(
            HitchState::decode(&Message::new(
                PGN_FRONT_HITCH,
                hitch.encode().to_vec(),
                bad_source,
            )),
            None,
            "TIM hitch status from invalid source 0x{bad_source:02X} must not decode"
        );
        assert_eq!(
            AuxValveCommand::decode(&Message::new(
                PGN_AUX_VALVE_0_7,
                aux.encode().to_vec(),
                bad_source,
            )),
            None,
            "TIM aux-valve command/status from invalid source 0x{bad_source:02X} must not decode"
        );
    }

    assert_eq!(
        PtoState::decode(&Message::with_addressing(
            PGN_FRONT_PTO,
            pto.encode().to_vec(),
            0x44,
            0x45,
            Priority::Default,
        )),
        None,
        "TIM PTO status is a PDU2 broadcast envelope and must reject destination-specific metadata"
    );
    assert_eq!(
        HitchState::decode(&Message::with_addressing(
            PGN_FRONT_HITCH,
            hitch.encode().to_vec(),
            0x44,
            0x45,
            Priority::Default,
        )),
        None,
        "TIM hitch status is a PDU2 broadcast envelope and must reject destination-specific metadata"
    );
    assert_eq!(
        AuxValveCommand::decode(&Message::with_addressing(
            PGN_AUX_VALVE_0_7,
            aux.encode().to_vec(),
            0x44,
            0x45,
            Priority::Default,
        )),
        None,
        "TIM aux-valve status is a PDU2 broadcast envelope and must reject destination-specific metadata"
    );

    assert_eq!(
        PtoState::decode(&Message::new(PGN_FRONT_HITCH, pto.encode().to_vec(), 0x44,)),
        None,
        "TIM PTO state decoder must stay bound to PTO status PGNs"
    );
    assert_eq!(
        HitchState::decode(&Message::new(PGN_FRONT_PTO, hitch.encode().to_vec(), 0x44,)),
        None,
        "TIM hitch state decoder must stay bound to hitch status PGNs"
    );
}
