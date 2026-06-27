#[test]
fn fixture_isobus_niu_router_reassembles_routed_tp_etp_sessions() {
    let raw_id =
        |name: &str| parse_hex_u64(parse_named_text_value(ISOBUS_NIU_CONTROL_HEX, name)) as u32;

    let mut router = Router::new(NiuConfig::default());
    router.niu_mut().start().unwrap();
    let tractor_cf = Name::default().with_identity_number(0x100);
    let implement_cf = Name::default().with_identity_number(0x200);
    router.add_translation(tractor_cf, 0x10, 0x20).unwrap();
    router.add_translation(implement_cf, 0x42, 0x52).unwrap();

    // TP.CMDT: 0x10 -> 0x42 on the tractor bus must become 0x20 -> 0x52
    // on the implement bus, while the reverse CTS/EoMA control frames must
    // translate back so the sender can finish normally.
    let tp_payload = incrementing_payload(20);
    let mut tp_tx = TransportProtocol::new();
    let mut tp_rx = TransportProtocol::new();
    let tp_completed = Rc::new(RefCell::new(Vec::new()));
    let tp_observed = Rc::clone(&tp_completed);
    tp_rx
        .on_complete
        .subscribe(move |session| tp_observed.borrow_mut().push(session.clone()));

    let rts = tp_tx
        .send(
            PGN_PROPRIETARY_A,
            &tp_payload,
            0x10,
            0x42,
            0,
            Priority::Default,
        )
        .expect("TP send starts before routing");
    assert_eq!(rts.len(), 1);
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);
    assert_eq!(rts[0].id.raw, raw_id("router_tp_cm_low_src10_dst42_raw_id"));

    let routed_rts = router
        .process_frame(rts[0], Side::Tractor, 0)
        .expect("router forwards translated TP RTS");
    assert_eq!(
        routed_rts.id.raw,
        raw_id("router_tp_cm_low_translated_src20_dst52_raw_id")
    );
    assert_eq!(routed_rts.payload(), TP_RTS_20B_PGN_EF00.as_slice());

    let cts = tp_rx.process_frame(&routed_rts, 1);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].source(), 0x52);
    assert_eq!(cts[0].destination(), 0x20);
    let routed_cts = router
        .process_frame(cts[0], Side::Implement, 1)
        .expect("router translates TP CTS back to tractor side");
    assert_eq!(
        routed_cts.id.raw,
        raw_id("router_tp_cm_low_translated_src42_dst10_raw_id")
    );
    assert_eq!(routed_cts.payload(), TP_CTS_20B_PGN_EF00.as_slice());
    assert!(tp_tx.process_frame(&routed_cts, 0).is_empty());

    let dt_frames = tp_tx.get_pending_data_frames();
    assert_eq!(dt_frames.len(), 3);
    let mut tp_eoma = None;
    for (idx, frame) in dt_frames.iter().enumerate() {
        assert_eq!(frame.destination(), 0x42);
        if idx == 0 {
            assert_eq!(frame.id.raw, raw_id("router_tp_dt_low_src10_dst42_raw_id"));
        }
        let routed_dt = router
            .process_frame(*frame, Side::Tractor, 10 + idx as u32)
            .expect("router forwards TP DT");
        if idx == 0 {
            assert_eq!(
                routed_dt.id.raw,
                raw_id("router_tp_dt_low_translated_src20_dst52_raw_id")
            );
            assert_eq!(routed_dt.payload(), TP_DT_SEQ1_20B_PAYLOAD.as_slice());
        }
        for response in tp_rx.process_frame(&routed_dt, 1) {
            assert_eq!(
                response.id.raw,
                raw_id("router_tp_cm_low_impl_src52_dst20_raw_id")
            );
            assert_eq!(response.payload(), TP_EOMA_20B_PGN_EF00.as_slice());
            let routed_response = router
                .process_frame(response, Side::Implement, 20)
                .expect("router translates TP EoMA back to sender");
            assert_eq!(
                routed_response.id.raw,
                raw_id("router_tp_cm_low_translated_src42_dst10_raw_id")
            );
            tp_eoma = Some(routed_response);
        }
    }
    let tp_eoma = tp_eoma.expect("TP receiver emits EoMA through router");
    assert!(tp_tx.process_frame(&tp_eoma, 0).is_empty());
    assert!(tp_tx.active_sessions().is_empty());
    assert!(tp_rx.active_sessions().is_empty());
    {
        let completed = tp_completed.borrow();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].pgn, PGN_PROPRIETARY_A);
        assert_eq!(completed[0].source_address, 0x20);
        assert_eq!(completed[0].destination_address, 0x52);
        assert_eq!(completed[0].data, tp_payload);
    }

    // ETP: prove the same translation survives the RTS/CTS/DPO/DT/EoMA loop
    // for a payload that is larger than classic TP.
    let etp_payload = incrementing_payload(2000);
    let mut etp_tx = ExtendedTransportProtocol::new();
    let mut etp_rx = ExtendedTransportProtocol::new();
    let etp_completed = Rc::new(RefCell::new(Vec::new()));
    let etp_observed = Rc::clone(&etp_completed);
    etp_rx
        .on_complete
        .subscribe(move |session| etp_observed.borrow_mut().push(session.clone()));

    let etp_rts = etp_tx
        .send(PGN_TRANSFER, &etp_payload, 0x10, 0x42, 0, Priority::Default)
        .expect("ETP send starts before routing");
    assert_eq!(etp_rts.len(), 1);
    assert_eq!(etp_rts[0].data, *ETP_RTS_2000B_PGN_CA00);
    assert_eq!(
        etp_rts[0].id.raw,
        raw_id("router_etp_cm_low_src10_dst42_raw_id")
    );
    let routed_etp_rts = router
        .process_frame(etp_rts[0], Side::Tractor, 100)
        .expect("router forwards translated ETP RTS");
    assert_eq!(
        routed_etp_rts.id.raw,
        raw_id("router_etp_cm_low_translated_src20_dst52_raw_id")
    );

    let mut to_etp_tx = etp_rx.process_frame(&routed_etp_rts, 1);
    assert_eq!(to_etp_tx.len(), 1);
    assert_eq!(
        to_etp_tx[0].id.raw,
        raw_id("router_etp_cm_low_impl_src52_dst20_raw_id")
    );
    assert_eq!(to_etp_tx[0].payload(), ETP_CTS_2000B_PGN_CA00.as_slice());

    let mut etp_eoma = None;
    for window in 0..32 {
        for response in to_etp_tx.drain(..) {
            assert_eq!(
                response.id.raw,
                raw_id("router_etp_cm_low_impl_src52_dst20_raw_id")
            );
            let routed_response = router
                .process_frame(response, Side::Implement, 110 + window)
                .expect("router translates ETP control frame back to sender");
            assert_eq!(
                routed_response.id.raw,
                raw_id("router_etp_cm_low_translated_src42_dst10_raw_id")
            );
            if routed_response.data[0] == 0x17 {
                assert_eq!(
                    routed_response.payload(),
                    ETP_EOMA_2000B_PGN_CA00.as_slice()
                );
                etp_eoma = Some(routed_response);
            } else {
                assert!(etp_tx.process_frame(&routed_response, 0).is_empty());
            }
        }
        if etp_eoma.is_some() {
            break;
        }

        let pending = etp_tx.get_pending_data_frames();
        assert!(!pending.is_empty(), "ETP sender stalled before routed EoMA");
        for frame in &pending {
            let routed_frame = router
                .process_frame(*frame, Side::Tractor, 150 + window)
                .expect("router forwards ETP DPO/DT frame");
            match frame.pgn() {
                PGN_ETP_CM => {
                    assert_eq!(frame.id.raw, raw_id("router_etp_cm_low_src10_dst42_raw_id"));
                    assert_eq!(
                        routed_frame.id.raw,
                        raw_id("router_etp_cm_low_translated_src20_dst52_raw_id")
                    );
                }
                PGN_ETP_DT => {
                    assert_eq!(frame.id.raw, raw_id("router_etp_dt_low_src10_dst42_raw_id"));
                    assert_eq!(
                        routed_frame.id.raw,
                        raw_id("router_etp_dt_low_translated_src20_dst52_raw_id")
                    );
                }
                other => panic!("unexpected ETP routed PGN {other:#X}"),
            }
            for response in etp_rx.process_frame(&routed_frame, 1) {
                to_etp_tx.push(response);
            }
        }
    }

    let etp_eoma = etp_eoma.expect("ETP receiver emits EoMA through router");
    assert!(etp_tx.process_frame(&etp_eoma, 0).is_empty());
    assert!(etp_tx.active_sessions().is_empty());
    assert!(etp_rx.active_sessions().is_empty());
    {
        let completed = etp_completed.borrow();
        assert_eq!(completed.len(), 1);
        assert_eq!(completed[0].pgn, PGN_TRANSFER);
        assert_eq!(completed[0].source_address, 0x20);
        assert_eq!(completed[0].destination_address, 0x52);
        assert_eq!(completed[0].data, etp_payload);
    }
}

