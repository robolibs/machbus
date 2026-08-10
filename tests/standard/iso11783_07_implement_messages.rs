use machbus::isobus::implement::{
    AuxValveCommandMsg, AuxValveFlowMsg, CurvatureCommandStatus, DriveStrategyCmd,
    DriveStrategyMode, ExitReasonCode, GenericSaeBs02SlotValue, GroundBasedSpeedDist,
    GuidanceLimitStatus, GuidanceMachineInfo, GuidanceSystemCmd, HitchCommand, HitchCommandMsg,
    HitchPtoCombinedCmd, HitchRollPitchCmd, HitchStatus, LightState, LightingState, LimitStatus,
    MAX_AUX_VALVES, MachineDirection, MachineSelectedSpeedFull, MachineSelectedSpeedMsg,
    MachineSpeedCommandMsg, MechanicalLockout, PtoCommand, PtoCommandMsg, PtoStatus,
    RequestResetCommandStatus, Signal, SpeedExitCode, SpeedSource, TractorControlModeMsg,
    TractorMode,
    ValveCommand, ValveFailSafe, ValveLimitStatus, ValveState, WheelBasedSpeedDist,
    estimated_flow_pgn, measured_flow_pgn,
};
use machbus::j1939::shortcut_button::{self, ShortcutButtonState};
use machbus::j1939::{
    HB_COMM_ERROR_TIMEOUT_MS, HbReceiverState, HeartbeatReceiver, HeartbeatRequest,
    HeartbeatTracker, PGN_HEARTBEAT_REQUEST, SpeedAndDistance, heartbeat::hb_seq,
};
use machbus::net::pgn_defs::{
    PGN_AUX_VALVE_CMD, PGN_FRONT_HITCH_ROLL_PITCH_CMD, PGN_HEARTBEAT, PGN_MACHINE_SPEED,
    PGN_REAR_HITCH_ROLL_PITCH_CMD, PGN_SHORTCUT_BUTTON, PGN_TIME_DATE, PGN_WHEEL_SPEED,
};
use machbus::net::{BROADCAST_ADDRESS, Message, NULL_ADDRESS, Priority};
use std::cell::RefCell;
use std::rc::Rc;

#[test]
fn implement_hitch_and_pto_command_codecs_reject_reserved_command_bytes() {
    let hitch = HitchCommandMsg {
        command: HitchCommand::Position,
        target_position: 12_345,
        rate: 42,
    };
    let hitch_bytes = hitch.encode();
    assert_eq!(HitchCommandMsg::decode(&hitch_bytes), Some(hitch));

    let mut bad_hitch = hitch_bytes;
    bad_hitch[4] = 4;
    assert_eq!(HitchCommandMsg::decode(&bad_hitch), None);

    let pto = PtoCommandMsg {
        command: PtoCommand::SetSpeed,
        target_speed_rpm: 4_000,
        ramp_rate: 7,
    };
    let pto_bytes = pto.encode();
    assert_eq!(PtoCommandMsg::decode(&pto_bytes), Some(pto));

    let mut bad_pto = pto_bytes;
    bad_pto[4] = 4;
    assert_eq!(PtoCommandMsg::decode(&bad_pto), None);
}

