use machbus::geo::Wgs;
use machbus::isobus::tc::peer_control::MAX_PEER_CONTROL_SOURCE_ELEMENT;
use machbus::isobus::tc::{
    DDI, DDOP, DeviceElement, DeviceElementType, DeviceObject, ElementNumber, GeoPoint,
    MeasurementTriggerRuntime, ObjectPoolActivationError, ObjectPoolDeletionErrors,
    ObjectPoolErrorCodes, PeerControlAssignment, PeerControlInterface, PrescriptionMap,
    PrescriptionZone, ProcessDataAcknowledgeErrorCodes, ProcessDataCommands, ServerOptions,
    TC_SERVER_OPTIONS_KNOWN_MASK, TC_STATUS_INTERVAL_MS, TCClientConfig, TCClientTaskStatus,
    TCGEOInterface, TCServerConfig, TCState, TaskControllerClient, TaskControllerServer,
    TriggerMethod, geo_ddi, prescription_rate_from_engineering,
    prescription_rate_process_data_payload, prescription_rate_to_engineering, tc_cmd,
    tc_options_byte_is_valid,
};
use machbus::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use machbus::net::pgn_defs::{
    PGN_ECU_TO_TC, PGN_GNSS_POSITION, PGN_TC_TO_ECU, PGN_WORKING_SET_MASTER,
};
use machbus::net::{ErrorCode, Message, Priority};
use std::cell::RefCell;
use std::rc::Rc;

fn minimal_client_ddop() -> DDOP {
    DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("implement"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device),
        )
}

fn fixed_tc_response(command: u8, status: u8) -> Vec<u8> {
    let mut data = [0xFFu8; 8];
    data[0] = command;
    data[1] = status;
    data.to_vec()
}

fn valid_version_response() -> Vec<u8> {
    vec![tc_cmd::VERSION_RESPONSE, 4, 0xFF, 0, 0x00, 1, 1, 0]
}

#[test]
fn tc_process_data_command_nibbles_round_trip_known_values() {
    for command in [
        ProcessDataCommands::DeviceDescriptor,
        ProcessDataCommands::RequestValue,
        ProcessDataCommands::Value,
        ProcessDataCommands::PeerControlAssignment,
        ProcessDataCommands::Acknowledge,
        ProcessDataCommands::Status,
        ProcessDataCommands::ClientTask,
    ] {
        assert_eq!(ProcessDataCommands::from_u8(command.as_u8()), command);
    }
}

#[test]
fn tc_public_error_status_decoders_reject_noncanonical_bytes() {
    for (raw, error) in [
        (0x00, ObjectPoolActivationError::NoErrors),
        (0x01, ObjectPoolActivationError::ThereAreErrorsInTheDDOP),
        (
            0x02,
            ObjectPoolActivationError::TaskControllerRanOutOfMemoryDuringActivation,
        ),
        (0x04, ObjectPoolActivationError::AnyOtherError),
        (
            0x08,
            ObjectPoolActivationError::DifferentDDOPExistsWithSameStructureLabel,
        ),
    ] {
        assert_eq!(ObjectPoolActivationError::try_from_u8(raw), Some(error));
        assert_eq!(ObjectPoolActivationError::from_u8(raw), error);
    }

    for (raw, error) in [
        (
            0x00,
            ObjectPoolDeletionErrors::ObjectPoolIsReferencedByTaskData,
        ),
        (
            0x01,
            ObjectPoolDeletionErrors::ServerCannotCheckForObjectPoolReferences,
        ),
        (0xFF, ObjectPoolDeletionErrors::ErrorDetailsNotAvailable),
    ] {
        assert_eq!(ObjectPoolDeletionErrors::try_from_u8(raw), Some(error));
        assert_eq!(ObjectPoolDeletionErrors::from_u8(raw), error);
    }

    for (raw, error) in [
        (0x00, ObjectPoolErrorCodes::NoErrors),
        (0x01, ObjectPoolErrorCodes::MethodOrAttributeNotSupported),
        (0x02, ObjectPoolErrorCodes::UnknownObjectReference),
        (0x04, ObjectPoolErrorCodes::AnyOtherError),
        (0x08, ObjectPoolErrorCodes::DDOPWasDeletedFromVolatileMemory),
    ] {
        assert_eq!(ObjectPoolErrorCodes::try_from_u8(raw), Some(error));
        assert_eq!(ObjectPoolErrorCodes::from_u8(raw), error);
    }

    for (raw, error) in [
        (0x00, ProcessDataAcknowledgeErrorCodes::NoError),
        (
            0x01,
            ProcessDataAcknowledgeErrorCodes::ElementNotSupportedByThisDevice,
        ),
        (
            0x02,
            ProcessDataAcknowledgeErrorCodes::ValueIsOutsideValidRange,
        ),
        (
            0x03,
            ProcessDataAcknowledgeErrorCodes::NoProcessingResourcesAvailable,
        ),
        (
            0x04,
            ProcessDataAcknowledgeErrorCodes::DDEXValueNotSupported,
        ),
    ] {
        assert_eq!(
            ProcessDataAcknowledgeErrorCodes::try_from_u8(raw),
            Some(error)
        );
        assert_eq!(ProcessDataAcknowledgeErrorCodes::from_u8(raw), error);
    }

    for reserved in [0x03, 0x05, 0x06, 0x07, 0x09, 0x10, 0xFE, 0xFF] {
        assert_eq!(ObjectPoolActivationError::try_from_u8(reserved), None);
        assert_eq!(ObjectPoolErrorCodes::try_from_u8(reserved), None);
    }

    for reserved in [0x02, 0x03, 0x04, 0x10, 0xFE] {
        assert_eq!(ObjectPoolDeletionErrors::try_from_u8(reserved), None);
    }

    for reserved in [0x05, 0x06, 0x07, 0x08, 0x10, 0xFE, 0xFF] {
        assert_eq!(
            ProcessDataAcknowledgeErrorCodes::try_from_u8(reserved),
            None
        );
    }
}

