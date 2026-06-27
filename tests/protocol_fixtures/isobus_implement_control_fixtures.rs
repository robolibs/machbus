#[test]
fn fixture_isobus_implement_controls_status_vectors_are_stable() {
    let rear_pto_cmd = PtoCommandMsg {
        command: PtoCommand::Engage,
        target_speed_rpm: 4320,
        ramp_rate: 50,
    };
    let rear_pto_cmd_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "rear_pto_cmd_engage_540rpm",
    );
    assert_eq!(rear_pto_cmd.encode(), rear_pto_cmd_bytes);
    assert_eq!(
        PtoCommandMsg::decode(&rear_pto_cmd_bytes).unwrap(),
        rear_pto_cmd
    );
    let rear_pto_cmd_frame = Frame::from_message(
        Priority::Default,
        PGN_REAR_PTO_CMD,
        0x80,
        BROADCAST_ADDRESS,
        &rear_pto_cmd_bytes,
    );
    assert_eq!(rear_pto_cmd_frame.pgn(), PGN_REAR_PTO_CMD);

    let aux_cmd = AuxValveCommandMsg {
        valve_index: 7,
        command: ValveCommand::Extend,
        flow_rate: 250,
    };
    let aux_cmd_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "aux_valve_cmd_7_extend_100pct",
    );
    assert_eq!(aux_cmd.encode(), aux_cmd_bytes);
    assert_eq!(AuxValveCommandMsg::decode(&aux_cmd_bytes).unwrap(), aux_cmd);
    assert_eq!(aux_cmd.pgn(), PGN_AUX_VALVE_CMD + 7);

    let control_mode = TractorControlModeMsg {
        hitch_mode: TractorMode::Automatic,
        pto_mode: TractorMode::Manual,
        front_hitch_mode: TractorMode::NotAvailable,
        front_pto_mode: TractorMode::Automatic,
        speed_control_state: 0x5A,
    };
    let control_mode_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "tractor_control_mode_mixed",
    );
    assert_eq!(control_mode.encode(), control_mode_bytes);
    assert_eq!(
        TractorControlModeMsg::decode(&control_mode_bytes).unwrap(),
        control_mode
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_TRACTOR_CONTROL_MODE,
            0x80,
            BROADCAST_ADDRESS,
            &control_mode_bytes,
        )
        .pgn(),
        PGN_TRACTOR_CONTROL_MODE
    );

    let wheel = WheelBasedSpeedDist {
        speed_mps: 5.5,
        distance_m: 12_345.678,
        direction: MachineDirection::Forward,
        max_power_time_min: 60,
        key_switch_state: 1,
        implement_start_stop_operations_state: 1,
        operator_direction_reversed_state: 0,
    };
    let wheel_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "wheel_speed_dist_5_5mps_12345_678m",
    );
    assert_eq!(wheel.encode(), wheel_bytes);
    let decoded_wheel = WheelBasedSpeedDist::decode(&wheel_bytes).unwrap();
    assert!((decoded_wheel.speed_mps - wheel.speed_mps).abs() < 1e-3);
    assert!((decoded_wheel.distance_m - wheel.distance_m).abs() < 1e-3);
    assert_eq!(decoded_wheel.direction, MachineDirection::Forward);
    assert_eq!(decoded_wheel.max_power_time_min, 60);
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_WHEEL_BASED_SPEED_DIST,
            0x80,
            BROADCAST_ADDRESS,
            &wheel_bytes,
        )
        .pgn(),
        PGN_WHEEL_BASED_SPEED_DIST
    );

    let ground = GroundBasedSpeedDist {
        speed_mps: 3.0,
        distance_m: 100.0,
        direction: MachineDirection::Reverse,
    };
    let ground_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "ground_speed_dist_3mps_100m_reverse",
    );
    assert_eq!(ground.encode(), ground_bytes);
    assert_eq!(
        GroundBasedSpeedDist::decode(&ground_bytes)
            .unwrap()
            .direction,
        MachineDirection::Reverse
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_GROUND_BASED_SPEED_DIST,
            0x80,
            BROADCAST_ADDRESS,
            &ground_bytes,
        )
        .pgn(),
        PGN_GROUND_BASED_SPEED_DIST
    );

    let selected_full = MachineSelectedSpeedFull {
        speed_mps: 2.5,
        distance_m: 1000.0,
        direction: MachineDirection::Forward,
        source: SpeedSource::GroundBased,
        limit_status: 1,
        exit_code: 0x42,
    };
    let selected_full_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_selected_speed_full_2_5mps_1000m",
    );
    assert_eq!(selected_full.encode(), selected_full_bytes);
    let decoded_selected_full = MachineSelectedSpeedFull::decode(&selected_full_bytes).unwrap();
    assert!((decoded_selected_full.speed_mps - 2.5).abs() < 1e-3);
    assert_eq!(decoded_selected_full.source, SpeedSource::GroundBased);

    let selected_status = MachineSelectedSpeedMsg {
        speed_raw: 5000,
        direction: MachineDirection::Forward,
        source: SpeedSource::GroundBased,
        limit_status: SpeedExitCode::OperatorLimited,
    };
    let selected_status_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_selected_speed_status_5mps",
    );
    assert_eq!(selected_status.encode(), selected_status_bytes);
    assert_eq!(
        MachineSelectedSpeedMsg::decode(&selected_status_bytes).unwrap(),
        selected_status
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_MACHINE_SELECTED_SPEED,
            0x80,
            BROADCAST_ADDRESS,
            &selected_status_bytes,
        )
        .pgn(),
        PGN_MACHINE_SELECTED_SPEED
    );

    let speed_cmd = MachineSpeedCommandMsg::default()
        .with_speed_mps(2.5)
        .with_direction(MachineDirection::Forward);
    let speed_cmd_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_speed_cmd_2_5mps_forward",
    );
    assert_eq!(speed_cmd.encode(), speed_cmd_bytes);
    assert_eq!(
        MachineSpeedCommandMsg::decode(&speed_cmd_bytes).unwrap(),
        speed_cmd
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_MACHINE_SELECTED_SPEED_CMD,
            0x80,
            BROADCAST_ADDRESS,
            &speed_cmd_bytes,
        )
        .pgn(),
        PGN_MACHINE_SELECTED_SPEED_CMD
    );

    let rear_hitch_status = HitchStatus {
        position_percent: 200,
        in_work_indication: 1,
        limit_status: LimitStatus::OperatorLimited,
        exit_code: ExitReasonCode::OperatorCmd,
        draft_force_n: -100_000.0,
        is_rear: true,
    };
    let rear_hitch_status_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_status_rear_80pct_operator",
    );
    assert_eq!(rear_hitch_status.encode(), rear_hitch_status_bytes);
    let decoded_hitch = HitchStatus::decode(&rear_hitch_status_bytes, true).unwrap();
    assert_eq!(decoded_hitch.limit_status, LimitStatus::OperatorLimited);
    assert_eq!(decoded_hitch.exit_code, ExitReasonCode::OperatorCmd);

    let front_pto_status = PtoStatus {
        shaft_speed_rpm: 540.0,
        engagement: 1,
        limit_status: LimitStatus::SystemLimited,
        exit_code: ExitReasonCode::Fault,
        economy_mode: 0,
        is_rear: false,
    };
    let front_pto_status_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "pto_status_front_540rpm",
    );
    assert_eq!(front_pto_status.encode(), front_pto_status_bytes);
    assert_eq!(
        PtoStatus::decode(&front_pto_status_bytes, false)
            .unwrap()
            .pgn(),
        PGN_FRONT_PTO
    );

    let aux_flow = AuxValveFlowMsg {
        valve_index: 3,
        extend_flow_percent: 200,
        retract_flow_percent: 50,
        state: ValveState::Extending,
        limit_status: ValveLimitStatus::OperatorLimited,
        fail_safe: ValveFailSafe::Float,
    };
    let aux_flow_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "aux_valve_flow_3_extending",
    );
    assert_eq!(aux_flow.encode(), aux_flow_bytes);
    assert_eq!(
        AuxValveFlowMsg::decode(&aux_flow_bytes, 3).unwrap(),
        aux_flow
    );
    assert_eq!(
        estimated_flow_pgn(3),
        Some(PGN_AUX_VALVE_ESTIMATED_FLOW_BASE + 3)
    );
    assert_eq!(
        measured_flow_pgn(3),
        Some(PGN_AUX_VALVE_MEASURED_FLOW_BASE + 3)
    );

    let lighting = LightingState {
        left_turn: LightState::On,
        right_turn: LightState::Off,
        low_beam: LightState::On,
        high_beam: LightState::NotAvailable,
        beacon: LightState::On,
        front_work: LightState::On,
        rear_work: LightState::On,
        backup: LightState::Off,
        ..Default::default()
    };
    let lighting_bytes =
        parse_named_hex_frame(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, "lighting_typical");
    assert_eq!(lighting.encode(), lighting_bytes);
    assert_eq!(LightingState::decode(&lighting_bytes).unwrap(), lighting);
    assert_eq!(
        LightingState::from_message(&Message::new(
            PGN_LIGHTING_DATA,
            lighting_bytes.to_vec(),
            0x80,
        ))
        .unwrap(),
        lighting
    );

    let curvature = CurvatureCommand {
        curvature: 0.5,
        curvature_rate: 0.0,
    };
    let curvature_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_curvature_cmd_0_5",
    );
    assert_eq!(curvature.encode(), curvature_bytes);
    assert!(
        (CurvatureCommand::decode(&curvature_bytes)
            .unwrap()
            .curvature
            - 0.5)
            .abs()
            < 0.25
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_GUIDANCE_CURVATURE_CMD,
            0x80,
            BROADCAST_ADDRESS,
            &curvature_bytes,
        )
        .pgn(),
        PGN_GUIDANCE_CURVATURE_CMD
    );

    let machine_info = GuidanceMachineInfo {
        estimated_curvature: -2.5,
        lockout: MechanicalLockout::Active,
        steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
        steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
        request_reset_status: RequestResetCommandStatus::ResetRequired,
        guidance_limit_status: GuidanceLimitStatus::LimitedLow,
        guidance_system_command_exit_reason_code: 27,
        remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
    };
    let machine_info_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_machine_info_neg2_5",
    );
    assert_eq!(machine_info.encode(), machine_info_bytes);
    assert_eq!(
        GuidanceMachineInfo::decode(&machine_info_bytes)
            .unwrap()
            .lockout,
        MechanicalLockout::Active
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_GUIDANCE_MACHINE_INFO,
            0x80,
            BROADCAST_ADDRESS,
            &machine_info_bytes,
        )
        .pgn(),
        PGN_GUIDANCE_MACHINE_INFO
    );

    let system_status = GuidanceSystemStatus {
        estimated_curvature: 1.0,
        readiness: SteeringReadiness::FullyReady,
        integrity_level: 2,
    };
    let system_status_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_system_status_1_0",
    );
    assert_eq!(system_status.encode(), system_status_bytes);
    assert_eq!(
        GuidanceSystemStatus::decode(&system_status_bytes)
            .unwrap()
            .readiness,
        SteeringReadiness::FullyReady
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_GUIDANCE_SYSTEM,
            0x80,
            BROADCAST_ADDRESS,
            &system_status_bytes,
        )
        .pgn(),
        PGN_GUIDANCE_SYSTEM
    );

    let drive_strategy = DriveStrategyCmd {
        mode: DriveStrategyMode::MaxEconomy,
        target_speed_limit_percent: 200,
        target_engine_load_percent: 150,
    };
    let drive_strategy_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "drive_strategy_max_economy",
    );
    assert_eq!(drive_strategy.encode(), drive_strategy_bytes);
    assert_eq!(
        DriveStrategyCmd::decode(&drive_strategy_bytes).unwrap(),
        drive_strategy
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_DRIVE_STRATEGY_CMD,
            0x80,
            BROADCAST_ADDRESS,
            &drive_strategy_bytes,
        )
        .pgn(),
        PGN_DRIVE_STRATEGY_CMD
    );

    let guidance_system_cmd = GuidanceSystemCmd {
        commanded_curvature: -1.5,
        status: CurvatureCommandStatus::IntendedToSteer,
    };
    let guidance_system_cmd_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_system_cmd_neg1_5",
    );
    assert_eq!(guidance_system_cmd.encode(), guidance_system_cmd_bytes);
    assert_eq!(
        GuidanceSystemCmd::decode(&guidance_system_cmd_bytes)
            .unwrap()
            .status,
        CurvatureCommandStatus::IntendedToSteer
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_GUIDANCE_SYSTEM_CMD,
            0x80,
            BROADCAST_ADDRESS,
            &guidance_system_cmd_bytes,
        )
        .pgn(),
        PGN_GUIDANCE_SYSTEM_CMD
    );

    let hitch_pto = HitchPtoCombinedCmd {
        hitch_position: 30_000,
        pto_speed_raw: 4320,
        hitch_cmd: 1,
        pto_cmd: 1,
    };
    let hitch_pto_bytes =
        parse_named_hex_frame(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, "hitch_pto_combined");
    assert_eq!(hitch_pto.encode(), hitch_pto_bytes);
    assert_eq!(
        HitchPtoCombinedCmd::decode(&hitch_pto_bytes).unwrap(),
        hitch_pto
    );
    assert_eq!(
        Frame::from_message(
            Priority::Default,
            PGN_HITCH_PTO_COMBINED_CMD,
            0x80,
            BROADCAST_ADDRESS,
            &hitch_pto_bytes,
        )
        .pgn(),
        PGN_HITCH_PTO_COMBINED_CMD
    );

    let hitch_roll_pitch = HitchRollPitchCmd {
        roll_position: 12_345,
        pitch_position: 23_456,
        is_front: true,
    };
    let hitch_roll_pitch_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_roll_pitch_front",
    );
    assert_eq!(hitch_roll_pitch.encode(), hitch_roll_pitch_bytes);
    assert_eq!(
        HitchRollPitchCmd::decode(&hitch_roll_pitch_bytes, true).unwrap(),
        hitch_roll_pitch
    );
    assert_eq!(hitch_roll_pitch.pgn(), PGN_FRONT_HITCH_ROLL_PITCH_CMD);

    let facilities = TractorFacilities {
        rear_hitch_position: true,
        rear_pto_speed: true,
        wheel_based_speed: true,
        ground_based_distance: true,
        wheel_based_direction: true,
        lighting: true,
        rear_hitch_command: true,
        aux_valve_command: true,
        front_hitch_position: true,
        front_pto_engagement: true,
        navigation: true,
        guidance: true,
        machine_selected_speed: true,
        machine_selected_speed_command: true,
        rear_hitch_exit_code: true,
        rear_pto_speed_limit_status: true,
        aux_valve_limit_status: true,
        aux_valve_exit_code: true,
        front_hitch_limit_status: true,
        front_pto_engagement_request: true,
        front_pto_exit_code: true,
        ..Default::default()
    };
    let facilities_bytes = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "tractor_facilities_mixed",
    );
    assert_eq!(facilities.encode(), facilities_bytes);
    assert_eq!(
        TractorFacilities::decode(&facilities_bytes).unwrap(),
        facilities
    );
    assert_eq!(
        TractorFacilitiesRole::Response.pgn(),
        PGN_TRACTOR_FACILITIES_RESPONSE
    );
    assert_eq!(
        TractorFacilitiesRole::Required.pgn(),
        PGN_REQUIRED_TRACTOR_FACILITIES
    );
}