#[test]
fn implement_heartbeat_tracker_rejects_wrong_pgn_sources_width_and_reserved_sequences() {
    let mut tracker = HeartbeatTracker::new(100);
    tracker.track(0x22);
    let received: Rc<RefCell<Vec<(u8, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let received_log = received.clone();
    tracker
        .on_heartbeat_received
        .subscribe(move |event| received_log.borrow_mut().push(*event));

    for msg in [
        Message::new(PGN_TIME_DATE, vec![7], 0x22),
        Message::new(PGN_HEARTBEAT, vec![7], NULL_ADDRESS),
        Message::new(PGN_HEARTBEAT, vec![7], BROADCAST_ADDRESS),
        Message::with_addressing(PGN_HEARTBEAT, vec![7], 0x22, 0x42, Priority::Default),
        Message::new(PGN_HEARTBEAT, vec![hb_seq::RESERVED_LOW], 0x22),
        Message::new(PGN_HEARTBEAT, vec![8, 0x00], 0x22),
    ] {
        tracker.handle_message(&msg);
    }
    assert_eq!(tracker.last_sequence(0x22), Some(0));
    assert!(received.borrow().is_empty());

    tracker.handle_message(&Message::new(
        PGN_HEARTBEAT,
        vec![9, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        0x22,
    ));
    assert_eq!(tracker.last_sequence(0x22), Some(9));
    assert_eq!(received.borrow().as_slice(), &[(0x22, 9)]);
}

#[test]
fn implement_heartbeat_request_full_message_helper_rejects_invalid_envelopes() {
    let request = HeartbeatRequest::for_heartbeat(100);
    let encoded = request.encode().unwrap();
    assert_eq!(
        HeartbeatRequest::from_message(&Message::with_addressing(
            PGN_HEARTBEAT_REQUEST,
            encoded.to_vec(),
            0x22,
            0x23,
            Priority::Default,
        )),
        Some(request)
    );

    for msg in [
        Message::new(PGN_HEARTBEAT, encoded.to_vec(), 0x22),
        Message::with_addressing(
            PGN_HEARTBEAT_REQUEST,
            encoded.to_vec(),
            NULL_ADDRESS,
            0x23,
            Priority::Default,
        ),
        Message::with_addressing(
            PGN_HEARTBEAT_REQUEST,
            encoded.to_vec(),
            BROADCAST_ADDRESS,
            0x23,
            Priority::Default,
        ),
        Message::new(PGN_HEARTBEAT_REQUEST, encoded.to_vec(), 0x22),
        Message::with_addressing(
            PGN_HEARTBEAT_REQUEST,
            encoded.to_vec(),
            0x22,
            NULL_ADDRESS,
            Priority::Default,
        ),
        Message::with_addressing(
            PGN_HEARTBEAT_REQUEST,
            encoded[..7].to_vec(),
            0x22,
            0x23,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            HeartbeatRequest::from_message(&msg),
            None,
            "heartbeat request helper must reject wrong PGN invalid endpoints and malformed payload before decode"
        );
    }
}

#[test]
fn implement_heartbeat_receiver_reserved_sequences_do_not_refresh_comm_timer() {
    let mut receiver = HeartbeatReceiver::new();
    receiver.process(0);
    assert_eq!(receiver.state(), HbReceiverState::Normal);

    receiver.update(HB_COMM_ERROR_TIMEOUT_MS);
    assert_eq!(
        receiver.state(),
        HbReceiverState::Normal,
        "the receiver uses a strict timeout boundary and must not fire early"
    );

    receiver.process(hb_seq::RESERVED_LOW);
    receiver.update(1);
    assert_eq!(
        receiver.state(),
        HbReceiverState::CommError,
        "reserved heartbeat sequence bytes are ignored and must not refresh liveness"
    );
}

#[test]
fn implement_public_try_decoders_reject_noncanonical_packed_bytes() {
    assert_eq!(
        GuidanceLimitStatus::try_from_u8(3),
        Some(GuidanceLimitStatus::LimitedLow)
    );
    assert_eq!(GuidanceLimitStatus::try_from_u8(0xF8 | 3), None);
    assert_eq!(GuidanceLimitStatus::try_from_u8(4), None);

    assert_eq!(
        ValveLimitStatus::try_from_u8(2),
        Some(ValveLimitStatus::SystemLimited)
    );
    assert_eq!(ValveLimitStatus::try_from_u8(0xF8 | 2), None);
    assert_eq!(ValveLimitStatus::try_from_u8(5), None);

    assert_eq!(TractorMode::try_from_u8(1), Some(TractorMode::Automatic));
    assert_eq!(TractorMode::try_from_u8(0xFC | 1), None);
    assert_eq!(TractorMode::try_from_u8(2), None);
}

#[test]
fn implement_public_packed_status_decoders_reject_noncanonical_bytes() {
    for (raw, state) in [
        (0, LightState::Off),
        (1, LightState::On),
        (2, LightState::Error),
        (3, LightState::NotAvailable),
    ] {
        assert_eq!(LightState::try_from_u8(raw), Some(state));
        assert_eq!(LightState::from_u8(raw), state);
    }

    for (raw, direction) in [
        (0, MachineDirection::Reverse),
        (1, MachineDirection::Forward),
        (2, MachineDirection::Error),
        (3, MachineDirection::NotAvailable),
    ] {
        assert_eq!(MachineDirection::try_from_u8(raw), Some(direction));
        assert_eq!(MachineDirection::from_u8(raw), direction);
    }

    for (raw, source) in [
        (0, SpeedSource::WheelBased),
        (1, SpeedSource::GroundBased),
        (2, SpeedSource::NavigationBased),
        (3, SpeedSource::Blended),
    ] {
        assert_eq!(SpeedSource::try_from_u8(raw), Some(source));
        assert_eq!(SpeedSource::from_u8(raw), source);
    }

    for (raw, status) in [
        (0, SpeedExitCode::NotLimited),
        (1, SpeedExitCode::OperatorLimited),
        (2, SpeedExitCode::SystemLimited),
        (3, SpeedExitCode::NotAvailable),
    ] {
        assert_eq!(SpeedExitCode::try_from_u8(raw), Some(status));
        assert_eq!(SpeedExitCode::from_u8(raw), status);
    }

    for (raw, status) in [
        (0, LimitStatus::NotLimited),
        (1, LimitStatus::OperatorLimited),
        (2, LimitStatus::SystemLimited),
        (3, LimitStatus::NotAvailable),
    ] {
        assert_eq!(LimitStatus::try_from_u8(raw), Some(status));
        assert_eq!(LimitStatus::from_u8(raw), status);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(LightState::try_from_u8(packed_or_reserved), None);
        assert_eq!(MachineDirection::try_from_u8(packed_or_reserved), None);
        assert_eq!(SpeedExitCode::try_from_u8(packed_or_reserved), None);
        assert_eq!(LimitStatus::try_from_u8(packed_or_reserved), None);
    }

    // SpeedSource is a 3-bit field, so 4 (simulated) and 7 (not available) are
    // real values rather than reserved ones. Only 5 and 6 are unassigned.
    assert_eq!(SpeedSource::try_from_u8(4), Some(SpeedSource::Simulated));
    assert_eq!(SpeedSource::try_from_u8(7), Some(SpeedSource::NotAvailable));
    for reserved_or_packed in [5, 6, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(SpeedSource::try_from_u8(reserved_or_packed), None);
    }

    let lighting = LightingState {
        left_turn: LightState::On,
        right_turn: LightState::Error,
        beacon: LightState::On,
        running: LightState::Off,
        hazard: LightState::On,
        backup: LightState::Error,
        ..LightingState::default()
    };
    assert_eq!(LightingState::decode(&lighting.encode()), Some(lighting));

    let selected = MachineSelectedSpeedMsg {
        speed_raw: 3210,
        direction: MachineDirection::Forward,
        source: SpeedSource::GroundBased,
        limit_status: SpeedExitCode::OperatorLimited,
    };
    assert_eq!(
        MachineSelectedSpeedMsg::decode(&selected.encode()),
        Some(selected)
    );

    let selected_full = MachineSelectedSpeedFull {
        speed_mps: 4.2.into(),
        distance_m: 99.0.into(),
        direction: MachineDirection::Reverse,
        source: SpeedSource::NavigationBased,
        limit_status: 5,
        exit_code: 0x33,
    };
    let decoded_selected_full = MachineSelectedSpeedFull::decode(&selected_full.encode()).unwrap();
    assert_eq!(decoded_selected_full.direction, MachineDirection::Reverse);
    assert_eq!(decoded_selected_full.source, SpeedSource::NavigationBased);
    assert_eq!(decoded_selected_full.limit_status, 5);

    let wheel = WheelBasedSpeedDist {
        speed_mps: 1.25.into(),
        distance_m: 12.5.into(),
        direction: MachineDirection::Forward,
        key_switch_state: 1,
        implement_start_stop_operations_state: 2,
        operator_direction_reversed_state: 0,
        ..WheelBasedSpeedDist::default()
    };
    assert_eq!(
        WheelBasedSpeedDist::decode(&wheel.encode()).map(|decoded| decoded.direction),
        Some(MachineDirection::Forward)
    );

    let ground = GroundBasedSpeedDist {
        speed_mps: 1.25.into(),
        distance_m: 12.5.into(),
        direction: MachineDirection::Reverse,
    };
    assert_eq!(
        GroundBasedSpeedDist::decode(&ground.encode()).map(|decoded| decoded.direction),
        Some(MachineDirection::Reverse)
    );

    let hitch = HitchStatus {
        position_percent: Signal::Value(40.0),
        in_work_indication: 1,
        limit_status: LimitStatus::OperatorLimited,
        exit_code: ExitReasonCode::Fault,
        draft_force_n: Signal::Value(-10_000.0),
        is_rear: true,
    };
    assert_eq!(
        HitchStatus::decode(&hitch.encode(), true).map(|decoded| decoded.limit_status),
        Some(LimitStatus::OperatorLimited)
    );

    let pto = PtoStatus {
        shaft_speed_rpm: Signal::Value(540.0),
        engagement: 1,
        limit_status: LimitStatus::SystemLimited,
        exit_code: ExitReasonCode::Fault,
        economy_mode: 1,
        is_rear: false,
    };
    assert_eq!(
        PtoStatus::decode(&pto.encode(), false).map(|decoded| decoded.limit_status),
        Some(LimitStatus::SystemLimited)
    );
}

#[test]
fn implement_shortcut_button_rejects_wrong_pgn_invalid_source_and_noncanonical_payloads() {
    let payload = shortcut_button::encode_with_transition_count(
        ShortcutButtonState::StopImplementOperations,
        7,
    );
    let valid = Message::new(PGN_SHORTCUT_BUTTON, payload.to_vec(), 0x74);
    assert_eq!(
        shortcut_button::decode_message(&valid),
        Some(shortcut_button::ShortcutButtonMessage {
            state: ShortcutButtonState::StopImplementOperations,
            transition_count: 7,
        })
    );

    for msg in [
        Message::new(PGN_TIME_DATE, payload.to_vec(), 0x74),
        Message::new(PGN_SHORTCUT_BUTTON, payload.to_vec(), NULL_ADDRESS),
        Message::new(PGN_SHORTCUT_BUTTON, payload.to_vec(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_SHORTCUT_BUTTON,
            payload.to_vec(),
            0x74,
            0x42,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            shortcut_button::decode_message(&msg),
            None,
            "Shortcut Button status must be bound to the correct PGN and a usable source/destination envelope"
        );
    }

    let mut bad_reserved = payload;
    bad_reserved[7] = 0x04;
    assert_eq!(
        shortcut_button::decode_message(&Message::new(
            PGN_SHORTCUT_BUTTON,
            bad_reserved.to_vec(),
            0x74,
        )),
        None
    );

    let mut bad_tail = payload;
    bad_tail[0] = 0x00;
    assert_eq!(
        shortcut_button::decode_message(&Message::new(
            PGN_SHORTCUT_BUTTON,
            bad_tail.to_vec(),
            0x74,
        )),
        None
    );
}

#[test]
fn implement_shortcut_button_public_state_decoder_rejects_noncanonical_bytes() {
    for (raw, state) in [
        (0, ShortcutButtonState::StopImplementOperations),
        (1, ShortcutButtonState::PermitAllImplementsToOperate),
        (2, ShortcutButtonState::Error),
        (3, ShortcutButtonState::NotAvailable),
    ] {
        assert_eq!(ShortcutButtonState::try_from_u8(raw), Some(state));
        assert_eq!(ShortcutButtonState::from_u8(raw), state);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(
            ShortcutButtonState::try_from_u8(packed_or_reserved),
            None,
            "strict Shortcut Button state decoder must reject packed bytes"
        );
    }

    let payload = shortcut_button::encode_with_transition_count(
        ShortcutButtonState::PermitAllImplementsToOperate,
        3,
    );
    assert_eq!(
        shortcut_button::decode_message(
            &Message::new(PGN_SHORTCUT_BUTTON, payload.to_vec(), 0x74,)
        )
        .map(|decoded| decoded.state),
        Some(ShortcutButtonState::PermitAllImplementsToOperate)
    );
}

#[test]
fn implement_speed_distance_message_helper_rejects_invalid_envelopes() {
    let measurement = SpeedAndDistance {
        speed_mps: Some(2.5),
        distance_m: Some(42.0),
        timestamp_us: 0,
    };
    let mut valid = Message::new(PGN_WHEEL_SPEED, measurement.encode().to_vec(), 0x71);
    valid.timestamp_us = 55;
    let decoded = SpeedAndDistance::from_message(&valid)
        .expect("supported speed-distance PGN and usable source should decode");
    assert_eq!(decoded.timestamp_us, 55);
    assert_eq!(decoded.speed_mps, measurement.speed_mps);
    assert_eq!(decoded.distance_m, measurement.distance_m);

    for msg in [
        Message::new(PGN_TIME_DATE, measurement.encode().to_vec(), 0x71),
        Message::new(
            PGN_MACHINE_SPEED,
            measurement.encode().to_vec(),
            NULL_ADDRESS,
        ),
        Message::new(
            PGN_WHEEL_SPEED,
            measurement.encode().to_vec(),
            BROADCAST_ADDRESS,
        ),
        Message::with_addressing(
            PGN_WHEEL_SPEED,
            measurement.encode().to_vec(),
            0x71,
            0x42,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            SpeedAndDistance::from_message(&msg),
            None,
            "full-message speed-distance helpers must bind to supported PGNs and usable source/destination envelopes"
        );
    }
}

#[test]
fn implement_aux_valve_and_machine_speed_reject_reserved_payload_bits() {
    let aux = AuxValveCommandMsg {
        valve_index: 15,
        command: ValveCommand::Float,
        flow_rate: 250,
    };
    let aux_bytes = aux.encode();
    assert_eq!(AuxValveCommandMsg::decode(&aux_bytes), Some(aux));

    let mut bad_aux_index = aux_bytes;
    bad_aux_index[0] = 16;
    assert_eq!(AuxValveCommandMsg::decode(&bad_aux_index), None);

    let selected = MachineSelectedSpeedMsg {
        speed_raw: 1_234,
        direction: MachineDirection::Forward,
        source: SpeedSource::GroundBased,
        limit_status: SpeedExitCode::NotLimited,
    };
    let selected_bytes = selected.encode();
    assert_eq!(
        MachineSelectedSpeedMsg::decode(&selected_bytes),
        Some(selected)
    );

    let mut bad_reserved = selected_bytes;
    bad_reserved[4] &= 0x3F;
    assert_eq!(MachineSelectedSpeedMsg::decode(&bad_reserved), None);

    let command = MachineSpeedCommandMsg::default()
        .with_speed_mps(2.5)
        .with_direction(MachineDirection::Forward);
    let command_bytes = command.encode();
    assert_eq!(
        MachineSpeedCommandMsg::decode(&command_bytes),
        Some(command)
    );
}

#[test]
fn implement_machine_speed_command_rejects_noncanonical_shape_without_losing_sentinels() {
    let command = MachineSpeedCommandMsg::default()
        .with_speed_mps(65.535)
        .with_direction(MachineDirection::Forward);
    let encoded = command.encode();
    assert_eq!(command.target_speed_raw, 0xFFFE);
    assert_eq!(
        MachineSpeedCommandMsg::decode(&encoded),
        Some(command),
        "large commanded speeds clamp below the not-available sentinel"
    );

    let not_available = MachineSpeedCommandMsg::default();
    let not_available_decoded = MachineSpeedCommandMsg::decode(&not_available.encode()).unwrap();
    assert_eq!(not_available_decoded.target_speed_raw, 0xFFFF);
    assert_eq!(not_available_decoded.target_speed_mps(), 0.0);
    assert_eq!(
        not_available_decoded.direction_cmd,
        MachineDirection::NotAvailable
    );

    let mut bad_reserved_bits = encoded;
    bad_reserved_bits[2] &= 0x03;
    assert_eq!(MachineSpeedCommandMsg::decode(&bad_reserved_bits), None);

    let mut bad_tail = encoded;
    bad_tail[3] = 0x00;
    assert_eq!(MachineSpeedCommandMsg::decode(&bad_tail), None);
}

#[test]
fn implement_aux_valve_flow_rejects_reserved_limit_status_values() {
    let flow = AuxValveFlowMsg {
        valve_index: 3,
        extend_flow_percent: 125,
        retract_flow_percent: 250,
        state: ValveState::Extending,
        limit_status: ValveLimitStatus::SystemLimited,
        fail_safe: ValveFailSafe::Float,
    };
    let encoded = flow.encode();
    assert_eq!(AuxValveFlowMsg::decode(&encoded, 3), Some(flow));

    for reserved_limit in 3..=5 {
        let mut bad_limit = encoded;
        bad_limit[2] &= !(0x07 << 2);
        bad_limit[2] |= reserved_limit << 2;
        assert_eq!(AuxValveFlowMsg::decode(&bad_limit, 3), None);
    }

    let mut bad_reserved_bit = encoded;
    bad_reserved_bit[2] &= 0x7F;
    assert_eq!(AuxValveFlowMsg::decode(&bad_reserved_bit, 3), None);

    let mut bad_tail = encoded;
    bad_tail[7] = 0x00;
    assert_eq!(AuxValveFlowMsg::decode(&bad_tail, 3), None);

    assert_eq!(AuxValveFlowMsg::decode(&encoded, 16), None);
}

#[test]
fn implement_aux_valve_flow_rejects_reserved_flow_sentinel_band() {
    let flow = AuxValveFlowMsg {
        valve_index: 0,
        extend_flow_percent: 250,
        retract_flow_percent: 0xFF,
        state: ValveState::Blocked,
        limit_status: ValveLimitStatus::NotAvailable,
        fail_safe: ValveFailSafe::Block,
    };
    let encoded = flow.encode();
    assert_eq!(AuxValveFlowMsg::decode(&encoded, 0), Some(flow));

    for reserved_flow in 251..=254 {
        let mut bad_extend = encoded;
        bad_extend[0] = reserved_flow;
        assert_eq!(
            AuxValveFlowMsg::decode(&bad_extend, 0),
            None,
            "reserved extend-flow value {reserved_flow} must not decode as a percentage"
        );

        let mut bad_retract = encoded;
        bad_retract[1] = reserved_flow;
        assert_eq!(
            AuxValveFlowMsg::decode(&bad_retract, 0),
            None,
            "reserved retract-flow value {reserved_flow} must not decode as a percentage"
        );
    }
}

#[test]
fn implement_aux_valve_public_state_decoders_reject_noncanonical_bytes() {
    for (raw, state) in [
        (0, ValveState::Blocked),
        (1, ValveState::Extending),
        (2, ValveState::Retracting),
        (3, ValveState::FloatPosition),
    ] {
        assert_eq!(ValveState::try_from_u8(raw), Some(state));
        assert_eq!(ValveState::from_u8(raw), state);
    }

    for (raw, fail_safe) in [
        (0, ValveFailSafe::Block),
        (1, ValveFailSafe::Float),
        (2, ValveFailSafe::Extend),
        (3, ValveFailSafe::Retract),
    ] {
        assert_eq!(ValveFailSafe::try_from_u8(raw), Some(fail_safe));
        assert_eq!(ValveFailSafe::from_u8(raw), fail_safe);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(ValveState::try_from_u8(packed_or_reserved), None);
        assert_eq!(ValveFailSafe::try_from_u8(packed_or_reserved), None);
    }

    let flow = AuxValveFlowMsg {
        valve_index: 7,
        extend_flow_percent: 80,
        retract_flow_percent: 40,
        state: ValveState::Retracting,
        limit_status: ValveLimitStatus::Error,
        fail_safe: ValveFailSafe::Extend,
    };
    assert_eq!(AuxValveFlowMsg::decode(&flow.encode(), 7), Some(flow));
}

#[test]
fn implement_drive_strategy_preserves_upper_edge_values_and_rejects_reserved_modes() {
    let drive = DriveStrategyCmd {
        mode: DriveStrategyMode::MaxSpeed,
        target_speed_limit_percent: 0xFE,
        target_engine_load_percent: 0xFE,
    };
    let encoded = drive.encode();
    assert_eq!(DriveStrategyCmd::decode(&encoded), Some(drive));

    let mut bad_mode = encoded;
    bad_mode[0] = 0x04;
    assert_eq!(
        DriveStrategyCmd::decode(&bad_mode),
        None,
        "reserved drive-strategy mode values must be rejected without changing field edges"
    );

    let mut bad_tail = encoded;
    bad_tail[3] = 0x00;
    assert_eq!(
        DriveStrategyCmd::decode(&bad_tail),
        None,
        "fixed-width Drive Strategy commands must keep unused bytes canonical"
    );
}

#[test]
fn implement_guidance_system_command_rejects_reserved_bits_and_curvature_sentinel_band() {
    let command = GuidanceSystemCmd {
        commanded_curvature: 12.5,
        status: CurvatureCommandStatus::IntendedToSteer,
    };
    let encoded = command.encode();
    let decoded = GuidanceSystemCmd::decode(&encoded).unwrap();
    assert_eq!(decoded.status, CurvatureCommandStatus::IntendedToSteer);
    assert!((decoded.commanded_curvature - 12.5).abs() < 0.25);

    let low_edge = [0x00, 0x00, 0xFC, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(
        (GuidanceSystemCmd::decode(&low_edge)
            .unwrap()
            .commanded_curvature
            - -8032.0)
            .abs()
            < f64::EPSILON
    );

    let high_edge = [0xFF, 0xFA, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(
        (GuidanceSystemCmd::decode(&high_edge)
            .unwrap()
            .commanded_curvature
            - 8031.75)
            .abs()
            < f64::EPSILON
    );

    for invalid_raw in [0xFB00_u16, 0xFFFE, 0xFFFF] {
        let mut bad_curvature = encoded;
        bad_curvature[0..2].copy_from_slice(&invalid_raw.to_le_bytes());
        assert_eq!(
            GuidanceSystemCmd::decode(&bad_curvature),
            None,
            "curvature raw value 0x{invalid_raw:04X} must stay outside the accepted range"
        );
    }

    let mut bad_reserved_status_bits = encoded;
    bad_reserved_status_bits[2] &= 0x03;
    assert_eq!(GuidanceSystemCmd::decode(&bad_reserved_status_bits), None);

    let mut bad_tail = encoded;
    bad_tail[3] = 0x00;
    assert_eq!(GuidanceSystemCmd::decode(&bad_tail), None);
}

#[test]
fn implement_guidance_command_public_status_decoder_rejects_noncanonical_bytes() {
    for (raw, status) in [
        (0, CurvatureCommandStatus::NotIntendedToSteer),
        (1, CurvatureCommandStatus::IntendedToSteer),
        (2, CurvatureCommandStatus::ErrorIndication),
        (3, CurvatureCommandStatus::NotAvailable),
    ] {
        assert_eq!(CurvatureCommandStatus::try_from_u8(raw), Some(status));
        assert_eq!(CurvatureCommandStatus::from_u8(raw), status);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(
            CurvatureCommandStatus::try_from_u8(packed_or_reserved),
            None
        );
    }

    let command = GuidanceSystemCmd {
        commanded_curvature: -1.0,
        status: CurvatureCommandStatus::ErrorIndication,
    };
    assert_eq!(
        GuidanceSystemCmd::decode(&command.encode()).map(|decoded| decoded.status),
        Some(CurvatureCommandStatus::ErrorIndication)
    );
}

#[test]
fn implement_hitch_pto_combined_and_roll_pitch_commands_reject_reserved_controls_and_padding() {
    let combined = HitchPtoCombinedCmd {
        hitch_position: 40_000,
        pto_speed_raw: 4_320,
        hitch_cmd: 2,
        pto_cmd: 1,
    };
    let combined_bytes = combined.encode();
    assert_eq!(HitchPtoCombinedCmd::decode(&combined_bytes), Some(combined));
    assert!((combined.pto_speed_rpm() - 540.0).abs() < 0.125);

    let mut bad_combined_controls = combined_bytes;
    bad_combined_controls[4] |= 0x10;
    assert_eq!(HitchPtoCombinedCmd::decode(&bad_combined_controls), None);

    let mut bad_combined_tail = combined_bytes;
    bad_combined_tail[5] = 0x00;
    assert_eq!(HitchPtoCombinedCmd::decode(&bad_combined_tail), None);

    let front_roll_pitch = HitchRollPitchCmd {
        roll_position: 20_000,
        pitch_position: 40_000,
        is_front: true,
    };
    let rear_roll_pitch = HitchRollPitchCmd {
        is_front: false,
        ..front_roll_pitch
    };
    assert_eq!(front_roll_pitch.pgn(), PGN_FRONT_HITCH_ROLL_PITCH_CMD);
    assert_eq!(rear_roll_pitch.pgn(), PGN_REAR_HITCH_ROLL_PITCH_CMD);

    let roll_pitch_bytes = front_roll_pitch.encode();
    assert_eq!(
        HitchRollPitchCmd::decode(&roll_pitch_bytes, true),
        Some(front_roll_pitch)
    );
    assert_eq!(
        HitchRollPitchCmd::decode(&roll_pitch_bytes, false),
        Some(rear_roll_pitch)
    );

    let mut bad_roll_pitch_tail = roll_pitch_bytes;
    bad_roll_pitch_tail[4] = 0x00;
    assert_eq!(HitchRollPitchCmd::decode(&bad_roll_pitch_tail, true), None);
}

#[test]
fn implement_aux_valve_pgn_index_boundaries_are_explicit_and_symmetric() {
    let first = AuxValveCommandMsg {
        valve_index: 0,
        command: ValveCommand::Extend,
        flow_rate: 250,
    };
    let last = AuxValveCommandMsg {
        valve_index: MAX_AUX_VALVES - 1,
        command: ValveCommand::Retract,
        flow_rate: 0xFFFF,
    };
    assert_eq!(first.try_pgn(), Some(PGN_AUX_VALVE_CMD));
    assert_eq!(
        last.try_pgn(),
        Some(PGN_AUX_VALVE_CMD + u32::from(MAX_AUX_VALVES - 1))
    );
    assert_eq!(AuxValveCommandMsg::decode(&first.encode()), Some(first));
    assert_eq!(AuxValveCommandMsg::decode(&last.encode()), Some(last));

    let invalid = AuxValveCommandMsg {
        valve_index: MAX_AUX_VALVES,
        ..Default::default()
    };
    assert!(!invalid.has_valid_valve_index());
    assert_eq!(invalid.try_pgn(), None);
    assert_eq!(AuxValveCommandMsg::decode(&invalid.encode()), None);

    assert!(estimated_flow_pgn(0).is_some());
    assert!(measured_flow_pgn(0).is_some());
    assert!(estimated_flow_pgn(MAX_AUX_VALVES - 1).is_some());
    assert!(measured_flow_pgn(MAX_AUX_VALVES - 1).is_some());
    assert_eq!(estimated_flow_pgn(MAX_AUX_VALVES), None);
    assert_eq!(measured_flow_pgn(MAX_AUX_VALVES), None);
}

#[test]
fn implement_lighting_state_rejects_non_canonical_fixed_frames() {
    let lighting = LightingState {
        left_turn: LightState::On,
        right_turn: LightState::NotAvailable,
        low_beam: LightState::Off,
        high_beam: LightState::On,
        rear_work: LightState::Off,
        front_work: LightState::On,
        beacon: LightState::Off,
        hazard: LightState::On,
        ..Default::default()
    };
    let encoded = lighting.encode();
    assert_eq!(LightingState::decode(&encoded), Some(lighting));
    assert_eq!(LightingState::decode(&encoded[..7]), None);

    let mut bad_tail = encoded;
    bad_tail[4] = 0x00;
    assert_eq!(LightingState::decode(&bad_tail), None);
}

#[test]
fn implement_guidance_public_status_decoders_reject_noncanonical_bytes() {
    for (raw, status) in [
        (0, MechanicalLockout::NotActive),
        (1, MechanicalLockout::Active),
        (2, MechanicalLockout::Error),
        (3, MechanicalLockout::NotAvailable),
    ] {
        assert_eq!(MechanicalLockout::try_from_u8(raw), Some(status));
        assert_eq!(MechanicalLockout::from_u8(raw), status);
    }

    for (raw, status) in [
        (0, GenericSaeBs02SlotValue::DisabledOffPassive),
        (1, GenericSaeBs02SlotValue::EnabledOnActive),
        (2, GenericSaeBs02SlotValue::ErrorIndication),
        (3, GenericSaeBs02SlotValue::NotAvailableTakeNoAction),
    ] {
        assert_eq!(GenericSaeBs02SlotValue::try_from_u8(raw), Some(status));
        assert_eq!(GenericSaeBs02SlotValue::from_u8(raw), status);
    }

    for (raw, status) in [
        (0, RequestResetCommandStatus::ResetNotRequired),
        (1, RequestResetCommandStatus::ResetRequired),
        (2, RequestResetCommandStatus::Error),
        (3, RequestResetCommandStatus::NotAvailable),
    ] {
        assert_eq!(RequestResetCommandStatus::try_from_u8(raw), Some(status));
        assert_eq!(RequestResetCommandStatus::from_u8(raw), status);
    }

    for packed_or_reserved in [0x04, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(MechanicalLockout::try_from_u8(packed_or_reserved), None);
        assert_eq!(
            GenericSaeBs02SlotValue::try_from_u8(packed_or_reserved),
            None
        );
        assert_eq!(
            RequestResetCommandStatus::try_from_u8(packed_or_reserved),
            None
        );
    }

    for reserved_or_packed in [4, 5, 0x08, 0x10, 0x40, 0xFF] {
        assert_eq!(GuidanceLimitStatus::try_from_u8(reserved_or_packed), None);
    }

    let info = GuidanceMachineInfo {
        estimated_curvature: Some(0.75),
        lockout: MechanicalLockout::Active,
        steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
        steering_input_position_status: GenericSaeBs02SlotValue::ErrorIndication,
        request_reset_status: RequestResetCommandStatus::ResetRequired,
        guidance_limit_status: GuidanceLimitStatus::NonRecoverableFault,
        guidance_system_command_exit_reason_code: 12,
        remote_engage_switch_status: GenericSaeBs02SlotValue::DisabledOffPassive,
    };
    assert_eq!(GuidanceMachineInfo::decode(&info.encode()), Some(info));
}

#[test]
fn implement_guidance_messages_reject_reserved_status_values_and_padding() {
    let info = GuidanceMachineInfo {
        estimated_curvature: Some(-2.5),
        lockout: MechanicalLockout::Active,
        steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
        steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
        request_reset_status: RequestResetCommandStatus::ResetRequired,
        guidance_limit_status: GuidanceLimitStatus::LimitedLow,
        guidance_system_command_exit_reason_code: 27,
        remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
    };
    let encoded = info.encode();
    let decoded = GuidanceMachineInfo::decode(&encoded).unwrap();
    assert!(decoded.estimated_curvature.is_some_and(|k| (k + 2.5).abs() < 0.25));
    assert_eq!(
        decoded.guidance_limit_status,
        GuidanceLimitStatus::LimitedLow
    );

    for reserved_limit in [4, 5] {
        let mut bad_limit = encoded;
        bad_limit[3] &= 0x1F;
        bad_limit[3] |= reserved_limit << 5;
        assert_eq!(GuidanceMachineInfo::decode(&bad_limit), None);
    }

    let mut bad_lower_reserved_bits = encoded;
    bad_lower_reserved_bits[3] |= 0x01;
    assert!(GuidanceMachineInfo::decode(&bad_lower_reserved_bits).is_some());

    let mut bad_tail = encoded;
    bad_tail[5] = 0x00;
    assert_eq!(GuidanceMachineInfo::decode(&bad_tail), None);
}

#[test]
fn implement_hitch_status_rejects_reserved_exit_codes_without_payload_slop() {
    let status = HitchStatus {
        position_percent: Signal::Value(80.0),
        in_work_indication: 1,
        limit_status: LimitStatus::OperatorLimited,
        exit_code: ExitReasonCode::Fault,
        draft_force_n: Signal::Value(-100_000.0),
        is_rear: true,
    };
    let encoded = status.encode();
    assert_eq!(
        HitchStatus::decode(&encoded, true).unwrap().exit_code,
        ExitReasonCode::Fault
    );

    for reserved_exit in 4..=6 {
        let mut bad_exit = encoded;
        bad_exit[1] &= 0x8F;
        bad_exit[1] |= reserved_exit << 4;
        assert_eq!(HitchStatus::decode(&bad_exit, true), None);
    }

    let mut bad_reserved_bit = encoded;
    bad_reserved_bit[1] |= 0x80;
    assert_eq!(HitchStatus::decode(&bad_reserved_bit, true), None);

    let mut bad_tail = encoded;
    bad_tail[4] = 0x00;
    assert_eq!(HitchStatus::decode(&bad_tail, true), None);
}

#[test]
fn implement_exit_reason_public_decoders_reject_noncanonical_bytes() {
    for (raw, code) in [
        (0, ExitReasonCode::NoExit),
        (1, ExitReasonCode::OperatorCmd),
        (2, ExitReasonCode::SystemCmd),
        (3, ExitReasonCode::Fault),
        (7, ExitReasonCode::NotAvailable),
    ] {
        assert_eq!(ExitReasonCode::try_from_hitch_u8(raw), Some(code));
        assert_eq!(ExitReasonCode::from_u8(raw), code);
    }

    for raw in 0..=3 {
        assert_eq!(
            ExitReasonCode::try_from_pto_u8(raw),
            Some(ExitReasonCode::from_u8(raw))
        );
    }

    for packed_or_reserved in [4, 5, 6, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(ExitReasonCode::try_from_hitch_u8(packed_or_reserved), None);
    }
    for packed_or_reserved in [4, 5, 6, 7, 0x08, 0x10, 0x40, 0xFC, 0xFF] {
        assert_eq!(ExitReasonCode::try_from_pto_u8(packed_or_reserved), None);
    }

    let pto = PtoStatus {
        shaft_speed_rpm: Signal::Value(540.0),
        engagement: 1,
        limit_status: LimitStatus::SystemLimited,
        exit_code: ExitReasonCode::Fault,
        economy_mode: 0,
        is_rear: true,
    };
    assert_eq!(
        PtoStatus::decode(&pto.encode(), true).map(|decoded| decoded.exit_code),
        Some(ExitReasonCode::Fault)
    );
}

#[test]
fn implement_speed_distance_status_rejects_unavailable_speed_and_reserved_position_values() {
    let wheel = WheelBasedSpeedDist {
        speed_mps: 5.5.into(),
        distance_m: 12_345.0.into(),
        direction: MachineDirection::Forward,
        max_power_time_min: 60,
        key_switch_state: 1,
        implement_start_stop_operations_state: 1,
        operator_direction_reversed_state: 0,
    };
    let wheel_bytes = wheel.encode();
    assert_eq!(
        WheelBasedSpeedDist::decode(&wheel_bytes).unwrap().direction,
        MachineDirection::Forward
    );
    // A speed the TECU does not provide is reported as such; the rest of the
    // PG (direction, key switch, power time) is still mandatory data and must
    // survive. Dropping the whole frame here is what blinded the stack to
    // every real ground-speed message.
    let mut na_wheel_speed = wheel_bytes;
    na_wheel_speed[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    let decoded = WheelBasedSpeedDist::decode(&na_wheel_speed)
        .expect("a not-available speed must not discard the whole PG");
    assert!(decoded.speed_mps.is_not_available());
    assert_eq!(decoded.speed_mps.value(), None);
    assert_eq!(decoded.direction, MachineDirection::Forward);
    assert_eq!(decoded.max_power_time_min, 60);

    let ground = GroundBasedSpeedDist {
        speed_mps: 3.0.into(),
        distance_m: 100.0.into(),
        direction: MachineDirection::Reverse,
    };
    let ground_bytes = ground.encode();
    assert_eq!(GroundBasedSpeedDist::decode(&ground_bytes), Some(ground));
    let mut na_ground_speed = ground_bytes;
    na_ground_speed[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    let decoded_ground = GroundBasedSpeedDist::decode(&na_ground_speed)
        .expect("a machine with no ground-speed sensor still sends a valid PG");
    assert!(decoded_ground.speed_mps.is_not_available());
    assert_eq!(decoded_ground.direction, MachineDirection::Reverse);

    let selected = MachineSelectedSpeedFull {
        speed_mps: 2.5.into(),
        distance_m: 1_000.0.into(),
        direction: MachineDirection::Forward,
        source: SpeedSource::GroundBased,
        limit_status: 1,
        exit_code: 0x42,
    };
    let selected_bytes = selected.encode();
    assert_eq!(
        MachineSelectedSpeedFull::decode(&selected_bytes)
            .unwrap()
            .source,
        SpeedSource::GroundBased
    );
    let mut na_selected_speed = selected_bytes;
    na_selected_speed[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    let decoded_selected = MachineSelectedSpeedFull::decode(&na_selected_speed)
        .expect("a not-available selected speed must not discard the whole PG");
    assert!(decoded_selected.speed_mps.is_not_available());
    assert_eq!(decoded_selected.source, SpeedSource::GroundBased);

    let pto = PtoStatus {
        shaft_speed_rpm: Signal::Value(540.0),
        engagement: 1,
        limit_status: LimitStatus::SystemLimited,
        exit_code: ExitReasonCode::Fault,
        economy_mode: 0,
        is_rear: false,
    };
    let pto_bytes = pto.encode();
    assert_eq!(
        PtoStatus::decode(&pto_bytes, false).unwrap().limit_status,
        LimitStatus::SystemLimited
    );
    // K2 — ISO 11783-9 §4.4.2.1 requires a tractor with no PTO fitted to
    // broadcast not-available here. Dropping the frame hid the engagement,
    // economy mode, limit status and exit code in the same byte (G4).
    let mut no_pto_fitted = pto_bytes;
    no_pto_fitted[0..2].copy_from_slice(&u16::MAX.to_le_bytes());
    let decoded = PtoStatus::decode(&no_pto_fitted, false)
        .expect("an unfitted PTO must not drop the whole PG");
    assert_eq!(decoded.shaft_speed_rpm, Signal::NotAvailable);
    assert_eq!(decoded.limit_status, LimitStatus::SystemLimited);
    assert_eq!(decoded.exit_code, ExitReasonCode::Fault);

    let hitch = HitchStatus {
        position_percent: Signal::Value(100.0),
        in_work_indication: 1,
        limit_status: LimitStatus::OperatorLimited,
        exit_code: ExitReasonCode::Fault,
        draft_force_n: Signal::Value(-100_000.0),
        is_rear: true,
    };
    let hitch_bytes = hitch.encode();
    // Raw 250 is the top of the 0.4 %/bit range: 100 %.
    assert_eq!(hitch_bytes[0], 250);
    assert_eq!(
        HitchStatus::decode(&hitch_bytes, true)
            .unwrap()
            .position_percent,
        Signal::Value(100.0)
    );
    let mut not_available_hitch_position = hitch_bytes;
    not_available_hitch_position[0] = 0xFF;
    assert_eq!(
        HitchStatus::decode(&not_available_hitch_position, true)
            .unwrap()
            .position_percent,
        Signal::NotAvailable
    );
    // B4 — 0xFE is the error indicator, not a decode failure: a hitch whose
    // position sensor has failed still reports the limit status and exit code
    // that say why (G4).
    let mut faulted_hitch_position = hitch_bytes;
    faulted_hitch_position[0] = 0xFE;
    let faulted = HitchStatus::decode(&faulted_hitch_position, true)
        .expect("a failed position sensor must not drop the PG");
    assert_eq!(faulted.position_percent, Signal::Error);
    assert_eq!(faulted.limit_status, LimitStatus::OperatorLimited);
    assert_eq!(faulted.exit_code, ExitReasonCode::Fault);
    // K1 — ISO 11783-9 §4.4.2.1 requires a tractor with no draft sensor to
    // broadcast not-available here, so the frame must still decode.
    let mut no_draft_sensor = hitch_bytes;
    no_draft_sensor[2..4].copy_from_slice(&0xFFFFu16.to_le_bytes());
    let no_draft = HitchStatus::decode(&no_draft_sensor, true)
        .expect("an unfitted draft sensor must not drop the PG");
    assert_eq!(no_draft.draft_force_n, Signal::NotAvailable);
    assert_eq!(no_draft.position_percent, Signal::Value(100.0));
    for reserved_position in 251..=253 {
        let mut bad_hitch_position = hitch_bytes;
        bad_hitch_position[0] = reserved_position;
        assert_eq!(
            HitchStatus::decode(&bad_hitch_position, true),
            None,
            "reserved hitch position value {reserved_position} must not decode as a percentage"
        );
    }
}

#[test]
fn implement_speed_distance_status_rejects_reserved_signal_ranges_before_scaling() {
    let wheel = WheelBasedSpeedDist {
        speed_mps: 5.5.into(),
        distance_m: 12_345.0.into(),
        direction: MachineDirection::Forward,
        max_power_time_min: 60,
        key_switch_state: 1,
        implement_start_stop_operations_state: 1,
        operator_direction_reversed_state: 0,
    };
    let wheel_bytes = wheel.encode();
    // Only the reserved band is a genuine decode failure. The error and
    // not-available bands are states the transmitter is entitled to report,
    // and they must be distinguishable from each other.
    let mut reserved = wheel_bytes;
    reserved[0..2].copy_from_slice(&0xFB00_u16.to_le_bytes());
    assert_eq!(WheelBasedSpeedDist::decode(&reserved), None);

    for (raw, expect_error) in [(0xFE00_u16, true), (0xFFFF, false)] {
        let mut frame = wheel_bytes;
        frame[0..2].copy_from_slice(&raw.to_le_bytes());
        let decoded = WheelBasedSpeedDist::decode(&frame)
            .unwrap_or_else(|| panic!("raw 0x{raw:04X} is a reportable state, not a bad frame"));
        assert_eq!(
            decoded.speed_mps.is_error(),
            expect_error,
            "raw 0x{raw:04X}"
        );
        assert_eq!(
            decoded.speed_mps.is_not_available(),
            !expect_error,
            "raw 0x{raw:04X}"
        );
        assert_eq!(decoded.distance_m.value(), Some(12_345.0));
    }

    let mut reserved_distance = wheel_bytes;
    reserved_distance[2..6].copy_from_slice(&0xFB00_0000_u32.to_le_bytes());
    assert_eq!(WheelBasedSpeedDist::decode(&reserved_distance), None);

    for (raw, expect_error) in [(0xFE00_0000_u32, true), (0xFFFF_FFFF, false)] {
        let mut frame = wheel_bytes;
        frame[2..6].copy_from_slice(&raw.to_le_bytes());
        let decoded = WheelBasedSpeedDist::decode(&frame)
            .unwrap_or_else(|| panic!("raw 0x{raw:08X} is a reportable state, not a bad frame"));
        assert_eq!(
            decoded.distance_m.is_error(),
            expect_error,
            "raw 0x{raw:08X}"
        );
        // A bad distance must never take a valid speed down with it.
        assert_eq!(decoded.speed_mps.value(), Some(5.5));
    }

    let ground = GroundBasedSpeedDist {
        speed_mps: 3.0.into(),
        distance_m: 100.0.into(),
        direction: MachineDirection::Reverse,
    };
    let ground_bytes = ground.encode();
    let mut bad_ground_speed = ground_bytes;
    bad_ground_speed[0..2].copy_from_slice(&0xFB00_u16.to_le_bytes());
    assert_eq!(GroundBasedSpeedDist::decode(&bad_ground_speed), None);
    let mut bad_ground_distance = ground_bytes;
    bad_ground_distance[2..6].copy_from_slice(&0xFB00_0000_u32.to_le_bytes());
    assert_eq!(GroundBasedSpeedDist::decode(&bad_ground_distance), None);

    // The error band is reportable, not malformed.
    let mut error_ground_speed = ground_bytes;
    error_ground_speed[0..2].copy_from_slice(&0xFE00_u16.to_le_bytes());
    assert!(
        GroundBasedSpeedDist::decode(&error_ground_speed)
            .expect("error band decodes")
            .speed_mps
            .is_error()
    );

    let selected = MachineSelectedSpeedFull {
        speed_mps: 2.5.into(),
        distance_m: 1_000.0.into(),
        direction: MachineDirection::Forward,
        source: SpeedSource::GroundBased,
        limit_status: 1,
        exit_code: 0x42,
    };
    let selected_bytes = selected.encode();
    let mut error_selected_speed = selected_bytes;
    error_selected_speed[0..2].copy_from_slice(&0xFE00_u16.to_le_bytes());
    assert!(
        MachineSelectedSpeedFull::decode(&error_selected_speed)
            .expect("error band decodes")
            .speed_mps
            .is_error()
    );
    let mut error_selected_distance = selected_bytes;
    error_selected_distance[2..6].copy_from_slice(&0xFE00_0000_u32.to_le_bytes());
    assert!(
        MachineSelectedSpeedFull::decode(&error_selected_distance)
            .expect("error band decodes")
            .distance_m
            .is_error()
    );
    // Reserved raws remain genuine decode failures.
    let mut reserved_selected = selected_bytes;
    reserved_selected[0..2].copy_from_slice(&0xFB00_u16.to_le_bytes());
    assert_eq!(MachineSelectedSpeedFull::decode(&reserved_selected), None);

    let pto = PtoStatus {
        shaft_speed_rpm: Signal::Value(540.0),
        engagement: 1,
        limit_status: LimitStatus::SystemLimited,
        exit_code: ExitReasonCode::Fault,
        economy_mode: 0,
        is_rear: false,
    };
    let pto_bytes = pto.encode();
    // The indicator bands are reported; only a reserved raw is a decode failure.
    for (raw, expected) in [
        (0xFE00_u16, Signal::Error),
        (0xFFFF, Signal::NotAvailable),
    ] {
        let mut special = pto_bytes;
        special[0..2].copy_from_slice(&raw.to_le_bytes());
        let decoded = PtoStatus::decode(&special, false)
            .unwrap_or_else(|| panic!("PTO speed raw {raw:#06X} must not drop the PG"));
        assert_eq!(decoded.shaft_speed_rpm, expected);
        assert_eq!(decoded.engagement, 1);
    }
    let mut reserved_pto = pto_bytes;
    reserved_pto[0..2].copy_from_slice(&0xFB00u16.to_le_bytes());
    assert_eq!(PtoStatus::decode(&reserved_pto, false), None);

    let hitch = HitchStatus {
        position_percent: Signal::Value(100.0),
        in_work_indication: 1,
        limit_status: LimitStatus::OperatorLimited,
        exit_code: ExitReasonCode::Fault,
        draft_force_n: Signal::Value(-100_000.0),
        is_rear: true,
    };
    let hitch_bytes = hitch.encode();
    // K1/G4 — the indicator bands are reported, not fatal: only a reserved raw
    // is a decode failure. A tractor with no draft sensor is *required* by
    // ISO 11783-9 §4.4.2.1 to broadcast 0xFFFF here.
    for (raw, expected) in [
        (0xFE00_u16, Signal::Error),
        (0xFFFF, Signal::NotAvailable),
    ] {
        let mut special = hitch_bytes;
        special[2..4].copy_from_slice(&raw.to_le_bytes());
        let decoded = HitchStatus::decode(&special, true)
            .unwrap_or_else(|| panic!("draft raw {raw:#06X} must not drop the PG"));
        assert_eq!(decoded.draft_force_n, expected);
        assert_eq!(decoded.position_percent, Signal::Value(100.0));
    }
    let mut reserved = hitch_bytes;
    reserved[2..4].copy_from_slice(&0xFB00u16.to_le_bytes());
    assert_eq!(HitchStatus::decode(&reserved, true), None);

    let saturated = WheelBasedSpeedDist {
        speed_mps: 1.0e9.into(),
        distance_m: 1.0e12.into(),
        ..wheel
    }
    .encode();
    assert_eq!(&saturated[0..2], &0xFAFF_u16.to_le_bytes());
    assert_eq!(&saturated[2..6], &0xFAFF_FFFF_u32.to_le_bytes());
    assert!(WheelBasedSpeedDist::decode(&saturated).is_some());
}

#[test]
fn implement_tractor_control_mode_rejects_reserved_modes_and_non_ff_tail() {
    let control = TractorControlModeMsg {
        hitch_mode: TractorMode::Manual,
        pto_mode: TractorMode::Automatic,
        front_hitch_mode: TractorMode::NotAvailable,
        front_pto_mode: TractorMode::Manual,
        speed_control_state: 0x55,
    };
    let encoded = control.encode();
    assert_eq!(TractorControlModeMsg::decode(&encoded), Some(control));

    for shift in [0, 2, 4, 6] {
        let mut reserved_slot = encoded;
        reserved_slot[0] &= !(0x03 << shift);
        reserved_slot[0] |= 0x02 << shift;
        assert_eq!(TractorControlModeMsg::decode(&reserved_slot), None);
    }

    let mut bad_tail = encoded;
    bad_tail[2] = 0x00;
    assert_eq!(TractorControlModeMsg::decode(&bad_tail), None);
}