#[test]
fn tc_version_capability_option_bytes_reject_reserved_bits_before_state_update() {
    let defined_options = ServerOptions::SupportsDocumentation as u8
        | ServerOptions::SupportsTCGEOWithoutPositionBasedControl as u8
        | ServerOptions::SupportsTCGEOWithPositionBasedControl as u8
        | ServerOptions::SupportsPeerControlAssignment as u8
        | ServerOptions::SupportsImplementSectionControl as u8;
    assert_eq!(TC_SERVER_OPTIONS_KNOWN_MASK, defined_options);
    assert!(tc_options_byte_is_valid(0x00));
    assert!(tc_options_byte_is_valid(defined_options));
    for reserved in [0x20, 0x40, 0x80, 0xE0] {
        assert!(!tc_options_byte_is_valid(reserved));
        let config = TCServerConfig::default()
            .with_booms(1)
            .with_sections(1)
            .with_options(reserved);
        let err = config
            .validate()
            .expect_err("server config must reject reserved TC option bits");
        assert_eq!(err.code, ErrorCode::InvalidData);

        let mut server = TaskControllerServer::new(config);
        let err = server
            .start()
            .expect_err("TC server start must reject reserved option bits");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert_eq!(
            server.state(),
            machbus::isobus::tc::TCServerState::Disconnected
        );
    }

    for invalid_config in [
        TCServerConfig::default().with_booms(0).with_sections(1),
        TCServerConfig::default().with_booms(1).with_sections(0),
        TCServerConfig::default().with_booms(1).with_sections(0xFF),
    ] {
        let mut server = TaskControllerServer::new(invalid_config);
        let err = server
            .start()
            .expect_err("TC server start must reject unadvertisable topology counts");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert_eq!(
            server.state(),
            machbus::isobus::tc::TCServerState::Disconnected,
            "invalid topology counts must not advance the TC server to WaitForClients"
        );
    }

    let valid_config = TCServerConfig::default()
        .with_booms(1)
        .with_sections(1)
        .with_options(defined_options);
    valid_config.validate().unwrap();
    let mut server = TaskControllerServer::new(valid_config);
    server.start().unwrap();
    let response = server
        .try_handle_client_message(&Message::new(
            PGN_ECU_TO_TC,
            TaskControllerClient::build_version_request().to_vec(),
            0x42,
        ))
        .unwrap();
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].data[0], tc_cmd::VERSION_RESPONSE);
    assert_eq!(response[0].data[3], defined_options);
    assert_eq!(response[0].data[4], 0x00);

    let mut client = TaskControllerClient::new(TCClientConfig::default());
    client.set_ddop(minimal_client_ddop());
    client.connect().unwrap();
    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0],
            0x33,
        ))
        .unwrap();
    client.update(0);
    client.update(0);
    assert_eq!(client.state(), TCState::WaitForVersion);
    let mut reserved_version = valid_version_response();
    reserved_version[3] = 0x20;
    let err = client
        .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, reserved_version.clone(), 0x33))
        .expect_err("TC client must reject reserved option bits before version update");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::WaitForVersion);
    assert_eq!(client.tc_version(), 0);
    assert!(
        client
            .handle_tc_message(&Message::new(PGN_TC_TO_ECU, reserved_version, 0x33))
            .is_empty(),
        "compatibility wrapper must ignore reserved option bits"
    );
    assert_eq!(client.tc_version(), 0);

    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.start().unwrap();
    let request = server.handle_working_set_master(&Message::new(
        PGN_WORKING_SET_MASTER,
        vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        0x42,
    ));
    assert_eq!(request.len(), 1);
    let mut reserved_client_version = valid_version_response();
    reserved_client_version[3] = 0x40;
    let err = server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, reserved_client_version, 0x42))
        .expect_err("TC server must reject reserved option bits before version update");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(server.get_client_version(0x42), 0);
}

#[test]
fn tc_process_data_try_handlers_reject_malformed_payloads_before_callbacks() {
    let mut client = TaskControllerClient::new(TCClientConfig::default());
    client.set_ddop(minimal_client_ddop());
    client.on_value_request(|_, _| panic!("malformed client RequestValue reached callback"));
    client.on_value_command(|_, _, _| panic!("malformed client Value reached callback"));
    client.connect().unwrap();
    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0],
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::SendWorkingSetMaster);

    let mut bad_client_request = TaskControllerServer::build_request_value(1, 0x1234)
        .unwrap()
        .to_vec();
    bad_client_request[4] = 0x00;
    let err = client
        .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, bad_client_request, 0x33))
        .expect_err("malformed RequestValue must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::SendWorkingSetMaster);

    let bad_client_value = vec![
        ProcessDataCommands::Value.as_u8(),
        0x00,
        0x34,
        0x12,
        0x01,
        0x00,
        0x00,
    ];
    let err = client
        .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, bad_client_value, 0x33))
        .expect_err("malformed Value must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);

    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.start().unwrap();
    server.on_value_request(|_, _, _| panic!("malformed server RequestValue reached callback"));
    server.on_value_received(|_, _, _, _| panic!("malformed server Value reached callback"));
    server.on_peer_control_assignment(|_, _, _, _| {
        panic!("malformed server PeerControlAssignment reached callback")
    });

    let mut bad_server_request = TaskControllerServer::build_request_value(1, 0x1234)
        .unwrap()
        .to_vec();
    bad_server_request[7] = 0x00;
    let err = server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, bad_server_request, 0x42))
        .expect_err("malformed RequestValue must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);

    for payload in [
        vec![
            ProcessDataCommands::Value.as_u8(),
            0x00,
            0x34,
            0x12,
            0x01,
            0x00,
            0x00,
        ],
        vec![
            ProcessDataCommands::SetValueAndAcknowledge.as_u8(),
            0x00,
            0x34,
            0x12,
            0x01,
            0x00,
            0x00,
        ],
        vec![ProcessDataCommands::PeerControlAssignment.as_u8(), 0x00],
    ] {
        let err = server
            .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, payload, 0x42))
            .expect_err("malformed process-data frame must be explicit InvalidData");
        assert_eq!(err.code, ErrorCode::InvalidData);
    }
    assert!(
        server.clients().is_empty(),
        "malformed process data must not create client state"
    );
}