#[test]
fn fixture_isobus_vt_object_pool_children_and_invalid_graphs_are_stable() {
    let valid = parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "valid_ws_datamask");
    let pool = VTObjectPool::deserialize(&valid).unwrap();
    assert_eq!(pool.serialize().unwrap(), valid);
    assert_eq!(pool.size(), 2);

    let ws = pool.find(1).unwrap();
    assert_eq!(ws.r#type, VTObjectType::WorkingSet);
    // Working Set fixed body is 4 bytes: background, selectable, active mask.
    assert_eq!(ws.body, vec![0, 0, 0, 0]);
    assert_eq!(
        ws.children.iter().map(|id| id.raw()).collect::<Vec<_>>(),
        vec![2]
    );
    assert_eq!(pool.find(2).unwrap().r#type, VTObjectType::DataMask);
    pool.validate().unwrap();

    let key = parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "key_fixed_body_child_tail");
    let pool = VTObjectPool::deserialize(&key).unwrap();
    assert_eq!(pool.serialize().unwrap(), key);
    let key = pool.find(7).unwrap();
    assert_eq!(key.r#type, VTObjectType::Key);
    assert_eq!(key.body, vec![0xAA, 2]);
    assert_eq!(
        key.children.iter().map(|id| id.raw()).collect::<Vec<_>>(),
        vec![8]
    );

    let missing_child =
        parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "missing_child_ws_datamask");
    let pool = VTObjectPool::deserialize(&missing_child).unwrap();
    assert!(pool.validate().is_err());

    let duplicate = parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "duplicate_object_id");
    assert!(VTObjectPool::deserialize(&duplicate).is_err());

    let unknown_type = parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "unknown_object_type");
    assert!(VTObjectPool::deserialize(&unknown_type).is_err());

    let malformed_child_tail =
        parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "malformed_key_child_tail");
    assert!(VTObjectPool::deserialize(&malformed_child_tail).is_err());
}

