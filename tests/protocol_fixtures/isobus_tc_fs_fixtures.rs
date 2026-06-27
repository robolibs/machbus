#[test]
fn fixture_isobus_tc_object_pool_transfer_and_activation_are_stable() {
    let ddop = parse_named_hex_bytes(ISOBUS_TC_DDOP_HEX, "sprayer_one_section_all_object_types");
    let transfer = parse_named_hex_bytes(
        ISOBUS_TC_OBJECT_POOL_HEX,
        "object_pool_transfer_sprayer_one_section",
    );
    assert_eq!(transfer[0], tc_cmd::OBJECT_POOL_TRANSFER);
    assert_eq!(&transfer[1..], ddop.as_slice());

    let success = parse_named_hex_frame(ISOBUS_TC_OBJECT_POOL_HEX, "object_pool_response_success");
    assert_eq!(success[0], tc_cmd::OBJECT_POOL_RESPONSE);
    assert_eq!(success[1], ObjectPoolErrorCodes::NoErrors.as_u8());

    let error = parse_named_hex_frame(ISOBUS_TC_OBJECT_POOL_HEX, "object_pool_response_error");
    assert_eq!(error[0], tc_cmd::OBJECT_POOL_RESPONSE);
    assert_eq!(error[1], ObjectPoolErrorCodes::AnyOtherError.as_u8());

    let activate = parse_named_hex_frame(ISOBUS_TC_OBJECT_POOL_HEX, "activate_pool_request");
    assert_eq!(activate[0], tc_cmd::ACTIVATE_POOL);
    assert!(activate[1..].iter().all(|&b| b == 0xFF));

    let activate_ok =
        parse_named_hex_frame(ISOBUS_TC_OBJECT_POOL_HEX, "activate_pool_response_success");
    assert_eq!(activate_ok[0], tc_cmd::ACTIVATE_RESPONSE);
    assert_eq!(activate_ok[1], ObjectPoolActivationError::NoErrors.as_u8());

    let activate_err = parse_named_hex_frame(
        ISOBUS_TC_OBJECT_POOL_HEX,
        "activate_pool_response_ddop_error",
    );
    assert_eq!(activate_err[0], tc_cmd::ACTIVATE_RESPONSE);
    assert_eq!(
        activate_err[1],
        ObjectPoolActivationError::ThereAreErrorsInTheDDOP.as_u8()
    );

    let deactivate = parse_named_hex_frame(ISOBUS_TC_OBJECT_POOL_HEX, "deactivate_pool_request");
    assert_eq!(deactivate[0], tc_cmd::ACTIVATE_POOL);
    assert_eq!(deactivate[1], 0x00);
    assert!(deactivate[2..].iter().all(|&b| b == 0xFF));

    let delete = parse_named_hex_frame(ISOBUS_TC_OBJECT_POOL_HEX, "delete_pool_request");
    assert_eq!(delete[0], tc_cmd::DELETE_POOL);
    assert!(delete[1..].iter().all(|&b| b == 0xFF));

    let mut malformed_server = TaskControllerServer::new(valid_tc_server_config());
    malformed_server.start().unwrap();
    assert!(
        malformed_server
            .handle_client_message(&Message::new(PGN_TC_TO_ECU, transfer.clone(), 0x80))
            .is_empty(),
        "TC server must ignore object-pool payloads delivered under the wrong PGN"
    );
    for invalid_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert!(
            malformed_server
                .handle_client_message(&Message::new(
                    PGN_ECU_TO_TC,
                    transfer.clone(),
                    invalid_source,
                ))
                .is_empty(),
            "TC server must ignore object-pool payloads from invalid source 0x{invalid_source:02X}"
        );
        assert!(
            malformed_server.clients().is_empty(),
            "invalid TC client source must not create a server client entry"
        );
    }
    for malformed in [
        "malformed_activate_pool_short",
        "malformed_activate_pool_bad_padding",
        "malformed_deactivate_pool_short",
        "malformed_deactivate_pool_bad_padding",
        "malformed_delete_pool_short",
        "malformed_delete_pool_bad_padding",
    ] {
        let out = malformed_server.handle_client_message(&Message::new(
            PGN_ECU_TO_TC,
            parse_named_hex_bytes(ISOBUS_TC_OBJECT_POOL_HEX, malformed),
            0x80,
        ));
        assert!(
            out.is_empty(),
            "{malformed} must not trigger TC lifecycle responses"
        );
        assert!(
            malformed_server.clients().is_empty(),
            "{malformed} must not create a TC client entry"
        );
    }

    let mut malformed_client = TaskControllerClient::new(Default::default());
    malformed_client.set_ddop(DDOP::deserialize(&ddop).unwrap());
    malformed_client.connect().unwrap();
    malformed_client.handle_tc_message(&Message::new(
        PGN_ECU_TO_TC,
        parse_named_hex_frame(ISOBUS_TC_PROCESS_DATA_HEX, "tc_status_server_v4_b1_s8_c3").to_vec(),
        0x33,
    ));
    assert_eq!(
        malformed_client.state(),
        TCState::WaitForServerStatus,
        "TC client must ignore a server-status payload delivered under the wrong PGN"
    );
    for invalid_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        malformed_client.handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            parse_named_hex_frame(ISOBUS_TC_PROCESS_DATA_HEX, "tc_status_server_v4_b1_s8_c3")
                .to_vec(),
            invalid_source,
        ));
        assert_eq!(
            malformed_client.state(),
            TCState::WaitForServerStatus,
            "TC client must ignore server status from invalid source 0x{invalid_source:02X}"
        );
    }
    malformed_client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        parse_named_hex_frame(ISOBUS_TC_PROCESS_DATA_HEX, "tc_status_server_v4_b1_s8_c3").to_vec(),
        0x33,
    ));
    malformed_client.update(1); // Working Set Master.
    malformed_client.update(1); // Version request.
    assert_eq!(malformed_client.state(), TCState::WaitForVersion);
    malformed_client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_TC_PROCESS_DATA_HEX,
            "tech_capabilities_response_v4_b1_s8_c3",
        )
        .to_vec(),
        0x34,
    ));
    assert_eq!(
        malformed_client.state(),
        TCState::WaitForVersion,
        "TC client must ignore handshake responses from a different bound TC source"
    );
    for malformed in [
        "malformed_version_response_short",
        "malformed_version_response_bad_padding",
    ] {
        malformed_client.handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            parse_named_hex_bytes(ISOBUS_TC_OBJECT_POOL_HEX, malformed),
            0x33,
        ));
        assert_eq!(
            malformed_client.state(),
            TCState::WaitForVersion,
            "{malformed} must not advance the TC client"
        );
    }
    malformed_client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_TC_PROCESS_DATA_HEX,
            "tech_capabilities_response_v4_b1_s8_c3",
        )
        .to_vec(),
        0x33,
    ));
    assert_eq!(malformed_client.state(), TCState::RequestStructureLabel);
    malformed_client.update(1);
    assert_eq!(malformed_client.state(), TCState::WaitForStructureLabel);
    malformed_client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        vec![tc_cmd::STRUCTURE_LABEL],
        0x33,
    ));
    assert_eq!(malformed_client.state(), TCState::WaitForStructureLabel);
    malformed_client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        vec![
            tc_cmd::STRUCTURE_LABEL,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x33,
    ));
    assert_eq!(malformed_client.state(), TCState::TransferDDOP);
    malformed_client.update(1);
    assert_eq!(malformed_client.state(), TCState::WaitForPoolResponse);
    for malformed in [
        "malformed_object_pool_response_short",
        "malformed_object_pool_response_bad_padding",
    ] {
        malformed_client.handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            parse_named_hex_bytes(ISOBUS_TC_OBJECT_POOL_HEX, malformed),
            0x33,
        ));
        assert_eq!(
            malformed_client.state(),
            TCState::WaitForPoolResponse,
            "{malformed} must not advance the TC client"
        );
    }
    malformed_client.handle_tc_message(&Message::new(PGN_TC_TO_ECU, success.to_vec(), 0x33));
    assert_eq!(malformed_client.state(), TCState::ActivatePool);
    malformed_client.update(1);
    assert_eq!(malformed_client.state(), TCState::WaitForActivation);
    for malformed in [
        "malformed_activate_response_short",
        "malformed_activate_response_bad_padding",
    ] {
        malformed_client.handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            parse_named_hex_bytes(ISOBUS_TC_OBJECT_POOL_HEX, malformed),
            0x33,
        ));
        assert_eq!(
            malformed_client.state(),
            TCState::WaitForActivation,
            "{malformed} must not advance the TC client"
        );
    }
    malformed_client.handle_tc_message(&Message::new(PGN_TC_TO_ECU, activate_ok.to_vec(), 0x33));
    assert_eq!(malformed_client.state(), TCState::Connected);
    malformed_client
        .reupload_ddop(DDOP::deserialize(&ddop).unwrap())
        .unwrap();
    malformed_client.update(1);
    assert_eq!(malformed_client.state(), TCState::WaitForDeactivation);
    malformed_client.handle_tc_message(&Message::new(
        PGN_TC_TO_ECU,
        parse_named_hex_bytes(
            ISOBUS_TC_OBJECT_POOL_HEX,
            "malformed_activate_response_short",
        ),
        0x33,
    ));
    assert_eq!(malformed_client.state(), TCState::WaitForDeactivation);
    malformed_client.handle_tc_message(&Message::new(PGN_TC_TO_ECU, activate_ok.to_vec(), 0x33));
    assert_eq!(malformed_client.state(), TCState::DeletePool);
    malformed_client.update(1);
    assert_eq!(malformed_client.state(), TCState::WaitForDeletePool);
    for malformed in [
        "malformed_delete_response_short",
        "malformed_delete_response_bad_padding",
    ] {
        malformed_client.handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            parse_named_hex_bytes(ISOBUS_TC_OBJECT_POOL_HEX, malformed),
            0x33,
        ));
        assert_eq!(
            malformed_client.state(),
            TCState::WaitForDeletePool,
            "{malformed} must not advance the TC client"
        );
    }

    let delete_response = parse_named_hex_frame(
        ISOBUS_TC_OBJECT_POOL_HEX,
        "delete_pool_response_details_unavailable",
    );
    assert_eq!(delete_response[0], tc_cmd::DELETE_POOL_RESPONSE);
    assert_eq!(
        delete_response[1],
        ObjectPoolDeletionErrors::ErrorDetailsNotAvailable.as_u8()
    );

    let mut empty_server = TaskControllerServer::new(valid_tc_server_config());
    empty_server.start().unwrap();
    let out =
        empty_server.handle_client_message(&Message::new(PGN_ECU_TO_TC, activate.to_vec(), 0x80));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].data, activate_err);
    assert!(
        empty_server.clients().is_empty(),
        "activation error before DDOP upload must not allocate a TC client slot"
    );

    let mut server = TaskControllerServer::new(valid_tc_server_config());
    server.start().unwrap();
    let out = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, transfer, 0x80));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].data, success);
    assert_eq!(server.clients()[0].ddop.serialize().unwrap(), ddop);
    assert!(!server.clients()[0].pool_activated);

    let out = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, activate.to_vec(), 0x80));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].data, activate_ok);
    assert!(server.clients()[0].pool_activated);

    let out = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, deactivate.to_vec(), 0x80));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].data, activate_ok);
    assert_eq!(server.clients()[0].ddop.serialize().unwrap(), ddop);
    assert!(!server.clients()[0].pool_activated);

    let out = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, delete.to_vec(), 0x80));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].data, delete_response);
    assert!(server.clients()[0].ddop.devices().is_empty());
    assert!(!server.clients()[0].pool_activated);
}