#[test]
fn tc_process_data_try_handlers_reject_reserved_and_unsupported_commands() {
    let mut client = TaskControllerClient::new(TCClientConfig::default());
    client.set_ddop(minimal_client_ddop());
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.start().unwrap();

    for reserved in [0x0B, 0x0C] {
        let payload = vec![reserved, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let err = client
            .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, payload.clone(), 0x33))
            .expect_err("reserved TC-to-ECU process-data command must be InvalidData");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(
            client
                .handle_tc_message(&Message::new(PGN_TC_TO_ECU, payload.clone(), 0x33))
                .is_empty(),
            "compatibility client wrapper must ignore reserved process-data commands"
        );

        let err = server
            .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, payload.clone(), 0x42))
            .expect_err("reserved ECU-to-TC process-data command must be InvalidData");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(
            server
                .handle_client_message(&Message::new(PGN_ECU_TO_TC, payload, 0x42))
                .is_empty(),
            "compatibility server wrapper must ignore reserved process-data commands"
        );
        assert!(server.clients().is_empty());
    }

    for unsupported in [
        0x11,
        ProcessDataCommands::MeasurementTimeInterval.as_u8(),
        ProcessDataCommands::MeasurementDistanceInterval.as_u8(),
        ProcessDataCommands::Acknowledge.as_u8(),
        ProcessDataCommands::ClientTask.as_u8(),
    ] {
        let payload = vec![unsupported, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        let err = client
            .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, payload.clone(), 0x33))
            .expect_err("unsupported TC-to-ECU process-data command must be explicit");
        assert_eq!(err.code, ErrorCode::InvalidState);
        assert!(
            client
                .handle_tc_message(&Message::new(PGN_TC_TO_ECU, payload.clone(), 0x33))
                .is_empty()
        );

        let err = server
            .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, payload.clone(), 0x42))
            .expect_err("unsupported ECU-to-TC process-data command must be explicit");
        assert_eq!(err.code, ErrorCode::InvalidState);
        assert!(
            server
                .handle_client_message(&Message::new(PGN_ECU_TO_TC, payload, 0x42))
                .is_empty()
        );
        assert!(server.clients().is_empty());
    }
}

#[test]
fn tc_client_lifecycle_responses_reject_malformed_fixed_frames_without_state_progress() {
    let mut client = TaskControllerClient::new(TCClientConfig::default());
    client.set_ddop(
        DDOP::default()
            .with_device(
                DeviceObject::default()
                    .with_id(1)
                    .with_designator("implement")
                    .with_structure_label(*b"AGBUS1 ")
                    .with_localization_label([1, 2, 3, 4, 5, 6, 7]),
            )
            .with_element(
                DeviceElement::default()
                    .with_id(2)
                    .with_type(DeviceElementType::Device),
            ),
    );
    client.connect().unwrap();

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0],
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::SendWorkingSetMaster);
    client.update(0);
    client.update(0);
    assert_eq!(client.state(), TCState::WaitForVersion);

    for bad_version in [
        vec![tc_cmd::VERSION_RESPONSE, 4, 0xFF, 0, 0x01, 1, 1, 0],
        vec![tc_cmd::VERSION_RESPONSE, 4],
    ] {
        let err = client
            .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, bad_version, 0x33))
            .expect_err("malformed version response must be explicit InvalidData");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert_eq!(client.state(), TCState::WaitForVersion);
        assert_eq!(client.tc_version(), 0);
    }

    client
        .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, valid_version_response(), 0x33))
        .unwrap();
    assert_eq!(client.state(), TCState::RequestStructureLabel);

    client.update(0);
    assert_eq!(client.state(), TCState::WaitForStructureLabel);
    let err = client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![tc_cmd::STRUCTURE_LABEL, 0xFF],
            0x33,
        ))
        .expect_err("malformed structure-label response must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::WaitForStructureLabel);

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![
                tc_cmd::STRUCTURE_LABEL,
                b'A',
                b'G',
                b'B',
                b'U',
                b'S',
                b'1',
                b' ',
            ],
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::RequestLocalizationLabel);

    client.update(0);
    assert_eq!(client.state(), TCState::WaitForLocalizationLabel);
    let err = client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![tc_cmd::LOCALIZATION_LABEL, 0xFF],
            0x33,
        ))
        .expect_err("malformed localization-label response must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::WaitForLocalizationLabel);

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![
                tc_cmd::LOCALIZATION_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::TransferDDOP);

    client.update(0);
    assert_eq!(client.state(), TCState::WaitForPoolResponse);
    let err = client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![
                tc_cmd::OBJECT_POOL_RESPONSE,
                0x00,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x33,
        ))
        .expect_err("malformed object-pool response must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::WaitForPoolResponse);

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            fixed_tc_response(tc_cmd::OBJECT_POOL_RESPONSE, 0x00),
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::ActivatePool);

    client.update(0);
    assert_eq!(client.state(), TCState::WaitForActivation);
    let err = client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![
                tc_cmd::ACTIVATE_RESPONSE,
                0x00,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x33,
        ))
        .expect_err("malformed activate response must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::WaitForActivation);

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            fixed_tc_response(tc_cmd::ACTIVATE_RESPONSE, 0x00),
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::Connected);

    client.reupload_ddop(minimal_client_ddop()).unwrap();
    assert_eq!(client.state(), TCState::DeactivatePool);
    client.update(0);
    assert_eq!(client.state(), TCState::WaitForDeactivation);
    let err = client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![
                tc_cmd::ACTIVATE_RESPONSE,
                0x00,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x33,
        ))
        .expect_err("malformed deactivate response must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::WaitForDeactivation);

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            fixed_tc_response(tc_cmd::ACTIVATE_RESPONSE, 0x00),
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::DeletePool);

    client.update(0);
    assert_eq!(client.state(), TCState::WaitForDeletePool);
    let err = client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![
                tc_cmd::DELETE_POOL_RESPONSE,
                0x00,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x33,
        ))
        .expect_err("malformed delete response must be explicit InvalidData");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert_eq!(client.state(), TCState::WaitForDeletePool);

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            fixed_tc_response(tc_cmd::DELETE_POOL_RESPONSE, 0x00),
            0x33,
        ))
        .unwrap();
    assert_eq!(client.state(), TCState::TransferDDOP);
}

#[test]
fn tc_client_rejects_non_canonical_status_frames_before_server_binding() {
    let mut client = TaskControllerClient::new(TCClientConfig::default());
    client.set_ddop(minimal_client_ddop());
    client.connect().unwrap();
    assert_eq!(client.state(), TCState::WaitForServerStatus);
    assert_eq!(client.tc_address(), NULL_ADDRESS);

    for payload in [
        vec![ProcessDataCommands::Status.as_u8(), 0, 0, 0, 0, 0, 0, 0],
        vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0],
        vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0, 0],
    ] {
        let err = client
            .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, payload, 0x33))
            .expect_err("malformed TC status must be reported explicitly");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert_eq!(client.state(), TCState::WaitForServerStatus);
        assert_eq!(
            client.tc_address(),
            NULL_ADDRESS,
            "malformed TC status must not bind the client to a server source"
        );
    }

    client
        .try_handle_tc_message(&Message::new(
            PGN_TC_TO_ECU,
            vec![tc_cmd::TC_STATUS, 0, 0, 0, 0, 0, 0, 0],
            0x33,
        ))
        .unwrap();
    assert_eq!(client.tc_address(), 0x33);
    assert_eq!(client.state(), TCState::SendWorkingSetMaster);
}