#[test]
fn fixture_isobus_vt_server_working_set_storage_format_is_stable() {
    let dir = temp_fixture_dir("vt_sws");
    let mut sws = ServerWorkingSet {
        client_address: 0x42,
        storage_path: dir.clone(),
        ..Default::default()
    };
    let pool_data = parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "valid_ws_datamask");
    let stored = StoredPoolVersion {
        label: "V1".into(),
        pool_data: pool_data.clone(),
        timestamp_us: 0x0102_0304_0506_0708,
        size_bytes: pool_data.len() as u32,
        vt_version: 5,
        object_count: 2,
    };

    assert!(sws.save_version_to_disk(&stored));
    let stored_path = sws.get_client_storage_dir().join("5631.vtp");
    assert_eq!(
        fs::read(&stored_path).unwrap(),
        parse_named_hex_bytes(ISOBUS_VT_SERVER_WORKING_SET_HEX, "stored_v1_label_v1")
    );

    let loaded = sws.load_version_from_disk("V1").unwrap();
    assert_eq!(loaded, stored);
    assert!(sws.load_version("V1"));
    assert!(sws.pool_uploaded);
    assert!(sws.pool_activated);
    assert_eq!(sws.pool.serialize().unwrap(), pool_data);

    fs::write(
        &stored_path,
        parse_named_hex_bytes(
            ISOBUS_VT_SERVER_WORKING_SET_HEX,
            "stored_v1_header_label_v2",
        ),
    )
    .unwrap();
    assert!(
        sws.load_version_from_disk("V1").is_none(),
        "filename and stored header label must agree"
    );

    fs::write(
        &stored_path,
        parse_named_hex_bytes(
            ISOBUS_VT_SERVER_WORKING_SET_HEX,
            "stored_v1_oversized_header",
        ),
    )
    .unwrap();
    assert!(
        sws.load_version_from_disk("V1").is_none(),
        "oversized stored pools must be rejected before allocation"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fixture_isobus_iop_byte_walker_golden_corpus_is_stable() {
    let corpus = parse_named_hex_bytes(ISOBUS_IOP_PARSER_HEX, "container_and_number_variable");
    let objects = parse_iop_data(&corpus).unwrap();
    assert_eq!(objects.len(), 2);
    assert_eq!(objects[0].id, 1);
    assert_eq!(objects[0].type_byte, 3);
    // Container body: width(2) height(2) hidden(1) num_objects(1) num_macros(1).
    assert_eq!(objects[0].body, [100, 0, 200, 0, 0, 0, 0]);
    assert_eq!(objects[1].id, 2);
    assert_eq!(objects[1].type_byte, 21);
    // NumberVariable body: the 4-byte value (no header width/height).
    assert_eq!(objects[1].body, [0x0D, 0xF0, 0xFE, 0xCA]);
    assert!(validate(&corpus));
    assert_eq!(
        hash_to_version(&corpus),
        parse_named_text_value(
            ISOBUS_IOP_PARSER_HEX,
            "container_and_number_variable_version"
        )
    );

    let output = parse_iop_data(&parse_named_hex_bytes(
        ISOBUS_IOP_PARSER_HEX,
        "output_string_hello",
    ))
    .unwrap();
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].id, 0x10);
    assert_eq!(output[0].type_byte, 11);
    // OutputString body: 13 fixed bytes, the 5-byte value, then zero macros.
    assert_eq!(output[0].body.len(), 19);
    assert_eq!(&output[0].body[13..18], b"Hello");

    let macro_obj = parse_iop_data(&parse_named_hex_bytes(
        ISOBUS_IOP_PARSER_HEX,
        "macro_three_bytes",
    ))
    .unwrap();
    assert_eq!(macro_obj.len(), 1);
    assert_eq!(macro_obj[0].id, 0x20);
    assert_eq!(macro_obj[0].type_byte, 28);
    // Macro body: num_bytes(2) then the 3 raw command bytes.
    assert_eq!(macro_obj[0].body, [3, 0, 0xAA, 0xBB, 0xCC]);

    for malformed in [
        "too_short_header",
        "trailing_partial_header",
        "truncated_number_variable",
    ] {
        let data = parse_named_hex_bytes(ISOBUS_IOP_PARSER_HEX, malformed);
        assert!(parse_iop_data(&data).is_err(), "{malformed} must reject");
        assert!(!validate(&data), "{malformed} must not validate");
    }
}

