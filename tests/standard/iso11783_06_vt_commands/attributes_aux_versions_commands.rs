#[test]
fn vt_server_get_attribute_value_reports_current_state_and_errors() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let get_attribute = |object_id: u16, attribute_id: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_ATTRIBUTE_VALUE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = attribute_id;
        data.to_vec()
    };
    let change_attribute = |object_id: u16, attribute_id: u8, value: u32| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_ATTRIBUTE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = attribute_id;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        data.to_vec()
    };
    let change_size = |object_id: u16, width: u16, height: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_SIZE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3..5].copy_from_slice(&width.to_le_bytes());
        data[5..7].copy_from_slice(&height.to_le_bytes());
        data.to_vec()
    };
    let change_end_point = |object_id: u16, width: u16, height: u16, direction: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_END_POINT;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3..5].copy_from_slice(&width.to_le_bytes());
        data[5..7].copy_from_slice(&height.to_le_bytes());
        data[7] = direction;
        data.to_vec()
    };
    let change_priority = |alarm_id: u16, priority: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_PRIORITY;
        data[1..3].copy_from_slice(&alarm_id.to_le_bytes());
        data[3] = priority;
        data.to_vec()
    };
    let change_soft_key_mask = |mask_type: u8, mask_id: u16, soft_key_mask_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_SOFT_KEY_MASK;
        data[1] = mask_type;
        data[2..4].copy_from_slice(&mask_id.to_le_bytes());
        data[4..6].copy_from_slice(&soft_key_mask_id.to_le_bytes());
        data.to_vec()
    };
    let select_colour_map = |object_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_COLOUR_MAP;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data.to_vec()
    };
    let enable_disable = |object_id: u16, enabled: bool| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::ENABLE_DISABLE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = u8::from(enabled);
        data.to_vec()
    };
    let hide_show = |object_id: u16, visible: bool| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::HIDE_SHOW;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = u8::from(visible);
        data.to_vec()
    };
    let change_string = |object_id: u16, value: &[u8]| {
        let mut data = vec![0xFFu8; 8];
        data[0] = cmd::CHANGE_STRING_VALUE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3..5].copy_from_slice(&(value.len() as u16).to_le_bytes());
        data[5..5 + value.len()].copy_from_slice(value);
        data
    };
    let get_response = |server: &mut VTServer, object_id: u16, attribute_id: u8| {
        let response = server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            get_attribute(object_id, attribute_id),
            0x42,
        ));
        assert_eq!(response.len(), 1);
        assert_eq!(response[0].dest, Some(0x42));
        response[0].data.clone()
    };
    let success = |object_id: u16, attribute_id: u8, value: u32| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_ATTRIBUTE_VALUE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = attribute_id;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        data.to_vec()
    };
    let error = |object_id: u16, attribute_id: u8, error_bits: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_ATTRIBUTE_VALUE;
        data[1..3].copy_from_slice(&ObjectID::NULL.to_le_bytes());
        data[3] = attribute_id;
        data[4..6].copy_from_slice(&object_id.to_le_bytes());
        data[6] = error_bits;
        data.to_vec()
    };

    assert_eq!(
        get_response(&mut server, 13, 1),
        success(13, 1, 0),
        "Colour Palette options are a standard readable zero-valued attribute"
    );
    assert_eq!(get_response(&mut server, 23, 1), success(23, 1, 1));
    assert_eq!(
        get_response(&mut server, 23, 2),
        success(23, 2, 0),
        "Picture Graphic AID 2 reports Options"
    );
    assert_eq!(
        get_response(&mut server, 23, 3),
        success(23, 3, 0xFF),
        "Picture Graphic AID 3 reports Transparency Colour"
    );
    assert_eq!(
        get_response(&mut server, 23, 4),
        success(23, 4, 1),
        "Picture Graphic AID 4 reports read-only Actual Width"
    );
    assert_eq!(
        get_response(&mut server, 23, 5),
        success(23, 5, 1),
        "Picture Graphic AID 5 reports read-only Actual Height"
    );
    assert_eq!(
        get_response(&mut server, 23, 6),
        success(23, 6, 2),
        "Picture Graphic AID 6 reports read-only Format"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(23, 2, 0x07),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(23, 3, 0x2A),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(23, 4, 9),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 23, 2),
        success(23, 2, 0x03),
        "Picture Graphic Options Change Attribute preserves the static raw/RLE bit"
    );
    assert_eq!(
        get_response(&mut server, 23, 3),
        success(23, 3, 0x2A),
        "Picture Graphic Transparency Colour is the writable AID 3"
    );
    assert_eq!(
        get_response(&mut server, 23, 4),
        success(23, 4, 1),
        "Picture Graphic Actual Width is read-only for Change Attribute"
    );
    assert_eq!(
        get_response(&mut server, 16, 1),
        success(16, 1, 5),
        "Working Set Special Controls AID 1 reports Number of Bytes to Follow"
    );
    assert_eq!(
        get_response(&mut server, 16, 2),
        success(16, 2, u32::from(ObjectID::NULL.raw())),
        "Working Set Special Controls AID 2 reports the retained colour-map selection"
    );
    assert_eq!(
        get_response(&mut server, 16, 3),
        success(16, 3, 13),
        "Working Set Special Controls AID 3 reports the retained colour-palette selection"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(16, 2, 10),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 16, 2),
        success(16, 2, 10),
        "Change Attribute AID 2 updates the WSSC colour-map selection"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        select_colour_map(ObjectID::NULL.raw()),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 16, 2),
        success(16, 2, u32::from(ObjectID::NULL.raw())),
        "Select Colour Map NULL must clear stale WSSC AID 2 Change Attribute state"
    );
    assert_eq!(
        get_response(&mut server, 16, 3),
        success(16, 3, u32::from(ObjectID::NULL.raw())),
        "Select Colour Map NULL must clear the WSSC colour-palette selection too"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(16, 3, 13),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        select_colour_map(10),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 16, 2),
        success(16, 2, 10),
        "Select Colour Map to a ColourMap object drives WSSC AID 2 Get Attribute Value"
    );
    assert_eq!(
        get_response(&mut server, 16, 3),
        success(16, 3, 13),
        "Selecting a ColourMap object does not clobber the retained WSSC palette selection"
    );
    assert_eq!(
        get_response(&mut server, 14, 1),
        success(14, 1, 2),
        "String Variable AID 1 reports the fixed maximum length, not the current string bytes"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_string(14, b"A"), 0x42));
    assert_eq!(
        get_response(&mut server, 14, 1),
        success(14, 1, 2),
        "Change String Value pads the value but must not change the String Variable length attribute"
    );
    assert_eq!(
        get_response(&mut server, 4, 4),
        success(4, 4, 0),
        "Input List AID 4 reports the inline selected index when the variable reference is NULL"
    );
    assert_eq!(
        get_response(&mut server, 20, 4),
        success(20, 4, 0),
        "Output List AID 4 reports the inline selected index when the variable reference is NULL"
    );
    let retained_effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(4, 4, 1),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 4, 4),
        success(4, 4, 1),
        "Input List value (AID 4) is settable via Change Attribute, matching the reference VT stack"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(20, 4, 1),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 20, 4),
        success(20, 4, 1),
        "Output List value (AID 4) is settable via Change Attribute, matching the reference VT stack"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(4, 4, 0x0100),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(20, 4, 0x0100),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 4, 4),
        success(4, 4, 0x0100),
        "Input List value (AID 4) Change Attribute retains the full sent value in the attribute overlay"
    );
    assert_eq!(
        get_response(&mut server, 20, 4),
        success(20, 4, 0x0100),
        "Output List value (AID 4) Change Attribute retains the full sent value in the attribute overlay"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        retained_effect_count + 4,
        "accepted List Value AID changes append render replay effects (four List Value Change Attributes)"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 4, 0xFF, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 20, 0xFF, 1),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 4, 4),
        success(4, 4, 1),
        "Change Numeric Value updates the retained Input List selected index"
    );
    assert_eq!(
        get_response(&mut server, 20, 4),
        success(20, 4, 1),
        "Change Numeric Value updates the retained Output List selected index"
    );
    assert_eq!(
        get_response(&mut server, 2, 2),
        success(2, 2, 3),
        "Data Mask AID 2 reports the uploaded Soft Key Mask before runtime changes"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(2, 2, u32::from(ObjectID::NULL.raw())),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 2, 2),
        success(2, 2, u32::from(ObjectID::NULL.raw())),
        "Change Attribute on Data Mask AID 2 must allow the standard NULL Soft Key Mask clear"
    );
    assert_eq!(
        server.clients()[0].object_state.active_soft_key_mask,
        ObjectID::NULL,
        "Change Attribute on the visible mask's Soft Key Mask AID must update server selection state, not only Get Attribute Value"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_soft_key_mask(1, 2, ObjectID::NULL.raw()),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 2, 2),
        success(2, 2, u32::from(ObjectID::NULL.raw())),
        "Get Attribute Value must report retained Data Mask Change Soft Key Mask state, including NULL clears"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_soft_key_mask(2, 22, 3),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 22, 2),
        success(22, 2, 3),
        "Get Attribute Value must report retained Alarm Mask Change Soft Key Mask state"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(22, 2, u32::from(ObjectID::NULL.raw())),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 22, 2),
        success(22, 2, u32::from(ObjectID::NULL.raw())),
        "Change Attribute on Alarm Mask AID 2 must allow the standard NULL Soft Key Mask clear"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 4, 0xFF, 0),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 4, 4),
        success(4, 4, 0),
        "Change Numeric Value updates the retained Input List selected index reported by Get Attribute Value"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(5, 1, 77),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 5, 1),
        success(5, 1, 77),
        "Get Attribute Value must report retained generic attribute changes"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_attribute(5, 4, 6), 0x42));
    assert_eq!(
        get_response(&mut server, 5, 4),
        success(5, 4, 6),
        "Output String Font Attributes AID 4 accepts a concrete Font Attributes object"
    );
    let effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(5, 4, u32::from(ObjectID::NULL.raw())),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 5, 4),
        success(5, 4, 6),
        "Output String Font Attributes AID 4 has standard range 0..=65534, so NULL must be rejected"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        effect_count,
        "rejected NULL required Font Attributes references must not append render replay effects"
    );
    let effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(39, 3, u32::from(ObjectID::NULL.raw())),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 39, 3),
        success(39, 3, 6),
        "Input Boolean foreground Font Attributes AID 3 has standard range 0..=65534, so NULL must be rejected"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        effect_count,
        "rejected NULL Input Boolean foreground references must not append render replay effects"
    );

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_size(26, 44, 55), 0x42));
    assert_eq!(get_response(&mut server, 26, 1), success(26, 1, 44));
    assert_eq!(
        get_response(&mut server, 26, 2),
        success(26, 2, 55),
        "Get Attribute Value must report command-specific retained geometry state"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, hide_show(26, false), 0x42));
    assert_eq!(
        get_response(&mut server, 26, 3),
        success(26, 3, 1),
        "Hide/Show must update the Container hidden remembered state reported by Get Attribute Value"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(26, 3, 0),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .visibility
            .get(&ObjectID(26)),
        Some(&false),
        "Container Hidden AID 3 is read-only for Change Attribute"
    );
    assert_eq!(
        get_response(&mut server, 26, 3),
        success(26, 3, 1),
        "Get Attribute Value must keep reporting Hide/Show state after rejected Container Hidden Change Attribute"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_priority(22, AlarmPriority::Warning as u8),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 22, 3),
        success(22, 3, AlarmPriority::Warning as u8 as u32),
        "Get Attribute Value must report retained Alarm Mask priority changes"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(21, 1, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_end_point(21, 12, 13, 1),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 21, 1),
        success(21, 1, 7),
        "Output Line AID 1 is Line Attributes, not width"
    );
    assert_eq!(get_response(&mut server, 21, 2), success(21, 2, 12));
    assert_eq!(get_response(&mut server, 21, 3), success(21, 3, 13));
    assert_eq!(
        get_response(&mut server, 21, 4),
        success(21, 4, 1),
        "Output Line endpoint state must report standard width/height/direction AIDs"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(27, 1, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_size(27, 33, 44), 0x42));
    assert_eq!(
        get_response(&mut server, 27, 1),
        success(27, 1, 7),
        "Output Rectangle AID 1 is Line Attributes"
    );
    assert_eq!(get_response(&mut server, 27, 2), success(27, 2, 33));
    assert_eq!(get_response(&mut server, 27, 3), success(27, 3, 44));

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(28, 1, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_size(28, 55, 66), 0x42));
    assert_eq!(
        get_response(&mut server, 28, 1),
        success(28, 1, 7),
        "Output Ellipse AID 1 is Line Attributes"
    );
    assert_eq!(get_response(&mut server, 28, 2), success(28, 2, 55));
    assert_eq!(get_response(&mut server, 28, 3), success(28, 3, 66));

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(9, 3, 7),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 9, 3),
        success(9, 3, 7),
        "Output Polygon AID 3 is a required Line Attributes reference"
    );
    let accepted_before = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(9, 3, u32::from(ObjectID::NULL.raw())),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 9, 3),
        success(9, 3, 7),
        "Output Polygon AID 3 has standard range 0..=65534, so NULL must not be retained"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_before,
        "rejected NULL polygon Line Attributes must not emit render effects"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(33, 6, 0x02),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(33), 6)),
        Some(&0x03),
        "Button Options bit 0 is static and must be preserved in retained state"
    );
    assert!(server.clients()[0].object_state.accepted_effects.contains(
        &ServerRenderEffect::ChangeGenericAttribute {
            id: ObjectID(33),
            attribute_id: 6,
            value: 0x03,
        }
    ));
    assert_eq!(
        get_response(&mut server, 33, 6),
        success(33, 6, 0x03),
        "Get Attribute Value must report Button Options after ignoring runtime changes to bit 0"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(25, 15, 0x03),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        enable_disable(25, false),
        0x42,
    ));
    assert_eq!(
        get_response(&mut server, 25, 15),
        success(25, 15, 0x02),
        "Input Number Options 2 must merge Enable/Disable into bit 0 without losing real-time editing bit 1"
    );

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, enable_disable(4, false), 0x42));
    assert_eq!(
        get_response(&mut server, 4, 5),
        success(4, 5, 0x02),
        "Input List Options must merge Enable/Disable into bit 0 without losing real-time editing bit 1"
    );

    assert_eq!(
        get_response(&mut server, 0x7777, 1),
        error(0x7777, 1, 0x01),
        "unknown objects use the standard Invalid Object ID response shape"
    );
    assert_eq!(
        get_response(&mut server, 13, 2),
        error(13, 2, 0x02),
        "unsupported attributes use the standard Invalid Attribute ID response shape"
    );

    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    let mut malformed = get_attribute(13, 1);
    malformed[4] = 0;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, malformed, 0x42))
            .is_empty(),
        "malformed Get Attribute Value requests do not produce a response"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count,
        "Get Attribute Value is observational and must not append render replay effects"
    );
}

