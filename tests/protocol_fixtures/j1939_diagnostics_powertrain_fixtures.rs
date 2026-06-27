#[test]
fn fixture_j1939_diagnostic_request_response_payloads_are_stable() {
    let dm1_request =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_REQUEST_RESPONSE_HEX, "dm1_request_global");
    let dm2_request =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_REQUEST_RESPONSE_HEX, "dm2_request_global");
    assert_eq!(encode_request(PGN_DM1).unwrap(), dm1_request.as_slice());
    assert_eq!(encode_request(PGN_DM2).unwrap(), dm2_request.as_slice());
    assert_eq!(decode_request(&dm1_request), Some(PGN_DM1));
    assert_eq!(decode_request(&dm2_request), Some(PGN_DM2));

    let dm1_response = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_REQUEST_RESPONSE_HEX,
        "dm1_response_two_dtcs",
    );
    let decoded_dm1 = DmDtcList::decode(&dm1_response).expect("DM1 response decodes");
    assert_eq!(decoded_dm1.lamps.amber_warning, LampStatus::On);
    assert_eq!(
        decoded_dm1.dtcs,
        vec![
            Dtc {
                spn: 100,
                fmi: Fmi::BelowNormal,
                occurrence_count: 0,
            },
            Dtc {
                spn: 523_312,
                fmi: Fmi::AboveNormal,
                occurrence_count: 0,
            },
        ]
    );
    assert_eq!(decoded_dm1.encode(), dm1_response);

    let dm2_response = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_REQUEST_RESPONSE_HEX,
        "dm2_response_lamp_only",
    );
    let decoded_dm2 = DmDtcList::decode(&dm2_response).expect("DM2 response decodes");
    assert!(decoded_dm2.dtcs.is_empty());
    assert_eq!(decoded_dm2.encode(), dm2_response);

    let dm11_clear = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_REQUEST_RESPONSE_HEX,
        "dm11_request_clear_active",
    );
    assert_eq!(
        DmClearAllRequest::decode(&dm11_clear),
        Some(DmClearAllRequest)
    );
    assert_eq!(DmClearAllRequest.encode(), dm11_clear.as_slice());
}