#[test]
fn tc_status_frames_preserve_busy_source_and_client_server_binding() {
    let mut server = TaskControllerServer::new(
        TCServerConfig::default()
            .with_booms(2)
            .with_sections(3)
            .with_channels(3),
    );
    assert_eq!(
        server.update(TC_STATUS_INTERVAL_MS),
        None,
        "disconnected task controllers must not emit status frames"
    );
    server.start().unwrap();

    let idle = server.update(TC_STATUS_INTERVAL_MS).unwrap();
    assert_eq!(
        idle[0],
        0xF0 | ProcessDataCommands::Status.as_u8(),
        "TC status must use the canonical fixed command byte"
    );
    assert_eq!(idle[4] & 0x08, 0);
    assert_eq!(idle[5], 0x00);
    assert_eq!(idle[6], 0x00);
    assert_eq!(idle[7], 3);

    server.set_command_busy_for(0x42, ProcessDataCommands::RequestValue.as_u8());
    let busy = server.update(TC_STATUS_INTERVAL_MS).unwrap();
    assert_ne!(busy[4] & 0x08, 0);
    assert_eq!(busy[5], 0x42);
    assert_eq!(busy[6], ProcessDataCommands::RequestValue.as_u8());
    assert_eq!(busy[7], 3);

    server.set_command_busy(false);
    let cleared = server.update(TC_STATUS_INTERVAL_MS).unwrap();
    assert_eq!(cleared[4] & 0x08, 0);
    assert_eq!(cleared[5], 0x00);
    assert_eq!(cleared[6], 0x00);

    let mut client = TaskControllerClient::new(TCClientConfig::default());
    client.set_ddop(minimal_client_ddop());
    client.connect().unwrap();
    client
        .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, idle.to_vec(), 0x33))
        .unwrap();
    assert_eq!(client.tc_address(), 0x33);
    assert_eq!(client.state(), TCState::SendWorkingSetMaster);
    client.update(0);
    assert_eq!(client.state(), TCState::RequestVersion);
    client.update(0);
    assert_eq!(client.state(), TCState::WaitForVersion);

    let err = client
        .try_handle_tc_message(&Message::new(PGN_TC_TO_ECU, busy.to_vec(), 0x34))
        .expect_err("bound TC client must reject status from a different TC source");
    assert_eq!(err.code, ErrorCode::InvalidState);
    assert_eq!(client.tc_address(), 0x33);
    assert_eq!(
        client.state(),
        TCState::WaitForVersion,
        "wrong-source TC status must not move the lifecycle state"
    );

    let status = TaskControllerClient::build_status(
        TCClientTaskStatus::Active,
        0x33,
        ProcessDataCommands::RequestValue.as_u8(),
    );
    assert_eq!(&status[0..4], &[0xFF, 0xFF, 0xFF, 0xFF]);
    assert_eq!(status[4], TCClientTaskStatus::Active.as_u8());
    assert_eq!(status[5], 0x33);
    assert_eq!(status[6], ProcessDataCommands::RequestValue.as_u8());
    assert_eq!(status[7], 0x00);
}

#[test]
fn tc_geo_position_process_data_payloads_use_canonical_value_headers() {
    let mut geo = TCGEOInterface::new();
    geo.set_position(GeoPoint {
        position: Wgs::new(52.123_456_7, 5.765_432_1, 0.0),
        timestamp_us: 42,
    });

    let [lat, lon] = geo.position_process_data_payloads().unwrap();
    for payload in [lat, lon] {
        assert_eq!(
            payload[0] & 0x0F,
            ProcessDataCommands::Value.as_u8(),
            "TC-GEO position values must use the process-data Value command"
        );
        assert_eq!(
            payload[0] >> 4,
            0,
            "TC-GEO position values must not leak reserved element low bits"
        );
        assert_eq!(
            payload[1], 0,
            "TC-GEO position values must use a canonical zero element high byte"
        );
    }

    assert_eq!(
        u16::from_le_bytes([lat[2], lat[3]]),
        geo_ddi::ACTUAL_LATITUDE
    );
    assert_eq!(
        u16::from_le_bytes([lon[2], lon[3]]),
        geo_ddi::ACTUAL_LONGITUDE
    );
    assert_eq!(
        i32::from_le_bytes(lat[4..8].try_into().unwrap()),
        521_234_567
    );
    assert_eq!(
        i32::from_le_bytes(lon[4..8].try_into().unwrap()),
        57_654_321
    );
}

#[test]
fn tc_geo_gnss_position_ingress_rejects_invalid_envelopes_without_cache_mutation() {
    let mut geo = TCGEOInterface::new();
    let original = GeoPoint {
        position: Wgs::new(52.0, 5.0, 0.0),
        timestamp_us: 7,
    };
    geo.set_position(original);

    let mut valid_payload = Vec::with_capacity(8);
    valid_payload.extend_from_slice(&521_234_567i32.to_le_bytes());
    valid_payload.extend_from_slice(&57_654_321i32.to_le_bytes());

    for (message, expected_code) in [
        (
            Message::new(PGN_GNSS_POSITION + 1, valid_payload.clone(), 0x24),
            ErrorCode::InvalidPgn,
        ),
        (
            Message::new(PGN_GNSS_POSITION, valid_payload.clone(), NULL_ADDRESS),
            ErrorCode::InvalidAddress,
        ),
        (
            Message::new(PGN_GNSS_POSITION, valid_payload.clone(), BROADCAST_ADDRESS),
            ErrorCode::InvalidAddress,
        ),
        (
            Message::with_addressing(
                PGN_GNSS_POSITION,
                valid_payload.clone(),
                0x24,
                0x42,
                Priority::Default,
            ),
            ErrorCode::InvalidAddress,
        ),
        (
            Message::new(PGN_GNSS_POSITION, valid_payload[..7].to_vec(), 0x24),
            ErrorCode::InvalidData,
        ),
        (
            Message::new(
                PGN_GNSS_POSITION,
                [valid_payload.as_slice(), &[0xFF]].concat(),
                0x24,
            ),
            ErrorCode::InvalidData,
        ),
        (
            Message::new(
                PGN_GNSS_POSITION,
                [
                    i32::MAX.to_le_bytes().as_slice(),
                    57_654_321i32.to_le_bytes().as_slice(),
                ]
                .concat(),
                0x24,
            ),
            ErrorCode::InvalidData,
        ),
        (
            Message::new(
                PGN_GNSS_POSITION,
                [
                    900_000_001i32.to_le_bytes().as_slice(),
                    57_654_321i32.to_le_bytes().as_slice(),
                ]
                .concat(),
                0x24,
            ),
            ErrorCode::InvalidData,
        ),
        (
            Message::new(
                PGN_GNSS_POSITION,
                [
                    521_234_567i32.to_le_bytes().as_slice(),
                    (-1_800_000_001i32).to_le_bytes().as_slice(),
                ]
                .concat(),
                0x24,
            ),
            ErrorCode::InvalidData,
        ),
    ] {
        let err = geo
            .try_handle_gnss_position(&message)
            .expect_err("invalid TC-GEO GNSS position input must be rejected");
        assert_eq!(err.code, expected_code);
        assert_eq!(
            geo.current_position(),
            Some(original),
            "rejected TC-GEO GNSS input must not overwrite the last accepted position"
        );
    }

    let mut valid_message = Message::new(PGN_GNSS_POSITION, valid_payload, 0x24);
    valid_message.timestamp_us = 99;
    geo.try_handle_gnss_position(&valid_message).unwrap();
    let updated = geo.current_position().unwrap();
    assert_eq!(updated.timestamp_us, 99);
    assert!((updated.position.latitude - 52.123_456_7).abs() < 1e-7);
    assert!((updated.position.longitude - 5.765_432_1).abs() < 1e-7);
}