#[test]
fn fixture_isobus_tc_ddi_database_snapshot_is_stable() {
    let version: u32 = parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "version")
        .parse()
        .unwrap();
    let entries: usize = parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "entries")
        .parse()
        .unwrap();
    let fingerprint = parse_hex_u64(parse_named_text_value(
        ISOBUS_TC_DDI_DATABASE_SNAPSHOT,
        "fingerprint_fnv1a64",
    ));

    assert_eq!(DDI_DATABASE_VERSION, version);
    assert_eq!(DDIDatabase::version(), version);
    assert_eq!(DDI_DATABASE_SIZE, entries);
    assert_eq!(DDI_DATABASE.len(), entries);
    assert_eq!(DDI_DATABASE_FINGERPRINT_FNV1A64, fingerprint);
    assert_eq!(ddi_database_fingerprint(), fingerprint);
    assert_eq!(DDIDatabase::fingerprint(), fingerprint);

    let first = DDI_DATABASE.first().expect("DDI database is non-empty");
    assert_eq!(
        first.ddi.to_string(),
        parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "first_ddi")
    );
    assert_eq!(
        first.name,
        parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "first_name")
    );
    assert_eq!(
        first.unit,
        parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "first_unit")
    );

    let last = DDI_DATABASE.last().expect("DDI database is non-empty");
    assert_eq!(
        last.ddi.to_string(),
        parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "last_ddi")
    );
    assert_eq!(
        last.name,
        parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "last_name")
    );
    assert_eq!(
        last.unit,
        parse_named_text_value(ISOBUS_TC_DDI_DATABASE_SNAPSHOT, "last_unit")
    );

    let rate = ddi_lookup(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE).unwrap();
    assert_eq!(rate.name, "Setpoint Volume Per Area Application Rate");
    assert_eq!(rate.unit, "mm3/m2");
    assert!((rate.resolution - 0.01).abs() < 1e-9);
    assert_eq!(
        DDIDatabase::lookup(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE),
        Some(*rate)
    );
}