#[test]
fn vt_server_get_attribute_value_reports_working_set_special_controls_extension_count() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    let source = 0x42;
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_colour_map(
                10,
                &ColourMapBody {
                    entries: vec![0, 1],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_palette(
                13,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_00_00_00],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_working_set_special_controls(
                16,
                &WorkingSetSpecialControlsBody {
                    colour_map: ObjectID(10),
                    colour_palette: ObjectID(13),
                    languages: Vec::new(),
                    extra_bytes: vec![0xAA, 0xBB, 0xCC, 0xDD],
                },
            )
            .unwrap(),
        );

    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                source,
            ))
            .len(),
        1
    );
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(pool.serialize().unwrap());
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer, source))
            .is_empty()
    );
    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].data[1], 0x00);

    let mut request = [0xFFu8; 8];
    request[0] = cmd::GET_ATTRIBUTE_VALUE;
    request[1..3].copy_from_slice(&16u16.to_le_bytes());
    request[3] = 1;
    let response =
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, request.to_vec(), source));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].dest, Some(source));
    let mut expected = [0xFFu8; 8];
    expected[0] = cmd::GET_ATTRIBUTE_VALUE;
    expected[1..3].copy_from_slice(&16u16.to_le_bytes());
    expected[3] = 1;
    expected[4..8].copy_from_slice(&9u32.to_le_bytes());
    assert_eq!(
        response[0].data,
        expected.to_vec(),
        "WSSC AID 1 must include preserved unknown extension bytes in the uploaded byte count"
    );
}