#[test]
fn tc_geo_prescription_rate_payloads_validate_ddi_resolution_and_range() {
    let volume_per_area = DDI(machbus::isobus::tc::ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE);
    assert_eq!(
        prescription_rate_from_engineering(volume_per_area, 12.34).unwrap(),
        1234
    );
    assert!(
        (prescription_rate_to_engineering(volume_per_area, 1234).unwrap() - 12.34).abs() < 1e-9
    );
    assert_eq!(
        prescription_rate_process_data_payload(volume_per_area, 1234).unwrap(),
        [0x03, 0x00, 0x01, 0x00, 0xD2, 0x04, 0x00, 0x00],
        "TC-GEO prescription rates must emit canonical process-data Value payloads"
    );

    for invalid_engineering_value in [f64::NAN, f64::INFINITY, 1.0e20] {
        assert!(
            prescription_rate_from_engineering(volume_per_area, invalid_engineering_value).is_err(),
            "non-finite and out-of-range engineering rates must not encode to TC process data"
        );
    }
    for invalid_ddi in [geo_ddi::ACTUAL_LATITUDE, DDI(0xFFFF)] {
        assert!(
            prescription_rate_from_engineering(invalid_ddi, 1.0).is_err(),
            "non-rate or unknown DDIs must not be accepted as prescription-rate payloads"
        );
    }
    assert!(
        prescription_rate_process_data_payload(volume_per_area, -1).is_err(),
        "raw prescription rates below the DDI range must not emit process-data payloads"
    );
}

#[test]
fn tc_geo_prescription_rate_state_clears_when_position_leaves_all_zones() {
    let mut geo = TCGEOInterface::new();
    geo.add_prescription_map(PrescriptionMap {
        structure_label: "single-rate-zone".to_owned(),
        zones: vec![PrescriptionZone {
            boundary: vec![
                Wgs::new(0.0, 0.0, 0.0),
                Wgs::new(0.0, 1.0, 0.0),
                Wgs::new(1.0, 1.0, 0.0),
                Wgs::new(1.0, 0.0, 0.0),
            ],
            holes: Vec::new(),
            application_rate: 100,
        }],
    });
    let log = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    geo.on_application_rate_changed
        .subscribe(move |rate| sink.borrow_mut().push(*rate));

    geo.set_position(GeoPoint {
        position: Wgs::new(0.5, 0.5, 0.0),
        timestamp_us: 1,
    });
    geo.update(0);
    geo.update(0);
    assert_eq!(
        *log.borrow(),
        vec![100],
        "unchanged in-zone prescription rates must not be re-emitted"
    );

    geo.set_position(GeoPoint {
        position: Wgs::new(2.0, 2.0, 0.0),
        timestamp_us: 2,
    });
    geo.update(0);
    assert_eq!(
        *log.borrow(),
        vec![100],
        "leaving all prescription zones clears internal rate state without emitting a stale rate"
    );

    geo.set_position(GeoPoint {
        position: Wgs::new(0.5, 0.5, 0.0),
        timestamp_us: 3,
    });
    geo.update(0);
    assert_eq!(
        *log.borrow(),
        vec![100, 100],
        "re-entering a zone with the same rate after a no-rate interval must be observable"
    );
}

#[test]
fn tc_geo_clearing_prescription_maps_clears_rate_state_before_reload() {
    let mut geo = TCGEOInterface::new();
    let map = PrescriptionMap {
        structure_label: "reloadable-rate-zone".to_owned(),
        zones: vec![PrescriptionZone {
            boundary: vec![
                Wgs::new(0.0, 0.0, 0.0),
                Wgs::new(0.0, 1.0, 0.0),
                Wgs::new(1.0, 1.0, 0.0),
                Wgs::new(1.0, 0.0, 0.0),
            ],
            holes: Vec::new(),
            application_rate: 100,
        }],
    };
    geo.add_prescription_map(map.clone());
    geo.set_position(GeoPoint {
        position: Wgs::new(0.5, 0.5, 0.0),
        timestamp_us: 1,
    });

    let log = Rc::new(RefCell::new(Vec::new()));
    let sink = log.clone();
    geo.on_application_rate_changed
        .subscribe(move |rate| sink.borrow_mut().push(*rate));

    geo.update(0);
    assert_eq!(*log.borrow(), vec![100]);

    geo.clear_prescription_maps();
    geo.add_prescription_map(map);
    geo.update(0);
    assert_eq!(
        *log.borrow(),
        vec![100, 100],
        "reloading a prescription map must make the current rate observable even if the numeric value is unchanged"
    );
}

#[test]
fn tc_peer_control_assignment_preserves_nibble_split_and_rejects_unencodable_source() {
    let assignment = PeerControlAssignment::default()
        .from(ElementNumber(MAX_PEER_CONTROL_SOURCE_ELEMENT), DDI(1))
        .to(ElementNumber(2), DDI(3))
        .with_source(0x80)
        .with_destination(0x42);
    let encoded = assignment.try_encode().unwrap();
    assert_eq!(
        encoded[0] & 0x0F,
        ProcessDataCommands::PeerControlAssignment.as_u8()
    );

    let decoded = PeerControlAssignment::decode(&encoded, 0x80, 0x42).unwrap();
    assert_eq!(
        decoded.source_element,
        ElementNumber(MAX_PEER_CONTROL_SOURCE_ELEMENT)
    );
    assert_eq!(decoded.source_ddi, DDI(1));
    assert_eq!(decoded.destination_element, ElementNumber(2));
    assert_eq!(decoded.destination_ddi, DDI(3));

    let too_wide = PeerControlAssignment::default()
        .from(ElementNumber(MAX_PEER_CONTROL_SOURCE_ELEMENT + 1), DDI(1))
        .to(ElementNumber(2), DDI(3))
        .with_source(0x80)
        .with_destination(0x42);
    assert!(too_wide.try_encode().is_err());
}