#[test]
fn fixture_isobus_tractor_ecu_and_implement_sentinels_are_stable() {
    assert_eq!(
        TecuClassification::default().to_string(),
        parse_named_text_value(ISOBUS_TRACTOR_ECU_SNAPSHOT, "classification_default")
    );

    let class2_nf = TecuClassification {
        base_class: TecuClass::Class2,
        navigation: true,
        front_mounted: true,
        ..Default::default()
    };
    assert_eq!(
        class2_nf.to_string(),
        parse_named_text_value(ISOBUS_TRACTOR_ECU_SNAPSHOT, "classification_class2_nf")
    );

    let class3_all = TecuClassification {
        base_class: TecuClass::Class3,
        navigation: true,
        front_mounted: true,
        guidance: true,
        powertrain: true,
        motion_init: true,
        version: parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "classification_class3_all_version",
        )
        .parse::<u8>()
        .unwrap(),
        instance: parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "classification_class3_all_instance",
        )
        .parse::<u8>()
        .unwrap(),
    };
    assert_eq!(
        class3_all.to_string(),
        parse_named_text_value(ISOBUS_TRACTOR_ECU_SNAPSHOT, "classification_class3_all")
    );
    assert_eq!(class3_all.version, 2);
    assert_eq!(class3_all.instance, 1);

    let power = PowerConfig::default();
    assert_eq!(
        power.shutdown_max_time_ms,
        parse_named_text_value(ISOBUS_TRACTOR_ECU_SNAPSHOT, "power_default_shutdown_ms")
            .parse::<u32>()
            .unwrap()
    );
    assert_eq!(
        power.maintain_timeout_ms,
        parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "power_default_maintain_timeout_ms",
        )
        .parse::<u32>()
        .unwrap()
    );
    assert_eq!(
        power.ecu_pwr_current_amps,
        parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "power_default_ecu_pwr_current_amps",
        )
        .parse::<u8>()
        .unwrap()
    );
    assert_eq!(
        power.pwr_current_amps,
        parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "power_default_pwr_current_amps"
        )
        .parse::<u8>()
        .unwrap()
    );

    let tecu = TecuConfig::default();
    assert_eq!(
        tecu.facilities_broadcast_interval_ms,
        parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "tecu_default_facilities_interval_ms",
        )
        .parse::<u32>()
        .unwrap()
    );
    assert_eq!(
        tecu.status_broadcast_interval_ms,
        parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "tecu_default_status_interval_ms",
        )
        .parse::<u32>()
        .unwrap()
    );
    assert_eq!(
        tecu.enable_gateway.to_string(),
        parse_named_text_value(ISOBUS_TRACTOR_ECU_SNAPSHOT, "tecu_default_enable_gateway")
    );

    let req = TecuMaintainPowerRequest {
        requester: 0x80,
        ecu_pwr: true,
        pwr: false,
        timestamp_ms: 1_000,
    };
    assert_eq!(req.requester, 0x80);
    assert!(req.ecu_pwr);
    assert!(!req.pwr);
    assert_eq!(
        req.is_expired(3_000, power.maintain_timeout_ms).to_string(),
        parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "maintain_request_at_timeout_expired",
        )
    );
    assert_eq!(
        req.is_expired(3_001, power.maintain_timeout_ms).to_string(),
        parse_named_text_value(
            ISOBUS_TRACTOR_ECU_SNAPSHOT,
            "maintain_request_after_timeout_expired",
        )
    );
    assert_eq!(TecuPowerState::default(), TecuPowerState::PowerOff);
    assert_eq!(
        format!("{:?}", TecuPowerState::default()),
        parse_named_text_value(ISOBUS_TRACTOR_ECU_SNAPSHOT, "power_state_default")
    );

    let hitch_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_cmd_no_action_na",
    );
    assert_eq!(HitchCommandMsg::default().encode(), hitch_default);
    assert_eq!(
        HitchCommandMsg::decode(&hitch_default).unwrap(),
        HitchCommandMsg::default()
    );

    let pto_default =
        parse_named_hex_frame(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, "pto_cmd_no_action_na");
    assert_eq!(PtoCommandMsg::default().encode(), pto_default);
    assert_eq!(
        PtoCommandMsg::decode(&pto_default).unwrap(),
        PtoCommandMsg::default()
    );

    let aux_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "aux_valve_cmd_0_no_action_na",
    );
    assert_eq!(AuxValveCommandMsg::default().encode(), aux_default);
    assert_eq!(
        AuxValveCommandMsg::decode(&aux_default).unwrap(),
        AuxValveCommandMsg::default()
    );

    let selected_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_selected_speed_status_not_available",
    );
    assert_eq!(
        MachineSelectedSpeedMsg::default().encode(),
        selected_default
    );
    assert_eq!(
        MachineSelectedSpeedMsg::decode(&selected_default).unwrap(),
        MachineSelectedSpeedMsg::default()
    );
    assert_eq!(
        MachineSelectedSpeedMsg::decode(&selected_default)
            .unwrap()
            .speed_mps(),
        0.0
    );

    let speed_cmd_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_speed_cmd_not_available",
    );
    assert_eq!(
        MachineSpeedCommandMsg::default().encode(),
        speed_cmd_default
    );
    assert_eq!(
        MachineSpeedCommandMsg::decode(&speed_cmd_default).unwrap(),
        MachineSpeedCommandMsg::default()
    );

    let lighting_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "lighting_all_not_available",
    );
    assert_eq!(LightingState::default().encode(), lighting_default);
    assert_eq!(
        LightingState::decode(&lighting_default).unwrap(),
        LightingState::default()
    );

    let guidance_curvature_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_curvature_cmd_zero_default",
    );
    assert_eq!(
        CurvatureCommand::default().encode(),
        guidance_curvature_default
    );
    assert!(
        CurvatureCommand::decode(&guidance_curvature_default)
            .unwrap()
            .curvature
            .abs()
            < f64::EPSILON
    );

    let guidance_machine_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_machine_info_default",
    );
    assert_eq!(
        GuidanceMachineInfo::default().encode(),
        guidance_machine_default
    );
    assert_eq!(
        GuidanceMachineInfo::decode(&guidance_machine_default).unwrap(),
        GuidanceMachineInfo::default()
    );

    let guidance_status_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_system_status_default",
    );
    assert_eq!(
        GuidanceSystemStatus::default().encode(),
        guidance_status_default
    );
    assert_eq!(
        GuidanceSystemStatus::decode(&guidance_status_default).unwrap(),
        GuidanceSystemStatus::default()
    );

    let drive_strategy_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "drive_strategy_no_action_na",
    );
    assert_eq!(DriveStrategyCmd::default().encode(), drive_strategy_default);
    assert_eq!(
        DriveStrategyCmd::decode(&drive_strategy_default).unwrap(),
        DriveStrategyCmd::default()
    );

    let guidance_system_cmd_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_system_cmd_default",
    );
    assert_eq!(
        GuidanceSystemCmd::default().encode(),
        guidance_system_cmd_default
    );
    assert_eq!(
        GuidanceSystemCmd::decode(&guidance_system_cmd_default).unwrap(),
        GuidanceSystemCmd::default()
    );

    let hitch_pto_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_pto_combined_default",
    );
    assert_eq!(HitchPtoCombinedCmd::default().encode(), hitch_pto_default);
    assert_eq!(
        HitchPtoCombinedCmd::decode(&hitch_pto_default).unwrap(),
        HitchPtoCombinedCmd::default()
    );

    let hitch_roll_pitch_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_roll_pitch_default",
    );
    assert_eq!(
        HitchRollPitchCmd::default().encode(),
        hitch_roll_pitch_default
    );
    assert_eq!(
        HitchRollPitchCmd::decode(&hitch_roll_pitch_default, false).unwrap(),
        HitchRollPitchCmd::default()
    );

    let wheel_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "wheel_speed_dist_default_na_direction",
    );
    assert_eq!(WheelBasedSpeedDist::default().encode(), wheel_default);
    let decoded_wheel = WheelBasedSpeedDist::decode(&wheel_default).unwrap();
    assert_eq!(decoded_wheel.direction, MachineDirection::NotAvailable);
    assert_eq!(decoded_wheel.max_power_time_min, 0xFF);
    assert_eq!(decoded_wheel.key_switch_state, 0x03);
    assert_eq!(decoded_wheel.implement_start_stop_operations_state, 0x03);
    assert_eq!(decoded_wheel.operator_direction_reversed_state, 0x03);

    let ground_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "ground_speed_dist_default_na_direction",
    );
    assert_eq!(GroundBasedSpeedDist::default().encode(), ground_default);
    assert_eq!(
        GroundBasedSpeedDist::decode(&ground_default)
            .unwrap()
            .direction,
        MachineDirection::NotAvailable
    );

    let selected_full_default = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_selected_speed_full_default_na",
    );
    assert_eq!(
        MachineSelectedSpeedFull::default().encode(),
        selected_full_default
    );
    let decoded_full = MachineSelectedSpeedFull::decode(&selected_full_default).unwrap();
    assert_eq!(decoded_full.direction, MachineDirection::NotAvailable);
    assert_eq!(decoded_full.source, SpeedSource::WheelBased);
    assert_eq!(decoded_full.limit_status, 0x07);
    assert_eq!(decoded_full.exit_code, 0xFF);
}