#[test]
fn vt_aux_capability_discovery_requires_requested_session_and_canonical_channels() {
    let mut discovery = AuxCapabilityDiscovery::new();

    let valid_channel = AuxChannelCapability {
        channel_id: 1,
        aux_type: 1,
        resolution: 100,
        function_type: 2,
    };
    let mut response = vec![cmd::GET_SUPPORTED_OBJECTS, 0x01, 1];
    response.extend(valid_channel.encode());

    assert!(
        discovery
            .handle_response(&Message::new(PGN_VT_TO_ECU, response.clone(), 0x80))
            .is_none(),
        "unsolicited AUX capability responses must not populate discovery state"
    );
    assert!(!discovery.capabilities().discovery_complete);

    let request = discovery.request_capabilities().unwrap();
    assert_eq!(request[0], cmd::GET_SUPPORTED_OBJECTS);
    assert_eq!(request[1], 0x01);
    assert_eq!(request[2], 31);
    assert_eq!(request[3], 32);

    assert!(
        discovery
            .handle_response(&Message::new(PGN_REQUEST, response.clone(), 0x80))
            .is_none(),
        "AUX capability responses under the wrong PGN must be ignored"
    );
    assert!(discovery.is_request_pending());

    assert!(
        discovery
            .handle_response(&Message::new(PGN_VT_TO_ECU, response.clone(), NULL_ADDRESS))
            .is_none()
    );
    assert!(
        discovery
            .handle_response(&Message::new(
                PGN_VT_TO_ECU,
                response.clone(),
                BROADCAST_ADDRESS
            ))
            .is_none()
    );
    assert!(
        discovery
            .handle_response(&Message::with_addressing(
                PGN_VT_TO_ECU,
                response.clone(),
                0x80,
                NULL_ADDRESS,
                Priority::Default,
            ))
            .is_none()
    );
    assert!(discovery.is_request_pending());

    let mut reserved_aux_type = response.clone();
    reserved_aux_type[4] = 3;
    assert!(
        discovery
            .handle_response(&Message::new(PGN_VT_TO_ECU, reserved_aux_type, 0x80))
            .is_none(),
        "reserved channel types must not complete discovery"
    );
    assert!(discovery.is_request_pending());

    let caps = discovery
        .handle_response(&Message::new(PGN_VT_TO_ECU, response, 0x80))
        .unwrap();
    assert!(caps.discovery_complete);
    assert_eq!(caps.channels, vec![valid_channel]);
    assert!(!discovery.is_request_pending());

    let mut server = VTServer::new(VTServerConfig::default());
    assert!(server.set_aux_capabilities(vec![valid_channel]).is_ok());
    assert!(
        server
            .set_aux_capabilities(vec![AuxChannelCapability {
                channel_id: 2,
                aux_type: 3,
                resolution: 0,
                function_type: 0,
            }])
            .is_err()
    );
    assert_eq!(server.aux_capabilities(), &[valid_channel]);
}