#[test]
fn fixture_isobus_tc_process_data_status_and_error_paths_are_stable() {
    let tc_status =
        parse_named_hex_frame(ISOBUS_TC_PROCESS_DATA_HEX, "tc_status_server_v4_b1_s8_c3");
    let tech_request =
        parse_named_hex_bytes(ISOBUS_TC_PROCESS_DATA_HEX, "tech_capabilities_request");
    let tech_caps = parse_named_hex_frame(
        ISOBUS_TC_PROCESS_DATA_HEX,
        "tech_capabilities_response_v4_b1_s8_c3",
    );
    assert_eq!(tech_caps[0], tc_cmd::VERSION_RESPONSE);
    let request_value =
        parse_named_hex_frame(ISOBUS_TC_PROCESS_DATA_HEX, "request_value_elem3_ddi1234");
    let value_response = parse_named_hex_bytes(
        ISOBUS_TC_PROCESS_DATA_HEX,
        "value_response_elem3_ddi1234_i32_99",
    );
    let set_value = parse_named_hex_frame(
        ISOBUS_TC_PROCESS_DATA_HEX,
        "set_value_elem5_ddiCAFE_i32_305419896",
    );
    let set_value_ack = parse_named_hex_frame(
        ISOBUS_TC_PROCESS_DATA_HEX,
        "set_value_ack_elem5_ddiCAFE_i32_42",
    );
    let ack_no_error =
        parse_named_hex_bytes(ISOBUS_TC_PROCESS_DATA_HEX, "ack_elem5_ddiCAFE_no_error");
    let ack_value_outside_range = parse_named_hex_bytes(
        ISOBUS_TC_PROCESS_DATA_HEX,
        "ack_elem5_ddiCAFE_value_outside_range",
    );
    let ack_no_resources =
        parse_named_hex_bytes(ISOBUS_TC_PROCESS_DATA_HEX, "ack_elem5_ddiCAFE_no_resources");

    let mut server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_number(1)
            .with_version(4)
            .with_booms(1)
            .with_sections(8)
            .with_channels(3),
    );
    server.start().unwrap();
    assert_eq!(
        server.update(machbus::isobus::tc::TC_STATUS_INTERVAL_MS),
        Some(tc_status)
    );

    let tech_out = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, tech_request, 0x42));
    assert_eq!(tech_out.len(), 1);
    assert_eq!(tech_out[0].dest, Some(0x42));
    assert_eq!(tech_out[0].data, tech_caps);

    for name in [
        "malformed_tech_capabilities_request_bad_padding",
        "malformed_tech_capabilities_response_command_collision",
    ] {
        let mut malformed_tech_server = TaskControllerServer::new(
            TCServerConfig::default()
                .with_number(1)
                .with_version(4)
                .with_booms(1)
                .with_sections(8)
                .with_channels(3),
        );
        malformed_tech_server.start().unwrap();
        let malformed = parse_named_hex_bytes(ISOBUS_TC_PROCESS_DATA_HEX, name);
        assert!(
            malformed_tech_server
                .handle_client_message(&Message::new(PGN_ECU_TO_TC, malformed, 0x42))
                .is_empty(),
            "server accepted malformed tech-capability request {name}"
        );
        assert!(malformed_tech_server.clients().is_empty());
    }

    assert_eq!(
        TaskControllerServer::build_request_value(3, 0x1234).unwrap(),
        request_value
    );
    let mut client = TaskControllerClient::default();
    let client_value_log: Rc<RefCell<Vec<(ElementNumber, DDI)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let client_value_log_cb = client_value_log.clone();
    client.on_value_request(move |element, ddi| {
        client_value_log_cb.borrow_mut().push((element, ddi));
        Ok(99)
    });
    let response =
        client.handle_tc_message(&Message::new(PGN_TC_TO_ECU, request_value.to_vec(), 0x33));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].dest, Some(0x33));
    assert_eq!(response[0].data, value_response);
    assert_eq!(
        *client_value_log.borrow(),
        vec![(ElementNumber(3), DDI(0x1234))]
    );

    assert_eq!(
        TaskControllerServer::build_set_value(5, 0xCAFE, 0x1234_5678).unwrap(),
        set_value
    );
    let client_command_log: Rc<RefCell<Vec<(ElementNumber, DDI, i32)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let client_command_log_cb = client_command_log.clone();
    client.on_value_command(move |element, ddi, value| {
        client_command_log_cb
            .borrow_mut()
            .push((element, ddi, value));
        Ok(())
    });
    assert!(
        client
            .handle_tc_message(&Message::new(PGN_TC_TO_ECU, set_value.to_vec(), 0x33))
            .is_empty()
    );
    assert_eq!(
        *client_command_log.borrow(),
        vec![(ElementNumber(5), DDI(0xCAFE), 0x1234_5678)]
    );

    assert_eq!(
        TaskControllerServer::build_set_value_and_acknowledge(5, 0xCAFE, 42).unwrap(),
        set_value_ack
    );
    let ack = client.handle_tc_message(&Message::new(PGN_TC_TO_ECU, set_value_ack.to_vec(), 0x33));
    assert_eq!(ack.len(), 1);
    assert_eq!(ack[0].data, ack_no_error);

    client.on_value_command(|_, _, _| Err(Error::invalid_state("actuator busy")));
    let ack = client.handle_tc_message(&Message::new(PGN_TC_TO_ECU, set_value_ack.to_vec(), 0x33));
    assert_eq!(ack.len(), 1);
    assert_eq!(ack[0].data, ack_no_resources);

    server.on_value_received(|_, _, _, _| {
        Ok(ProcessDataAcknowledgeErrorCodes::ValueIsOutsideValidRange)
    });
    let ack =
        server.handle_client_message(&Message::new(PGN_ECU_TO_TC, set_value_ack.to_vec(), 0x42));
    assert_eq!(ack.len(), 1);
    assert_eq!(ack[0].data, ack_value_outside_range);

    server.on_value_request(|_, _, _| Err(Error::invalid_state("sensor offline")));
    assert!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, request_value.to_vec(), 0x42,))
            .is_empty()
    );

    let client_bad_request_log: Rc<RefCell<Vec<(ElementNumber, DDI)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let client_bad_request_log_cb = client_bad_request_log.clone();
    let mut strict_client = TaskControllerClient::default();
    strict_client.on_value_request(move |element, ddi| {
        client_bad_request_log_cb.borrow_mut().push((element, ddi));
        Ok(1)
    });

    let server_bad_request_log: Rc<RefCell<Vec<(ElementNumber, DDI, u8)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let server_bad_request_log_cb = server_bad_request_log.clone();
    let mut strict_server = TaskControllerServer::new(valid_tc_server_config());
    strict_server.start().unwrap();
    strict_server.on_value_request(move |element, ddi, source| {
        server_bad_request_log_cb
            .borrow_mut()
            .push((element, ddi, source));
        Ok(1)
    });

    assert!(
        strict_client
            .handle_tc_message(&Message::new(PGN_ECU_TO_TC, request_value.to_vec(), 0x33))
            .is_empty(),
        "TC client must ignore process-data payloads delivered under the wrong PGN"
    );
    assert!(
        strict_server
            .handle_client_message(&Message::new(PGN_TC_TO_ECU, request_value.to_vec(), 0x42))
            .is_empty(),
        "TC server must ignore process-data payloads delivered under the wrong PGN"
    );
    for invalid_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert!(
            strict_client
                .handle_tc_message(&Message::new(
                    PGN_TC_TO_ECU,
                    request_value.to_vec(),
                    invalid_source,
                ))
                .is_empty(),
            "TC client must ignore process-data from invalid source 0x{invalid_source:02X}"
        );
        assert!(
            strict_server
                .handle_client_message(&Message::new(
                    PGN_ECU_TO_TC,
                    request_value.to_vec(),
                    invalid_source,
                ))
                .is_empty(),
            "TC server must ignore process-data from invalid source 0x{invalid_source:02X}"
        );
    }
    assert!(client_bad_request_log.borrow().is_empty());
    assert!(server_bad_request_log.borrow().is_empty());

    for name in [
        "malformed_request_value_short",
        "malformed_request_value_bad_padding",
        "malformed_request_value_overlong",
    ] {
        let malformed = parse_named_hex_bytes(ISOBUS_TC_PROCESS_DATA_HEX, name);
        assert!(
            strict_client
                .handle_tc_message(&Message::new(PGN_TC_TO_ECU, malformed.clone(), 0x33))
                .is_empty(),
            "client accepted {name}"
        );
        assert!(
            strict_server
                .handle_client_message(&Message::new(PGN_ECU_TO_TC, malformed, 0x42))
                .is_empty(),
            "server accepted {name}"
        );
    }
    assert!(client_bad_request_log.borrow().is_empty());
    assert!(server_bad_request_log.borrow().is_empty());

    let reserved_command = parse_named_hex_bytes(
        ISOBUS_TC_PROCESS_DATA_HEX,
        "malformed_process_data_reserved_command",
    );
    let mut reserved_server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_number(1)
            .with_version(4)
            .with_booms(1)
            .with_sections(8),
    );
    reserved_server.start().unwrap();
    assert!(
        strict_client
            .handle_tc_message(&Message::new(PGN_TC_TO_ECU, reserved_command.clone(), 0x33,))
            .is_empty()
    );
    assert!(
        reserved_server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, reserved_command, 0x42))
            .is_empty(),
        "reserved process-data command nibbles must not be treated as TechnicalCapabilities"
    );
    assert!(reserved_server.clients().is_empty());

    let client_bad_command_log: Rc<RefCell<Vec<(ElementNumber, DDI, i32)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let client_bad_command_log_cb = client_bad_command_log.clone();
    strict_client.on_value_command(move |element, ddi, value| {
        client_bad_command_log_cb
            .borrow_mut()
            .push((element, ddi, value));
        Ok(())
    });
    type TcServerCommandLog = Rc<RefCell<Vec<(ElementNumber, DDI, i32, u8)>>>;
    let server_bad_command_log: TcServerCommandLog = Rc::new(RefCell::new(Vec::new()));
    let server_bad_command_log_cb = server_bad_command_log.clone();
    strict_server.on_value_received(move |element, ddi, value, source| {
        server_bad_command_log_cb
            .borrow_mut()
            .push((element, ddi, value, source));
        Ok(ProcessDataAcknowledgeErrorCodes::NoError)
    });

    let malformed_ack = parse_named_hex_bytes(
        ISOBUS_TC_PROCESS_DATA_HEX,
        "malformed_set_value_ack_overlong",
    );
    assert!(
        strict_client
            .handle_tc_message(&Message::new(PGN_TC_TO_ECU, malformed_ack.clone(), 0x33,))
            .is_empty()
    );
    assert!(
        strict_server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, malformed_ack, 0x42))
            .is_empty()
    );
    assert!(client_bad_command_log.borrow().is_empty());
    assert!(server_bad_command_log.borrow().is_empty());
}

