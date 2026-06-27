#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::tc::{DeviceElement, DeviceObject};
    use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
    use crate::net::pgn_defs::{PGN_ECU_TO_TC, PGN_TC_TO_ECU, PGN_WORKING_SET_MASTER};

    fn ecu_msg(data: Vec<u8>, src: Address) -> Message {
        Message::new(PGN_ECU_TO_TC, data, src)
    }

    fn valid_config() -> TCServerConfig {
        TCServerConfig::default().with_booms(1).with_sections(1)
    }

    #[test]
    fn config_builders() {
        let c = TCServerConfig::default()
            .with_number(1)
            .with_version(4)
            .with_booms(2)
            .with_sections(8)
            .with_channels(3)
            .with_options(0x10);
        assert_eq!(c.tc_number, 1);
        assert_eq!(c.num_sections, 8);
    }

    #[test]
    fn config_validation_rejects_unadvertisable_topology() {
        assert!(
            TCServerConfig::default()
                .with_booms(1)
                .with_sections(1)
                .validate()
                .is_ok()
        );

        let err = TCServerConfig::default()
            .with_booms(0)
            .with_sections(1)
            .validate()
            .expect_err("zero booms should be rejected");
        assert!(err.message.contains("num_booms"));

        let err = TCServerConfig::default()
            .with_booms(1)
            .with_sections(0)
            .validate()
            .expect_err("zero sections should be rejected");
        assert!(err.message.contains("num_sections"));

        let err = TCServerConfig::default()
            .with_booms(1)
            .with_sections(255)
            .validate()
            .expect_err("reserved section count should be rejected");
        assert!(err.message.contains("1..=254"));

        for channels in [0, 1, 16, MAX_TC_SERVER_CHANNELS] {
            TCServerConfig::default()
                .with_booms(1)
                .with_sections(1)
                .with_channels(channels)
                .validate()
                .unwrap_or_else(|err| panic!("channel count {channels} should be accepted: {err}"));
        }
    }

    #[test]
    fn channel_count_is_preserved_as_raw_capability_byte() {
        for channels in [0, 16, MAX_TC_SERVER_CHANNELS] {
            let mut s = TaskControllerServer::new(
                TCServerConfig::default()
                    .with_version(4)
                    .with_booms(1)
                    .with_sections(1)
                    .with_channels(channels),
            );
            s.start().unwrap();

            let status = s.update(TC_STATUS_INTERVAL_MS).unwrap();
            assert_eq!(status[7], channels);

            let out = s.handle_client_message(&ecu_msg(
                vec![
                    tc_cmd::VERSION_REQUEST,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                    0xFF,
                ],
                0x42,
            ));
            assert_eq!(out[0].data[5], 1);
            assert_eq!(out[0].data[6], 1);
            assert_eq!(out[0].data[7], channels);
        }
    }

    #[test]
    fn start_transitions_to_wait_for_clients() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        assert_eq!(s.state(), TCServerState::WaitForClients);
    }

    #[test]
    fn tech_capabilities_creates_client_and_replies() {
        let mut s = TaskControllerServer::new(valid_config().with_version(4));
        s.start().unwrap();
        let out = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::VERSION_REQUEST,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].dest, Some(0x42));
        assert_eq!(out[0].data[0], tc_cmd::VERSION_RESPONSE);
        assert_eq!(s.clients().len(), 1);
        assert_eq!(s.state(), TCServerState::Active);
    }

    #[test]
    fn working_set_master_registers_client_and_requests_version() {
        let mut s = TaskControllerServer::new(valid_config().with_version(4));
        s.start().unwrap();
        let out = s.handle_working_set_master(&Message::new(
            PGN_WORKING_SET_MASTER,
            vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            0x91,
        ));
        assert_eq!(s.clients().len(), 1);
        assert_eq!(s.get_client_version(0x91), 0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].pgn, PGN_TC_TO_ECU);
        assert_eq!(out[0].dest, Some(0x91));
        assert_eq!(
            out[0].data,
            [
                tc_cmd::VERSION_REQUEST,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
    }

    #[test]
    fn server_detects_client_loss_after_timeout() {
        use std::cell::RefCell;
        use std::rc::Rc;

        let mut s = TaskControllerServer::new(valid_config().with_version(4));
        s.start().unwrap();
        let wsm = |addr| {
            Message::new(
                PGN_WORKING_SET_MASTER,
                vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                addr,
            )
        };
        s.handle_working_set_master(&wsm(0x91));
        assert_eq!(s.clients().len(), 1);

        let lost: Rc<RefCell<Vec<Address>>> = Rc::new(RefCell::new(Vec::new()));
        let sink = lost.clone();
        s.on_client_disconnected
            .subscribe(move |&a| sink.borrow_mut().push(a));

        // Below the timeout, then a keepalive message: the client persists.
        s.update(TC_CLIENT_TIMEOUT_MS - 1);
        assert_eq!(s.clients().len(), 1);
        s.handle_working_set_master(&wsm(0x91)); // keepalive resets the timer
        s.update(TC_CLIENT_TIMEOUT_MS - 1);
        assert_eq!(s.clients().len(), 1, "keepalive must keep the client");

        // Silence past the full timeout drops the client + fires the event.
        s.update(TC_CLIENT_TIMEOUT_MS);
        assert!(s.clients().is_empty());
        assert_eq!(&*lost.borrow(), &[0x91]);
    }

    #[test]
    fn client_version_response_updates_tracked_version_and_event() {
        let mut s = TaskControllerServer::new(valid_config().with_version(4));
        s.start().unwrap();
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let seen_clone = seen.clone();
        s.on_client_version_received
            .subscribe(move |event| seen_clone.borrow_mut().push(*event));
        assert_eq!(
            s.handle_working_set_master(&Message::new(
                PGN_WORKING_SET_MASTER,
                vec![1, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                0x91,
            ))
            .len(),
            1
        );

        let out = s.handle_client_message(&ecu_msg(
            vec![0x10, 0x04, 0xFF, 0x1F, 0x00, 0x01, 0x20, 0x10],
            0x91,
        ));
        assert!(out.is_empty());
        assert_eq!(s.get_client_version(0x91), 4);
        assert_eq!(*seen.borrow(), vec![(0x91, 4)]);

        let out = s.request_client_version(0x91).unwrap();
        assert_eq!(out.dest, Some(0x91));
        assert_eq!(out.data[0], tc_cmd::VERSION_REQUEST);
    }

    #[test]
    fn label_requests_echo_configured_labels_and_create_client() {
        let mut s = TaskControllerServer::new(valid_config().with_version(4));
        s.start().unwrap();

        let no_structure = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::REQUEST_STRUCTURE_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(no_structure.len(), 1);
        assert_eq!(
            no_structure[0].data,
            [
                tc_cmd::STRUCTURE_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF
            ]
        );
        assert_eq!(s.clients().len(), 1);

        s.set_structure_label([1, 2, 3, 4, 5, 6, 7]);
        let structure = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::REQUEST_STRUCTURE_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(
            structure[0].data,
            [tc_cmd::STRUCTURE_LABEL, 1, 2, 3, 4, 5, 6, 7]
        );

        s.set_localization_label([8, 9, 10, 11, 12, 13, 14]);
        let localization = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::REQUEST_LOCALIZATION_LABEL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(
            localization[0].data,
            [tc_cmd::LOCALIZATION_LABEL, 8, 9, 10, 11, 12, 13, 14]
        );

        assert!(
            s.handle_client_message(&ecu_msg(vec![tc_cmd::REQUEST_STRUCTURE_LABEL], 0x43))
                .is_empty()
        );
        assert_eq!(s.clients().len(), 1);
    }

    #[test]
    fn malformed_tech_capabilities_request_does_not_create_client() {
        for payload in [
            vec![tc_cmd::VERSION_REQUEST],
            vec![
                tc_cmd::VERSION_REQUEST,
                0,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            vec![
                tc_cmd::VERSION_RESPONSE,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
        ] {
            let mut s = TaskControllerServer::new(valid_config().with_version(4));
            s.start().unwrap();
            assert!(s.handle_client_message(&ecu_msg(payload, 0x42)).is_empty());
            assert!(s.clients().is_empty());
            assert_eq!(s.state(), TCServerState::WaitForClients);
        }
    }

    #[test]
    fn inbound_messages_require_ecu_to_tc_pgn_and_valid_source() {
        let request = vec![
            tc_cmd::VERSION_REQUEST,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ];

        let mut wrong_pgn_server = TaskControllerServer::new(valid_config());
        wrong_pgn_server.start().unwrap();
        assert!(
            wrong_pgn_server
                .handle_client_message(&Message::new(PGN_TC_TO_ECU, request.clone(), 0x42))
                .is_empty()
        );
        assert!(wrong_pgn_server.clients().is_empty());
        assert_eq!(wrong_pgn_server.state(), TCServerState::WaitForClients);

        for bad_source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut server = TaskControllerServer::new(valid_config());
            server.start().unwrap();
            assert!(
                server
                    .handle_client_message(&ecu_msg(request.clone(), bad_source))
                    .is_empty()
            );
            assert!(server.clients().is_empty());
            assert_eq!(server.state(), TCServerState::WaitForClients);
        }
    }

    #[test]
    fn try_handle_client_message_reports_envelope_errors_without_state_mutation() {
        let request = vec![
            tc_cmd::VERSION_REQUEST,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ];
        let mut server = TaskControllerServer::new(valid_config());
        server.start().unwrap();

        let err = server
            .try_handle_client_message(&Message::new(PGN_TC_TO_ECU, request.clone(), 0x42))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidPgn);
        assert!(server.clients().is_empty());
        assert_eq!(server.state(), TCServerState::WaitForClients);

        let err = server
            .try_handle_client_message(&ecu_msg(request.clone(), NULL_ADDRESS))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidAddress);
        assert!(server.clients().is_empty());

        let err = server
            .try_handle_client_message(&ecu_msg(Vec::new(), 0x42))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(server.clients().is_empty());

        let err = server
            .try_handle_client_message(&ecu_msg(vec![tc_cmd::VERSION_REQUEST, 0x00], 0x42))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(server.clients().is_empty());

        let out = server
            .try_handle_client_message(&ecu_msg(request, 0x42))
            .unwrap();
        assert!(!out.is_empty());
        assert!(server.clients().iter().any(|client| client.address == 0x42));
    }

    #[test]
    fn try_handle_working_set_master_reports_envelope_errors_without_registration() {
        let mut server = TaskControllerServer::new(valid_config());
        server.start().unwrap();
        let good = vec![0xFF; 8];

        let err = server
            .try_handle_working_set_master(&Message::new(PGN_ECU_TO_TC, good.clone(), 0x42))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidPgn);
        assert!(server.clients().is_empty());

        let err = server
            .try_handle_working_set_master(&Message::new(
                PGN_WORKING_SET_MASTER,
                good.clone(),
                0xFF,
            ))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidAddress);
        assert!(server.clients().is_empty());

        let err = server
            .try_handle_working_set_master(&Message::new(
                PGN_WORKING_SET_MASTER,
                vec![0xFF; 7],
                0x42,
            ))
            .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(server.clients().is_empty());

        let out = server
            .try_handle_working_set_master(&Message::new(PGN_WORKING_SET_MASTER, good, 0x42))
            .unwrap();
        assert!(!out.is_empty());
        assert!(server.clients().iter().any(|client| client.address == 0x42));
    }

    #[test]
    fn status_emitted_at_cadence() {
        let mut s = TaskControllerServer::new(valid_config().with_number(7));
        s.start().unwrap();
        assert!(s.update(TC_STATUS_INTERVAL_MS - 1).is_none());
        let bytes = s.update(2).unwrap();
        assert_eq!(bytes[0], 0xF0 | ProcessDataCommands::Status.as_u8());
        assert_eq!(bytes[1], 7);
    }

    #[test]
    fn status_tracks_command_busy_source_and_command_byte() {
        let mut s = TaskControllerServer::new(
            valid_config()
                .with_number(7)
                .with_options(0x11)
                .with_channels(16),
        );
        s.start().unwrap();

        let idle = s.update(TC_STATUS_INTERVAL_MS).unwrap();
        assert_eq!(idle[0], 0xFE);
        assert_eq!(idle[4] & 0x08, 0x00);
        assert_eq!(idle[5], 0x00);
        assert_eq!(idle[6], 0x00);
        assert_eq!(idle[7], 16);

        s.set_command_busy_for(0x88, tc_cmd::OBJECT_POOL_TRANSFER);
        let busy = s.update(TC_STATUS_INTERVAL_MS).unwrap();
        assert_ne!(busy[4] & 0x08, 0x00);
        assert_eq!(busy[5], 0x88);
        assert_eq!(busy[6], tc_cmd::OBJECT_POOL_TRANSFER);

        s.set_command_busy(false);
        let cleared = s.update(TC_STATUS_INTERVAL_MS).unwrap();
        assert_eq!(cleared[4] & 0x08, 0x00);
        assert_eq!(cleared[5], 0x00);
        assert_eq!(cleared[6], 0x00);
    }

    #[test]
    fn process_data_builders_reject_unencodable_element_numbers() {
        let too_large = MAX_PROCESS_DATA_ELEMENT_NUMBER + 1;
        assert!(TaskControllerServer::build_request_value(too_large, 0x1234).is_err());
        assert!(TaskControllerServer::build_set_value(too_large, 0x1234, 1).is_err());
        assert!(
            TaskControllerServer::build_set_value_and_acknowledge(too_large, 0x1234, 1).is_err()
        );
        assert!(
            TaskControllerServer::build_time_interval_measurement_command(too_large, 0x1234, 1)
                .is_err()
        );
        assert!(
            TaskControllerServer::build_distance_interval_measurement_command(too_large, 0x1234, 1)
                .is_err()
        );
        assert!(
            TaskControllerServer::build_minimum_threshold_measurement_command(too_large, 0x1234, 1)
                .is_err()
        );
        assert!(
            TaskControllerServer::build_maximum_threshold_measurement_command(too_large, 0x1234, 1)
                .is_err()
        );
        assert!(
            TaskControllerServer::build_change_threshold_measurement_command(too_large, 0x1234, 1)
                .is_err()
        );

        let payload =
            TaskControllerServer::build_request_value(MAX_PROCESS_DATA_ELEMENT_NUMBER, 0x1234)
                .unwrap();
        assert_eq!(payload[0] >> 4, 0x0F);
        assert_eq!(payload[1], 0xFF);
    }

    #[test]
    fn request_value_callback_responds() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.on_value_request(|_elem, _ddi, _addr| Ok(42));
        let req = TaskControllerServer::build_request_value(3, 0x1234).unwrap();
        let out = s.handle_client_message(&ecu_msg(req.to_vec(), 0x42));
        assert_eq!(out.len(), 1);
        let v = i32::from_le_bytes(out[0].data[4..8].try_into().unwrap());
        assert_eq!(v, 42);
    }

    #[test]
    fn request_value_callback_error_is_silent() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.on_value_request(|_, _, _| Err(crate::net::error::Error::invalid_state("offline")));
        let req = TaskControllerServer::build_request_value(3, 0x1234).unwrap();
        let out = s.handle_client_message(&ecu_msg(req.to_vec(), 0x42));
        assert!(out.is_empty());
    }

    #[test]
    fn measurement_trigger_with_initial_emits_baseline_request_value() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        let initial = s
            .configure_measurement_trigger_with_initial(
                MeasurementTriggerRuntime::new(0x42, 7, 0x1234).with_time_interval_ms(100),
            )
            .unwrap();
        // The mandatory initial value request goes out immediately.
        assert_eq!(initial.dest, Some(0x42));
        assert_eq!(
            initial.data,
            TaskControllerServer::build_request_value(7, 0x1234)
                .unwrap()
                .to_vec()
        );
        // The trigger is registered (no double-emit before the interval elapses).
        assert_eq!(s.measurement_triggers().len(), 1);
        assert!(s.update_measurements(99).is_empty());
    }

    #[test]
    fn measurement_runtime_emits_time_and_distance_request_value_commands() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.configure_measurement_trigger(
            MeasurementTriggerRuntime::new(0x42, 7, 0x1234)
                .with_time_interval_ms(100)
                .with_distance_interval_mm(250),
        )
        .unwrap();

        assert!(s.update_measurements(99).is_empty());
        let time_due = s.update_measurements(1);
        assert_eq!(time_due.len(), 1);
        assert_eq!(time_due[0].dest, Some(0x42));
        assert_eq!(
            time_due[0].data,
            TaskControllerServer::build_request_value(7, 0x1234)
                .unwrap()
                .to_vec()
        );

        assert!(s.record_measurement_distance(0x42, 249).is_empty());
        let distance_due = s.record_measurement_distance(0x42, 1);
        assert_eq!(distance_due.len(), 1);
        assert_eq!(
            distance_due[0].data,
            TaskControllerServer::build_request_value(7, 0x1234)
                .unwrap()
                .to_vec()
        );
    }

    #[test]
    fn measurement_runtime_emits_threshold_and_change_request_value_commands() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.configure_measurement_trigger(
            MeasurementTriggerRuntime::new(0x42, 5, 0xCAFE)
                .with_trigger(crate::isobus::tc::TriggerMethod::OnChange)
                .with_minimum_threshold(-10)
                .with_maximum_threshold(100)
                .with_change_threshold(25),
        )
        .unwrap();

        let mk_value = |value: i32| {
            TaskControllerServer::build_set_value(5, 0xCAFE, value)
                .unwrap()
                .to_vec()
        };
        assert!(
            s.handle_client_message(&ecu_msg(mk_value(0), 0x42))
                .is_empty(),
            "initial value seeds the runtime without a synthetic change"
        );

        let changed = s.handle_client_message(&ecu_msg(mk_value(10), 0x42));
        assert_eq!(changed.len(), 1);
        assert_eq!(
            changed[0].data,
            TaskControllerServer::build_request_value(5, 0xCAFE)
                .unwrap()
                .to_vec()
        );

        let max_crossed = s.handle_client_message(&ecu_msg(mk_value(100), 0x42));
        assert_eq!(max_crossed.len(), 1);

        let min_crossed = s.handle_client_message(&ecu_msg(mk_value(-10), 0x42));
        assert_eq!(min_crossed.len(), 1);

        s.clear_measurement_trigger(0x42, 5, 0xCAFE);
        assert!(
            s.handle_client_message(&ecu_msg(mk_value(200), 0x42))
                .is_empty()
        );
    }

    #[test]
    fn value_callback_receives_decoded_fields() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        let log: Rc<RefCell<Vec<(ElementNumber, DDI, i32)>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        s.on_value_received(move |elem, ddi, value, _addr| {
            lc.borrow_mut().push((elem, ddi, value));
            Ok(ProcessDataAcknowledgeErrorCodes::NoError)
        });
        let payload = TaskControllerServer::build_set_value(5, 0xCAFE, 0x1234_5678).unwrap();
        s.handle_client_message(&ecu_msg(payload.to_vec(), 0x42));
        assert_eq!(
            *log.borrow(),
            vec![(ElementNumber(5), DDI(0xCAFE), 0x1234_5678i32)]
        );
    }

    #[test]
    fn set_value_and_acknowledge_uses_callback_error_code() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.on_value_received(|_, _, _, _| {
            Ok(ProcessDataAcknowledgeErrorCodes::ValueIsOutsideValidRange)
        });
        let payload = TaskControllerServer::build_set_value_and_acknowledge(5, 0xCAFE, 42).unwrap();
        let out = s.handle_client_message(&ecu_msg(payload.to_vec(), 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].data[0] & 0x0F,
            ProcessDataCommands::Acknowledge.as_u8()
        );
        assert_eq!(out[0].data[0] >> 4, 5);
        assert_eq!(
            out[0].data[4],
            ProcessDataAcknowledgeErrorCodes::ValueIsOutsideValidRange.as_u8()
        );
    }

    #[test]
    fn set_value_and_acknowledge_callback_error_maps_no_resources() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.on_value_received(|_, _, _, _| Err(crate::net::error::Error::invalid_state("busy")));
        let payload = TaskControllerServer::build_set_value_and_acknowledge(5, 0xCAFE, 42).unwrap();
        let out = s.handle_client_message(&ecu_msg(payload.to_vec(), 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].data[4],
            ProcessDataAcknowledgeErrorCodes::NoProcessingResourcesAvailable.as_u8()
        );
    }

    #[test]
    fn peer_control_acknowledges() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.on_peer_control_assignment(|_, _, _, _| Ok(()));
        let payload = PeerControlAssignment::default()
            .from(0x0123, 0xCAFE)
            .to(0x0456, 0x0BAD)
            .with_source(0x42)
            .with_destination(0x80)
            .try_encode()
            .unwrap();
        let mut msg = ecu_msg(payload.to_vec(), 0x42);
        msg.destination = 0x80;
        let out = s.handle_client_message(&msg);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].data[0] & 0x0F,
            ProcessDataCommands::Acknowledge.as_u8()
        );
        assert_eq!(out[0].data[0] >> 4, 0x03);
        assert_eq!(out[0].data[1], 0x12);
        assert_eq!(out[0].data[2..4], [0xFE, 0xCA]);
        assert_eq!(out[0].data[4], 0x00);
    }

    #[test]
    fn malformed_peer_control_payloads_do_not_invoke_callback_or_ack() {
        use std::cell::Cell;
        use std::rc::Rc;

        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        let calls = Rc::new(Cell::new(0u32));
        let calls_cb = calls.clone();
        s.on_peer_control_assignment(move |_, _, _, _| {
            calls_cb.set(calls_cb.get() + 1);
            Ok(())
        });

        let valid = PeerControlAssignment::default()
            .from(1, 0x1234)
            .to(2, 0x5678)
            .with_source(0x42)
            .with_destination(0x80)
            .try_encode()
            .unwrap();
        let mut wrong_command = valid;
        wrong_command[0] = (wrong_command[0] & 0xF0) | ProcessDataCommands::Value.as_u8();

        for payload in [
            valid[..7].to_vec(),
            {
                let mut overlong = valid.to_vec();
                overlong.push(0xFF);
                overlong
            },
            wrong_command.to_vec(),
        ] {
            assert!(s.handle_client_message(&ecu_msg(payload, 0x42)).is_empty());
        }
        assert!(
            s.handle_client_message(&ecu_msg(valid.to_vec(), NULL_ADDRESS))
                .is_empty()
        );
        assert!(
            s.handle_client_message(&ecu_msg(valid.to_vec(), BROADCAST_ADDRESS))
                .is_empty()
        );
        assert_eq!(calls.get(), 0);
    }

    fn dummy_ddop() -> DDOP {
        DDOP::default()
            .with_device(DeviceObject::default().with_id(1).with_designator("D"))
            .with_element(DeviceElement::default().with_id(2))
    }

    fn dummy_ddop_transfer() -> Vec<u8> {
        let mut transfer = vec![tc_cmd::OBJECT_POOL_TRANSFER];
        transfer.extend_from_slice(&dummy_ddop().serialize().unwrap());
        transfer
    }

    fn activate_pool_request() -> Vec<u8> {
        vec![
            tc_cmd::ACTIVATE_POOL,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ]
    }

    #[test]
    fn object_pool_transfer_and_activation_store_client_ddop() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();

        let transfer = dummy_ddop_transfer();
        let out = s.handle_client_message(&ecu_msg(transfer.clone(), 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::OBJECT_POOL_RESPONSE);
        assert_eq!(out[0].data[1], ObjectPoolErrorCodes::NoErrors.as_u8());
        assert_eq!(s.clients().len(), 1);
        assert_eq!(s.clients()[0].ddop.object_count(), 2);
        assert_eq!(s.clients()[0].last_ddop_transfer, transfer[1..]);
        assert!(!s.clients()[0].pool_activated);

        let out = s.handle_client_message(&ecu_msg(activate_pool_request(), 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::ACTIVATE_RESPONSE);
        assert_eq!(out[0].data[1], ObjectPoolActivationError::NoErrors.as_u8());
        assert!(s.clients()[0].pool_activated);
    }

    #[test]
    fn object_pool_deactivate_and_delete_clear_server_state() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();

        let transfer = dummy_ddop_transfer();
        let out = s.handle_client_message(&ecu_msg(transfer, 0x42));
        assert_eq!(out[0].data[1], ObjectPoolErrorCodes::NoErrors.as_u8());
        let out = s.handle_client_message(&ecu_msg(activate_pool_request(), 0x42));
        assert_eq!(out[0].data[1], ObjectPoolActivationError::NoErrors.as_u8());
        assert!(s.clients()[0].pool_activated);

        let out = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::ACTIVATE_POOL,
                0x00,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::ACTIVATE_RESPONSE);
        assert_eq!(out[0].data[1], ObjectPoolActivationError::NoErrors.as_u8());
        assert_eq!(s.clients()[0].ddop.object_count(), 2);
        assert!(!s.clients()[0].pool_activated);

        let out = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::DELETE_POOL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::DELETE_POOL_RESPONSE);
        assert_eq!(
            out[0].data[1],
            ObjectPoolDeletionErrors::ErrorDetailsNotAvailable.as_u8()
        );
        assert!(s.clients()[0].ddop.devices().is_empty());
        assert!(!s.clients()[0].pool_activated);
        assert!(s.clients()[0].last_ddop_transfer.is_empty());
    }

    #[test]
    fn invalid_object_pool_transfer_returns_error_without_activation() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();

        let out = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::OBJECT_POOL_TRANSFER,
                0xFF,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
            ],
            0x42,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::OBJECT_POOL_RESPONSE);
        assert_eq!(out[0].data[1], ObjectPoolErrorCodes::AnyOtherError.as_u8());
        assert!(s.clients().is_empty());
    }

    #[test]
    fn short_object_pool_transfer_returns_error_without_client_state() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();

        for payload in [
            vec![tc_cmd::OBJECT_POOL_TRANSFER],
            vec![
                tc_cmd::OBJECT_POOL_TRANSFER,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
            ],
        ] {
            let out = s.handle_client_message(&ecu_msg(payload, 0x42));
            assert_eq!(out.len(), 1);
            assert_eq!(out[0].data[0], tc_cmd::OBJECT_POOL_RESPONSE);
            assert_eq!(out[0].data[1], ObjectPoolErrorCodes::AnyOtherError.as_u8());
            assert!(s.clients().is_empty());
        }
    }

    #[test]
    fn duplicate_object_pool_transfer_is_idempotent_after_activation() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();

        let transfer = dummy_ddop_transfer();
        let out = s.handle_client_message(&ecu_msg(transfer.clone(), 0x42));
        assert_eq!(out[0].data[1], ObjectPoolErrorCodes::NoErrors.as_u8());
        let out = s.handle_client_message(&ecu_msg(activate_pool_request(), 0x42));
        assert_eq!(out[0].data[1], ObjectPoolActivationError::NoErrors.as_u8());
        assert!(s.clients()[0].pool_activated);

        let out = s.handle_client_message(&ecu_msg(transfer.clone(), 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::OBJECT_POOL_RESPONSE);
        assert_eq!(out[0].data[1], ObjectPoolErrorCodes::NoErrors.as_u8());
        assert_eq!(s.clients()[0].ddop.object_count(), 2);
        assert_eq!(s.clients()[0].last_ddop_transfer, transfer[1..]);
        assert!(
            s.clients()[0].pool_activated,
            "identical re-transfer must not deactivate an already activated pool"
        );

        let out = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::OBJECT_POOL_TRANSFER,
                0xFF,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
                0x00,
            ],
            0x42,
        ));
        assert_eq!(out[0].data[1], ObjectPoolErrorCodes::AnyOtherError.as_u8());
        assert_eq!(s.clients()[0].ddop.object_count(), 2);
        assert_eq!(s.clients()[0].last_ddop_transfer, transfer[1..]);
        assert!(
            s.clients()[0].pool_activated,
            "malformed transfer after activation must not corrupt or deactivate the current pool"
        );
    }

    #[test]
    fn activate_without_uploaded_pool_returns_activation_error() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        let errors: Rc<RefCell<Vec<ObjectPoolActivationError>>> = Rc::new(RefCell::new(Vec::new()));
        let errors_cb = errors.clone();
        s.on_pool_activation_error
            .subscribe(move |&err| errors_cb.borrow_mut().push(err));

        let out = s.handle_client_message(&ecu_msg(
            vec![
                tc_cmd::ACTIVATE_POOL,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x42,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], tc_cmd::ACTIVATE_RESPONSE);
        assert_eq!(
            out[0].data[1],
            ObjectPoolActivationError::ThereAreErrorsInTheDDOP.as_u8()
        );
        assert!(
            s.clients().is_empty(),
            "activation before any TC client/DDOP state must not allocate a client slot"
        );
        assert_eq!(
            *errors.borrow(),
            vec![ObjectPoolActivationError::ThereAreErrorsInTheDDOP]
        );
    }

    #[test]
    fn malformed_object_pool_lifecycle_requests_are_ignored() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();

        for payload in [
            vec![tc_cmd::ACTIVATE_POOL],
            vec![tc_cmd::ACTIVATE_POOL, 0x00],
            vec![tc_cmd::DELETE_POOL],
            vec![
                tc_cmd::ACTIVATE_POOL,
                0xFF,
                0xFF,
                0xFF,
                0x00,
                0xFF,
                0xFF,
                0xFF,
            ],
            vec![
                tc_cmd::ACTIVATE_POOL,
                0x00,
                0xFF,
                0xFF,
                0x00,
                0xFF,
                0xFF,
                0xFF,
            ],
            vec![
                tc_cmd::DELETE_POOL,
                0xFF,
                0xFF,
                0xFF,
                0x00,
                0xFF,
                0xFF,
                0xFF,
            ],
        ] {
            assert!(
                s.handle_client_message(&ecu_msg(payload, 0x42)).is_empty(),
                "malformed object-pool lifecycle request must not emit a response"
            );
            assert!(
                s.clients().is_empty(),
                "malformed lifecycle request must not create a client entry"
            );
        }
    }

    #[test]
    fn process_data_collisions_are_not_misread_as_pool_commands() {
        let mut s = TaskControllerServer::new(valid_config());
        s.start().unwrap();
        s.on_value_request(|_, _, _| Ok(42));
        let out = s.handle_client_message(&ecu_msg(
            TaskControllerServer::build_request_value(2, 0x1234)
                .unwrap()
                .to_vec(),
            0x42,
        ));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0] & 0x0F, ProcessDataCommands::Value.as_u8());
    }
}