#[test]
fn vt_aux_assignment_runtime_requires_uploaded_matching_objects_and_valid_status() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();

    assert!(
        server
            .assign_aux_input(0x42, ObjectID(21), ObjectID(20))
            .is_err(),
        "AUX assignment must not allocate state before an activated pool exists"
    );

    activate_aux_assignment_pool(&mut server, 0x42);

    assert!(
        server
            .assign_aux_input(0x42, ObjectID(21), ObjectID(20))
            .is_ok()
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .aux_assignments
            .get(&ObjectID(21)),
        Some(&ObjectID(20))
    );

    assert!(
        server
            .assign_aux_input(0x42, ObjectID(21), ObjectID(22))
            .is_err(),
        "classic AUX-O inputs must not bind to AUX-N function objects"
    );
    assert!(
        server
            .assign_aux_input(0x42, ObjectID(21), ObjectID(24))
            .is_err(),
        "AUX assignments must reject mismatched function/input types"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .aux_assignments
            .get(&ObjectID(21)),
        Some(&ObjectID(20)),
        "failed AUX assignment attempts must not replace the existing assignment"
    );

    let aux_o = AuxOFunction {
        function_number: 7,
        r#type: AuxFunctionType::Type1,
        state: AuxFunctionState::Variable,
        setpoint: 5000,
    };
    assert_eq!(
        server.handle_aux_input_status(
            0x42,
            &Message::new(PGN_AUX_INPUT_STATUS, aux_o.encode().to_vec(), 0x90),
        ),
        Ok(true)
    );
    let aux_state = server.clients()[0]
        .object_state
        .aux_input_states
        .get(&ObjectID(21))
        .copied()
        .unwrap();
    assert_eq!(aux_state.input_object, ObjectID(21));
    assert_eq!(aux_state.function_object, ObjectID(20));
    assert_eq!(aux_state.function_number, 7);
    assert_eq!(aux_state.r#type, AuxFunctionType::Type1);
    assert_eq!(aux_state.state, AuxFunctionState::Variable);
    assert_eq!(aux_state.setpoint, 5000);
    assert_eq!(aux_state.source, 0x90);

    let wrong_pgn = Message::new(PGN_ECU_TO_VT, aux_o.encode().to_vec(), 0x90);
    assert!(server.handle_aux_input_status(0x42, &wrong_pgn).is_err());
    assert_eq!(
        server.clients()[0]
            .object_state
            .aux_input_states
            .get(&ObjectID(21))
            .copied(),
        Some(aux_state),
        "wrong-envelope AUX input status must not mutate cached assignment state"
    );

    let mismatched_status_type = AuxOFunction {
        function_number: 7,
        r#type: AuxFunctionType::Type2,
        state: AuxFunctionState::Variable,
        setpoint: 5001,
    };
    assert!(
        server
            .handle_aux_input_status(
                0x42,
                &Message::new(
                    PGN_AUX_INPUT_STATUS,
                    mismatched_status_type.encode().to_vec(),
                    0x90,
                ),
            )
            .is_err()
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .aux_input_states
            .get(&ObjectID(21))
            .copied(),
        Some(aux_state)
    );

    let oversized_aux_o = AuxOFunction {
        function_number: 7,
        r#type: AuxFunctionType::Type1,
        state: AuxFunctionState::Variable,
        setpoint: 10_001,
    };
    assert!(
        server
            .handle_aux_input_status(
                0x42,
                &Message::new(
                    PGN_AUX_INPUT_STATUS,
                    oversized_aux_o.encode().to_vec(),
                    0x90
                ),
            )
            .is_err()
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .aux_input_states
            .get(&ObjectID(21))
            .copied(),
        Some(aux_state)
    );

    assert!(
        server
            .assign_aux_input(0x42, ObjectID(23), ObjectID(22))
            .is_ok()
    );
    let aux_n = AuxNFunction {
        function_number: 9,
        r#type: AuxFunctionType::Type2,
        state: AuxFunctionState::Variable,
        setpoint: 0xCAFE,
    };
    assert_eq!(
        server.handle_aux_input_status(
            0x42,
            &Message::new(PGN_AUX_INPUT_TYPE2, aux_n.encode().to_vec(), 0x91),
        ),
        Ok(true)
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .aux_input_states
            .get(&ObjectID(23))
            .map(|state| (state.function_object, state.setpoint, state.source)),
        Some((ObjectID(22), 0xCAFE, 0x91))
    );

    assert!(server.clear_aux_assignment(0x42, ObjectID(23)).is_ok());
    assert!(
        !server.clients()[0]
            .object_state
            .aux_assignments
            .contains_key(&ObjectID(23))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .aux_input_states
            .contains_key(&ObjectID(23))
    );
}