#[test]
fn fixture_isobus_tc_geo_prescription_edges_are_stable() {
    let primary = PrescriptionZone {
        boundary: vec![
            Wgs::new(0.0, 0.0, 0.0),
            Wgs::new(0.0, 2.0, 0.0),
            Wgs::new(2.0, 2.0, 0.0),
            Wgs::new(2.0, 0.0, 0.0),
        ],
        holes: vec![vec![
            Wgs::new(0.8, 0.8, 0.0),
            Wgs::new(0.8, 1.2, 0.0),
            Wgs::new(1.2, 1.2, 0.0),
            Wgs::new(1.2, 0.8, 0.0),
        ]],
        application_rate: 100,
    };
    let secondary = PrescriptionZone {
        boundary: vec![
            Wgs::new(1.0, 1.0, 0.0),
            Wgs::new(1.0, 3.0, 0.0),
            Wgs::new(3.0, 3.0, 0.0),
            Wgs::new(3.0, 1.0, 0.0),
        ],
        holes: Vec::new(),
        application_rate: 200,
    };
    assert!(point_in_prescription_zone(
        Wgs::new(0.5, 0.5, 0.0),
        &primary
    ));
    assert!(!point_in_prescription_zone(
        Wgs::new(0.9, 0.9, 0.0),
        &primary
    ));

    let mut tc = TCGEOInterface::new();
    assert_eq!(
        parse_named_text_value(ISOBUS_TC_GEO_PRESCRIPTION, "no_gnss_fix_position_payload"),
        "invalid_state"
    );
    assert!(tc.position_process_data_payloads().is_err());
    let mut gnss = Vec::with_capacity(8);
    gnss.extend_from_slice(&5_000_000_i32.to_le_bytes());
    gnss.extend_from_slice(&5_000_000_i32.to_le_bytes());
    tc.handle_gnss_position(&Message::new(PGN_GNSS_COG_SOG_RAPID, gnss.clone(), 0x23));
    assert!(
        tc.current_position().is_none(),
        "TC-GEO must ignore GNSS rapid-position bytes delivered under the wrong PGN"
    );
    for invalid_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        tc.handle_gnss_position(&Message::new(
            PGN_GNSS_POSITION_RAPID,
            gnss.clone(),
            invalid_source,
        ));
        assert!(
            tc.current_position().is_none(),
            "TC-GEO must ignore GNSS rapid-position bytes from invalid source 0x{invalid_source:02X}"
        );
    }
    tc.set_position(GeoPoint {
        position: Wgs::new(0.5, 0.5, 0.0),
        timestamp_us: 0,
    });
    assert_eq!(
        tc.position_process_data_payloads().unwrap(),
        [
            parse_named_hex_frame(ISOBUS_TC_GEO_PRESCRIPTION, "position_lat_0_5_payload"),
            parse_named_hex_frame(ISOBUS_TC_GEO_PRESCRIPTION, "position_lon_0_5_payload"),
        ]
    );

    tc.add_prescription_map(PrescriptionMap {
        structure_label: "edge-cases".to_string(),
        zones: vec![primary, secondary],
    });

    let rate_ddi = DDI(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE);
    let engineering_rate = tc
        .get_rate_at_position_engineering(Wgs::new(0.5, 0.5, 0.0), rate_ddi)
        .unwrap()
        .unwrap();
    let expected_engineering: f64 = parse_named_text_value(
        ISOBUS_TC_GEO_PRESCRIPTION,
        "inside_primary_volume_per_area_eng",
    )
    .parse()
    .unwrap();
    assert!((engineering_rate - expected_engineering).abs() < 1e-9);
    assert_eq!(
        tc.rate_process_data_payload_at_position(Wgs::new(0.5, 0.5, 0.0), rate_ddi)
            .unwrap()
            .unwrap(),
        parse_named_hex_frame(
            ISOBUS_TC_GEO_PRESCRIPTION,
            "inside_primary_rate_payload_ddi1"
        )
    );
    assert_eq!(
        prescription_rate_to_engineering(rate_ddi, 100).unwrap(),
        expected_engineering
    );
    assert_eq!(
        prescription_rate_process_data_payload(rate_ddi, 100).unwrap(),
        parse_named_hex_frame(
            ISOBUS_TC_GEO_PRESCRIPTION,
            "inside_primary_rate_payload_ddi1"
        )
    );

    assert_geo_rate(&tc, "inside_primary", Wgs::new(0.5, 0.5, 0.0));
    assert_geo_rate(&tc, "outer_boundary", Wgs::new(0.0, 1.0, 0.0));
    assert_geo_rate(&tc, "inside_overlap_first_match", Wgs::new(1.5, 1.5, 0.0));
    assert_geo_rate(&tc, "inside_hole", Wgs::new(0.9, 0.9, 0.0));
    assert_geo_rate(&tc, "hole_boundary", Wgs::new(0.8, 1.0, 0.0));
    assert_geo_rate(&tc, "inside_second_only", Wgs::new(2.5, 2.5, 0.0));
    assert_geo_rate(&tc, "outside", Wgs::new(5.0, 5.0, 0.0));
}