#[test]
fn fixture_j1939_fixed_size_diagnostic_codecs_are_stable() {
    let malformed_short = parse_named_hex_bytes(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "fixed8_short7");
    let malformed_overlong =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "fixed8_overlong9");

    let dm3_clear = parse_named_hex_frame(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "dm_clear_all");
    assert_eq!(dm3_clear, *DM3_CLEAR_PREVIOUS_REQUEST);
    assert_eq!(
        DmClearAllRequest::decode(&dm3_clear),
        Some(DmClearAllRequest)
    );
    assert!(DmClearAllRequest::decode(&malformed_short).is_none());
    assert!(DmClearAllRequest::decode(&malformed_overlong).is_none());

    let dm5 = parse_named_hex_frame(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "dm5_protocols_j1939_uds");
    let dm5_msg = DiagnosticProtocolId {
        protocols: DiagProtocol::J1939_73.as_u8() | DiagProtocol::Iso14229_3.as_u8(),
    };
    assert_eq!(dm5_msg.encode(), dm5);
    assert_eq!(DiagnosticProtocolId::decode(&dm5), Some(dm5_msg));
    assert!(DiagnosticProtocolId::decode(&malformed_short).is_none());
    assert!(DiagnosticProtocolId::decode(&malformed_overlong).is_none());
    assert!(
        DiagnosticProtocolId::decode(&parse_named_hex_frame(
            J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
            "dm5_bad_padding"
        ))
        .is_none()
    );

    let dm7 = parse_named_hex_frame(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "dm7_spn4660_test5");
    let dm7_msg = Dm7Command {
        spn: 0x1234,
        test_id: 5,
    };
    assert_eq!(dm7_msg.encode(), dm7);
    assert_eq!(Dm7Command::decode(&dm7), Some(dm7_msg));
    assert_eq!(
        Dm7Command {
            spn: 0x8_0000,
            test_id: 5,
        }
        .encode(),
        parse_named_hex_frame(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "dm7_spn_clamped")
    );
    assert!(Dm7Command::decode(&malformed_short).is_none());
    assert!(Dm7Command::decode(&malformed_overlong).is_none());
    for malformed in ["dm7_bad_spn_reserved_bits", "dm7_bad_tail"] {
        assert!(
            Dm7Command::decode(&parse_named_hex_frame(
                J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let dm13_announcement = parse_named_hex_frame(
        J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
        "dm13_suspension_announcement_5s",
    );
    let dm13_announcement_msg = Dm13Signals {
        suspend_duration_s: 5,
        ..Dm13Signals::default()
    };
    assert_eq!(dm13_announcement_msg.encode(), dm13_announcement);
    assert_eq!(
        Dm13Signals::decode(&dm13_announcement),
        Some(dm13_announcement_msg)
    );

    let dm13_primary_stop = parse_named_hex_frame(
        J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
        "dm13_primary_network_stop_temp_10s",
    );
    let dm13_primary_stop_msg = Dm13Signals {
        primary_vehicle_network: Dm13Command::SuspendBroadcast,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::DoNotCare,
        suspend_signal: Dm13SuspendSignal::PartialTemporarySuspension,
        suspend_duration_s: 10,
    };
    assert_eq!(dm13_primary_stop_msg.encode(), dm13_primary_stop);
    assert_eq!(
        Dm13Signals::decode(&dm13_primary_stop),
        Some(dm13_primary_stop_msg)
    );

    let dm13_current_stop = parse_named_hex_frame(
        J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
        "dm13_current_link_stop_indefinite_10s",
    );
    let dm13_current_stop_msg = Dm13Signals {
        primary_vehicle_network: Dm13Command::DoNotCare,
        sae_j1922_network: Dm13Command::DoNotCare,
        sae_j1587_network: Dm13Command::DoNotCare,
        current_data_link: Dm13Command::SuspendBroadcast,
        suspend_signal: Dm13SuspendSignal::IndefiniteSuspension,
        suspend_duration_s: 10,
    };
    assert_eq!(dm13_current_stop_msg.encode(), dm13_current_stop);
    assert_eq!(
        Dm13Signals::decode(&dm13_current_stop),
        Some(dm13_current_stop_msg)
    );
    assert!(Dm13Signals::decode(&malformed_short).is_none());
    assert!(Dm13Signals::decode(&malformed_overlong).is_none());
    for malformed in ["dm13_bad_reserved_byte", "dm13_bad_suspend_signal"] {
        assert!(
            Dm13Signals::decode(&parse_named_hex_frame(
                J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must reject"
        );
    }

    let dm22 = parse_named_hex_frame(
        J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
        "dm22_clear_active_spn74565_erratic",
    );
    let dm22_msg = Dm22Message {
        control: Dm22Control::ClearActive,
        nack_reason: None,
        spn: 0x1_2345,
        fmi: Fmi::Erratic,
    };
    assert_eq!(dm22_msg.encode(), dm22);
    assert_eq!(Dm22Message::decode(&dm22), Some(dm22_msg));
    assert_eq!(
        Dm22Message {
            control: Dm22Control::ClearActive,
            nack_reason: None,
            spn: 0x8_0000,
            fmi: Fmi::Erratic,
        }
        .encode(),
        parse_named_hex_frame(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "dm22_spn_clamped")
    );
    assert!(Dm22Message::decode(&malformed_short).is_none());
    assert!(Dm22Message::decode(&malformed_overlong).is_none());
    for malformed in ["dm22_bad_reserved_padding", "dm22_bad_non_nack_reason"] {
        assert!(
            Dm22Message::decode(&parse_named_hex_frame(
                J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must reject"
        );
    }

    let dm25 = parse_named_hex_frame(
        J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
        "dm25_spn74565_erratic_frame2",
    );
    let dm25_msg = Dm25Request {
        spn: 0x1_2345,
        fmi: Fmi::Erratic,
        frame_number: 2,
    };
    assert_eq!(dm25_msg.encode(), dm25);
    assert_eq!(Dm25Request::decode(&dm25), Some(dm25_msg));
    assert_eq!(
        Dm25Request {
            spn: 0x8_0000,
            fmi: Fmi::Erratic,
            frame_number: 2,
        }
        .encode(),
        parse_named_hex_frame(J1939_DIAGNOSTIC_FIXED_CODECS_HEX, "dm25_spn_clamped")
    );
    assert!(Dm25Request::decode(&malformed_short).is_none());
    assert!(Dm25Request::decode(&malformed_overlong).is_none());
    for malformed in [
        "dm25_bad_spn_reserved_bits",
        "dm25_bad_fmi_reserved_bits",
        "dm25_bad_tail",
    ] {
        assert!(
            Dm25Request::decode(&parse_named_hex_frame(
                J1939_DIAGNOSTIC_FIXED_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
}

#[test]
fn fixture_j1939_variable_diagnostic_codecs_reject_prefix_payloads() {
    let dtc = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dtc_spn74565_erratic_oc7",
    );
    let expected_dtc = Dtc {
        spn: 0x1_2345,
        fmi: Fmi::Erratic,
        occurrence_count: 7,
    };
    assert_eq!(expected_dtc.encode(), dtc.as_slice());
    assert_eq!(Dtc::decode(&dtc), Some(expected_dtc));
    assert_eq!(
        Dtc {
            spn: 0x8_0000,
            fmi: Fmi::Erratic,
            occurrence_count: 0xFF,
        }
        .encode()
        .as_slice(),
        parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dtc_spn_clamped_occurrence_clamped",
        )
    );
    assert!(
        Dtc::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dtc_overlong5",
        ))
        .is_none()
    );

    let dm4 = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dm4_driver_info_one_dtc",
    );
    let expected_dm4 = Dm4Message {
        mil_status: LampStatus::On,
        red_stop_lamp: LampStatus::Off,
        amber_warning: LampStatus::On,
        protect_lamp: LampStatus::Error,
        dtcs: vec![Dtc {
            spn: 42,
            fmi: Fmi::CurrentLow,
            occurrence_count: 3,
        }],
    };
    assert_eq!(expected_dm4.encode(), dm4);
    assert_eq!(Dm4Message::decode(&dm4), Some(expected_dm4));
    for malformed in ["dm4_bad_reserved_byte", "dm4_trailing_partial"] {
        assert!(
            Dm4Message::decode(&parse_named_hex_bytes(
                J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let dm8 = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dm8_spn22136_test1_pass_value1234_min1000_max1500",
    );
    let expected_dm8 = Dm8TestResult {
        spn: 0x5678,
        test_id: 1,
        test_result: 0,
        test_value: 1234,
        test_limit_min: 1000,
        test_limit_max: 1500,
    };
    assert_eq!(expected_dm8.encode(), dm8);
    assert_eq!(Dm8TestResult::decode(&dm8), Some(expected_dm8));
    assert_eq!(
        Dm8TestResult {
            spn: 0x8_0000,
            test_id: 1,
            test_result: 0,
            test_value: 1234,
            test_limit_min: 1000,
            test_limit_max: 1500,
        }
        .encode(),
        parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dm8_spn_clamped_test1_pass_value1234_min1000_max1500",
        )
    );
    assert!(
        Dm8TestResult::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dm8_bad_spn_reserved_bits",
        ))
        .is_none()
    );

    let dm21 = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dm21_readiness_example",
    );
    let expected_dm21 = Dm21Readiness {
        distance_with_mil_on_km: 100,
        distance_since_codes_cleared_km: 5_000,
        minutes_with_mil_on: 60,
        time_since_codes_cleared_min: 1_440,
        comprehensive_component: 0xAA,
        fuel_system: 0xBB,
        misfire: 0xCC,
    };
    assert_eq!(expected_dm21.encode(), dm21);
    assert_eq!(Dm21Readiness::decode(&dm21), Some(expected_dm21));

    let product_id = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "product_identification_acme_x42_sn1",
    );
    let expected_product_id = ProductIdentification {
        make: "Acme".into(),
        model: "X42".into(),
        serial_number: "SN-1".into(),
    };
    assert_eq!(expected_product_id.encode().unwrap(), product_id);
    assert_eq!(
        ProductIdentification::decode(&product_id),
        Some(expected_product_id)
    );
    for malformed in [
        "product_identification_missing_field",
        "product_identification_trailing_data",
    ] {
        assert!(
            ProductIdentification::decode(&parse_named_hex_bytes(
                J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
    assert_eq!(
        ProductIdentification::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "product_identification_extended_latin1",
        )),
        Some(ProductIdentification {
            make: "Acme".into(),
            model: "X42".into(),
            serial_number: "SNÿ".into(),
        })
    );

    let software_id = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "software_identification_three_versions",
    );
    let expected_software_id = SoftwareIdentification {
        versions: vec!["1.0.0".into(), "1.0.1".into(), "BETA".into()],
    };
    assert_eq!(expected_software_id.encode().unwrap(), software_id);
    assert_eq!(
        SoftwareIdentification::decode(&software_id),
        Some(expected_software_id)
    );
    for malformed in [
        "software_identification_missing_final_delimiter",
        "software_identification_empty",
    ] {
        assert!(
            SoftwareIdentification::decode(&parse_named_hex_bytes(
                J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
    assert_eq!(
        SoftwareIdentification::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "software_identification_extended_latin1",
        )),
        Some(SoftwareIdentification {
            versions: vec!["1.0.ÿ".into()],
        })
    );

    let dm9_request =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX, "dm9_vin_request");
    assert_eq!(
        Dm9VehicleIdentificationRequest.encode().unwrap().as_slice(),
        dm9_request.as_slice()
    );
    assert_eq!(
        Dm9VehicleIdentificationRequest::decode(&dm9_request),
        Some(Dm9VehicleIdentificationRequest)
    );
    assert_eq!(
        Dm9VehicleIdentificationRequest::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dm9_vin_request_padded",
        )),
        Some(Dm9VehicleIdentificationRequest)
    );

    let dm10_vin = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dm10_vehicle_identification",
    );
    let expected_vin = Dm10VehicleIdentification {
        vin: "1HGBH41JXMN109186".into(),
    };
    assert_eq!(expected_vin.encode().unwrap(), dm10_vin);
    assert_eq!(
        Dm10VehicleIdentification::decode(&dm10_vin),
        Some(expected_vin)
    );
    for malformed in [
        "dm10_vehicle_identification_missing_final_delimiter",
        "dm10_vehicle_identification_trailing_data",
        "dm10_vehicle_identification_non_ascii",
    ] {
        assert!(
            Dm10VehicleIdentification::decode(&parse_named_hex_bytes(
                J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let record_short =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX, "record_short10");
    let record_overlong =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX, "record_overlong12");
    assert!(Dm8TestResult::decode(&record_short).is_none());
    assert!(Dm8TestResult::decode(&record_overlong).is_none());
    assert!(Dm21Readiness::decode(&record_short).is_none());
    assert!(Dm21Readiness::decode(&record_overlong).is_none());

    let dm20_empty =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX, "dm20_empty_padded");
    assert_eq!(Dm20Response::default().encode(), dm20_empty);
    assert_eq!(
        Dm20Response::decode(&dm20_empty),
        Some(Dm20Response::default())
    );

    let dm20_one_ratio =
        parse_named_hex_bytes(J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX, "dm20_one_ratio");
    let expected_dm20 = Dm20Response {
        ignition_cycles: 10,
        obd_monitoring_conditions_met: 5,
        ratios: vec![MonitorPerformanceRatio {
            spn: 0x100,
            numerator: 80,
            denominator: 100,
        }],
    };
    assert_eq!(expected_dm20.encode(), dm20_one_ratio);
    assert_eq!(Dm20Response::decode(&dm20_one_ratio), Some(expected_dm20));
    assert_eq!(
        Dm20Response {
            ignition_cycles: 10,
            obd_monitoring_conditions_met: 5,
            ratios: vec![MonitorPerformanceRatio {
                spn: 0x8_0000,
                numerator: 80,
                denominator: 100,
            }],
        }
        .encode(),
        parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dm20_spn_clamped_ratio"
        )
    );
    for malformed in [
        "dm20_short7",
        "dm20_truncated_ratio_as_8",
        "dm20_misaligned10",
        "dm20_one_ratio_bad_spn_reserved_bits",
    ] {
        assert!(
            Dm20Response::decode(&parse_named_hex_bytes(
                J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let freeze = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "freeze_frame_one_snapshot",
    );
    let expected_freeze = FreezeFrame {
        dtc: Dtc {
            spn: 0x123,
            fmi: Fmi::VoltageHigh,
            occurrence_count: 2,
        },
        timestamp_ms: 0xCAFE_F00D,
        snapshots: vec![SpnSnapshot {
            spn: 0x100,
            value: 1500,
        }],
    };
    assert_eq!(expected_freeze.encode().unwrap(), freeze);
    assert_eq!(FreezeFrame::decode(&freeze), Some(expected_freeze));
    assert_eq!(
        FreezeFrame {
            dtc: Dtc {
                spn: 0x8_0000,
                fmi: Fmi::VoltageHigh,
                occurrence_count: 3,
            },
            timestamp_ms: 0xCAFE_F00D,
            snapshots: vec![SpnSnapshot {
                spn: 0x8_0000,
                value: 1500,
            }],
        }
        .encode()
        .unwrap(),
        parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "freeze_frame_clamped_spn_snapshot",
        )
    );
    for malformed in [
        "freeze_truncated",
        "freeze_count_mismatch",
        "freeze_overlong",
        "freeze_bad_snapshot_spn_reserved_bits",
    ] {
        assert!(
            FreezeFrame::decode(&parse_named_hex_bytes(
                J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
}

#[test]
fn fixture_j1939_heartbeat_and_maintain_power_codecs_are_stable() {
    let heartbeat_init =
        parse_named_hex_bytes(J1939_HEARTBEAT_MAINTAIN_POWER_HEX, "heartbeat_init");
    let heartbeat_seq0 =
        parse_named_hex_bytes(J1939_HEARTBEAT_MAINTAIN_POWER_HEX, "heartbeat_seq0");
    let heartbeat_seq250 =
        parse_named_hex_bytes(J1939_HEARTBEAT_MAINTAIN_POWER_HEX, "heartbeat_seq250");
    let heartbeat_sender_error =
        parse_named_hex_bytes(J1939_HEARTBEAT_MAINTAIN_POWER_HEX, "heartbeat_sender_error");
    let heartbeat_shutdown =
        parse_named_hex_bytes(J1939_HEARTBEAT_MAINTAIN_POWER_HEX, "heartbeat_shutdown");
    let heartbeat_request = parse_named_hex_bytes(
        J1939_HEARTBEAT_MAINTAIN_POWER_HEX,
        "heartbeat_request_agisostack",
    );

    assert_eq!(heartbeat_init, [hb_seq::INIT]);
    assert_eq!(heartbeat_seq0, [0]);
    assert_eq!(heartbeat_seq250, [hb_seq::MAX_NORMAL]);
    assert_eq!(heartbeat_sender_error, [hb_seq::SENDER_ERROR]);
    assert_eq!(heartbeat_shutdown, [hb_seq::SHUTDOWN]);

    let mut sender = HeartbeatSender::default();
    assert_eq!(vec![sender.next_sequence()], heartbeat_init);
    assert_eq!(vec![sender.next_sequence()], heartbeat_seq0);
    for _ in 1..hb_seq::MAX_NORMAL {
        sender.next_sequence();
    }
    assert_eq!(vec![sender.next_sequence()], heartbeat_seq250);
    sender.signal_error();
    assert_eq!(vec![sender.next_sequence()], heartbeat_sender_error);
    sender.signal_shutdown();
    assert_eq!(vec![sender.next_sequence()], heartbeat_shutdown);

    let heartbeat_frame = Frame::from_message(
        Priority::Default,
        PGN_HEARTBEAT,
        0x80,
        BROADCAST_ADDRESS,
        &heartbeat_init,
    );
    assert_eq!(heartbeat_frame.pgn(), PGN_HEARTBEAT);
    assert_eq!(heartbeat_frame.destination(), BROADCAST_ADDRESS);
    assert_eq!(heartbeat_frame.payload(), heartbeat_init.as_slice());

    let expected_request = HeartbeatRequest::for_heartbeat(100);
    assert_eq!(
        expected_request.encode().unwrap(),
        heartbeat_request.as_slice()
    );
    assert_eq!(
        HeartbeatRequest::decode(&heartbeat_request),
        Some(expected_request)
    );
    let request_frame = Frame::from_message(
        Priority::Default,
        PGN_HEARTBEAT_REQUEST,
        0x41,
        0xF4,
        &heartbeat_request,
    );
    assert_eq!(request_frame.id.raw, 0x18CC_F441);
    assert_eq!(request_frame.destination(), 0xF4);
    assert_eq!(request_frame.payload(), heartbeat_request.as_slice());

    let inactive = parse_named_hex_frame(
        J1939_HEARTBEAT_MAINTAIN_POWER_HEX,
        "maintain_all_inactive_no_request",
    );
    let expected_inactive = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Inactive,
        implement_park_state: MaintainPowerState::Inactive,
        implement_ready_to_work_state: MaintainPowerState::Inactive,
        implement_transport_state: MaintainPowerState::Inactive,
        maintain_actuator_power: MaintainPowerRequirement::NoFurtherRequirement,
        maintain_ecu_power: MaintainPowerRequirement::NoFurtherRequirement,
        timestamp_us: 0,
    };
    assert_eq!(expected_inactive.encode(), inactive);
    assert_eq!(
        MaintainPowerData::decode(&inactive).unwrap(),
        expected_inactive
    );

    let cf_request = parse_named_hex_frame(
        J1939_HEARTBEAT_MAINTAIN_POWER_HEX,
        "maintain_all_active_request",
    );
    let expected_request = MaintainPowerData {
        implement_in_work_state: MaintainPowerState::Active,
        implement_park_state: MaintainPowerState::Active,
        implement_ready_to_work_state: MaintainPowerState::Active,
        implement_transport_state: MaintainPowerState::Active,
        maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
        timestamp_us: 0,
    };
    assert_eq!(expected_request.encode(), cf_request);
    assert_eq!(
        MaintainPowerData::decode(&cf_request).unwrap(),
        expected_request
    );
    assert!(
        MaintainPowerData::decode(&parse_named_hex_bytes(
            J1939_HEARTBEAT_MAINTAIN_POWER_HEX,
            "maintain_short7",
        ))
        .is_none()
    );
    assert!(
        MaintainPowerData::decode(&parse_named_hex_bytes(
            J1939_HEARTBEAT_MAINTAIN_POWER_HEX,
            "maintain_overlong9",
        ))
        .is_none()
    );
    for malformed in ["maintain_bad_reserved_flags", "maintain_bad_reserved_tail"] {
        assert!(
            MaintainPowerData::decode(&parse_named_hex_bytes(
                J1939_HEARTBEAT_MAINTAIN_POWER_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let keyoff = parse_named_hex_frame(
        J1939_HEARTBEAT_MAINTAIN_POWER_HEX,
        "maintain_all_inactive_no_request",
    );
    let mut tecu = PowerManager::new(PowerRole::Tecu);
    tecu.key_off();
    let broadcasts = tecu.update(100);
    assert_eq!(broadcasts.len(), 1);
    assert_eq!(broadcasts[0].encode(), keyoff);

    let keyoff_msg = Message::new(PGN_MAINTAIN_POWER, keyoff.to_vec(), 0xF0);
    let mut cf = PowerManager::new(PowerRole::Cf);
    cf.handle_message(&keyoff_msg);
    assert_eq!(cf.state(), PowerState::Running);

    let maintain_frame = Frame::from_message(
        Priority::Default,
        PGN_MAINTAIN_POWER,
        0xF0,
        BROADCAST_ADDRESS,
        &keyoff,
    );
    assert_eq!(maintain_frame.pgn(), PGN_MAINTAIN_POWER);
    assert_eq!(maintain_frame.destination(), BROADCAST_ADDRESS);
    assert_eq!(maintain_frame.data, keyoff);
}

#[test]
fn fixture_j1939_engine_powertrain_codecs_are_stable() {
    let eec1 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "eec1_50_75_45_1500rpm_src10_starter5",
    );
    let expected_eec1 = Eec1 {
        engine_torque_percent: 50.0,
        driver_demand_percent: 75.0,
        actual_engine_percent: 45.0,
        engine_speed_rpm: 1500.0,
        starter_mode: 0x05,
        source_address: 0x10,
    };
    assert_eq!(expected_eec1.encode(), eec1);
    let decoded_eec1 = Eec1::decode(&eec1).unwrap();
    assert_eq!(decoded_eec1.engine_torque_percent, 50.0);
    assert_eq!(decoded_eec1.driver_demand_percent, 75.0);
    assert_eq!(decoded_eec1.actual_engine_percent, 45.0);
    assert_eq!(decoded_eec1.engine_speed_rpm, 1500.0);
    assert_eq!(decoded_eec1.starter_mode, 0x05);
    assert_eq!(decoded_eec1.source_address, 0x10);

    let eec2 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "eec2_pedal200_load65_limit80",
    );
    let expected_eec2 = Eec2 {
        accel_pedal_position: 200,
        engine_load_percent: 65.0,
        accel_pedal_low_idle: 1,
        accel_pedal_kickdown: 0,
        road_speed_limit: 80,
    };
    assert_eq!(expected_eec2.encode(), eec2);
    let decoded_eec2 = Eec2::decode(&eec2).unwrap();
    assert_eq!(decoded_eec2.accel_pedal_position, 200);
    assert_eq!(decoded_eec2.engine_load_percent, 65.0);
    assert_eq!(decoded_eec2.accel_pedal_low_idle, 1);
    assert_eq!(decoded_eec2.accel_pedal_kickdown, 0);
    assert_eq!(decoded_eec2.road_speed_limit, 80);

    let engine_temp1 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_temp1_90_50_100_110_60",
    );
    let expected_temp1 = EngineTemp1 {
        coolant_temp_c: 90.0,
        fuel_temp_c: 50.0,
        oil_temp_c: 100.0,
        turbo_oil_temp_c: 110.0,
        intercooler_temp_c: 60.0,
    };
    assert_eq!(expected_temp1.encode(), engine_temp1);
    let decoded_temp1 = EngineTemp1::decode(&engine_temp1).unwrap();
    assert_eq!(decoded_temp1.coolant_temp_c, 90.0);
    assert_eq!(decoded_temp1.fuel_temp_c, 50.0);
    assert!((decoded_temp1.oil_temp_c - 100.0).abs() < 0.1);
    assert!((decoded_temp1.turbo_oil_temp_c - 110.0).abs() < 0.1);
    assert_eq!(decoded_temp1.intercooler_temp_c, 60.0);

    let engine_hours = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_hours_12345_7_1e9rev",
    );
    let expected_hours = EngineHours {
        total_hours: 12_345.7,
        total_revolutions: 1_000_000_000.0,
    };
    assert_eq!(expected_hours.encode(), engine_hours);
    let decoded_hours = EngineHours::decode(&engine_hours).unwrap();
    assert!((decoded_hours.total_hours - expected_hours.total_hours).abs() < 0.1);
    assert!((decoded_hours.total_revolutions - expected_hours.total_revolutions).abs() < 1000.0);

    let fuel_economy =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "fuel_economy_25_6_5_80");
    let expected_fuel = FuelEconomy {
        fuel_rate_lph: 25.0,
        instantaneous_lph: 6.5,
        throttle_position: 80.0,
    };
    assert_eq!(expected_fuel.encode(), fuel_economy);
    let decoded_fuel = FuelEconomy::decode(&fuel_economy).unwrap();
    assert!((decoded_fuel.fuel_rate_lph - 25.0).abs() < 0.1);
    assert!((decoded_fuel.instantaneous_lph - 6.5).abs() < 0.01);
    assert!((decoded_fuel.throttle_position - 80.0).abs() < 0.5);

    let tsc1 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "tsc1_speed_1200_torque50",
    );
    let expected_tsc1 = Tsc1 {
        override_mode: OverrideControlMode::SpeedControl,
        requested_speed_rpm: 1200.0,
        requested_torque_percent: 50.0,
    };
    assert_eq!(expected_tsc1.encode(), tsc1);
    let decoded_tsc1 = Tsc1::decode(&tsc1).unwrap();
    assert_eq!(
        decoded_tsc1.override_mode,
        OverrideControlMode::SpeedControl
    );
    assert_eq!(decoded_tsc1.requested_speed_rpm, 1200.0);
    assert_eq!(decoded_tsc1.requested_torque_percent, 50.0);

    let etc1 = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "etc1_gear5_6_1500rpm");
    let expected_etc1 = Etc1 {
        current_gear: 5,
        selected_gear: 6,
        output_shaft_speed_rpm: 1500.0,
        shift_in_progress: 1,
        torque_converter_lockup: 2,
    };
    assert_eq!(expected_etc1.encode(), etc1);
    let decoded_etc1 = Etc1::decode(&etc1).unwrap();
    assert_eq!(decoded_etc1.current_gear, 5);
    assert_eq!(decoded_etc1.selected_gear, 6);
    assert_eq!(decoded_etc1.output_shaft_speed_rpm, 1500.0);
    assert_eq!(decoded_etc1.shift_in_progress, 1);
    assert_eq!(decoded_etc1.torque_converter_lockup, 2);

    let transmission_oil =
        parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "transmission_oil_80");
    let expected_transmission_oil = TransmissionOilTemp { oil_temp_c: 80.0 };
    assert_eq!(expected_transmission_oil.encode(), transmission_oil);
    assert!(
        (TransmissionOilTemp::decode(&transmission_oil)
            .unwrap()
            .oil_temp_c
            - 80.0)
            .abs()
            < 0.05
    );

    let cruise = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "cruise_65_5_set65");
    let expected_cruise = CruiseControl {
        wheel_speed_kmh: 65.5,
        cc_active: 1,
        brake_switch: 0,
        clutch_switch: 0,
        park_brake: 0,
        cc_set_speed_kmh: 65.0,
    };
    assert_eq!(expected_cruise.encode(), cruise);
    let decoded_cruise = CruiseControl::decode(&cruise).unwrap();
    assert!((decoded_cruise.wheel_speed_kmh - 65.5).abs() < 1.0 / 256.0);
    assert_eq!(decoded_cruise.cc_active, 1);
    assert_eq!(decoded_cruise.brake_switch, 0);
    assert_eq!(decoded_cruise.clutch_switch, 0);
    assert_eq!(decoded_cruise.park_brake, 0);
    assert!((decoded_cruise.cc_set_speed_kmh - 65.0).abs() < 1.0 / 256.0);

    let speed_distance = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "speed_distance_5mps_1234_5m",
    );
    let expected_speed_distance = SpeedAndDistance {
        speed_mps: Some(5.0),
        distance_m: Some(1234.5),
        timestamp_us: 0,
    };
    assert_eq!(expected_speed_distance.encode(), speed_distance);
    let decoded_speed_distance = SpeedAndDistance::decode(&speed_distance).unwrap();
    assert_eq!(decoded_speed_distance.speed_mps, Some(5.0));
    assert_eq!(decoded_speed_distance.distance_m, Some(1234.5));

    let speed_na = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "speed_distance_not_available",
    );
    assert_eq!(SpeedAndDistance::default().encode(), speed_na);
    let decoded_speed_na = SpeedAndDistance::decode(&speed_na).unwrap();
    assert_eq!(decoded_speed_na.speed_mps, None);
    assert_eq!(decoded_speed_na.distance_m, None);
}

#[test]
fn fixture_j1939_remaining_engine_powertrain_codecs_are_stable() {
    let eec3 = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "eec3_25_1800rpm_asym50");
    let expected_eec3 = Eec3 {
        nominal_friction_percent: 25.0,
        desired_operating_speed_rpm: 1800.0,
        operating_speed_asymmetry: 50,
    };
    assert_eq!(expected_eec3.encode(), eec3);
    let decoded_eec3 = Eec3::decode(&eec3).unwrap();
    assert!((decoded_eec3.nominal_friction_percent - 25.0).abs() < 1.0);
    assert!((decoded_eec3.desired_operating_speed_rpm - 1800.0).abs() < 0.125);
    assert_eq!(decoded_eec3.operating_speed_asymmetry, 50);

    let engine_temp2 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_temp2_95_105_55_200",
    );
    let expected_temp2 = EngineTemp2 {
        engine_oil_temp_c: 95.0,
        turbo_oil_temp_c: 105.0,
        engine_intercooler_temp_c: 55.0,
        turbo_1_temp_c: 200.0,
    };
    assert_eq!(expected_temp2.encode(), engine_temp2);
    let decoded_temp2 = EngineTemp2::decode(&engine_temp2).unwrap();
    assert!((decoded_temp2.engine_oil_temp_c - 95.0).abs() < 0.1);
    assert!((decoded_temp2.turbo_oil_temp_c - 105.0).abs() < 0.1);
    assert_eq!(decoded_temp2.engine_intercooler_temp_c, 55.0);
    assert!((decoded_temp2.turbo_1_temp_c - 200.0).abs() < 0.1);

    let fluid = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "engine_fluid_lp_400_200_levels_crank",
    );
    let expected_fluid = EngineFluidLp {
        oil_pressure_kpa: 400.0,
        coolant_pressure_kpa: 200.0,
        oil_level_percent: 200,
        coolant_level_percent: 220,
        fuel_delivery_pressure_kpa: 300.0,
        crankcase_pressure_kpa: 0.5,
    };
    assert_eq!(expected_fluid.encode(), fluid);
    let decoded_fluid = EngineFluidLp::decode(&fluid).unwrap();
    assert_eq!(decoded_fluid.oil_level_percent, 200);
    assert_eq!(decoded_fluid.coolant_level_percent, 220);
    assert!((decoded_fluid.fuel_delivery_pressure_kpa - 300.0).abs() < 4.0);
    assert!((decoded_fluid.oil_pressure_kpa - 400.0).abs() < 4.0);
    assert!((decoded_fluid.coolant_pressure_kpa - 200.0).abs() < 2.0);
    assert!((decoded_fluid.crankcase_pressure_kpa - 0.5).abs() < 0.05);

    let vep1 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "vep1_12_5_14_0_12_0_alt50",
    );
    let expected_vep1 = Vep1 {
        battery_voltage_v: 12.5,
        alternator_current_a: 50.0,
        charging_system_voltage_v: 14.0,
        key_switch_voltage_v: 12.0,
    };
    assert_eq!(expected_vep1.encode(), vep1);
    let decoded_vep1 = Vep1::decode(&vep1).unwrap();
    assert!((decoded_vep1.battery_voltage_v - 12.5).abs() < 0.05);
    assert!((decoded_vep1.charging_system_voltage_v - 14.0).abs() < 0.05);
    assert!((decoded_vep1.key_switch_voltage_v - 12.0).abs() < 0.05);
    assert_eq!(decoded_vep1.alternator_current_a, 50.0);

    let ambient = parse_named_hex_frame(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "ambient_101_25_30_22");
    let expected_ambient = AmbientConditions {
        barometric_pressure_kpa: 101.0,
        ambient_air_temp_c: 25.0,
        intake_air_temp_c: 30.0,
        road_surface_temp_c: 22.0,
    };
    assert_eq!(expected_ambient.encode(), ambient);
    let decoded_ambient = AmbientConditions::decode(&ambient).unwrap();
    assert_eq!(decoded_ambient.barometric_pressure_kpa, 101.0);
    assert!((decoded_ambient.ambient_air_temp_c - 25.0).abs() < 0.1);
    assert_eq!(decoded_ambient.intake_air_temp_c, 30.0);
    assert!((decoded_ambient.road_surface_temp_c - 22.0).abs() < 0.1);

    let dash = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "dash_display_levels_filters_temp",
    );
    let expected_dash = DashDisplay {
        fuel_level_percent: 200,
        washer_fluid_level: 180,
        fuel_filter_diff_kpa: 50.0,
        oil_filter_diff_kpa: 25.0,
        cargo_ambient_temp_c: 20.0,
    };
    assert_eq!(expected_dash.encode(), dash);
    let decoded_dash = DashDisplay::decode(&dash).unwrap();
    assert_eq!(decoded_dash.fuel_level_percent, 200);
    assert_eq!(decoded_dash.washer_fluid_level, 180);
    assert_eq!(decoded_dash.fuel_filter_diff_kpa, 50.0);
    assert_eq!(decoded_dash.oil_filter_diff_kpa, 25.0);
    assert!((decoded_dash.cargo_ambient_temp_c - 20.0).abs() < 0.1);

    let position = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "vehicle_position_zero_zero",
    );
    let expected_position = VehiclePosition {
        latitude_deg: 0.0,
        longitude_deg: 0.0,
    };
    assert_eq!(expected_position.encode(), position);
    let decoded_position = VehiclePosition::decode(&position).unwrap();
    assert!((decoded_position.latitude_deg - 0.0).abs() < 1e-6);
    assert!((decoded_position.longitude_deg - 0.0).abs() < 1e-6);

    let fuel_consumption = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "fuel_consumption_250_5_12345",
    );
    let expected_consumption = FuelConsumption {
        trip_fuel_l: 250.5,
        total_fuel_l: 12_345.0,
    };
    assert_eq!(expected_consumption.encode(), fuel_consumption);
    let decoded_consumption = FuelConsumption::decode(&fuel_consumption).unwrap();
    assert!((decoded_consumption.trip_fuel_l - 250.5).abs() < 0.5);
    assert!((decoded_consumption.total_fuel_l - 12_345.0).abs() < 0.5);

    let aftertreatment1 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "aftertreatment1_def75_nox",
    );
    let expected_at1 = Aftertreatment1 {
        def_tank_level: 75.0,
        intake_nox_ppm: 1500.0,
        outlet_nox_ppm: 50.0,
        intake_nox_reading_status: 1,
        outlet_nox_reading_status: 1,
    };
    assert_eq!(expected_at1.encode(), aftertreatment1);
    let decoded_at1 = Aftertreatment1::decode(&aftertreatment1).unwrap();
    assert!((decoded_at1.def_tank_level - 75.0).abs() < 0.5);
    assert!((decoded_at1.intake_nox_ppm - 1500.0).abs() < 0.05);
    assert!((decoded_at1.outlet_nox_ppm - 50.0).abs() < 0.05);
    assert_eq!(decoded_at1.intake_nox_reading_status, 1);
    assert_eq!(decoded_at1.outlet_nox_reading_status, 1);

    let aftertreatment2 = parse_named_hex_frame(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "aftertreatment2_diff5_5_def32_5_soot75",
    );
    let expected_at2 = Aftertreatment2 {
        dpf_differential_pressure_kpa: 5.5,
        def_concentration: 32.5,
        dpf_soot_load_percent: 75.0,
        dpf_active_regeneration_status: 2,
        dpf_passive_regeneration_status: 1,
    };
    assert_eq!(expected_at2.encode(), aftertreatment2);
    let decoded_at2 = Aftertreatment2::decode(&aftertreatment2).unwrap();
    assert!((decoded_at2.dpf_differential_pressure_kpa - 5.5).abs() < 0.1);
    assert!((decoded_at2.def_concentration - 32.4).abs() < 0.1);
    assert!((decoded_at2.dpf_soot_load_percent - 74.8).abs() < 0.5);
    assert_eq!(decoded_at2.dpf_active_regeneration_status, 2);
    assert_eq!(decoded_at2.dpf_passive_regeneration_status, 1);

    let component_id = parse_named_hex_bytes(
        J1939_ENGINE_POWERTRAIN_CODECS_HEX,
        "component_id_acme_x1000",
    );
    let expected_component = ComponentIdentification {
        make: "Acme".into(),
        model: "X1000".into(),
        serial_number: "SN-001".into(),
        unit_number: "U-42".into(),
    };
    assert_eq!(expected_component.encode(), component_id);
    assert_eq!(
        ComponentIdentification::decode(&component_id),
        Some(expected_component)
    );

    let vehicle_id =
        parse_named_hex_bytes(J1939_ENGINE_POWERTRAIN_CODECS_HEX, "vehicle_id_sample_vin");
    let expected_vehicle_id = VehicleIdentification {
        vin: "1HGBH41JXMN109186".into(),
    };
    assert_eq!(expected_vehicle_id.encode(), vehicle_id);
    assert_eq!(
        VehicleIdentification::decode(&vehicle_id),
        Some(expected_vehicle_id)
    );

    for malformed in [
        "component_id_missing_field",
        "component_id_trailing_data",
        "component_id_non_ascii",
    ] {
        assert!(
            ComponentIdentification::decode(&parse_named_hex_bytes(
                J1939_ENGINE_POWERTRAIN_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    for malformed in [
        "vehicle_id_missing_delimiter",
        "vehicle_id_trailing_data",
        "vehicle_id_non_ascii",
    ] {
        assert!(
            VehicleIdentification::decode(&parse_named_hex_bytes(
                J1939_ENGINE_POWERTRAIN_CODECS_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
}