#[test]
fn vt_client_version_labels_are_validated_before_state_mutation() {
    let mut client = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut client);

    for label in ["", "ABCDEFGH", ".", "..", "BAD/L", "BAD\\L", "BAD L", "é"] {
        assert!(
            client.store_version(label).is_err(),
            "invalid classic label must be rejected before store"
        );
        assert_eq!(client.state(), VTState::Connected);
        assert!(
            client.load_version(label).is_err(),
            "invalid classic label must be rejected before load"
        );
        assert_eq!(
            client.state(),
            VTState::Connected,
            "failed classic load validation must not enter end-of-pool wait"
        );
        assert!(
            client.delete_version(label).is_err(),
            "invalid classic label must be rejected before delete"
        );
    }

    let store = client.store_version("V1").unwrap();
    assert_eq!(store.data[0], cmd::STORE_VERSION);
    assert_eq!(&store.data[1..8], b"V1     ");

    let load = client.load_version("V1").unwrap();
    assert_eq!(load.data[0], cmd::LOAD_VERSION);
    assert_eq!(client.state(), VTState::WaitForEndOfPool);

    let mut extended = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut extended);
    assert_eq!(extended.extended_version_label(), "");

    let too_long = "A".repeat(cmd::EXTENDED_VERSION_LABEL_SIZE + 1);
    for label in [
        "",
        too_long.as_str(),
        ".",
        "..",
        "BAD/L",
        "BAD\\L",
        "BAD L",
        "é",
    ] {
        assert!(extended.send_extended_store_version(label).is_err());
        assert_eq!(
            extended.extended_version_label(),
            "",
            "failed extended store validation must not cache the rejected label"
        );
        assert_eq!(extended.state(), VTState::Connected);
        assert!(extended.send_extended_load_version(label).is_err());
        assert_eq!(
            extended.state(),
            VTState::Connected,
            "failed extended load validation must not enter end-of-pool wait"
        );
        assert!(extended.send_extended_delete_version(label).is_err());
    }

    let store = extended.send_extended_store_version("MY-LABEL").unwrap();
    assert_eq!(store.data[0], cmd::EXTENDED_STORE_VERSION);
    assert_eq!(store.data[1], cmd::EXTENDED_VERSION_SUBFUNCTION);
    assert_eq!(store.data.len(), 2 + cmd::EXTENDED_VERSION_LABEL_SIZE);
    assert_eq!(&store.data[2..10], b"MY-LABEL");
    assert_eq!(extended.extended_version_label(), "MY-LABEL");

    let load = extended.send_extended_load_version("MY-LABEL").unwrap();
    assert_eq!(load.data[0], cmd::EXTENDED_LOAD_VERSION);
    assert_eq!(load.data[1], cmd::EXTENDED_VERSION_SUBFUNCTION);
    assert_eq!(extended.state(), VTState::WaitForEndOfPool);
}