#[test]
fn fixture_isobus_tc_peer_control_assignment_lifecycle_is_stable() {
    let assignment = PeerControlAssignment::default()
        .from(0x0123, 0xCAFE)
        .to(0x0456, 0x0BAD)
        .with_source(0x44)
        .with_destination(0x55);
    let bytes = parse_named_hex_frame(
        ISOBUS_TC_PEER_CONTROL_HEX,
        "assignment_src_elem0123_ddiCAFE_dst_elem0456_ddi0BAD",
    );
    assert_eq!(assignment.try_encode().unwrap(), bytes);
    assert_eq!(
        bytes[0] & 0x0F,
        ProcessDataCommands::PeerControlAssignment.as_u8()
    );
    assert_eq!(
        PeerControlAssignment::decode(&bytes, 0x44, 0x55).unwrap(),
        assignment
    );
    let max_assignment = PeerControlAssignment::default()
        .from(0x0FFF, 0xFFFF)
        .to(0x0FFF, 0xFFFF)
        .with_source(0x01)
        .with_destination(0x02);
    assert_eq!(
        max_assignment.try_encode().unwrap(),
        parse_named_hex_frame(
            ISOBUS_TC_PEER_CONTROL_HEX,
            "assignment_src_elem0FFF_ddiFFFF_dst_elem0FFF_ddiFFFF",
        )
    );
    assert_eq!(
        PeerControlAssignment::decode(
            &parse_named_hex_frame(
                ISOBUS_TC_PEER_CONTROL_HEX,
                "assignment_src_elem0FFF_ddiFFFF_dst_elem0FFF_ddiFFFF",
            ),
            0x01,
            0x02,
        )
        .unwrap(),
        max_assignment
    );

    for malformed in [
        "malformed_assignment_short7",
        "malformed_assignment_overlong9",
        "malformed_assignment_wrong_command",
    ] {
        assert!(
            PeerControlAssignment::decode(
                &parse_named_hex_bytes(ISOBUS_TC_PEER_CONTROL_HEX, malformed),
                0x44,
                0x55,
            )
            .is_err(),
            "{malformed} must be rejected before peer-control registry mutation"
        );
    }
    for (source, destination) in [
        (NULL_ADDRESS, 0x55),
        (BROADCAST_ADDRESS, 0x55),
        (0x44, NULL_ADDRESS),
    ] {
        assert!(
            PeerControlAssignment::decode(&bytes, source, destination).is_err(),
            "peer-control assignment must reject invalid source/destination addresses"
        );
    }

    let mut peer = PeerControlInterface::new();
    let added: Rc<RefCell<Vec<PeerControlAssignment>>> = Rc::new(RefCell::new(Vec::new()));
    let removed: Rc<RefCell<Vec<PeerControlAssignment>>> = Rc::new(RefCell::new(Vec::new()));
    let changed: Rc<RefCell<Vec<PeerControlAssignment>>> = Rc::new(RefCell::new(Vec::new()));
    let added_cb = added.clone();
    let removed_cb = removed.clone();
    let changed_cb = changed.clone();
    peer.on_assignment_added
        .subscribe(move |&a| added_cb.borrow_mut().push(a));
    peer.on_assignment_removed
        .subscribe(move |&a| removed_cb.borrow_mut().push(a));
    peer.on_assignment_state_changed
        .subscribe(move |&a| changed_cb.borrow_mut().push(a));

    peer.add_assignment(assignment).unwrap();
    assert!(peer.add_assignment(assignment).is_err());
    let other_source = assignment.with_source(0x45);
    peer.add_assignment(other_source).unwrap();
    assert_eq!(peer.assignments().len(), 2);
    assert_eq!(*added.borrow(), vec![assignment, other_source]);
    assert!(peer.activate_assignment(0x0123, 0xCAFE, true).is_err());

    peer.activate_assignment_from(0x44, 0x0123, 0xCAFE, true)
        .unwrap();
    peer.activate_assignment_from(0x44, 0x0123, 0xCAFE, true)
        .unwrap();
    assert_eq!(changed.borrow().len(), 1);
    assert!(changed.borrow()[0].active);

    peer.remove_assignment_from(0x44, 0x0123, 0xCAFE).unwrap();
    let mut active_assignment = assignment;
    active_assignment.active = true;
    assert_eq!(*removed.borrow(), vec![active_assignment]);
    peer.clear_assignments();
    assert_eq!(*removed.borrow(), vec![active_assignment, other_source]);
    assert!(peer.assignments().is_empty());

    let mut invalid_peer = PeerControlInterface::new();
    assert!(
        invalid_peer
            .add_assignment(PeerControlAssignment::default())
            .is_err()
    );
}

#[test]
fn fixture_isobus_rear_hitch_raise_is_stable() {
    let cmd = HitchCommandMsg::decode(REAR_HITCH_RAISE).expect("hitch fixture must decode");
    assert_eq!(cmd.command, HitchCommand::Raise);
    assert_eq!(cmd.target_position, 0xFFFF);
    assert_eq!(cmd.rate, 0xFF);
    assert_eq!(cmd.encode(), *REAR_HITCH_RAISE);
}

