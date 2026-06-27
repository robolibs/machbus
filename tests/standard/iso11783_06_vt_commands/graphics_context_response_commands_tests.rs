#[test]
fn vt_server_emits_graphics_context_response_and_accepts_standard_padding() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let mut set_foreground = [0xFFu8; 8];
    set_foreground[0] = cmd::GRAPHICS_CONTEXT;
    set_foreground[1..3].copy_from_slice(&11u16.to_le_bytes());
    set_foreground[3] = 0x02;
    set_foreground[4] = 7;

    let out = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        set_foreground.to_vec(),
        0x42,
    ));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].dest, Some(0x42));
    assert_eq!(
        out[0].data,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x02, 0, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        server.clients()[0].object_state.graphics_contexts.last().unwrap().payload,
        vec![7],
        "single-frame FF padding is stripped before retaining the replay payload"
    );
}

#[test]
fn vt_server_accepts_zero_length_graphics_context_draw_text() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let out = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        vec![
            cmd::GRAPHICS_CONTEXT,
            11,
            0,
            0x0D,
            1, // transparent text background
            0, // zero text bytes is valid per F.56
            0xFF,
            0xFF,
        ],
        0x42,
    ));
    assert_eq!(out.len(), 1);
    assert_eq!(
        out[0].data,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x0D, 0, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        server.clients()[0].object_state.graphics_contexts.last().unwrap().payload,
        vec![1, 0],
        "zero-length Draw Text retains the canonical counted payload without FF16 padding"
    );
}

#[test]
fn vt_server_graphics_context_response_reports_object_subcommand_parameter_and_result_errors() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let graphics_command = |object_id: u16, subcommand: u8, payload: &[u8]| {
        let mut data = vec![cmd::GRAPHICS_CONTEXT];
        data.extend_from_slice(&object_id.to_le_bytes());
        data.push(subcommand);
        data.extend_from_slice(payload);
        while data.len() < 8 {
            data.push(0xFF);
        }
        data
    };

    let invalid_object = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_command(0x7777, 0x02, &[9]),
        0x42,
    ));
    assert_eq!(invalid_object[0].data[4], 0x01);

    let invalid_subcommand = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_command(11, 0x15, &[]),
        0x42,
    ));
    assert_eq!(invalid_subcommand[0].data[4], 0x02);

    let invalid_parameter = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x02],
        0x42,
    ));
    assert_eq!(invalid_parameter[0].data[4], 0x04);

    let invalid_results = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_command(11, 0x04, &8u16.to_le_bytes()),
        0x42,
    ));
    assert_eq!(
        invalid_results[0].data[4],
        0x08,
        "line-attribute selection with a Fill Attributes object is a result/reference error"
    );

    assert_eq!(
        server.clients()[0].object_state.graphics_contexts.len(),
        0,
        "failed Graphics Context commands return F.57 responses but do not mutate replay state"
    );
}

#[test]
fn vt_client_emits_graphics_context_response_events_and_rejects_reserved_bits() {
    let mut client = VTClient::new(VTClientConfig::default());
    let log: Rc<RefCell<Vec<(ObjectID, u8, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let captured = log.clone();
    client
        .on_graphics_context_response
        .subscribe(move |&event| captured.borrow_mut().push(event));

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x02, 0x04, 0xFF, 0xFF, 0xFF],
        0x80,
    ));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x02, 0x20, 0xFF, 0xFF, 0xFF],
        0x80,
    ));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x02, 0, 0xFF, 0xFF, 0],
        0x80,
    ));

    assert_eq!(*log.borrow(), vec![(ObjectID(11), 0x02, 0x04)]);
}