#[test]
fn vt_client_version_list_labels_require_canonical_bytes_before_events() {
    let mut client = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut client);

    let classic_log: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
    let classic_seen = classic_log.clone();
    client
        .on_versions_received
        .subscribe(move |labels| classic_seen.borrow_mut().push(labels.clone()));

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::GET_VERSIONS_RESPONSE, 1, b'V', b'1', 0, b'X', 0, 0],
        0x80,
    ));
    assert!(
        classic_log.borrow().is_empty(),
        "classic version-list labels must reject hidden bytes after padding"
    );

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![
            cmd::GET_VERSIONS_RESPONSE,
            1,
            b'B',
            b'A',
            b'D',
            b'/',
            b'L',
            b' ',
            b' ',
        ],
        0x80,
    ));
    assert!(
        classic_log.borrow().is_empty(),
        "classic version-list labels must reject non-canonical label bytes"
    );

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![
            cmd::GET_VERSIONS_RESPONSE,
            1,
            b'V',
            b'1',
            b' ',
            b' ',
            b' ',
            b' ',
            b' ',
        ],
        0x80,
    ));
    assert_eq!(*classic_log.borrow(), vec![vec!["V1".to_string()]]);

    let mut extended = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut extended);
    let extended_log: Rc<RefCell<Vec<Vec<String>>>> = Rc::new(RefCell::new(Vec::new()));
    let extended_seen = extended_log.clone();
    extended
        .on_extended_versions_received
        .subscribe(move |labels| extended_seen.borrow_mut().push(labels.clone()));

    let mut hidden = vec![
        cmd::EXTENDED_GET_VERSIONS,
        cmd::EXTENDED_VERSION_SUBFUNCTION,
        1,
    ];
    hidden.extend_from_slice(b"V1");
    hidden.push(0);
    hidden.push(b'X');
    hidden.resize(3 + cmd::EXTENDED_VERSION_LABEL_SIZE, 0);
    extended.handle_vt_message(&Message::new(PGN_VT_TO_ECU, hidden, 0x80));
    assert!(
        extended_log.borrow().is_empty(),
        "extended version-list labels must reject hidden bytes after padding"
    );
    assert!(
        !extended.vt_supports_extended_versions(),
        "rejected extended label lists must not update feature support state"
    );

    let mut good = vec![
        cmd::EXTENDED_GET_VERSIONS,
        cmd::EXTENDED_VERSION_SUBFUNCTION,
        1,
    ];
    good.extend_from_slice(b"MY-LABEL");
    good.resize(3 + cmd::EXTENDED_VERSION_LABEL_SIZE, b' ');
    extended.handle_vt_message(&Message::new(PGN_VT_TO_ECU, good, 0x80));
    assert_eq!(*extended_log.borrow(), vec![vec!["MY-LABEL".to_string()]]);
    assert!(extended.vt_supports_extended_versions());
}