#[test]
fn fixture_isobus_file_server_codecs_and_operations_are_stable() {
    let classic = FileServerProperties {
        version_number: 1,
        max_simultaneous_files: 16,
        supports_directories: true,
        supports_volume_management: true,
        supports_file_attributes: true,
        supports_move_file: true,
        supports_delete_file: true,
    };
    let classic_bytes = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "classic_properties_all_caps");
    assert_eq!(classic.encode(), classic_bytes);
    assert_eq!(FileServerProperties::decode(&classic_bytes), Some(classic));
    for malformed in [
        "classic_properties_short2",
        "classic_properties_overlong9",
        "classic_properties_bad_padding",
    ] {
        assert!(
            FileServerProperties::decode(&parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, malformed))
                .is_none(),
            "{malformed} must be rejected"
        );
    }

    let status = FileServerStatus {
        busy: true,
        number_of_open_files: 2,
    };
    let status_bytes = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "server_status_busy_two_open");
    assert_eq!(status.encode(), status_bytes);
    assert_eq!(FileServerStatus::decode(&status_bytes), Some(status));
    for malformed in [
        "server_status_short1",
        "server_status_overlong9",
        "server_status_bad_padding",
    ] {
        assert!(
            FileServerStatus::decode(&parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, malformed))
                .is_none(),
            "{malformed} must be rejected"
        );
    }

    let ccm = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "ccm_tan7");
    assert_eq!(encode_ccm(7), ccm);
    assert_eq!(
        CCMMessage::decode(&ccm),
        Some(CCMMessage {
            version: 0xFF,
            tan: 7
        })
    );
    for malformed in ["ccm_short1", "ccm_overlong9", "ccm_bad_padding"] {
        assert!(
            CCMMessage::decode(&parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, malformed)).is_none(),
            "{malformed} must be rejected"
        );
    }

    let props_v2 = FileServerPropertiesV2 {
        version_number: 2,
        max_open_files: 32,
        supports_volumes: false,
        supports_long_filenames: true,
        max_simultaneous_clients: 8,
    };
    let props_v2_bytes =
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "properties_v2_no_volumes_long_names");
    assert_eq!(props_v2.encode(), props_v2_bytes);
    assert_eq!(
        FileServerPropertiesV2::decode(&props_v2_bytes),
        Some(props_v2)
    );
    assert!(
        FileServerPropertiesV2::decode(&parse_named_hex_bytes(
            ISOBUS_FS_CODECS_HEX,
            "properties_v2_bad_padding"
        ))
        .is_none()
    );

    let volume = VolumeStatus {
        name: "DISK".to_string(),
        state: VolumeStateV2::Mounted,
        total_bytes: 1_000_000,
        free_bytes: 500_000,
        removable: true,
    };
    let volume_bytes = parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "volume_status_disk");
    assert_eq!(volume.encode().unwrap(), volume_bytes);
    assert_eq!(VolumeStatus::decode(&volume_bytes), Some(volume));
    for malformed in [
        "volume_status_bad_reserved_state",
        "volume_status_truncated_name",
        "volume_status_overlong",
    ] {
        assert!(
            VolumeStatus::decode(&parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, malformed)).is_none(),
            "{malformed} must be rejected"
        );
    }

    let nack = FSNack {
        command_code: FSFunction::MoveFile.as_u8(),
        error_code: 0x01,
    };
    let nack_bytes = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "nack_move_not_supported");
    assert_eq!(nack.encode(), nack_bytes);
    assert_eq!(FSError::from_u8(20), FSError::NotSupported);
    assert_eq!(
        FileAttributes::ReadOnly | FileAttributes::Archive,
        0x21,
        "attribute bit layout stays stable"
    );

    let mut server = IsoFileServer::new(IsoFileServerConfig::default());
    server.add_file("log.txt", b"abc".to_vec(), 0).unwrap();
    let mut client = FileClient::new(FileClientConfig::default());
    let connect = client.connect_to_server(0x10).unwrap();
    let mut props_response = vec![
        FSFunction::GetFileServerProperties.as_u8(),
        connect.data[1],
        FSError::Success.as_u8(),
    ];
    props_response.extend_from_slice(&FileServerProperties::default().encode());
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        props_response,
        0x10,
    ));
    assert!(client.is_connected());
    let expected_fs_frame_with_tan = |name: &str, tan: u8| {
        let mut expected = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, name);
        expected[1] = tan;
        expected
    };
    let expected_fs_bytes_with_tan = |name: &str, tan: u8| {
        let mut expected = parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, name);
        expected[1] = tan;
        expected
    };

    let open_req = client
        .open_file("log.txt", OpenFlags::ReadWrite.bit())
        .unwrap();
    assert_eq!(open_req.pgn, PGN_FILE_CLIENT_TO_SERVER);
    let expected_open = expected_fs_bytes_with_tan("open_request_log_txt_rw", open_req.data[1]);
    assert_eq!(open_req.data.as_slice(), expected_open.as_slice());
    let open_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        open_req.data.clone(),
        0x20,
    ));
    assert_eq!(open_resp.len(), 1);
    assert_eq!(open_resp[0].pgn, PGN_FILE_SERVER_TO_CLIENT);
    let expected_open_response =
        expected_fs_frame_with_tan("open_response_handle1", open_req.data[1]);
    assert_eq!(
        open_resp[0].data.as_slice(),
        expected_open_response.as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        open_resp[0].data.clone(),
        0x10,
    ));

    let read_req = client.read_file(1, 3).expect("handle 1 opened");
    let expected_read_request =
        expected_fs_frame_with_tan("read_request_handle1_count3", read_req.data[1]);
    assert_eq!(read_req.data.as_slice(), expected_read_request.as_slice());
    let read_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        read_req.data.clone(),
        0x20,
    ));
    let expected_read_response = expected_fs_bytes_with_tan("read_response_abc", read_req.data[1]);
    assert_eq!(
        read_resp[0].data.as_slice(),
        expected_read_response.as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        read_resp[0].data.clone(),
        0x10,
    ));

    let seek_req = client.seek_file(1, 0).expect("handle 1 opened");
    let expected_seek_request =
        expected_fs_frame_with_tan("seek_request_handle1_zero", seek_req.data[1]);
    assert_eq!(seek_req.data.as_slice(), expected_seek_request.as_slice());
    let seek_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        seek_req.data.clone(),
        0x20,
    ));
    let expected_seek_response = expected_fs_frame_with_tan("seek_response_ok", seek_req.data[1]);
    assert_eq!(
        seek_resp[0].data.as_slice(),
        expected_seek_response.as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        seek_resp[0].data.clone(),
        0x10,
    ));

    let write_req = client.write_file(1, b"XYZ").expect("handle 1 opened");
    let expected_write_request =
        expected_fs_frame_with_tan("write_request_handle1_xyz", write_req.data[1]);
    assert_eq!(write_req.data.as_slice(), expected_write_request.as_slice());
    let write_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        write_req.data.clone(),
        0x20,
    ));
    let expected_write_response =
        expected_fs_frame_with_tan("write_response_xyz", write_req.data[1]);
    assert_eq!(
        write_resp[0].data.as_slice(),
        expected_write_response.as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        write_resp[0].data.clone(),
        0x10,
    ));

    let close_req = client.close_file(1).expect("handle 1 opened");
    let expected_close_request =
        expected_fs_frame_with_tan("close_request_handle1", close_req.data[1]);
    assert_eq!(close_req.data.as_slice(), expected_close_request.as_slice());
    let close_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        close_req.data.clone(),
        0x20,
    ));
    let expected_close_response =
        expected_fs_frame_with_tan("close_response_ok", close_req.data[1]);
    assert_eq!(
        close_resp[0].data.as_slice(),
        expected_close_response.as_slice()
    );

    let mut strict_server = IsoFileServer::new(IsoFileServerConfig::default());
    let malformed_open =
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "malformed_open_request_bad_tail");
    let malformed_open_resp = strict_server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        malformed_open,
        0x20,
    ));
    assert_eq!(
        malformed_open_resp[0].data[2],
        FSError::MalformedRequest.as_u8()
    );
    assert!(strict_server.open_files().is_empty());

    let padded_open =
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "open_request_a_write_create_padded");
    let padded_open_resp = strict_server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        padded_open,
        0x20,
    ));
    assert_eq!(padded_open_resp[0].data[2], FSError::Success.as_u8());
    let handle = padded_open_resp[0].data[3];

    let malformed_write =
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "malformed_write_request_bad_tail");
    let malformed_write_resp = strict_server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        malformed_write,
        0x20,
    ));
    assert_eq!(
        malformed_write_resp[0].data[2],
        FSError::MalformedRequest.as_u8()
    );

    let mut malformed_read =
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "malformed_read_request_bad_tail");
    malformed_read[2] = handle;
    let malformed_read_resp = strict_server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        malformed_read,
        0x20,
    ));
    assert_eq!(
        malformed_read_resp[0].data[2],
        FSError::MalformedRequest.as_u8()
    );

    let malformed_change_dir = parse_named_hex_bytes(
        ISOBUS_FS_CODECS_HEX,
        "malformed_change_dir_request_bad_tail",
    );
    let malformed_change_dir_resp = strict_server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        malformed_change_dir,
        0x20,
    ));
    assert_eq!(
        malformed_change_dir_resp[0].data[2],
        FSError::MalformedRequest.as_u8()
    );

    let mut ccm_server = IsoFileServer::new(IsoFileServerConfig::default());
    let malformed_ccm = parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "malformed_ccm_bad_tail");
    let malformed_ccm_resp = ccm_server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        malformed_ccm,
        0x20,
    ));
    assert!(malformed_ccm_resp.is_empty());
    assert!(ccm_server.clients().is_empty());

    let mut strict_client = FileClient::new(FileClientConfig::default());
    let connect = strict_client.connect_to_server(0x10).unwrap();
    let connect_tan = connect.data[1];
    let mut props_response = vec![
        FSFunction::GetFileServerProperties.as_u8(),
        connect_tan,
        FSError::Success.as_u8(),
    ];
    props_response.extend_from_slice(&FileServerProperties::default().encode());
    strict_client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        props_response,
        0x10,
    ));
    assert!(strict_client.is_connected());

    let open = strict_client.open_file("a", OpenFlags::Read.bit()).unwrap();
    let open_tan = open.data[1];
    let mut open_response = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "open_response_handle1");
    open_response[1] = open_tan;
    strict_client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        open_response.to_vec(),
        0x10,
    ));

    let read = strict_client.read_file(1, 4).unwrap();
    let read_tan = read.data[1];
    type FsReadEvents = Rc<RefCell<Vec<Result<Vec<u8>, FSError>>>>;
    let read_events: FsReadEvents = Rc::new(RefCell::new(Vec::new()));
    let read_events_cb = read_events.clone();
    strict_client
        .on_read_response
        .subscribe(move |(_, r)| read_events_cb.borrow_mut().push(r.clone()));
    let mut malformed_read_response =
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "malformed_read_response_short_count");
    malformed_read_response[1] = read_tan;
    strict_client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        malformed_read_response,
        0x10,
    ));
    assert_eq!(*read_events.borrow(), vec![Err(FSError::MalformedRequest)]);

    let dir = strict_client.get_current_directory().unwrap();
    let dir_tan = dir.data[1];
    type FsDirEvents = Rc<RefCell<Vec<Result<String, FSError>>>>;
    let dir_events: FsDirEvents = Rc::new(RefCell::new(Vec::new()));
    let dir_events_cb = dir_events.clone();
    strict_client
        .on_current_directory_response
        .subscribe(move |(_, r)| dir_events_cb.borrow_mut().push(r.clone()));
    let mut malformed_dir_response = parse_named_hex_bytes(
        ISOBUS_FS_CODECS_HEX,
        "malformed_get_directory_response_short_count",
    );
    malformed_dir_response[1] = dir_tan;
    strict_client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        malformed_dir_response,
        0x10,
    ));
    assert_eq!(*dir_events.borrow(), vec![Err(FSError::MalformedRequest)]);

    let volume_events: Rc<RefCell<Vec<machbus::isobus::fs::VolumeState>>> =
        Rc::new(RefCell::new(Vec::new()));
    let volume_events_cb = volume_events.clone();
    strict_client
        .on_volume_status
        .subscribe(move |&v| volume_events_cb.borrow_mut().push(v));
    strict_client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "malformed_volume_status_bad_tail"),
        0x10,
    ));
    assert!(volume_events.borrow().is_empty());
    strict_client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        parse_named_hex_bytes(
            ISOBUS_FS_CODECS_HEX,
            "malformed_volume_status_reserved_state",
        ),
        0x10,
    ));
    assert!(volume_events.borrow().is_empty());
}