#[test]
fn tc_peer_control_decode_rejects_bad_envelope_without_partial_assignment() {
    let assignment = PeerControlAssignment::default()
        .from(ElementNumber(1), DDI(2))
        .to(ElementNumber(3), DDI(4))
        .with_source(0x80)
        .with_destination(0x42);
    let encoded = assignment.try_encode().unwrap();

    let mut wrong_command = encoded;
    wrong_command[0] = (wrong_command[0] & 0xF0) | ProcessDataCommands::Value.as_u8();
    assert!(PeerControlAssignment::decode(&wrong_command, 0x80, 0x42).is_err());
    assert!(PeerControlAssignment::decode(&encoded[..7], 0x80, 0x42).is_err());
    assert!(PeerControlAssignment::decode(&encoded, NULL_ADDRESS, 0x42).is_err());
    assert!(PeerControlAssignment::decode(&encoded, BROADCAST_ADDRESS, 0x42).is_err());
    assert!(PeerControlAssignment::decode(&encoded, 0x80, NULL_ADDRESS).is_err());

    let mut registry = PeerControlInterface::new();
    assert!(
        registry
            .add_assignment(PeerControlAssignment::default())
            .is_err(),
        "invalid default addresses must not be admitted to the registry"
    );
    assert!(
        registry.assignments().is_empty(),
        "malformed input must not leave a partial peer-control assignment behind"
    );
}

#[test]
fn tc_peer_control_rejects_global_destination_without_registry_mutation() {
    let assignment = PeerControlAssignment::default()
        .from(ElementNumber(1), DDI(2))
        .to(ElementNumber(3), DDI(4))
        .with_source(0x80)
        .with_destination(BROADCAST_ADDRESS);
    let encoded = assignment.encode();

    assert!(
        assignment.try_encode().is_err(),
        "peer-control assignments must target a concrete destination address"
    );
    assert!(
        PeerControlAssignment::decode(&encoded, 0x80, BROADCAST_ADDRESS).is_err(),
        "decoded peer-control assignments must reject the global destination address"
    );

    let mut registry = PeerControlInterface::new();
    assert!(registry.add_assignment(assignment).is_err());
    assert!(
        registry.assignments().is_empty(),
        "rejected global-destination assignments must not mutate registry state"
    );
}

#[test]
fn tc_peer_control_registry_deduplicates_by_source_triplet_and_rejects_ambiguous_lookup() {
    let mut registry = PeerControlInterface::new();
    let first = PeerControlAssignment::default()
        .from(ElementNumber(1), DDI(2))
        .to(ElementNumber(3), DDI(4))
        .with_source(0x80)
        .with_destination(0x42);
    let same_source = first.to(ElementNumber(5), DDI(6));
    let different_source_address = first.with_source(0x81);

    registry.add_assignment(first).unwrap();
    assert!(
        registry.add_assignment(same_source).is_err(),
        "one source address/element/DDI triplet must not resolve to two destinations"
    );
    registry.add_assignment(different_source_address).unwrap();
    assert_eq!(registry.assignments().len(), 2);

    assert!(
        registry
            .activate_assignment(ElementNumber(1), DDI(2), true)
            .is_err(),
        "legacy source-only lookup is ambiguous once two source addresses match"
    );
    registry
        .activate_assignment_from(0x81, ElementNumber(1), DDI(2), true)
        .unwrap();
    assert!(registry.assignments()[1].active);
}

#[test]
fn tc_process_data_set_value_and_acknowledge_preserves_element_nibble_and_error() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.on_value_received(|element, ddi, value, source| {
        assert_eq!(source, 0x42);
        assert_eq!(element, ElementNumber(0x0ABC));
        assert_eq!(ddi, DDI(0x1234));
        assert_eq!(value, -77);
        Ok(ProcessDataAcknowledgeErrorCodes::ValueIsOutsideValidRange)
    });

    let payload = TaskControllerServer::build_set_value_and_acknowledge(0x0ABC, 0x1234, -77)
        .expect("12-bit element number should encode");
    let out = server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, payload.to_vec(), 0x42))
        .unwrap();

    assert_eq!(out.len(), 1);
    assert_eq!(out[0].dest, Some(0x42));
    assert_eq!(out[0].data[0] >> 4, 0x0C);
    assert_eq!(
        out[0].data[0] & 0x0F,
        ProcessDataCommands::Acknowledge.as_u8()
    );
    assert_eq!(out[0].data[1], 0xAB);
    assert_eq!(&out[0].data[2..4], &[0x34, 0x12]);
    assert_eq!(
        out[0].data[4],
        ProcessDataAcknowledgeErrorCodes::ValueIsOutsideValidRange.as_u8()
    );
    assert!(out[0].data[5..].iter().all(|byte| *byte == 0xFF));
}

#[test]
fn tc_process_data_measurement_builders_reject_unencodable_element_numbers() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    assert!(TaskControllerServer::build_request_value(0x1000, 0x1234).is_err());
    assert!(TaskControllerServer::build_set_value(0x1000, 0x1234, 1).is_err());
    assert!(TaskControllerServer::build_set_value_and_acknowledge(0x1000, 0x1234, 1).is_err());

    let trigger = MeasurementTriggerRuntime::new(0x42, 0x1000, 0x1234).with_time_interval_ms(10);
    assert!(server.configure_measurement_trigger(trigger).is_err());
    assert!(
        server.measurement_triggers().is_empty(),
        "unencodable trigger identifiers must not leave partial runtime state"
    );
}