#[test]
fn fixture_isobus_implement_min_max_and_error_edges_are_stable() {
    let hitch_min =
        parse_named_hex_frame(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, "hitch_cmd_lower_min");
    let hitch_min_msg = HitchCommandMsg {
        command: HitchCommand::Lower,
        target_position: 0,
        rate: 0,
    };
    assert_eq!(hitch_min_msg.encode(), hitch_min);
    assert_eq!(HitchCommandMsg::decode(&hitch_min).unwrap(), hitch_min_msg);

    let hitch_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_cmd_position_100pct_max_rate",
    );
    let hitch_max_msg = HitchCommandMsg {
        command: HitchCommand::Position,
        target_position: 40_000,
        rate: 250,
    };
    assert_eq!(hitch_max_msg.encode(), hitch_max);
    assert_eq!(HitchCommandMsg::decode(&hitch_max).unwrap(), hitch_max_msg);
    for pgn in [PGN_FRONT_HITCH_CMD, PGN_REAR_HITCH_CMD] {
        assert_eq!(
            Frame::from_message(Priority::Default, pgn, 0x80, BROADCAST_ADDRESS, &hitch_max).pgn(),
            pgn
        );
    }

    let pto_zero = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "pto_cmd_disengage_zero",
    );
    let pto_zero_msg = PtoCommandMsg {
        command: PtoCommand::Disengage,
        target_speed_rpm: 0,
        ramp_rate: 0,
    };
    assert_eq!(pto_zero_msg.encode(), pto_zero);
    assert_eq!(PtoCommandMsg::decode(&pto_zero).unwrap(), pto_zero_msg);

    let pto_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "pto_cmd_set_speed_max_non_na",
    );
    let pto_max_msg = PtoCommandMsg {
        command: PtoCommand::SetSpeed,
        target_speed_rpm: 0xFFFE,
        ramp_rate: 0xFE,
    };
    assert_eq!(pto_max_msg.encode(), pto_max);
    assert_eq!(PtoCommandMsg::decode(&pto_max).unwrap(), pto_max_msg);
    for pgn in [PGN_FRONT_PTO_CMD, PGN_REAR_PTO_CMD] {
        assert_eq!(
            Frame::from_message(Priority::Default, pgn, 0x80, BROADCAST_ADDRESS, &pto_max).pgn(),
            pgn
        );
    }

    let aux_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "aux_valve_cmd_15_block_max_non_na",
    );
    let aux_max_msg = AuxValveCommandMsg {
        valve_index: 15,
        command: ValveCommand::Block,
        flow_rate: 0xFFFE,
    };
    assert_eq!(aux_max_msg.encode(), aux_max);
    assert_eq!(AuxValveCommandMsg::decode(&aux_max).unwrap(), aux_max_msg);
    assert_eq!(aux_max_msg.try_pgn(), Some(PGN_AUX_VALVE_CMD + 15));

    let aux_bad_index = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "aux_valve_cmd_invalid_index16",
    );
    assert!(AuxValveCommandMsg::decode(&aux_bad_index).is_none());

    let control_manual = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "tractor_control_mode_all_manual",
    );
    let control_manual_msg = TractorControlModeMsg {
        hitch_mode: TractorMode::Manual,
        pto_mode: TractorMode::Manual,
        front_hitch_mode: TractorMode::Manual,
        front_pto_mode: TractorMode::Manual,
        speed_control_state: 0,
    };
    assert_eq!(control_manual_msg.encode(), control_manual);
    assert_eq!(
        TractorControlModeMsg::decode(&control_manual).unwrap(),
        control_manual_msg
    );

    let control_auto = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "tractor_control_mode_all_auto",
    );
    let control_auto_msg = TractorControlModeMsg {
        hitch_mode: TractorMode::Automatic,
        pto_mode: TractorMode::Automatic,
        front_hitch_mode: TractorMode::Automatic,
        front_pto_mode: TractorMode::Automatic,
        speed_control_state: 0,
    };
    assert_eq!(control_auto_msg.encode(), control_auto);
    assert_eq!(
        TractorControlModeMsg::decode(&control_auto).unwrap(),
        control_auto_msg
    );

    let wheel_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "wheel_speed_dist_max_error",
    );
    let wheel_max_msg = WheelBasedSpeedDist {
        speed_mps: 1.0e9,
        distance_m: 1.0e12,
        direction: MachineDirection::Error,
        max_power_time_min: 0xFE,
        key_switch_state: 2,
        implement_start_stop_operations_state: 2,
        operator_direction_reversed_state: 2,
    };
    assert_eq!(wheel_max_msg.encode(), wheel_max);
    let decoded_wheel_max = WheelBasedSpeedDist::decode(&wheel_max).unwrap();
    assert!((decoded_wheel_max.speed_mps - 64.255).abs() < 0.001);
    assert!((decoded_wheel_max.distance_m - 4_211_081.215).abs() < 0.001);
    assert_eq!(decoded_wheel_max.direction, MachineDirection::Error);

    let ground_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "ground_speed_dist_max_error",
    );
    let ground_max_msg = GroundBasedSpeedDist {
        speed_mps: 1.0e9,
        distance_m: 1.0e12,
        direction: MachineDirection::Error,
    };
    assert_eq!(ground_max_msg.encode(), ground_max);
    let decoded_ground_max = GroundBasedSpeedDist::decode(&ground_max).unwrap();
    assert!((decoded_ground_max.speed_mps - 64.255).abs() < 0.001);
    assert!((decoded_ground_max.distance_m - 4_211_081.215).abs() < 0.001);
    assert_eq!(decoded_ground_max.direction, MachineDirection::Error);

    let selected_full_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_selected_speed_full_max",
    );
    let selected_full_max_msg = MachineSelectedSpeedFull {
        speed_mps: 1.0e9,
        distance_m: 1.0e12,
        direction: MachineDirection::Error,
        source: SpeedSource::Blended,
        limit_status: 6,
        exit_code: 0xFE,
    };
    assert_eq!(selected_full_max_msg.encode(), selected_full_max);
    let decoded_selected_full_max = MachineSelectedSpeedFull::decode(&selected_full_max).unwrap();
    assert_eq!(decoded_selected_full_max.direction, MachineDirection::Error);
    assert_eq!(decoded_selected_full_max.source, SpeedSource::Blended);
    assert_eq!(decoded_selected_full_max.limit_status, 6);
    assert_eq!(decoded_selected_full_max.exit_code, 0xFE);

    let selected_status_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_selected_speed_status_max",
    );
    let selected_status_max_msg = MachineSelectedSpeedMsg {
        speed_raw: 0xFFFE,
        direction: MachineDirection::Error,
        source: SpeedSource::NavigationBased,
        limit_status: SpeedExitCode::SystemLimited,
    };
    assert_eq!(selected_status_max_msg.encode(), selected_status_max);
    assert_eq!(
        MachineSelectedSpeedMsg::decode(&selected_status_max).unwrap(),
        selected_status_max_msg
    );

    let speed_cmd_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "machine_speed_cmd_max_reverse",
    );
    let speed_cmd_max_msg = MachineSpeedCommandMsg {
        target_speed_raw: 0xFFFE,
        direction_cmd: MachineDirection::Reverse,
    };
    assert_eq!(speed_cmd_max_msg.encode(), speed_cmd_max);
    assert_eq!(
        MachineSpeedCommandMsg::decode(&speed_cmd_max).unwrap(),
        speed_cmd_max_msg
    );

    let hitch_status_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_status_front_max",
    );
    let hitch_status_max_msg = HitchStatus {
        position_percent: 250,
        in_work_indication: 3,
        limit_status: LimitStatus::NotAvailable,
        exit_code: ExitReasonCode::NotAvailable,
        draft_force_n: 1.0e12,
        is_rear: false,
    };
    assert_eq!(hitch_status_max_msg.encode(), hitch_status_max);
    assert_eq!(
        HitchStatus::decode(&hitch_status_max, false).unwrap().pgn(),
        PGN_FRONT_HITCH
    );

    let pto_status_max =
        parse_named_hex_frame(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, "pto_status_rear_max");
    let pto_status_max_msg = PtoStatus {
        shaft_speed_rpm: 1.0e9,
        engagement: 2,
        limit_status: LimitStatus::SystemLimited,
        exit_code: ExitReasonCode::Fault,
        economy_mode: 2,
        is_rear: true,
    };
    assert_eq!(pto_status_max_msg.encode(), pto_status_max);
    assert_eq!(
        PtoStatus::decode(&pto_status_max, true).unwrap().pgn(),
        PGN_REAR_PTO
    );

    let lighting_off =
        parse_named_hex_frame(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, "lighting_all_off");
    assert_eq!(
        LightingState {
            left_turn: LightState::Off,
            right_turn: LightState::Off,
            low_beam: LightState::Off,
            high_beam: LightState::Off,
            front_fog: LightState::Off,
            rear_fog: LightState::Off,
            beacon: LightState::Off,
            running: LightState::Off,
            rear_work: LightState::Off,
            front_work: LightState::Off,
            side_work: LightState::Off,
            hazard: LightState::Off,
            backup: LightState::Off,
            center_stop: LightState::Off,
            left_stop: LightState::Off,
            right_stop: LightState::Off,
        }
        .encode(),
        lighting_off
    );

    let lighting_error =
        parse_named_hex_frame(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, "lighting_all_error");
    assert_eq!(
        LightingState {
            left_turn: LightState::Error,
            right_turn: LightState::Error,
            low_beam: LightState::Error,
            high_beam: LightState::Error,
            front_fog: LightState::Error,
            rear_fog: LightState::Error,
            beacon: LightState::Error,
            running: LightState::Error,
            rear_work: LightState::Error,
            front_work: LightState::Error,
            side_work: LightState::Error,
            hazard: LightState::Error,
            backup: LightState::Error,
            center_stop: LightState::Error,
            left_stop: LightState::Error,
            right_stop: LightState::Error,
        }
        .encode(),
        lighting_error
    );

    let curvature_min = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_curvature_cmd_min_raw",
    );
    let curvature_min_msg = CurvatureCommand {
        curvature: -8032.0,
        curvature_rate: 0.0,
    };
    assert_eq!(curvature_min_msg.encode(), curvature_min);
    assert!(
        (CurvatureCommand::decode(&curvature_min).unwrap().curvature - -8032.0).abs()
            < f64::EPSILON
    );

    let curvature_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_curvature_cmd_max_non_na",
    );
    let curvature_max_msg = CurvatureCommand {
        curvature: 8031.75,
        curvature_rate: 0.0,
    };
    assert_eq!(curvature_max_msg.encode(), curvature_max);
    assert!(
        (CurvatureCommand::decode(&curvature_max).unwrap().curvature - 8031.75).abs()
            < f64::EPSILON
    );

    let guidance_machine_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_machine_info_max_error",
    );
    let guidance_machine_max_msg = GuidanceMachineInfo {
        estimated_curvature: 8031.75,
        lockout: MechanicalLockout::Error,
        steering_system_readiness_state: GenericSaeBs02SlotValue::ErrorIndication,
        steering_input_position_status: GenericSaeBs02SlotValue::ErrorIndication,
        request_reset_status: RequestResetCommandStatus::Error,
        guidance_limit_status: GuidanceLimitStatus::NotAvailable,
        guidance_system_command_exit_reason_code: 0x3E,
        remote_engage_switch_status: GenericSaeBs02SlotValue::ErrorIndication,
    };
    assert_eq!(guidance_machine_max_msg.encode(), guidance_machine_max);
    let decoded_guidance_machine_max = GuidanceMachineInfo::decode(&guidance_machine_max).unwrap();
    assert!((decoded_guidance_machine_max.estimated_curvature - 8031.75).abs() < f64::EPSILON);
    assert_eq!(
        decoded_guidance_machine_max.lockout,
        MechanicalLockout::Error
    );
    assert_eq!(
        decoded_guidance_machine_max.request_reset_status,
        RequestResetCommandStatus::Error
    );
    assert_eq!(
        decoded_guidance_machine_max.guidance_limit_status,
        GuidanceLimitStatus::NotAvailable
    );
    assert_eq!(
        decoded_guidance_machine_max.guidance_system_command_exit_reason_code,
        0x3E
    );

    let guidance_status_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_system_status_max_error",
    );
    let guidance_status_max_msg = GuidanceSystemStatus {
        estimated_curvature: 8031.75,
        readiness: SteeringReadiness::Error,
        integrity_level: 3,
    };
    assert_eq!(guidance_status_max_msg.encode(), guidance_status_max);
    let decoded_guidance_status_max = GuidanceSystemStatus::decode(&guidance_status_max).unwrap();
    assert!((decoded_guidance_status_max.estimated_curvature - 8031.75).abs() < f64::EPSILON);
    assert_eq!(
        decoded_guidance_status_max.readiness,
        SteeringReadiness::Error
    );
    assert_eq!(decoded_guidance_status_max.integrity_level, 3);

    let drive_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "drive_strategy_max_speed_limits",
    );
    let drive_max_msg = DriveStrategyCmd {
        mode: DriveStrategyMode::MaxSpeed,
        target_speed_limit_percent: 0xFE,
        target_engine_load_percent: 0xFE,
    };
    assert_eq!(drive_max_msg.encode(), drive_max);
    assert_eq!(DriveStrategyCmd::decode(&drive_max).unwrap(), drive_max_msg);

    let guidance_cmd_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "guidance_system_cmd_max",
    );
    let guidance_cmd_max_msg = GuidanceSystemCmd {
        commanded_curvature: 8031.75,
        status: CurvatureCommandStatus::ErrorIndication,
    };
    assert_eq!(guidance_cmd_max_msg.encode(), guidance_cmd_max);
    let decoded_guidance_cmd_max = GuidanceSystemCmd::decode(&guidance_cmd_max).unwrap();
    assert!((decoded_guidance_cmd_max.commanded_curvature - 8031.75).abs() < 0.25);
    assert_eq!(
        decoded_guidance_cmd_max.status,
        CurvatureCommandStatus::ErrorIndication
    );

    let hitch_pto_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_pto_combined_max_non_na",
    );
    let hitch_pto_max_msg = HitchPtoCombinedCmd {
        hitch_position: 0xFFFE,
        pto_speed_raw: 0xFFFE,
        hitch_cmd: 2,
        pto_cmd: 2,
    };
    assert_eq!(hitch_pto_max_msg.encode(), hitch_pto_max);
    assert_eq!(
        HitchPtoCombinedCmd::decode(&hitch_pto_max).unwrap(),
        hitch_pto_max_msg
    );

    let roll_pitch_max = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "hitch_roll_pitch_rear_max",
    );
    let roll_pitch_max_msg = HitchRollPitchCmd {
        roll_position: 0xFFFE,
        pitch_position: 0xFFFE,
        is_front: false,
    };
    assert_eq!(roll_pitch_max_msg.encode(), roll_pitch_max);
    assert_eq!(
        HitchRollPitchCmd::decode(&roll_pitch_max, false).unwrap(),
        roll_pitch_max_msg
    );
    assert_eq!(roll_pitch_max_msg.pgn(), PGN_REAR_HITCH_ROLL_PITCH_CMD);

    let facilities_none = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "tractor_facilities_none",
    );
    assert_eq!(TractorFacilities::default().encode(), facilities_none);
    assert_eq!(
        TractorFacilities::decode(&facilities_none).unwrap(),
        TractorFacilities::default()
    );

    let facilities_all = parse_named_hex_frame(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "tractor_facilities_all",
    );
    let mut facilities_all_msg = TractorFacilities::default()
        .with_class1_all()
        .with_class2_all()
        .with_class3_all()
        .with_class3_v2_all()
        .with_front_v2_all();
    facilities_all_msg.front_hitch_position = true;
    facilities_all_msg.front_hitch_in_work = true;
    facilities_all_msg.front_pto_speed = true;
    facilities_all_msg.front_pto_engagement = true;
    facilities_all_msg.front_hitch_command = true;
    facilities_all_msg.front_pto_command = true;
    facilities_all_msg.navigation = true;
    facilities_all_msg.guidance = true;
    facilities_all_msg.machine_selected_speed = true;
    facilities_all_msg.machine_selected_speed_command = true;
    assert_eq!(facilities_all_msg.encode(), facilities_all);
    assert_eq!(
        TractorFacilities::decode(&facilities_all).unwrap(),
        facilities_all_msg
    );

    for name in [
        "malformed_hitch_cmd_short4",
        "malformed_hitch_cmd_bad_padding",
        "malformed_hitch_cmd_reserved_command",
    ] {
        assert!(
            HitchCommandMsg::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_pto_cmd_short4",
        "malformed_pto_cmd_bad_padding",
        "malformed_pto_cmd_reserved_command",
    ] {
        assert!(
            PtoCommandMsg::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_aux_valve_cmd_short3",
        "malformed_aux_valve_cmd_bad_padding",
        "malformed_aux_valve_cmd_reserved_command",
    ] {
        assert!(
            AuxValveCommandMsg::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_tractor_control_short1",
        "malformed_tractor_control_bad_padding",
        "malformed_tractor_control_reserved_mode",
    ] {
        assert!(
            TractorControlModeMsg::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_lighting_short3",
        "malformed_lighting_bad_padding",
    ] {
        assert!(
            LightingState::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_machine_selected_speed_short4",
        "malformed_machine_selected_speed_bad_padding",
        "malformed_machine_selected_speed_reserved_bits",
    ] {
        assert!(
            MachineSelectedSpeedMsg::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_machine_speed_cmd_short2",
        "malformed_machine_speed_cmd_bad_padding",
        "malformed_machine_speed_cmd_reserved_bits",
    ] {
        assert!(
            MachineSpeedCommandMsg::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    let malformed_speed_distance = parse_named_hex_bytes(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "malformed_speed_distance_short7",
    );
    assert!(WheelBasedSpeedDist::decode(&malformed_speed_distance).is_none());
    assert!(GroundBasedSpeedDist::decode(&malformed_speed_distance).is_none());
    assert!(MachineSelectedSpeedFull::decode(&malformed_speed_distance).is_none());
    for name in [
        "malformed_ground_speed_dist_bad_padding",
        "malformed_ground_speed_dist_reserved_bits",
    ] {
        assert!(
            GroundBasedSpeedDist::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    assert!(
        MachineSelectedSpeedFull::decode(&parse_named_hex_bytes(
            ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
            "malformed_machine_selected_speed_full_reserved_bit",
        ))
        .is_none()
    );
    for name in [
        "malformed_hitch_status_short3",
        "malformed_hitch_status_bad_padding",
        "malformed_hitch_status_reserved_bit",
    ] {
        assert!(
            HitchStatus::decode(
                &parse_named_hex_bytes(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, name),
                false,
            )
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_pto_status_short2",
        "malformed_pto_status_bad_padding",
    ] {
        assert!(
            PtoStatus::decode(
                &parse_named_hex_bytes(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, name),
                true,
            )
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_aux_valve_flow_bad_padding",
        "malformed_aux_valve_flow_bad_reserved_bit",
    ] {
        assert!(
            AuxValveFlowMsg::decode(
                &parse_named_hex_bytes(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, name),
                3,
            )
            .is_none(),
            "{name} must be rejected"
        );
    }
    assert!(
        AuxValveFlowMsg::decode(
            &parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                "aux_valve_flow_3_extending",
            ),
            16,
        )
        .is_none()
    );
    assert!(
        CurvatureCommand::decode(&parse_named_hex_bytes(
            ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
            "malformed_guidance_curvature_bad_padding",
        ))
        .is_none()
    );
    for name in [
        "malformed_guidance_machine_bad_padding",
        "malformed_guidance_machine_reserved_control_bits",
    ] {
        assert!(
            GuidanceMachineInfo::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_guidance_system_status_bad_padding",
        "malformed_guidance_system_status_reserved_readiness",
        "malformed_guidance_system_status_reserved_control_bits",
    ] {
        assert!(
            GuidanceSystemStatus::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_drive_strategy_short2",
        "malformed_drive_strategy_bad_padding",
        "malformed_drive_strategy_reserved_mode",
    ] {
        assert!(
            DriveStrategyCmd::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_guidance_system_cmd_short3",
        "malformed_guidance_system_cmd_bad_padding",
    ] {
        assert!(
            GuidanceSystemCmd::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_hitch_pto_short4",
        "malformed_hitch_pto_bad_padding",
        "malformed_hitch_pto_reserved_control_bits",
    ] {
        assert!(
            HitchPtoCombinedCmd::decode(&parse_named_hex_bytes(
                ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
                name,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }
    for name in [
        "malformed_hitch_roll_pitch_short3",
        "malformed_hitch_roll_pitch_bad_padding",
    ] {
        assert!(
            HitchRollPitchCmd::decode(
                &parse_named_hex_bytes(ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX, name),
                false,
            )
            .is_none(),
            "{name} must be rejected"
        );
    }
    assert!(
        TractorFacilities::decode(&parse_named_hex_bytes(
            ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
            "malformed_tractor_facilities_short3",
        ))
        .is_none()
    );

    let malformed_fixed8_short = parse_named_hex_bytes(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "malformed_fixed8_short7",
    );
    let malformed_fixed8_overlong = parse_named_hex_bytes(
        ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX,
        "malformed_fixed8_overlong9",
    );
    for malformed in [&malformed_fixed8_short, &malformed_fixed8_overlong] {
        assert!(HitchCommandMsg::decode(malformed).is_none());
        assert!(PtoCommandMsg::decode(malformed).is_none());
        assert!(AuxValveCommandMsg::decode(malformed).is_none());
        assert!(TractorControlModeMsg::decode(malformed).is_none());
        assert!(WheelBasedSpeedDist::decode(malformed).is_none());
        assert!(GroundBasedSpeedDist::decode(malformed).is_none());
        assert!(MachineSelectedSpeedFull::decode(malformed).is_none());
        assert!(HitchStatus::decode(malformed, false).is_none());
        assert!(PtoStatus::decode(malformed, true).is_none());
        assert!(AuxValveFlowMsg::decode(malformed, 3).is_none());
        assert!(LightingState::decode(malformed).is_none());
        assert!(CurvatureCommand::decode(malformed).is_none());
        assert!(GuidanceMachineInfo::decode(malformed).is_none());
        assert!(GuidanceSystemStatus::decode(malformed).is_none());
        assert!(MachineSelectedSpeedMsg::decode(malformed).is_none());
        assert!(MachineSpeedCommandMsg::decode(malformed).is_none());
        assert!(DriveStrategyCmd::decode(malformed).is_none());
        assert!(GuidanceSystemCmd::decode(malformed).is_none());
        assert!(HitchPtoCombinedCmd::decode(malformed).is_none());
        assert!(HitchRollPitchCmd::decode(malformed, false).is_none());
    }
    assert!(TractorFacilities::decode(&malformed_fixed8_overlong).is_none());
}

#[test]
fn fixture_isobus_control_functionalities_codecs_are_stable() {
    let min_cf = parse_named_hex_bytes(ISOBUS_CONTROL_FUNCTIONALITIES_HEX, "min_cf_heartbeat");
    let mut min_builder = Functionalities::new();
    min_builder.set_minimum_control_function_option_state(
        MinimumControlFunctionOptions::SupportOfHeartbeatProducer,
        true,
    );
    min_builder.set_minimum_control_function_option_state(
        MinimumControlFunctionOptions::SupportOfHeartbeatConsumer,
        true,
    );
    assert_eq!(min_builder.serialize(), min_cf);
    assert_eq!(
        Functionalities::decode(&min_cf).unwrap(),
        vec![FunctionalityData {
            functionality: Functionality::MinimumControlFunction,
            generation: 1,
            option_bytes: vec![0x0C],
        }]
    );
    assert_eq!(
        Functionalities::decode(&parse_named_hex_bytes(
            ISOBUS_CONTROL_FUNCTIONALITIES_HEX,
            "agisostack_min_cf_no_options"
        ))
        .unwrap(),
        vec![FunctionalityData {
            functionality: Functionality::MinimumControlFunction,
            generation: 1,
            option_bytes: vec![],
        }]
    );

    let capability_mix =
        parse_named_hex_bytes(ISOBUS_CONTROL_FUNCTIONALITIES_HEX, "capability_mix");
    let mut f = Functionalities::new();
    f.set_minimum_control_function_option_state(
        MinimumControlFunctionOptions::SupportOfHeartbeatProducer,
        true,
    );
    f.set_minimum_control_function_option_state(
        MinimumControlFunctionOptions::SupportOfHeartbeatConsumer,
        true,
    );
    f.set_functionality_supported(Functionality::AuxNInputs, 2, true);
    f.aux_n_inputs_options = 0x4003;
    f.set_functionality_supported(Functionality::TaskControllerSectionControlServer, 1, true);
    f.tc_sc_server_booms = 2;
    f.tc_sc_server_sections = 16;
    f.set_functionality_supported(Functionality::BasicTractorEcuServer, 1, true);
    f.set_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::Class2NoOptions, true);
    f.set_basic_tractor_ecu_server_option_state(
        BasicTractorEcuOptions::ClassRequiredLighting,
        true,
    );
    f.set_basic_tractor_ecu_server_option_state(BasicTractorEcuOptions::GuidanceOption, true);
    f.set_functionality_supported(Functionality::TractorImplementManagementServer, 1, true);
    f.set_tim_server_option(
        TractorImplementManagementOptions::FrontPtoDisengagementIsSupported,
        true,
    );
    f.set_tim_server_option(
        TractorImplementManagementOptions::RearHitchPositionIsSupported,
        true,
    );
    f.set_tim_server_option(
        TractorImplementManagementOptions::GuidanceCurvatureIsSupported,
        true,
    );
    f.set_tim_server_aux_valve(0, true, true);
    f.set_tim_server_aux_valve(3, true, false);
    f.set_tim_server_aux_valve(31, false, true);
    f.set_functionality_supported(Functionality::FileServer, 1, true);
    f.set_functionality_supported(Functionality::FileServerClient, 1, true);
    assert_eq!(f.serialize(), capability_mix);

    let decoded = Functionalities::decode(&capability_mix).unwrap();
    assert_eq!(decoded.len(), 7);
    assert_eq!(decoded[1].functionality, Functionality::AuxNInputs);
    assert_eq!(decoded[1].option_bytes, vec![0x03, 0x40]);
    assert_eq!(
        decoded[3],
        FunctionalityData {
            functionality: Functionality::BasicTractorEcuServer,
            generation: 1,
            option_bytes: vec![0x26],
        }
    );
    assert_eq!(
        decoded[4],
        FunctionalityData {
            functionality: Functionality::TractorImplementManagementServer,
            generation: 1,
            option_bytes: vec![0x01, 0x20, 0x20, 0x43, 0, 0, 0, 0, 0, 0, 0x80],
        }
    );
    assert_eq!(
        decoded[5],
        FunctionalityData {
            functionality: Functionality::FileServer,
            generation: 1,
            option_bytes: vec![],
        }
    );
    assert_eq!(
        decoded[6],
        FunctionalityData {
            functionality: Functionality::FileServerClient,
            generation: 1,
            option_bytes: vec![],
        }
    );

    assert_eq!(Functionality::from_u8(0x63), None);
    for name in [
        "unknown_functionality",
        "truncated_tim_server",
        "trailing_after_min_cf",
        "duplicate_min_cf",
    ] {
        let payload = parse_named_hex_bytes(ISOBUS_CONTROL_FUNCTIONALITIES_HEX, name);
        assert!(
            Functionalities::decode(&payload).is_err(),
            "malformed fixture {name} decoded successfully"
        );
    }
}