#[test]
fn fixture_isobus_file_server_error_responses_are_stable() {
    let mut server = IsoFileServer::new(IsoFileServerConfig::default());
    server
        .add_file(
            "read_only.txt",
            b"ro".to_vec(),
            FileAttributes::ReadOnly.bit(),
        )
        .unwrap();
    server.add_file("empty.txt", Vec::new(), 0).unwrap();

    let invalid_path = b"..\\secret.txt";
    let mut invalid_open = vec![
        FSFunction::OpenFile.as_u8(),
        0x01,
        invalid_path.len() as u8,
        OpenFlags::Write | OpenFlags::Create,
    ];
    invalid_open.extend_from_slice(invalid_path);
    let invalid_open_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, invalid_open, 0x20));
    assert_eq!(
        invalid_open_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "open_error_invalid_path").as_slice()
    );

    let missing = b"missing.txt";
    let mut missing_open = vec![
        FSFunction::OpenFile.as_u8(),
        0x02,
        missing.len() as u8,
        OpenFlags::Read.bit(),
    ];
    missing_open.extend_from_slice(missing);
    let missing_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, missing_open, 0x20));
    assert_eq!(
        missing_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "open_error_not_found").as_slice()
    );

    let read_only = b"read_only.txt";
    let mut read_only_open = vec![
        FSFunction::OpenFile.as_u8(),
        0x21,
        read_only.len() as u8,
        OpenFlags::Read.bit(),
    ];
    read_only_open.extend_from_slice(read_only);
    let read_only_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        read_only_open,
        0x20,
    ));
    let read_only_handle = read_only_resp[0].data[3];
    let write_denied = vec![
        FSFunction::WriteFile.as_u8(),
        0x03,
        read_only_handle,
        1,
        0,
        b'X',
    ];
    let write_denied_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, write_denied, 0x20));
    assert_eq!(
        write_denied_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "write_error_invalid_access").as_slice()
    );

    let close_read_only = vec![FSFunction::CloseFile.as_u8(), 0x24, read_only_handle];
    let close_read_only_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        close_read_only,
        0x20,
    ));
    assert_eq!(
        close_read_only_resp[0].data[2],
        FSError::Success.as_u8(),
        "the read-only access-handle fixture must be closed before testing read-only media attrs"
    );

    let mut read_only_rw_open = vec![
        FSFunction::OpenFile.as_u8(),
        0x23,
        read_only.len() as u8,
        OpenFlags::ReadWrite.bit(),
    ];
    read_only_rw_open.extend_from_slice(read_only);
    let read_only_rw_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        read_only_rw_open,
        0x20,
    ));
    let read_only_rw_handle = read_only_rw_resp[0].data[3];
    let read_only_write = vec![
        FSFunction::WriteFile.as_u8(),
        0x09,
        read_only_rw_handle,
        1,
        0,
        b'!',
    ];
    let read_only_write_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        read_only_write,
        0x20,
    ));
    assert_eq!(
        read_only_write_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "write_error_read_only_access").as_slice()
    );

    let close_invalid = vec![FSFunction::CloseFile.as_u8(), 0x04, 0x7E];
    let close_invalid_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        close_invalid,
        0x20,
    ));
    assert_eq!(
        close_invalid_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "close_error_invalid_handle").as_slice()
    );

    let empty = b"empty.txt";
    let mut empty_open = vec![
        FSFunction::OpenFile.as_u8(),
        0x22,
        empty.len() as u8,
        OpenFlags::Read.bit(),
    ];
    empty_open.extend_from_slice(empty);
    let empty_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, empty_open, 0x20));
    let empty_handle = empty_resp[0].data[3];
    let eof_read = vec![
        FSFunction::ReadFile.as_u8(),
        0x05,
        empty_handle,
        1,
        0,
        0xFF,
        0xFF,
        0xFF,
    ];
    let eof_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, eof_read, 0x20));
    assert_eq!(
        eof_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "read_error_eof").as_slice()
    );

    let malformed_move = vec![FSFunction::MoveFile.as_u8(), 0x06];
    let malformed_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        malformed_move,
        0x20,
    ));
    assert_eq!(
        malformed_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "move_error_malformed_request").as_slice()
    );

    let invalid_dir = b"safe\\..";
    let mut change_dir = vec![
        FSFunction::ChangeDirectory.as_u8(),
        0x07,
        invalid_dir.len() as u8,
    ];
    change_dir.extend_from_slice(invalid_dir);
    let change_dir_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, change_dir, 0x20));
    assert_eq!(
        change_dir_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "change_dir_error_invalid_source").as_slice()
    );

    let mut removed_server = IsoFileServer::new(IsoFileServerConfig::default());
    let _ = removed_server.set_volume_removed();
    let removed_path = b"media.txt";
    let mut removed_open = vec![
        FSFunction::OpenFile.as_u8(),
        0x08,
        removed_path.len() as u8,
        OpenFlags::Read.bit(),
    ];
    removed_open.extend_from_slice(removed_path);
    let removed_resp = removed_server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        removed_open,
        0x20,
    ));
    assert_eq!(
        removed_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "open_error_media_not_present").as_slice()
    );
}