#[test]
fn fixture_isobus_vt_v6_helper_codecs_are_stable() {
    let gesture_bytes =
        parse_named_hex_bytes(ISOBUS_VT_V6_HELPERS_HEX, "touch_gesture_tap_neg100_200");
    assert_eq!(
        TouchGesture {
            r#type: GestureType::Tap,
            x: -100,
            y: 200,
            duration_ms: 0,
            distance: 0,
            scale: 1.0,
            rotation_deg: 0.0,
            touch_count: 1,
            target_object: VTObjectID(0xCAFE),
        }
        .encode(),
        gesture_bytes
    );
    let decoded_gesture =
        TouchGesture::decode(&gesture_bytes).expect("valid touch gesture fixture must decode");
    assert_eq!(decoded_gesture.r#type, GestureType::Tap);
    assert_eq!(decoded_gesture.x, -100);
    assert_eq!(decoded_gesture.y, 200);
    assert_eq!(decoded_gesture.target_object, VTObjectID(0xCAFE));

    let graphics_bytes =
        parse_named_hex_bytes(ISOBUS_VT_V6_HELPERS_HEX, "graphics_context_v6_basic");
    assert_eq!(
        GraphicsContextV6 {
            transparency: 200,
            line_style: 1,
            line_width: 5,
            fill_color_rgb: 0x12_34_56,
            line_color_rgb: 0x78_9A_BC,
            anti_aliasing: true,
            blend_mode: 2,
        }
        .encode(),
        graphics_bytes
    );
    let decoded_graphics = GraphicsContextV6::decode(&graphics_bytes)
        .expect("valid graphics-context fixture must decode");
    assert_eq!(decoded_graphics.transparency, 200);
    assert_eq!(decoded_graphics.line_style, 1);
    assert_eq!(decoded_graphics.line_width, 5);
    assert_eq!(decoded_graphics.fill_color_rgb, 0x12_34_56);
    assert_eq!(decoded_graphics.line_color_rgb, 0x78_9A_BC);
    assert!(decoded_graphics.anti_aliasing);
    assert_eq!(decoded_graphics.blend_mode, 2);

    for fixture in [
        "touch_gesture_bad_type",
        "touch_gesture_short11",
        "touch_gesture_overlong13",
    ] {
        assert!(
            TouchGesture::decode(&parse_named_hex_bytes(ISOBUS_VT_V6_HELPERS_HEX, fixture))
                .is_none(),
            "{fixture} must be rejected"
        );
    }

    for fixture in [
        "graphics_context_v6_bad_line_style",
        "graphics_context_v6_bad_antialias",
        "graphics_context_v6_bad_blend",
        "graphics_context_v6_short11",
    ] {
        assert!(
            GraphicsContextV6::decode(&parse_named_hex_bytes(ISOBUS_VT_V6_HELPERS_HEX, fixture))
                .is_none(),
            "{fixture} must be rejected"
        );
    }
}

#[test]
fn fixture_isobus_vt_auxiliary_capability_discovery_is_stable() {
    let request = parse_named_hex_frame(ISOBUS_VT_AUX_CAPS_HEX, "aux_caps_request");
    let mut discovery = AuxCapabilityDiscovery::new();
    assert_eq!(discovery.request_capabilities().unwrap(), request);
    assert!(discovery.is_request_pending());
    assert!(discovery.request_capabilities().is_err());

    let response = parse_named_hex_bytes(ISOBUS_VT_AUX_CAPS_HEX, "aux_caps_two_channel_response");
    let caps = discovery
        .handle_response(&Message::new(PGN_VT_TO_ECU, response.clone(), 0x80))
        .expect("fixture response must decode");
    assert_eq!(caps.vt_version, 5);
    assert!(caps.discovery_complete);
    assert_eq!(
        caps.channels,
        vec![
            AuxChannelCapability {
                channel_id: 1,
                aux_type: 0,
                resolution: 0,
                function_type: 1,
            },
            AuxChannelCapability {
                channel_id: 2,
                aux_type: 1,
                resolution: 1024,
                function_type: 2,
            },
        ]
    );
    assert!(!discovery.is_request_pending());

    let mut truncated = AuxCapabilityDiscovery::new();
    truncated.request_capabilities().unwrap();
    assert!(
        truncated
            .handle_response(&Message::new(
                PGN_VT_TO_ECU,
                response[..response.len() - 1].to_vec(),
                0x80,
            ))
            .is_none()
    );
    assert!(truncated.is_request_pending());

    let mut trailing = AuxCapabilityDiscovery::new();
    trailing.request_capabilities().unwrap();
    assert!(
        trailing
            .handle_response(&Message::new(
                PGN_VT_TO_ECU,
                [response.as_slice(), &[0xFF]].concat(),
                0x80,
            ))
            .is_none()
    );
    assert!(trailing.is_request_pending());

    let mut wrong_subfunction = AuxCapabilityDiscovery::new();
    wrong_subfunction.request_capabilities().unwrap();
    let mut wrong_response = response;
    wrong_response[1] = 0x02;
    assert!(
        wrong_subfunction
            .handle_response(&Message::new(PGN_VT_TO_ECU, wrong_response, 0x80))
            .is_none()
    );
    assert!(wrong_subfunction.is_request_pending());
}

#[test]
fn fixture_isobus_vt_command_responses_and_client_server_flow_are_stable() {
    let pool_bytes = parse_named_hex_bytes(ISOBUS_VT_OBJECT_POOL_HEX, "valid_ws_datamask");
    let pool = VTObjectPool::deserialize(&pool_bytes).expect("VT pool fixture decodes");
    let mut client = VTClient::new(VTClientConfig::default());
    client.set_self_address(0x42);
    client.set_object_pool(pool);
    client.connect().unwrap();

    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();

    for fixture in [
        "object_pool_transfer_empty",
        "object_pool_transfer_partial_header",
        "object_pool_transfer_unknown_type",
    ] {
        let mut bad_server = VTServer::new(VTServerConfig::default());
        bad_server.start().unwrap();
        assert!(
            bad_server
                .handle_ecu_message(&Message::with_addressing(
                    PGN_ECU_TO_VT,
                    parse_named_hex_bytes(ISOBUS_VT_COMMANDS_HEX, fixture),
                    0x55,
                    0x80,
                    Priority::Default,
                ))
                .is_empty()
        );
        assert!(
            bad_server.clients().is_empty(),
            "malformed fixture {fixture} must not create VT client state"
        );
        assert_eq!(bad_server.state(), VTServerState::WaitForClientStatus);
    }

    let vt_status = parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "vt_status_vt5_no_active_ws");
    let status = server
        .update(VT_STATUS_INTERVAL_MS)
        .expect("server emits VT status at cadence");
    assert_eq!(status, vt_status);

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, status.to_vec(), 0x80));
    assert_eq!(client.state(), VTState::SendWorkingSetMaster);
    assert_eq!(client.vt_address(), 0x80);
    assert_eq!(client.vt_version_value(), 5);

    let working_set_master = client.update(1);
    assert_eq!(working_set_master.len(), 1);
    assert_eq!(working_set_master[0].pgn, PGN_WORKING_SET_MASTER);
    assert_eq!(working_set_master[0].dest, None);
    assert_eq!(
        working_set_master[0].data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "working_set_master_one_member")
    );
    assert_eq!(client.state(), VTState::SendGetMemory);

    let get_memory = client.update(1);
    assert_eq!(get_memory.len(), 1);
    assert_eq!(get_memory[0].pgn, PGN_ECU_TO_VT);
    assert_eq!(get_memory[0].dest, Some(0x80));
    assert_eq!(
        get_memory[0].data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "get_memory_valid_ws_datamask")
    );

    let memory_response = server.handle_ecu_message(&Message::with_addressing(
        PGN_ECU_TO_VT,
        get_memory[0].data.clone(),
        0x42,
        0x80,
        Priority::Default,
    ));
    assert_eq!(memory_response.len(), 1);
    assert_eq!(memory_response[0].dest, Some(0x42));
    assert_eq!(
        memory_response[0].data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "get_memory_response_success")
    );
    assert_eq!(server.state(), VTServerState::WaitForPoolUpload);

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        memory_response[0].data.clone(),
        0x80,
    ));
    assert_eq!(client.state(), VTState::UploadPool);

    let upload = client.update(1);
    assert_eq!(upload.len(), 1);
    assert_eq!(upload[0].pgn, PGN_ECU_TO_VT);
    assert_eq!(upload[0].dest, Some(0x80));
    assert_eq!(
        upload[0].data,
        parse_named_hex_bytes(
            ISOBUS_VT_COMMANDS_HEX,
            "object_pool_transfer_valid_ws_datamask"
        )
    );
    assert_eq!(client.state(), VTState::WaitForPoolStore);

    let end_of_pool = client.update(1_000);
    assert_eq!(end_of_pool.len(), 1);
    assert_eq!(
        end_of_pool[0].data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "end_of_object_pool_request")
    );

    let no_reply = server.handle_ecu_message(&Message::with_addressing(
        PGN_ECU_TO_VT,
        upload[0].data.clone(),
        0x42,
        0x80,
        Priority::Default,
    ));
    assert!(no_reply.is_empty());
    assert!(server.clients()[0].pool_uploaded);
    assert_eq!(server.clients()[0].pool.serialize().unwrap(), pool_bytes);

    let eop_response = server.handle_ecu_message(&Message::with_addressing(
        PGN_ECU_TO_VT,
        end_of_pool[0].data.clone(),
        0x42,
        0x80,
        Priority::Default,
    ));
    assert_eq!(eop_response.len(), 1);
    assert_eq!(
        eop_response[0].data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "end_of_object_pool_success")
    );
    assert_eq!(server.state(), VTServerState::Connected);
    assert_eq!(server.active_working_set(), 0x42);

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        eop_response[0].data.clone(),
        0x80,
    ));
    assert_eq!(client.state(), VTState::Connected);

    let update_dispatch_fixtures = [
        (
            UpdateOp::Numeric {
                id: VTObjectID(0x1234),
                value: 0xDEADBEEF,
            },
            "client_change_numeric_value_1234_deadbeef",
        ),
        (
            UpdateOp::String {
                id: VTObjectID(5),
                value: "hi".into(),
            },
            "client_change_string_value_obj5_hi",
        ),
        (
            UpdateOp::Visibility {
                id: VTObjectID(0x1234),
                visible: false,
            },
            "client_hide_object_1234",
        ),
        (
            UpdateOp::Enable {
                id: VTObjectID(0x1234),
                enabled: true,
            },
            "client_enable_object_1234",
        ),
        (
            UpdateOp::ActiveMask {
                working_set_id: VTObjectID(1),
                mask_id: VTObjectID(2),
            },
            "client_change_active_mask_ws1_mask2",
        ),
    ];
    for (op, fixture_name) in update_dispatch_fixtures {
        let outbound = op.to_client_outbound(&client).unwrap();
        assert_eq!(outbound.pgn, PGN_ECU_TO_VT);
        assert_eq!(outbound.dest, Some(0x80));
        assert_eq!(
            outbound.data,
            parse_named_hex_bytes(ISOBUS_VT_COMMANDS_HEX, fixture_name),
            "{fixture_name}"
        );
    }

    let mut error_server = VTServer::new(VTServerConfig::default());
    error_server.start().unwrap();
    let _ = error_server.handle_ecu_message(&Message::with_addressing(
        PGN_ECU_TO_VT,
        get_memory[0].data.clone(),
        0x42,
        0x80,
        Priority::Default,
    ));
    let rejected_eop = error_server.handle_ecu_message(&Message::with_addressing(
        PGN_ECU_TO_VT,
        end_of_pool[0].data.clone(),
        0x42,
        0x80,
        Priority::Default,
    ));
    assert_eq!(
        rejected_eop[0].data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "end_of_object_pool_pool_error")
    );

    assert_eq!(
        client.select_input_object(0x1234, 0x02).unwrap().data,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "client_select_input_object_1234_option02",
        )
    );
    assert_eq!(
        client.esc_input().unwrap().data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "client_esc_input")
    );
    assert_eq!(
        client
            .change_child_location(0x1000.into(), 0x2000.into(), 7, 9)
            .unwrap()
            .data,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "client_change_child_location_1000_2000_07_09",
        )
    );
    assert_eq!(
        client
            .change_child_position(0x1000.into(), 0x2000.into(), 300, 400)
            .unwrap()
            .data,
        parse_named_hex_bytes(
            ISOBUS_VT_COMMANDS_HEX,
            "client_change_child_position_1000_2000_300_400",
        )
    );
    assert_eq!(
        client
            .change_numeric_value(0x1234, 0xDEADBEEF)
            .unwrap()
            .data,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "client_change_numeric_value_1234_deadbeef",
        )
    );
    assert_eq!(
        client.set_audio_volume(80).unwrap().data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "client_set_audio_volume_80")
    );
    assert_eq!(
        client.store_version("V1").unwrap().data,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "client_store_version_v1")
    );

    let classic_versions_log: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
    let classic_versions_sink = classic_versions_log.clone();
    client
        .on_versions_received
        .subscribe(move |labels| classic_versions_sink.borrow_mut().push(labels.clone()));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "get_versions_response_empty").to_vec(),
        0x80,
    ));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_bytes(
            ISOBUS_VT_COMMANDS_HEX,
            "get_versions_response_empty_bad_tail",
        ),
        0x80,
    ));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "get_versions_response_one_truncated",
        )
        .to_vec(),
        0x80,
    ));
    assert_eq!(
        classic_versions_log.borrow().len(),
        1,
        "classic version-list count/padding mismatches must be ignored"
    );
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_bytes(ISOBUS_VT_COMMANDS_HEX, "get_versions_response_one_v1"),
        0x80,
    ));
    assert_eq!(classic_versions_log.borrow()[1], vec!["V1".to_string()]);

    let mut extended_versions_client = VTClient::new(VTClientConfig::default());
    let extended_versions_log: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
    let extended_versions_sink = extended_versions_log.clone();
    extended_versions_client
        .on_extended_versions_received
        .subscribe(move |labels| extended_versions_sink.borrow_mut().push(labels.clone()));
    extended_versions_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_bytes(
            ISOBUS_VT_COMMANDS_HEX,
            "extended_get_versions_response_empty_bad_tail",
        ),
        0x80,
    ));
    extended_versions_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "extended_get_versions_response_one_truncated",
        )
        .to_vec(),
        0x80,
    ));
    assert!(extended_versions_log.borrow().is_empty());
    assert!(!extended_versions_client.vt_supports_extended_versions());
    extended_versions_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "extended_get_versions_response_empty",
        )
        .to_vec(),
        0x80,
    ));
    extended_versions_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_bytes(
            ISOBUS_VT_COMMANDS_HEX,
            "extended_get_versions_response_one_v1",
        ),
        0x80,
    ));
    assert_eq!(
        *extended_versions_log.borrow(),
        vec![Vec::<String>::new(), vec!["V1".to_string()]]
    );
    assert!(extended_versions_client.vt_supports_extended_versions());

    let mut extended_store_client = VTClient::new(VTClientConfig::default());
    let extended_store_log: Rc<RefCell<Vec<(bool, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let extended_store_sink = extended_store_log.clone();
    extended_store_client
        .on_extended_store_response
        .subscribe(move |&response| extended_store_sink.borrow_mut().push(response));
    extended_store_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_bytes(ISOBUS_VT_COMMANDS_HEX, "extended_store_response_short"),
        0x80,
    ));
    assert!(extended_store_log.borrow().is_empty());
    extended_store_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "extended_store_response_success").to_vec(),
        0x80,
    ));
    assert_eq!(*extended_store_log.borrow(), vec![(true, 0xFF)]);

    let mut extended_load_client = VTClient::new(VTClientConfig::default());
    extended_load_client.set_self_address(0x42);
    extended_load_client.set_object_pool(
        VTObjectPool::deserialize(&pool_bytes).expect("VT pool fixture decodes for load client"),
    );
    extended_load_client.connect().unwrap();
    extended_load_client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vt_status.to_vec(), 0x80));
    assert_eq!(extended_load_client.update(1).len(), 1);
    assert_eq!(extended_load_client.update(1).len(), 1);
    extended_load_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "get_memory_response_success").to_vec(),
        0x80,
    ));
    assert_eq!(extended_load_client.update(1).len(), 1);
    assert_eq!(extended_load_client.update(1_000).len(), 1);
    extended_load_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "end_of_object_pool_success").to_vec(),
        0x80,
    ));
    assert_eq!(extended_load_client.state(), VTState::Connected);
    extended_load_client
        .send_extended_load_version("V1")
        .unwrap();
    assert_eq!(extended_load_client.state(), VTState::WaitForEndOfPool);
    extended_load_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_bytes(ISOBUS_VT_COMMANDS_HEX, "extended_load_response_short"),
        0x80,
    ));
    assert_eq!(
        extended_load_client.state(),
        VTState::WaitForEndOfPool,
        "short extended load response must not complete the reload FSM"
    );
    extended_load_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "extended_load_response_success").to_vec(),
        0x80,
    ));
    assert_eq!(extended_load_client.state(), VTState::Connected);

    assert_eq!(
        VTServer::build_button_activation(KeyActivationCode::Pressed, 0xCAFE, 0xBEEF, 7),
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "server_button_activation_pressed_cafe_beef_key7",
        )
    );
    assert_eq!(
        VTServer::build_soft_key_activation(KeyActivationCode::Held, 0xCAFE, 0xBEEF, 7),
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "server_soft_key_activation_held_cafe_beef_key7",
        )
    );
    let mut activation_client = VTClient::new(VTClientConfig::default());
    let soft_key_log: Rc<RefCell<Vec<(VTObjectID, KeyActivationCode)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let button_log: Rc<RefCell<Vec<(VTObjectID, KeyActivationCode)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let soft_key_log_sink = soft_key_log.clone();
    let button_log_sink = button_log.clone();
    activation_client
        .on_soft_key
        .subscribe(move |&v| soft_key_log_sink.borrow_mut().push(v));
    activation_client
        .on_button
        .subscribe(move |&v| button_log_sink.borrow_mut().push(v));
    activation_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "server_soft_key_activation_held_cafe_beef_key7",
        )
        .to_vec(),
        0x80,
    ));
    activation_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "server_button_activation_pressed_cafe_beef_key7",
        )
        .to_vec(),
        0x80,
    ));
    activation_client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::SOFT_KEY_ACTIVATION, 0x05, 0x00, 0x01],
        0x80,
    ));
    for malformed in [
        "server_soft_key_activation_bad_code",
        "server_soft_key_activation_bad_tail",
    ] {
        activation_client.handle_vt_message(&Message::new(
            PGN_VT_TO_ECU,
            parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, malformed).to_vec(),
            0x80,
        ));
    }
    for malformed in [
        "server_button_activation_bad_code",
        "server_button_activation_bad_tail",
    ] {
        activation_client.handle_vt_message(&Message::new(
            PGN_VT_TO_ECU,
            parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, malformed).to_vec(),
            0x80,
        ));
    }
    assert_eq!(
        *soft_key_log.borrow(),
        vec![(VTObjectID(0xCAFE), KeyActivationCode::Held)],
        "VT client must parse activation code/object id from canonical VT layout only"
    );
    assert_eq!(
        *button_log.borrow(),
        vec![(VTObjectID(0xCAFE), KeyActivationCode::Pressed)],
        "VT client must reject malformed button activation notifications"
    );
    assert_eq!(
        VTServer::build_change_numeric_value(0x1234, 0xDEADBEEF),
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "server_numeric_value_change_1234_deadbeef",
        )
    );
    assert_eq!(
        VTServer::build_change_string_value(0x05, "hi").unwrap(),
        parse_named_hex_bytes(ISOBUS_VT_COMMANDS_HEX, "server_string_value_change_obj5_hi")
    );
    let server_numeric_change = parse_named_hex_frame(
        ISOBUS_VT_COMMANDS_HEX,
        "server_numeric_value_change_1234_deadbeef",
    );
    let numeric_log: Rc<RefCell<Vec<(VTObjectID, u32)>>> = Rc::new(RefCell::new(Vec::new()));
    let numeric_log_sink = numeric_log.clone();
    client
        .on_numeric_value_change
        .subscribe(move |&v| numeric_log_sink.borrow_mut().push(v));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        server_numeric_change.to_vec(),
        0x80,
    ));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "server_numeric_value_change_bad_reserved",
        )
        .to_vec(),
        0x80,
    ));
    assert_eq!(
        *numeric_log.borrow(),
        vec![(VTObjectID(0x1234), 0xDEADBEEF)]
    );

    let server_string_change =
        parse_named_hex_bytes(ISOBUS_VT_COMMANDS_HEX, "server_string_value_change_obj5_hi");
    let string_log: Rc<RefCell<Vec<(VTObjectID, String)>>> = Rc::new(RefCell::new(Vec::new()));
    let string_log_sink = string_log.clone();
    client
        .on_string_value_change
        .subscribe(move |v| string_log_sink.borrow_mut().push(v.clone()));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        server_string_change.clone(),
        0x80,
    ));
    assert_eq!(
        *string_log.borrow(),
        vec![(VTObjectID(0x0005), "hi".into())]
    );

    let truncated_server_string = parse_named_hex_bytes(
        ISOBUS_VT_COMMANDS_HEX,
        "server_string_value_change_obj5_declares3_truncated",
    );
    let before = string_log.borrow().len();
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        truncated_server_string.clone(),
        0x80,
    ));
    assert_eq!(
        string_log.borrow().len(),
        before,
        "VT client must reject truncated string-change notifications"
    );
    let bad_tail_server_string = parse_named_hex_bytes(
        ISOBUS_VT_COMMANDS_HEX,
        "server_string_value_change_obj5_bad_tail",
    );
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        bad_tail_server_string.clone(),
        0x80,
    ));
    assert_eq!(
        string_log.borrow().len(),
        before,
        "VT client must reject string-change notifications with non-FF trailing padding"
    );

    let mut tracker = VTClientStateTracker::new();
    tracker.set_string_value(0x05, "old");
    tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, truncated_server_string, 0x80));
    assert_eq!(tracker.string_value(0x05), Some("old"));
    tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_tail_server_string, 0x80));
    assert_eq!(tracker.string_value(0x05), Some("old"));

    let truncated_client_string = parse_named_hex_bytes(
        ISOBUS_VT_COMMANDS_HEX,
        "client_change_string_value_obj5_declares3_truncated",
    );
    let server_string_log: Rc<RefCell<Vec<(VTObjectID, String)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let server_string_log_sink = server_string_log.clone();
    server
        .on_string_value_change
        .subscribe(move |v| server_string_log_sink.borrow_mut().push(v.clone()));
    assert!(
        server
            .handle_ecu_message(&Message::with_addressing(
                PGN_ECU_TO_VT,
                truncated_client_string,
                0x42,
                0x80,
                Priority::Default,
            ))
            .is_empty()
    );
    assert!(
        server_string_log.borrow().is_empty(),
        "VT server must reject truncated ECU string-change commands"
    );
    assert!(
        server
            .handle_ecu_message(&Message::with_addressing(
                PGN_ECU_TO_VT,
                parse_named_hex_frame(
                    ISOBUS_VT_COMMANDS_HEX,
                    "client_change_numeric_value_bad_reserved",
                )
                .to_vec(),
                0x42,
                0x80,
                Priority::Default,
            ))
            .is_empty()
    );
    assert!(
        server
            .handle_ecu_message(&Message::with_addressing(
                PGN_ECU_TO_VT,
                parse_named_hex_bytes(
                    ISOBUS_VT_COMMANDS_HEX,
                    "client_change_string_value_obj5_bad_tail"
                ),
                0x42,
                0x80,
                Priority::Default,
            ))
            .is_empty()
    );
    assert!(
        server_string_log.borrow().is_empty(),
        "VT server must reject bad reserved numeric and bad-tail string commands"
    );
    assert_eq!(
        VTServer::build_select_input_object(0x1234, true, true),
        parse_named_hex_frame(
            ISOBUS_VT_COMMANDS_HEX,
            "server_select_input_object_1234_selected_open",
        )
    );
    let unsupported = parse_named_hex_frame(ISOBUS_VT_COMMANDS_HEX, "unsupported_graphics_context");
    assert_eq!(
        VTServer::build_unsupported_function(cmd::GRAPHICS_CONTEXT),
        unsupported
    );
    assert!(
        server
            .handle_ecu_message(&Message::with_addressing(
                PGN_ECU_TO_VT,
                unsupported.to_vec(),
                0x42,
                0x80,
                Priority::Default,
            ))
            .is_empty(),
        "received UnsupportedVTFunction messages must not reply-loop"
    );
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, unsupported.to_vec(), 0x80));
    assert_eq!(client.unsupported_functions(), &[cmd::GRAPHICS_CONTEXT]);
}