#[test]
fn vt_client_version_operation_responses_require_canonical_fixed_frames_before_events_or_state() {
    let mut classic = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut classic);

    let store_log: Rc<RefCell<Vec<(bool, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let store_seen = store_log.clone();
    classic
        .on_store_version_response
        .subscribe(move |&response| store_seen.borrow_mut().push(response));

    classic.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::STORE_VERSION, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF],
        0x80,
    ));
    assert!(
        store_log.borrow().is_empty(),
        "classic Store Version responses must reject non-canonical fixed-frame tails"
    );
    classic.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::STORE_VERSION, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        0x80,
    ));
    assert_eq!(*store_log.borrow(), vec![(true, 0x00)]);

    let load_log: Rc<RefCell<Vec<(bool, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let load_seen = load_log.clone();
    classic
        .on_load_version_response
        .subscribe(move |&response| load_seen.borrow_mut().push(response));
    classic.load_version("V1").unwrap();
    assert_eq!(classic.state(), VTState::WaitForEndOfPool);
    classic.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::LOAD_VERSION, 0x00, 0x00, 0xFE, 0xFF, 0xFF, 0xFF, 0xFF],
        0x80,
    ));
    assert!(load_log.borrow().is_empty());
    assert_eq!(
        classic.state(),
        VTState::WaitForEndOfPool,
        "malformed Load Version responses must not advance the pool-load state"
    );
    classic.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::LOAD_VERSION, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
        0x80,
    ));
    assert_eq!(*load_log.borrow(), vec![(true, 0x00)]);
    assert_eq!(classic.state(), VTState::Connected);

    let mut extended = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut extended);
    let extended_store_log: Rc<RefCell<Vec<(bool, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let extended_store_seen = extended_store_log.clone();
    extended
        .on_extended_store_response
        .subscribe(move |&response| extended_store_seen.borrow_mut().push(response));
    extended.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![
            cmd::EXTENDED_STORE_VERSION,
            0x00,
            0x00,
            0x01,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert!(
        extended_store_log.borrow().is_empty(),
        "extended Store Version responses must reject non-canonical fixed-frame tails"
    );

    let extended_load_log: Rc<RefCell<Vec<(bool, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let extended_load_seen = extended_load_log.clone();
    extended
        .on_extended_load_response
        .subscribe(move |&response| extended_load_seen.borrow_mut().push(response));
    extended.send_extended_load_version("V1").unwrap();
    assert_eq!(extended.state(), VTState::WaitForEndOfPool);
    extended.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![
            cmd::EXTENDED_LOAD_VERSION,
            0x00,
            0x00,
            0x01,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert!(extended_load_log.borrow().is_empty());
    assert_eq!(extended.state(), VTState::WaitForEndOfPool);
    extended.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![
            cmd::EXTENDED_LOAD_VERSION,
            0x00,
            0x00,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(*extended_load_log.borrow(), vec![(true, 0x00)]);
    assert_eq!(extended.state(), VTState::Connected);
}

#[test]
fn vt_server_stored_version_labels_are_canonical_before_disk_load() {
    let dir = vt_standard_temp_dir("vt_stored_label");
    let mut sws = ServerWorkingSet {
        client_address: 0x42,
        ..Default::default()
    };
    sws.set_storage_path(&dir);
    assert!(sws.ensure_storage_dir());

    let path = sws.get_client_storage_dir().join("5631.vtp");
    let pool_data = minimal_object_pool().serialize().unwrap();

    let write_version = |label_field: [u8; 8]| {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"VTP1");
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&(pool_data.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&5u16.to_le_bytes());
        bytes.push(2);
        bytes.extend_from_slice(&label_field);
        bytes.extend_from_slice(&pool_data);
        fs::write(&path, bytes).unwrap();
    };

    write_version([b'V', b'1', 0, b'X', 0, 0, 0, 0]);
    assert!(
        !sws.load_version("V1"),
        "hidden non-padding bytes after the stored label terminator must not be accepted"
    );
    assert!(
        sws.stored_versions.is_empty(),
        "rejected persistent headers must not populate the version cache"
    );
    assert!(
        sws.pool.is_empty(),
        "rejected persistent headers must not restore the object pool"
    );

    write_version([b'V', b'1', 0, 0, 0, 0, 0, 0]);
    assert!(sws.load_version("V1"));
    assert_eq!(sws.stored_versions.len(), 1);
    assert_eq!(sws.pool.size(), 2);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn vt_client_language_command_rejects_invalid_codes_before_reload() {
    let mut client = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut client);
    assert_eq!(client.language(), LanguageCode { code: *b"en" });
    assert_eq!(client.vt_language(), LanguageCode { code: *b"en" });

    for bytes in [[0, b'e'], [b'd', 0], [0xFF, b'e'], [b'd', b' ']] {
        let mut data = [0u8; 8];
        data[0] = bytes[0];
        data[1] = bytes[1];
        client.handle_language_command(&Message::new(PGN_LANGUAGE_COMMAND, data.to_vec(), 0x80));
        assert_eq!(
            client.vt_language(),
            LanguageCode { code: *b"en" },
            "non-canonical language bytes must not update VT language"
        );
        assert_eq!(
            client.state(),
            VTState::Connected,
            "invalid language commands must not trigger a pool reload"
        );
    }

    client.handle_language_command(&Message::new(
        PGN_LANGUAGE_COMMAND,
        b"fr\0\0\0\0\0\0".to_vec(),
        0x81,
    ));
    assert_eq!(
        client.vt_language(),
        LanguageCode { code: *b"en" },
        "language commands from a non-negotiated VT source must be ignored"
    );
    assert_eq!(client.state(), VTState::Connected);

    client.handle_language_command(&Message::new(
        PGN_LANGUAGE_COMMAND,
        b"de\0\0\0\0\0\0".to_vec(),
        0x80,
    ));
    assert_eq!(client.vt_language(), LanguageCode { code: *b"de" });
    assert_eq!(client.state(), VTState::ReloadPool);
}

#[test]
fn vt_local_language_preferences_require_canonical_codes_before_state_mutation() {
    let mut client = VTClient::new(VTClientConfig::default());
    assert_eq!(client.language(), LanguageCode { code: *b"en" });

    for lang in ["", "e", "eng", "e1", "é"] {
        assert!(
            client.try_set_language_str(lang).is_err(),
            "local VT language preferences must reject non-canonical code {lang:?}"
        );
        assert_eq!(
            client.language(),
            LanguageCode { code: *b"en" },
            "failed local language validation must not mutate the client preference"
        );
    }

    assert!(
        client
            .try_set_language(LanguageCode { code: [0xFF, b'n'] })
            .is_err()
    );
    assert_eq!(client.language(), LanguageCode { code: *b"en" });

    client.try_set_language_str("de").unwrap();
    assert_eq!(client.language(), LanguageCode { code: *b"de" });
}