#[test]
fn fixture_isobus_file_server_volume_status_transitions_are_stable() {
    let mut server = IsoFileServer::new(IsoFileServerConfig::default().with_ccm_timeout(60_000));
    server.add_file("media.txt", b"abc".to_vec(), 0).unwrap();

    let path = b"media.txt";
    let mut open = vec![
        FSFunction::OpenFile.as_u8(),
        0x01,
        path.len() as u8,
        OpenFlags::ReadWrite.bit(),
    ];
    open.extend_from_slice(path);
    let open_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, open, 0x20));
    assert_eq!(open_resp[0].data[2], FSError::Success.as_u8());

    let preparing = server.prepare_volume_for_removal();
    assert_eq!(
        preparing[0].data.as_slice(),
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "volume_status_preparing_one_open").as_slice()
    );

    let removed = server.update(10_000);
    assert_eq!(
        removed[0].data.as_slice(),
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "volume_status_removed_zero_open").as_slice()
    );
}

#[test]
fn fixture_isobus_file_server_directory_workflow_is_stable() {
    let mut server = IsoFileServer::new(IsoFileServerConfig::default().with_ccm_timeout(60_000));
    server.add_directory("logs").unwrap();
    server
        .add_file("\\logs\\day1.txt", b"seed".to_vec(), 0)
        .unwrap();
    let mut client = FileClient::new(FileClientConfig::default());
    let connect = client.connect_to_server(0x10).unwrap();
    let mut props_response = vec![
        FSFunction::GetFileServerProperties.as_u8(),
        connect.data[1],
        FSError::Success.as_u8(),
    ];
    props_response.extend_from_slice(&FileServerProperties::default().encode());
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        props_response,
        0x10,
    ));
    assert!(client.is_connected());

    let change_logs_out = client.change_directory("logs").unwrap();
    let change_logs = parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "change_dir_logs_request");
    assert_eq!(change_logs_out.data, change_logs);
    let change_logs_resp =
        server.handle_client_message(&Message::new(PGN_FILE_CLIENT_TO_SERVER, change_logs, 0x20));
    assert_eq!(
        change_logs_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "change_dir_logs_response").as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        change_logs_resp[0].data.clone(),
        0x10,
    ));

    let cwd_logs_out = client.get_current_directory().unwrap();
    let cwd_logs = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "get_cwd_logs_request");
    assert_eq!(cwd_logs_out.data.as_slice(), cwd_logs.as_slice());
    let cwd_logs_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        cwd_logs.to_vec(),
        0x20,
    ));
    assert_eq!(
        cwd_logs_resp[0].data.as_slice(),
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "get_cwd_logs_response").as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        cwd_logs_resp[0].data.clone(),
        0x10,
    ));

    let open_relative_out = client.open_file("day1.txt", OpenFlags::Read.bit()).unwrap();
    let open_relative = parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "open_relative_day1_request");
    assert_eq!(open_relative_out.data, open_relative);
    let open_relative_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        open_relative,
        0x20,
    ));
    assert_eq!(
        open_relative_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "open_relative_day1_response").as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        open_relative_resp[0].data.clone(),
        0x10,
    ));

    let read_relative_out = client.read_file(1, 4).unwrap();
    let read_relative = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "read_relative_day1_request");
    assert_eq!(read_relative_out.data.as_slice(), read_relative.as_slice());
    let read_relative_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        read_relative.to_vec(),
        0x20,
    ));
    assert_eq!(
        read_relative_resp[0].data.as_slice(),
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "read_relative_day1_response").as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        read_relative_resp[0].data.clone(),
        0x10,
    ));

    let change_parent_out = client.change_directory("..").unwrap();
    let change_parent = parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "change_dir_parent_request");
    assert_eq!(change_parent_out.data, change_parent);
    let change_parent_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        change_parent,
        0x20,
    ));
    assert_eq!(
        change_parent_resp[0].data.as_slice(),
        parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "change_dir_parent_response").as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        change_parent_resp[0].data.clone(),
        0x10,
    ));

    let cwd_root_out = client.get_current_directory().unwrap();
    let cwd_root = parse_named_hex_frame(ISOBUS_FS_CODECS_HEX, "get_cwd_root_request");
    assert_eq!(cwd_root_out.data.as_slice(), cwd_root.as_slice());
    let cwd_root_resp = server.handle_client_message(&Message::new(
        PGN_FILE_CLIENT_TO_SERVER,
        cwd_root.to_vec(),
        0x20,
    ));
    assert_eq!(
        cwd_root_resp[0].data.as_slice(),
        parse_named_hex_bytes(ISOBUS_FS_CODECS_HEX, "get_cwd_root_response").as_slice()
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        cwd_root_resp[0].data.clone(),
        0x10,
    ));
}