#[test]
fn fixture_isobus_tc_ddop_codecs_and_invalid_graphs_are_stable() {
    let expected = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("Sprayer")
                .with_software_version("1.0")
                .with_serial_number("SN-1234")
                .with_structure_label([1, 2, 3, 4, 5, 6, 7])
                .with_localization_label([8, 9, 10, 11, 12, 13, 14]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Section)
                .with_number(5)
                .with_parent(1)
                .with_designator("S1")
                .with_children(vec![10, 20]),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(10)
                .with_ddi(DDI(0x1234))
                .with_trigger(TriggerMethod::TimeInterval)
                .with_trigger(TriggerMethod::ThresholdLimits)
                .with_presentation(ObjectID::NULL)
                .with_designator("PD1"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(20)
                .with_ddi(DDI(0xABCD))
                .with_value(-42)
                .with_presentation(ObjectID::NULL)
                .with_designator("Prop1"),
        )
        .with_value_presentation(
            DeviceValuePresentation::default()
                .with_id(30)
                .with_offset(100)
                .with_scale(0.001)
                .with_decimals(3)
                .with_unit("m"),
        );
    expected.validate().unwrap();

    let bytes = parse_named_hex_bytes(ISOBUS_TC_DDOP_HEX, "sprayer_one_section_all_object_types");
    assert_eq!(expected.serialize().unwrap(), bytes);

    let decoded = DDOP::deserialize(&bytes).expect("valid DDOP fixture must decode");
    decoded.validate().unwrap();
    assert_eq!(decoded.object_count(), 5);
    assert_eq!(decoded.devices()[0].designator, "Sprayer");
    assert_eq!(decoded.elements()[0].r#type, DeviceElementType::Section);
    assert_eq!(
        decoded.elements()[0].child_objects,
        vec![ObjectID(10), ObjectID(20)]
    );
    assert_eq!(decoded.process_data()[0].ddi, DDI(0x1234));
    assert_eq!(
        decoded.process_data()[0].trigger_methods,
        TriggerMethod::TimeInterval.as_u8() | TriggerMethod::ThresholdLimits.as_u8()
    );
    assert_eq!(decoded.properties()[0].value, -42);
    assert!((decoded.value_presentations()[0].scale - 0.001).abs() < 1e-6);
    assert_eq!(decoded.serialize().unwrap(), bytes);

    let drill = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("Drill")
                .with_software_version("2.1")
                .with_serial_number("DR-42")
                .with_structure_label([10, 11, 12, 13, 14, 15, 16])
                .with_localization_label([20, 21, 22, 23, 24, 25, 26]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Device)
                .with_designator("Root")
                .with_children(vec![3, 6, 7]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(3)
                .with_type(DeviceElementType::Function)
                .with_number(1)
                .with_parent(2)
                .with_designator("Meter")
                .with_children(vec![4, 5, 8, 9]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(4)
                .with_type(DeviceElementType::Section)
                .with_number(1)
                .with_parent(3)
                .with_designator("L")
                .with_children(vec![8]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(5)
                .with_type(DeviceElementType::Section)
                .with_number(2)
                .with_parent(3)
                .with_designator("R")
                .with_children(vec![8]),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(6)
                .with_ddi(DDI(67))
                .with_value(6000)
                .with_presentation(10)
                .with_designator("Width"),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(7)
                .with_ddi(DDI(134))
                .with_value(-250)
                .with_presentation(10)
                .with_designator("Offset"),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(8)
                .with_ddi(DDI(1))
                .with_trigger(TriggerMethod::OnChange)
                .with_presentation(10)
                .with_designator("Seed Rate"),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(9)
                .with_ddi(DDI(116))
                .with_trigger(TriggerMethod::Total)
                .with_presentation(10)
                .with_designator("Area"),
        )
        .with_value_presentation(
            DeviceValuePresentation::default()
                .with_id(10)
                .with_offset(0)
                .with_scale(0.01)
                .with_decimals(2)
                .with_unit("kg/ha"),
        );
    drill.validate().unwrap();
    let drill_bytes =
        parse_named_hex_bytes(ISOBUS_TC_DDOP_HEX, "metered_drill_two_sections_scaled_rate");
    assert_eq!(drill.serialize().unwrap(), drill_bytes);
    let decoded_drill = DDOP::deserialize(&drill_bytes).expect("drill DDOP fixture decodes");
    decoded_drill.validate().unwrap();
    assert_eq!(decoded_drill.object_count(), 10);
    assert_eq!(decoded_drill.elements()[1].child_objects, vec![4, 5, 8, 9]);
    assert_eq!(
        decoded_drill.process_data()[0].presentation_object_id,
        ObjectID(10)
    );
    assert!((decoded_drill.value_presentations()[0].scale - 0.01).abs() < 1e-6);
    assert_eq!(decoded_drill.serialize().unwrap(), drill_bytes);

    let invalid_type = parse_named_hex_bytes(ISOBUS_TC_DDOP_HEX, "invalid_unknown_element_type");
    assert!(DDOP::deserialize(&invalid_type).is_err());
    assert_eq!(invalid_type[0], TCObjectType::DeviceElement.as_u8());

    let duplicate = parse_named_hex_bytes(ISOBUS_TC_DDOP_HEX, "invalid_duplicate_object_id");
    let duplicate_ddop = DDOP::deserialize(&duplicate).expect("duplicate fixture still parses");
    assert!(duplicate_ddop.validate().is_err());

    let non_ascii =
        parse_named_hex_bytes(ISOBUS_TC_DDOP_HEX, "invalid_non_ascii_device_designator");
    assert!(DDOP::deserialize(&non_ascii).is_err());

    let overlong = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("A".repeat(usize::from(u8::MAX) + 1)),
        )
        .with_element(DeviceElement::default().with_id(2));
    assert!(overlong.serialize().is_err());
    assert!(overlong.validate().is_err());
}

#[test]
fn fixture_isobus_tc_ddop_helper_expectations_are_stable() {
    let expected_i32 = |name: &str| -> i32 {
        parse_named_text_value(ISOBUS_TC_DDOP_HELPER_EXPECTATIONS, name)
            .parse::<i32>()
            .unwrap()
    };
    let expected_u16 = |name: &str| -> u16 {
        parse_named_text_value(ISOBUS_TC_DDOP_HELPER_EXPECTATIONS, name)
            .parse::<u16>()
            .unwrap()
    };
    let expected_str =
        |name: &str| -> &str { parse_named_text_value(ISOBUS_TC_DDOP_HELPER_EXPECTATIONS, name) };

    let ddop = DDOP::default()
        .with_device(
            DeviceObject::default()
                .with_id(1)
                .with_designator("Two Section Sprayer"),
        )
        .with_element(
            DeviceElement::default()
                .with_id(2)
                .with_type(DeviceElementType::Connector)
                .with_parent(1)
                .with_designator("Connector")
                .with_children(vec![20]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(3)
                .with_type(DeviceElementType::Function)
                .with_parent(1)
                .with_designator("Boom")
                .with_children(vec![21, 4, 5]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(4)
                .with_type(DeviceElementType::Section)
                .with_number(1)
                .with_parent(3)
                .with_designator("S1")
                .with_children(vec![22, 23, 30, 31]),
        )
        .with_element(
            DeviceElement::default()
                .with_id(5)
                .with_type(DeviceElementType::Section)
                .with_number(2)
                .with_parent(3)
                .with_designator("S2")
                .with_children(vec![24, 25, 30, 31]),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(20)
                .with_ddi(ddi::CONNECTOR_PIVOT_X_OFFSET)
                .with_value(1000),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(21)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_X)
                .with_value(500),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(22)
                .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                .with_value(3000),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(23)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y)
                .with_value(-1500),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(24)
                .with_ddi(ddi::ACTUAL_WORKING_WIDTH)
                .with_value(3000),
        )
        .with_property(
            DeviceProperty::default()
                .with_id(25)
                .with_ddi(ddi::DEVICE_ELEMENT_OFFSET_Y)
                .with_value(1500),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(30)
                .with_ddi(ddi::SETPOINT_VOLUME_PER_AREA_APPLICATION_RATE)
                .with_trigger(TriggerMethod::OnChange)
                .with_designator("Target Rate"),
        )
        .with_process_data(
            DeviceProcessData::default()
                .with_id(31)
                .with_ddi(ddi::TOTAL_AREA)
                .with_trigger(TriggerMethod::Total)
                .with_designator("Covered Area"),
        );
    ddop.validate().unwrap();

    assert_eq!(
        DDOPHelpers::section_count(&ddop),
        expected_u16("section_count")
    );
    assert_eq!(
        DDOPHelpers::section_count_checked(&ddop),
        Some(expected_u16("section_count"))
    );

    let geometry = DDOPHelpers::extract_geometry(&ddop);
    assert_eq!(geometry.connector_x_mm, expected_i32("connector_x_mm"));
    assert_eq!(geometry.boom_offset_x_mm, expected_i32("boom_offset_x_mm"));
    assert_eq!(geometry.boom_offset_y_mm, expected_i32("boom_offset_y_mm"));
    assert_eq!(geometry.total_width_mm, expected_i32("total_width_mm"));
    assert_eq!(
        geometry.sections.len(),
        usize::from(expected_u16("section_count"))
    );

    for (idx, section) in geometry.sections.iter().enumerate() {
        assert_eq!(
            section.element_id,
            ObjectID(expected_u16(&format!("section_{idx}_id")))
        );
        assert_eq!(
            section.number,
            ElementNumber(expected_u16(&format!("section_{idx}_number")))
        );
        assert_eq!(
            section.designator,
            expected_str(&format!("section_{idx}_designator"))
        );
        assert_eq!(
            section.offset_x_mm,
            expected_i32(&format!("section_{idx}_offset_x_mm"))
        );
        assert_eq!(
            section.offset_y_mm,
            expected_i32(&format!("section_{idx}_offset_y_mm"))
        );
        assert_eq!(
            section.width_mm,
            expected_i32(&format!("section_{idx}_width_mm"))
        );
    }

    let rates = DDOPHelpers::extract_rates(&ddop);
    assert_eq!(rates.len(), usize::from(expected_u16("rate_count")));
    assert_eq!(rates[0].ddi, DDI(expected_u16("rate_0_ddi")));
    assert_eq!(rates[0].designator, expected_str("rate_0_designator"));

    let totals = DDOPHelpers::extract_totals(&ddop);
    assert_eq!(totals.len(), usize::from(expected_u16("total_count")));
    assert_eq!(totals[0].ddi, DDI(expected_u16("total_0_ddi")));
    assert_eq!(totals[0].designator, expected_str("total_0_designator"));

    assert_eq!(
        DDOPHelpers::find_parent_element(&ddop, ObjectID(22))
            .expect("width property should have a section parent")
            .id,
        ObjectID(4)
    );
}