#[test]
fn tc_measurement_command_builders_preserve_element_nibble_and_reject_overflow() {
    let cases = [
        (
            ProcessDataCommands::MeasurementTimeInterval,
            TaskControllerServer::build_time_interval_measurement_command(0x0ABC, 0x1234, 250),
        ),
        (
            ProcessDataCommands::MeasurementDistanceInterval,
            TaskControllerServer::build_distance_interval_measurement_command(0x0ABC, 0x1234, 500),
        ),
        (
            ProcessDataCommands::MeasurementMinimumWithinThreshold,
            TaskControllerServer::build_minimum_threshold_measurement_command(0x0ABC, 0x1234, 10),
        ),
        (
            ProcessDataCommands::MeasurementMaximumWithinThreshold,
            TaskControllerServer::build_maximum_threshold_measurement_command(0x0ABC, 0x1234, 20),
        ),
        (
            ProcessDataCommands::MeasurementChangeThreshold,
            TaskControllerServer::build_change_threshold_measurement_command(0x0ABC, 0x1234, 30),
        ),
    ];

    for (command, payload) in cases {
        let payload = payload.unwrap();
        assert_eq!(payload[0] & 0x0F, command.as_u8());
        assert_eq!(payload[0] >> 4, 0x0C);
        assert_eq!(payload[1], 0xAB);
        assert_eq!(&payload[2..4], &[0x34, 0x12]);
    }

    assert!(
        TaskControllerServer::build_time_interval_measurement_command(0x1000, 0x1234, 250).is_err()
    );
    assert!(
        TaskControllerServer::build_distance_interval_measurement_command(0x1000, 0x1234, 500)
            .is_err()
    );
    assert!(
        TaskControllerServer::build_minimum_threshold_measurement_command(0x1000, 0x1234, 10)
            .is_err()
    );
    assert!(
        TaskControllerServer::build_maximum_threshold_measurement_command(0x1000, 0x1234, 20)
            .is_err()
    );
    assert!(
        TaskControllerServer::build_change_threshold_measurement_command(0x1000, 0x1234, 30)
            .is_err()
    );
}

#[test]
fn tc_ddop_lifecycle_requests_do_not_allocate_clients_before_upload_state() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.start().unwrap();

    let activate = [
        tc_cmd::ACTIVATE_POOL,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
    ];
    let activation_response =
        server.handle_client_message(&Message::new(PGN_ECU_TO_TC, activate.to_vec(), 0x42));
    assert_eq!(activation_response.len(), 1);
    assert_eq!(activation_response[0].data[0], tc_cmd::ACTIVATE_RESPONSE);
    assert_eq!(
        activation_response[0].data[1],
        ObjectPoolActivationError::ThereAreErrorsInTheDDOP.as_u8()
    );
    assert!(
        server.clients().is_empty(),
        "activation before DDOP upload must not create a client slot"
    );

    let deactivate = [
        tc_cmd::ACTIVATE_POOL,
        0x00,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
    ];
    let deactivation_response =
        server.handle_client_message(&Message::new(PGN_ECU_TO_TC, deactivate.to_vec(), 0x42));
    assert_eq!(deactivation_response.len(), 1);
    assert_eq!(deactivation_response[0].data[0], tc_cmd::ACTIVATE_RESPONSE);
    assert_eq!(
        deactivation_response[0].data[1],
        ObjectPoolActivationError::ThereAreErrorsInTheDDOP.as_u8()
    );
    assert!(
        server.clients().is_empty(),
        "deactivation before DDOP upload must not create a client slot"
    );

    let delete = [
        tc_cmd::DELETE_POOL,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
    ];
    let delete_response =
        server.handle_client_message(&Message::new(PGN_ECU_TO_TC, delete.to_vec(), 0x42));
    assert_eq!(delete_response.len(), 1);
    assert_eq!(delete_response[0].data[0], tc_cmd::DELETE_POOL_RESPONSE);
    assert_eq!(
        delete_response[0].data[1],
        ObjectPoolDeletionErrors::ErrorDetailsNotAvailable.as_u8()
    );
    assert!(
        server.clients().is_empty(),
        "delete before DDOP upload must not create a client slot"
    );
}

#[test]
fn tc_lifecycle_requests_reject_malformed_fixed_frames_before_process_data_fallback() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.start().unwrap();

    for payload in [
        vec![
            tc_cmd::ACTIVATE_POOL,
            0x01,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        vec![tc_cmd::ACTIVATE_POOL, 0xFF, 0xFF],
        vec![
            tc_cmd::ACTIVATE_POOL,
            0x00,
            0xFF,
            0xFF,
            0xFF,
            0x00,
            0xFF,
            0xFF,
        ],
        vec![
            tc_cmd::DELETE_POOL,
            0x00,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        vec![tc_cmd::DELETE_POOL, 0xFF, 0xFF],
    ] {
        let err = server
            .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, payload, 0x42))
            .expect_err("malformed TC lifecycle request must be explicit InvalidData");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(
            server.clients().is_empty(),
            "malformed lifecycle requests must not be reinterpreted as process-data commands"
        );
    }
}

#[test]
fn tc_server_rejects_malformed_client_version_responses_without_version_update() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.start().unwrap();
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen_clone = seen.clone();
    server
        .on_client_version_received
        .subscribe(move |event| seen_clone.borrow_mut().push(*event));

    let request = server.handle_working_set_master(&Message::new(
        PGN_WORKING_SET_MASTER,
        vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        0x42,
    ));
    assert_eq!(request.len(), 1);
    assert_eq!(server.clients().len(), 1);
    assert_eq!(server.get_client_version(0x42), 0);

    for payload in [
        vec![tc_cmd::VERSION_RESPONSE, 4, 0xFF, 0x1F, 0x01, 1, 1, 0],
        vec![tc_cmd::VERSION_RESPONSE, 4, 0xFF],
    ] {
        let err = server
            .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, payload.clone(), 0x42))
            .expect_err("malformed client version response must be explicit InvalidData");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert_eq!(server.clients().len(), 1);
        assert_eq!(server.get_client_version(0x42), 0);
        assert!(seen.borrow().is_empty());

        assert!(
            server
                .handle_client_message(&Message::new(PGN_ECU_TO_TC, payload, 0x42))
                .is_empty(),
            "compatibility wrapper must ignore malformed client version responses"
        );
        assert_eq!(server.get_client_version(0x42), 0);
        assert!(seen.borrow().is_empty());
    }

    server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, valid_version_response(), 0x42))
        .unwrap();
    assert_eq!(server.get_client_version(0x42), 4);
    assert_eq!(*seen.borrow(), vec![(0x42, 4)]);
}

#[test]
fn tc_measurement_runtime_rejects_invalid_destination_and_empty_trigger_definition() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));

    for destination in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let trigger =
            MeasurementTriggerRuntime::new(destination, 7, 0x1234).with_time_interval_ms(100);
        assert_eq!(
            server
                .configure_measurement_trigger(trigger)
                .unwrap_err()
                .code,
            ErrorCode::InvalidAddress
        );
        assert!(server.measurement_triggers().is_empty());
    }

    let empty = MeasurementTriggerRuntime::new(0x42, 7, 0x1234);
    assert_eq!(
        server
            .configure_measurement_trigger(empty)
            .unwrap_err()
            .code,
        ErrorCode::InvalidData
    );
    assert!(server.measurement_triggers().is_empty());
}

#[test]
fn tc_measurement_runtime_emits_periodic_distance_and_value_trigger_requests() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));

    server
        .configure_measurement_trigger(
            MeasurementTriggerRuntime::new(0x42, 7, 0x1234)
                .with_time_interval_ms(100)
                .with_distance_interval_mm(250),
        )
        .unwrap();
    server
        .configure_measurement_trigger(
            MeasurementTriggerRuntime::new(0x42, 8, 0xCAFE)
                .with_trigger(TriggerMethod::OnChange)
                .with_change_threshold(10)
                .with_minimum_threshold(-5)
                .with_maximum_threshold(50),
        )
        .unwrap();

    let due = server.update_measurements(250);
    assert_eq!(due.len(), 2, "periodic scheduler must preserve overshoot");
    for frame in &due {
        assert_eq!(frame.dest, Some(0x42));
        assert_eq!(
            frame.data,
            TaskControllerServer::build_request_value(7, 0x1234)
                .unwrap()
                .to_vec()
        );
    }

    assert!(server.record_measurement_distance(0x42, 249).is_empty());
    let distance_due = server.record_measurement_distance(0x42, 1);
    assert_eq!(distance_due.len(), 1);
    assert_eq!(
        distance_due[0].data,
        TaskControllerServer::build_request_value(7, 0x1234)
            .unwrap()
            .to_vec()
    );

    let initial_value = TaskControllerServer::build_set_value(8, 0xCAFE, 0)
        .unwrap()
        .to_vec();
    assert!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, initial_value, 0x42))
            .is_empty(),
        "initial value establishes baseline without a synthetic change request"
    );

    let changed_value = TaskControllerServer::build_set_value(8, 0xCAFE, 12)
        .unwrap()
        .to_vec();
    let changed = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, changed_value, 0x42));
    assert_eq!(changed.len(), 1);
    assert_eq!(
        changed[0].data,
        TaskControllerServer::build_request_value(8, 0xCAFE)
            .unwrap()
            .to_vec()
    );

    let max_value = TaskControllerServer::build_set_value(8, 0xCAFE, 50)
        .unwrap()
        .to_vec();
    assert_eq!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, max_value, 0x42))
            .len(),
        1
    );
}

#[test]
fn tc_measurement_runtime_thresholds_trigger_only_on_crossings() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server
        .configure_measurement_trigger(
            MeasurementTriggerRuntime::new(0x42, 9, 0xCAFE)
                .with_minimum_threshold(-10)
                .with_maximum_threshold(50)
                .with_change_threshold(5),
        )
        .unwrap();

    let baseline = TaskControllerServer::build_set_value(9, 0xCAFE, 0)
        .unwrap()
        .to_vec();
    assert!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, baseline, 0x42))
            .is_empty(),
        "baseline values inside the threshold window must not synthesize a request"
    );

    let small_change = TaskControllerServer::build_set_value(9, 0xCAFE, 4)
        .unwrap()
        .to_vec();
    assert!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, small_change, 0x42))
            .is_empty(),
        "change-threshold triggers require the configured delta"
    );

    let change_due = TaskControllerServer::build_set_value(9, 0xCAFE, 9)
        .unwrap()
        .to_vec();
    let due = server.handle_client_message(&Message::new(PGN_ECU_TO_TC, change_due, 0x42));
    assert_eq!(due.len(), 1);
    assert_eq!(
        due[0].data,
        TaskControllerServer::build_request_value(9, 0xCAFE)
            .unwrap()
            .to_vec()
    );

    let minimum_crossing = TaskControllerServer::build_set_value(9, 0xCAFE, -10)
        .unwrap()
        .to_vec();
    assert_eq!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, minimum_crossing, 0x42))
            .len(),
        1
    );
    let still_below = TaskControllerServer::build_set_value(9, 0xCAFE, -11)
        .unwrap()
        .to_vec();
    assert_eq!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, still_below, 0x42))
            .len(),
        0,
        "remaining below a minimum threshold must not retrigger without crossing back"
    );

    let back_inside = TaskControllerServer::build_set_value(9, 0xCAFE, 0)
        .unwrap()
        .to_vec();
    assert_eq!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, back_inside, 0x42))
            .len(),
        1,
        "change threshold still applies while returning inside the window"
    );

    let maximum_crossing = TaskControllerServer::build_set_value(9, 0xCAFE, 50)
        .unwrap()
        .to_vec();
    assert_eq!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, maximum_crossing, 0x42))
            .len(),
        1
    );
    let still_above = TaskControllerServer::build_set_value(9, 0xCAFE, 51)
        .unwrap()
        .to_vec();
    assert_eq!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, still_above, 0x42))
            .len(),
        0,
        "remaining above a maximum threshold must not retrigger without crossing back"
    );
}

#[test]
fn tc_lifecycle_label_requests_require_canonical_fixed_frames_before_client_creation() {
    let mut server =
        TaskControllerServer::new(TCServerConfig::default().with_booms(1).with_sections(1));
    server.start().unwrap();

    let mut bad_structure = vec![tc_cmd::REQUEST_STRUCTURE_LABEL; 8];
    bad_structure[1] = 0x00;
    let err = server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, bad_structure.clone(), 0x42))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert!(server.clients().is_empty());
    assert!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, bad_structure, 0x42))
            .is_empty()
    );
    assert!(server.clients().is_empty());

    let mut bad_localization = vec![tc_cmd::REQUEST_LOCALIZATION_LABEL; 8];
    bad_localization[7] = 0x00;
    let err = server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, bad_localization.clone(), 0x42))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert!(server.clients().is_empty());
    assert!(
        server
            .handle_client_message(&Message::new(PGN_ECU_TO_TC, bad_localization, 0x42))
            .is_empty()
    );
    assert!(server.clients().is_empty());

    let mut structure = [0xFFu8; 8];
    structure[0] = tc_cmd::REQUEST_STRUCTURE_LABEL;
    let out = server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, structure.to_vec(), 0x42))
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].dest, Some(0x42));
    assert_eq!(out[0].data[0], tc_cmd::STRUCTURE_LABEL);
    assert_eq!(server.clients().len(), 1);

    let mut localization = [0xFFu8; 8];
    localization[0] = tc_cmd::REQUEST_LOCALIZATION_LABEL;
    let out = server
        .try_handle_client_message(&Message::new(PGN_ECU_TO_TC, localization.to_vec(), 0x42))
        .unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].dest, Some(0x42));
    assert_eq!(out[0].data[0], tc_cmd::LOCALIZATION_LABEL);
    assert_eq!(server.clients().len(), 1);
}
