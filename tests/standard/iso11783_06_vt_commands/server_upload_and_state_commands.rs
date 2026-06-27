#[test]
fn vt_server_object_pool_upload_window_is_per_client_and_not_unsolicited() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_standard_pool(&mut server, 0x42);
    assert!(server.clients()[0].pool.find(1u16).is_some());
    assert!(server.clients()[0].pool_activated);

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                alternate_object_pool_transfer(),
                0x42,
            ))
            .is_empty()
    );
    assert!(
        server.clients()[0].pool.find(10u16).is_none(),
        "a valid pool transfer outside the negotiated upload window must not replace an active pool"
    );
    assert!(
        server.clients()[0].pool_activated,
        "unsolicited pool data must not deactivate the current working set"
    );

    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                0x43,
            ))
            .len(),
        1
    );
    assert_eq!(server.clients().len(), 2);

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                vec![cmd::OBJECT_POOL_TRANSFER, 0x00, 0x00, 0x00],
                0x43,
            ))
            .is_empty()
    );
    assert!(
        server.clients()[1].pool.is_empty(),
        "malformed transfer payload must not consume or create per-client pool state"
    );

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                minimal_object_pool_transfer(),
                0x43,
            ))
            .is_empty()
    );
    assert!(server.clients()[1].pool_uploaded);
    assert!(!server.clients()[1].pool_activated);
    let mut malformed_end_of_pool = fixed_command(cmd::END_OF_POOL);
    malformed_end_of_pool[2] = 0x00;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, malformed_end_of_pool, 0x43,))
            .is_empty(),
        "non-canonical End Of Object Pool requests must not activate a pending pool"
    );
    assert!(!server.clients()[1].pool_activated);
    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        0x43,
    ));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].data[1], 0x00);
    assert!(server.clients()[1].pool_activated);
    assert!(
        server.clients()[0].pool_activated,
        "a second client's negotiated upload must not disturb the first active working set"
    );
}

#[test]
fn vt_server_delete_object_pool_requires_canonical_fixed_frame_before_reset() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_standard_pool(&mut server, 0x42);
    assert!(server.clients()[0].pool_activated);
    assert_eq!(server.active_working_set(), 0x42);
    let original_pool_size = server.clients()[0].pool.size();

    let mut malformed_delete = fixed_command(cmd::DELETE_OBJECT_POOL);
    malformed_delete[1] = 0x00;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, malformed_delete, 0x42))
            .is_empty()
    );
    assert_eq!(
        server.clients()[0].pool.size(),
        original_pool_size,
        "malformed Delete Object Pool must not clear the active pool"
    );
    assert!(server.clients()[0].pool_uploaded);
    assert!(server.clients()[0].pool_activated);
    assert_eq!(server.active_working_set(), 0x42);

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::DELETE_OBJECT_POOL),
                0x42,
            ))
            .is_empty()
    );
    assert!(server.clients()[0].pool.is_empty());
    assert!(!server.clients()[0].pool_uploaded);
    assert!(!server.clients()[0].pool_upload_allowed);
    assert!(!server.clients()[0].pool_activated);
    assert_eq!(
        server.active_working_set(),
        NULL_ADDRESS,
        "deleting the active client pool must release active-working-set ownership"
    );
}

#[test]
fn vt_server_reupload_failure_does_not_accept_stale_active_pool_as_new_upload() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_standard_pool(&mut server, 0x42);
    assert!(server.clients()[0].pool_uploaded);
    assert!(server.clients()[0].pool_activated);
    assert_eq!(server.active_working_set(), 0x42);
    let original_pool_size = server.clients()[0].pool.size();

    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::GET_MEMORY),
        0x42,
    ));
    assert_eq!(response.len(), 1);

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                vec![cmd::OBJECT_POOL_TRANSFER, 0x00, 0x00, 0x00],
                0x42,
            ))
            .is_empty()
    );
    assert_eq!(
        server.clients()[0].pool.size(),
        original_pool_size,
        "a malformed replacement upload must not overwrite the active pool"
    );

    let end_response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        0x42,
    ));
    assert_eq!(end_response.len(), 1);
    assert_ne!(
        end_response[0].data[1], 0x00,
        "EndOfPool after a failed replacement upload must not report success for the stale pool"
    );
    assert_ne!(end_response[0].data[6], 0x00);
    assert_eq!(
        server.clients()[0].pool.size(),
        original_pool_size,
        "failed reupload must leave the previous active pool available"
    );
    assert!(server.clients()[0].pool_activated);
    assert_eq!(server.active_working_set(), 0x42);
}

#[test]
fn vt_server_version_commands_do_not_create_clients_before_negotiation() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();

    let store = classic_label_command(cmd::STORE_VERSION, b"V1");
    let store_response =
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, store.clone(), 0x42));
    assert_eq!(store_response.len(), 1);
    assert_eq!(store_response[0].data[0], cmd::STORE_VERSION);
    assert_ne!(store_response[0].data[1], 0x00);
    assert!(
        server.clients().is_empty(),
        "version store before negotiation must not allocate a client slot"
    );

    let load = classic_label_command(cmd::LOAD_VERSION, b"V1");
    let load_response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, load, 0x42));
    assert_eq!(load_response.len(), 1);
    assert_eq!(load_response[0].data[0], cmd::LOAD_VERSION);
    assert_ne!(load_response[0].data[1], 0x00);
    assert!(
        server.clients().is_empty(),
        "version load before negotiation must not allocate a client slot"
    );

    let mut malformed_get_versions = fixed_command(cmd::GET_VERSIONS);
    malformed_get_versions[1] = 0x00;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, malformed_get_versions, 0x42,))
            .is_empty(),
        "non-canonical Get Versions requests must not produce a prefix-compatible response"
    );
    assert!(server.clients().is_empty());

    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                0x42,
            ))
            .len(),
        1
    );
    assert_eq!(server.clients().len(), 1);
    assert!(
        !server.clients()[0].pool_uploaded,
        "memory negotiation alone still does not imply a storable pool"
    );

    let store_response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, store, 0x42));
    assert_eq!(store_response.len(), 1);
    assert_ne!(store_response[0].data[1], 0x00);
    assert_eq!(
        server.clients().len(),
        1,
        "failed store must not create duplicate client state"
    );
}

#[test]
fn vt_server_runtime_commands_require_activated_object_pool_before_state_or_events() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();

    let numeric_events: Rc<RefCell<Vec<(ObjectID, u32)>>> = Rc::new(RefCell::new(Vec::new()));
    let numeric_log = numeric_events.clone();
    server
        .on_numeric_value_change
        .subscribe(move |event| numeric_log.borrow_mut().push(*event));

    let numeric_change = {
        let mut data = vec![cmd::CHANGE_NUMERIC_VALUE, 0x12, 0x00, 0xFF];
        data.extend(0x0102_0304u32.to_le_bytes());
        data
    };

    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, numeric_change.clone(), 0x42))
            .is_empty()
    );
    assert!(server.clients().is_empty());
    assert!(numeric_events.borrow().is_empty());

    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                0x42,
            ))
            .len(),
        1
    );
    assert_eq!(server.clients().len(), 1);

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, numeric_change.clone(), 0x42));
    assert!(
        server.clients()[0].object_state.numeric_values.is_empty(),
        "runtime object-state commands before pool activation must be ignored"
    );
    assert!(numeric_events.borrow().is_empty());

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        object_reference_pool_transfer(),
        0x42,
    ));
    assert!(server.clients()[0].pool_uploaded);
    assert!(!server.clients()[0].pool_activated);

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, numeric_change.clone(), 0x42));
    assert!(server.clients()[0].object_state.numeric_values.is_empty());
    assert!(numeric_events.borrow().is_empty());

    let end_response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        0x42,
    ));
    assert_eq!(end_response[0].data[1], 0x00);
    assert!(server.clients()[0].pool_activated);

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, numeric_change, 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(18)),
        Some(&0x0102_0304)
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects,
        vec![ServerRenderEffect::ChangeNumericValue {
            id: ObjectID(18),
            value: 0x0102_0304,
        }]
    );
    assert_eq!(*numeric_events.borrow(), vec![(ObjectID(18), 0x0102_0304)]);
}

#[test]
fn vt_server_records_accepted_render_effects_only_after_validation() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let hide_show = |id: u16, visible: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::HIDE_SHOW;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = visible;
        data.to_vec()
    };
    let enable_disable = |id: u16, enabled: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::ENABLE_DISABLE;
        data[1..3].copy_from_slice(&id.to_le_bytes());
        data[3] = enabled;
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, hide_show(26, 0), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, hide_show(2, 1), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, hide_show(2, 2), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, enable_disable(33, 0), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, enable_disable(32, 0), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, enable_disable(6, 0), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, enable_disable(33, 2), 0x42));

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        string_value_change(cmd::CHANGE_STRING_VALUE, 5, b"abc"),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        string_value_change(cmd::CHANGE_STRING_VALUE, 5, b"abcd"),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        string_value_change(cmd::CHANGE_STRING_VALUE, 2, b"abc"),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 7, 0xFF, 0x1112_1314),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 18, 0xFF, 0x1112_1314),
        0x42,
    ));
    let mut bad_string = string_value_change(cmd::CHANGE_STRING_VALUE, 5, b"bad");
    bad_string.push(0x00);
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_string, 0x42));

    assert_eq!(
        server.clients()[0].object_state.accepted_effects,
        vec![
            ServerRenderEffect::HideShow {
                id: ObjectID(26),
                visible: false,
            },
            ServerRenderEffect::EnableDisable {
                id: ObjectID(33),
                enabled: false,
            },
            ServerRenderEffect::EnableDisable {
                id: ObjectID(32),
                enabled: false,
            },
            ServerRenderEffect::ChangeStringValue {
                id: ObjectID(5),
                text: "abc".to_owned(),
            },
            ServerRenderEffect::ChangeNumericValue {
                id: ObjectID(18),
                value: 0x1112_1314,
            },
        ],
        "malformed, too-long, and wrong-target commands must not be replayed into the renderer"
    );
}

#[test]
fn vt_server_change_fill_attributes_requires_typed_non_null_pattern() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let change_fill_attributes = |fill_type: u8, colour: u8, pattern: ObjectID| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_FILL_ATTRIBUTES;
        data[1..3].copy_from_slice(&8u16.to_le_bytes());
        data[3] = fill_type;
        data[4] = colour;
        data[5..7].copy_from_slice(&pattern.to_le_bytes());
        data.to_vec()
    };

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_fill_attributes(0, 9, ObjectID::new(5)),
        0x42,
    ));
    assert!(
        server.clients()[0].object_state.attributes.is_empty(),
        "fixed Change Fill Attributes must reject non-PictureGraphic pattern references even when fill type is not pattern-fill"
    );
    assert!(
        server.clients()[0].object_state.accepted_effects.is_empty(),
        "rejected fixed Change Fill Attributes must not append render replay effects"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_fill_attributes(3, 4, ObjectID::NULL),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID::new(8), 3)),
        Some(&(ObjectID::NULL.raw() as u32)),
        "NULL remains a standard no-pattern selector for fixed Change Fill Attributes"
    );
    assert_eq!(server.clients()[0].object_state.accepted_effects.len(), 1);

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_fill_attributes(0, 5, ObjectID::new(23)),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID::new(8), 3)),
        Some(&23),
        "non-NULL fixed Change Fill Attributes patterns are valid when typed as PictureGraphic"
    );
}

#[test]
fn vt_server_external_object_pointer_numeric_value_is_four_byte_pointer_pair() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let pointer_value = (0x1234_u32 << 16) | u32::from(36_u16);
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 37, 0xFF, pointer_value),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(37)),
        Some(&pointer_value),
        "External Object Pointer Change Numeric Value carries External Reference NAME ID plus referenced Object ID"
    );
    assert!(server.clients()[0].object_state.accepted_effects.contains(
        &ServerRenderEffect::ChangeNumericValue {
            id: ObjectID(37),
            value: pointer_value,
        }
    ));

    let get_attribute = |object_id: u16, attribute_id: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_ATTRIBUTE_VALUE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = attribute_id;
        data.to_vec()
    };
    let get_response = |server: &mut VTServer, object_id: u16, attribute_id: u8| {
        let response = server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            get_attribute(object_id, attribute_id),
            0x42,
        ));
        assert_eq!(response.len(), 1);
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
    assert_eq!(get_response(&mut server, 37, 2), success(37, 2, 36));
    assert_eq!(get_response(&mut server, 37, 3), success(37, 3, 0x1234));

    let change_attribute = |object_id: u16, attribute_id: u8, value: u32| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_ATTRIBUTE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = attribute_id;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(37, 2, ObjectID::NULL.raw() as u32),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(37), 2)),
        Some(&(ObjectID::NULL.raw() as u32)),
        "External Object Pointer External Reference NAME Change Attribute must accept NULL to restore local-default fallback"
    );
    assert_eq!(
        get_response(&mut server, 37, 2),
        success(37, 2, ObjectID::NULL.raw() as u32)
    );

    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(
            cmd::CHANGE_NUMERIC_VALUE,
            37,
            0xFF,
            (0x5678_u32 << 16) | u32::from(5_u16),
        ),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(37)),
        Some(&pointer_value),
        "invalid External Reference NAME objects must not replace the retained pointer pair"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count,
        "invalid External Object Pointer numeric values must not append replay effects"
    );
}

#[test]
fn vt_server_numeric_value_rejects_invalid_scalar_and_pointer_values() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    for (object_id, value) in [(39, 1), (19, 5), (38, 23), (41, 23), (32, 255)] {
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, object_id, 0xFF, value),
            0x42,
        ));
        assert_eq!(
            server.clients()[0]
                .object_state
                .numeric_values
                .get(&ObjectID(object_id)),
            Some(&value),
            "valid Change Numeric Value should be retained for object {object_id}"
        );
    }

    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    for (object_id, invalid_value, old_value, reason) in [
        (39, 2, 1, "Input Boolean only admits TRUE/FALSE values"),
        (
            19,
            0x7777,
            5,
            "Object Pointer values must resolve to NULL or an uploaded object",
        ),
        (
            38,
            5,
            23,
            "Scaled Graphic values must resolve to a graphic value source",
        ),
        (
            38,
            19,
            23,
            "Scaled Graphic ObjectPointer values must resolve to a graphic value source",
        ),
        (
            41,
            5,
            23,
            "ObjectPointer retargets used by Scaled Graphic must remain graphic value sources",
        ),
        (
            32,
            2,
            255,
            "Animation Change Numeric Value must resolve to an existing child index or 255",
        ),
    ] {
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, object_id, 0xFF, invalid_value),
            0x42,
        ));
        assert_eq!(
            server.clients()[0]
                .object_state
                .numeric_values
                .get(&ObjectID(object_id)),
            Some(&old_value),
            "{reason}"
        );
    }
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count,
        "invalid Change Numeric Value commands must not append render replay effects"
    );
}

#[test]
fn vt_server_object_pointer_retargets_preserve_soft_key_and_key_group_context() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_key_pointer_context_pool(&mut server, 0x42);

    let change_pointer = |object_id: u16, value: u16| {
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, object_id, 0xFF, u32::from(value))
    };
    let change_attribute = |object_id: u16, attribute_id: u8, value: u32| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_ATTRIBUTE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = attribute_id;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        data.to_vec()
    };

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_pointer(40, 32), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(40)),
        Some(&32),
        "Key Group ObjectPointer children may retarget to another Key"
    );

    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    for (object_id, target, reason) in [
        (
            40,
            ObjectID::NULL.raw(),
            "Key Group ObjectPointer children must not retarget to NULL",
        ),
        (
            40,
            50,
            "Key Group ObjectPointer children must not retarget to non-Key objects",
        ),
    ] {
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            change_pointer(object_id, target),
            0x42,
        ));
        assert_eq!(
            server.clients()[0]
                .object_state
                .numeric_values
                .get(&ObjectID(object_id)),
            Some(&32),
            "{reason}"
        );
    }
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count,
        "invalid Key Group pointer retargets must not append replay effects"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_pointer(41, ObjectID::NULL.raw()),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(41)),
        Some(&u32::from(ObjectID::NULL.raw())),
        "Soft Key Mask ObjectPointer children may retarget to NULL reserved slots"
    );

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_pointer(41, 50), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(41)),
        Some(&u32::from(ObjectID::NULL.raw())),
        "Soft Key Mask ObjectPointer children must not retarget to non-Key objects"
    );

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_pointer(41, 32), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(41)),
        Some(&32),
        "Soft Key Mask ObjectPointer children may retarget back to Keys"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(42, 1, u32::from(ObjectID::NULL.raw())),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(42), 1)),
        Some(&u32::from(ObjectID::NULL.raw())),
        "Soft Key Mask External Object Pointer children may use NULL default reserved slots"
    );

    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(42, 1, 50),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(42), 1)),
        Some(&u32::from(ObjectID::NULL.raw())),
        "Soft Key Mask External Object Pointer defaults must not retarget to non-Key objects"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count,
        "invalid External Object Pointer default retargets must not append replay effects"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(42, 1, 32),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(42), 1)),
        Some(&32),
        "Soft Key Mask External Object Pointer defaults may retarget back to Keys"
    );
}

#[test]
fn vt_value_change_payloads_require_canonical_reserved_padding_and_utf8() {
    let valid_vt_numeric =
        numeric_value_change(cmd::NUMERIC_VALUE_CHANGE, 0x1234, 0xFF, 0x0102_0304);
    let bad_vt_numeric_reserved =
        numeric_value_change(cmd::NUMERIC_VALUE_CHANGE, 0x1234, 0x00, 0xA0A1_A2A3);
    let short_vt_numeric = valid_vt_numeric[..7].to_vec();

    let mut valid_vt_string = string_value_change(cmd::STRING_VALUE_CHANGE, 0x1234, b"ok");
    valid_vt_string.resize(8, 0xFF);
    let mut bad_vt_string_tail = string_value_change(cmd::STRING_VALUE_CHANGE, 0x1234, b"no");
    bad_vt_string_tail.push(0x00);
    let mut bad_vt_string_utf8 = string_value_change(cmd::STRING_VALUE_CHANGE, 0x1234, &[0xFF]);
    bad_vt_string_utf8.resize(8, 0xFF);

    let mut client = VTClient::new(VTClientConfig::default());
    let client_numeric_events: Rc<RefCell<Vec<(ObjectID, u32)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let client_numeric_log = client_numeric_events.clone();
    client
        .on_numeric_value_change
        .subscribe(move |event| client_numeric_log.borrow_mut().push(*event));
    let client_string_events: Rc<RefCell<Vec<(ObjectID, String)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let client_string_log = client_string_events.clone();
    client
        .on_string_value_change
        .subscribe(move |event| client_string_log.borrow_mut().push(event.clone()));

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, valid_vt_numeric.clone(), 0x80));
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_vt_numeric_reserved, 0x80));
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, short_vt_numeric, 0x80));
    assert_eq!(
        *client_numeric_events.borrow(),
        vec![(ObjectID(0x1234), 0x0102_0304)]
    );

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, valid_vt_string.clone(), 0x80));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        bad_vt_string_tail.clone(),
        0x80,
    ));
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        bad_vt_string_utf8.clone(),
        0x80,
    ));
    assert_eq!(
        *client_string_events.borrow(),
        vec![(ObjectID(0x1234), String::from("ok"))]
    );

    let mut tracker = VTClientStateTracker::new();
    tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, valid_vt_numeric, 0x80));
    assert_eq!(tracker.numeric_value(0x1234), Some(0x0102_0304));
    tracker.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        numeric_value_change(cmd::NUMERIC_VALUE_CHANGE, 0x1234, 0x00, 0xB0B1_B2B3),
        0x81,
    ));
    assert_eq!(tracker.numeric_value(0x1234), Some(0x0102_0304));
    assert_eq!(
        tracker.vt_address(),
        0x80,
        "malformed value change must not rebind the tracked VT source"
    );

    tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, valid_vt_string, 0x80));
    assert_eq!(tracker.string_value(0x1234), Some("ok"));
    tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_vt_string_tail, 0x81));
    tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_vt_string_utf8, 0x81));
    assert_eq!(tracker.string_value(0x1234), Some("ok"));
    assert_eq!(tracker.vt_address(), 0x80);

    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);
    let server_numeric_events: Rc<RefCell<Vec<(ObjectID, u32)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let server_numeric_log = server_numeric_events.clone();
    server
        .on_numeric_value_change
        .subscribe(move |event| server_numeric_log.borrow_mut().push(*event));
    let server_string_events: Rc<RefCell<Vec<(ObjectID, String)>>> =
        Rc::new(RefCell::new(Vec::new()));
    let server_string_log = server_string_events.clone();
    server
        .on_string_value_change
        .subscribe(move |event| server_string_log.borrow_mut().push(event.clone()));

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 0x0012, 0xFF, 0x1112_1314),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 0x0012, 0x00, 0x2122_2324),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(18)),
        Some(&0x1112_1314)
    );
    assert_eq!(
        *server_numeric_events.borrow(),
        vec![(ObjectID(18), 0x1112_1314)]
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 0x0013, 0xFF, 0x0000_0005),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 0x0013, 0xFF, 0x0100_0005),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 0x0014, 0xFF, 0x0000_0001),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 0x0014, 0xFF, 0x0000_0101),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        numeric_value_change(cmd::CHANGE_NUMERIC_VALUE, 0x0007, 0xFF, 0x0000_0001),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(19)),
        Some(&0x0000_0005),
        "2-byte numeric targets accept only low two value bytes"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .numeric_values
            .get(&ObjectID(20)),
        Some(&0x0000_0001),
        "1-byte numeric targets accept only the low value byte"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .numeric_values
            .contains_key(&ObjectID(7)),
        "non-numeric object targets must be rejected"
    );
    assert_eq!(
        *server_numeric_events.borrow(),
        vec![
            (ObjectID(18), 0x1112_1314),
            (ObjectID(19), 0x0000_0005),
            (ObjectID(20), 0x0000_0001),
        ]
    );

    let mut valid_ecu_string = string_value_change(cmd::CHANGE_STRING_VALUE, 0x0005, b"go");
    valid_ecu_string.resize(8, 0xFF);
    let mut bad_ecu_string_tail = string_value_change(cmd::CHANGE_STRING_VALUE, 0x0005, b"no");
    bad_ecu_string_tail.push(0x00);
    let mut bad_ecu_string_utf8 = string_value_change(cmd::CHANGE_STRING_VALUE, 0x0005, &[0xFF]);
    bad_ecu_string_utf8.resize(8, 0xFF);

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, valid_ecu_string, 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_ecu_string_tail, 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_ecu_string_utf8, 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .string_values
            .get(&ObjectID(5))
            .map(String::as_str),
        Some("go ")
    );
    assert_eq!(
        *server_string_events.borrow(),
        vec![(ObjectID(5), String::from("go "))]
    );
}

#[test]
fn vt_server_change_commands_reject_malformed_payloads_without_state_mutation() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_standard_pool(&mut server, 0x42);

    let mut hide_show = [0xFFu8; 8];
    hide_show[0] = cmd::HIDE_SHOW;
    hide_show[1..3].copy_from_slice(&0x1001u16.to_le_bytes());
    hide_show[3] = 1;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, hide_show.to_vec(), 0x42))
            .is_empty()
    );

    let state = &server.clients()[0].object_state;
    assert!(
        !state.visibility.contains_key(&ObjectID(0x1001)),
        "Hide/Show Object targets only activated Container objects"
    );

    let mut bad_bool = hide_show;
    bad_bool[1..3].copy_from_slice(&0x1002u16.to_le_bytes());
    bad_bool[3] = 2;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_bool.to_vec(), 0x42))
            .is_empty()
    );
    assert!(
        !server.clients()[0]
            .object_state
            .visibility
            .contains_key(&ObjectID(0x1002))
    );

    let mut bad_tail = hide_show;
    bad_tail[1..3].copy_from_slice(&0x1003u16.to_le_bytes());
    bad_tail[4] = 0;
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_tail.to_vec(), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .visibility
            .contains_key(&ObjectID(0x1003))
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        vec![cmd::CHANGE_STRING_VALUE, 0x04, 0x20, 0x03, 0x00, b'o'],
        0x42,
    ));
    assert!(
        !server.clients()[0]
            .object_state
            .string_values
            .contains_key(&ObjectID(0x2004))
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        vec![
            cmd::CHANGE_OBJECT_LABEL,
            0x05,
            0x20,
            3,
            b'o',
            0xFF,
            b'k',
            0xFF,
        ],
        0x42,
    ));
    assert!(
        !server.clients()[0]
            .object_state
            .object_labels
            .contains_key(&ObjectID(0x2005))
    );
}

#[test]
fn vt_server_change_active_mask_requires_uploaded_working_set_and_mask_references() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_standard_pool(&mut server, 0x42);

    let change_active_mask = |working_set_id: u16, mask_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_ACTIVE_MASK;
        data[1..3].copy_from_slice(&working_set_id.to_le_bytes());
        data[3..5].copy_from_slice(&mask_id.to_le_bytes());
        data.to_vec()
    };

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_active_mask(1, 2), 0x42));
    assert_eq!(
        server.clients()[0].object_state.active_data_mask,
        ObjectID(2)
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_active_mask(1, 0x7777),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.active_data_mask,
        ObjectID(2),
        "unknown mask references must not mutate active-mask state"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_active_mask(0x7777, 2),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.active_data_mask,
        ObjectID(2),
        "unknown Working Set references must not mutate active-mask state"
    );

    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_active_mask(1, 1), 0x42));
    assert_eq!(
        server.clients()[0].object_state.active_data_mask,
        ObjectID(2),
        "references to the wrong uploaded object type must not mutate active-mask state"
    );
}

#[test]
fn vt_server_reference_mutations_require_uploaded_object_types() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let soft_key_mask_command = |mask_type: u8, mask_id: u16, soft_key_mask_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_SOFT_KEY_MASK;
        data[1] = mask_type;
        data[2..4].copy_from_slice(&mask_id.to_le_bytes());
        data[4..6].copy_from_slice(&soft_key_mask_id.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        soft_key_mask_command(1, 2, 3),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.active_soft_key_mask,
        ObjectID(3)
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .soft_key_masks
            .get(&ObjectID(2)),
        Some(&ObjectID(3)),
        "mask type 1 applies to Data Mask soft-key references"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        soft_key_mask_command(2, 22, ObjectID::NULL.raw()),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .soft_key_masks
            .get(&ObjectID(22)),
        Some(&ObjectID::NULL),
        "mask type 2 applies to Alarm Mask soft-key references and may clear the mask with NULL"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        soft_key_mask_command(1, 0x7777, 3),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        soft_key_mask_command(2, 2, 3),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        soft_key_mask_command(3, 2, 3),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        soft_key_mask_command(1, 2, 5),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.active_soft_key_mask,
        ObjectID::NULL,
        "soft-key mask mutation must ignore unknown masks, mask-type/object mismatches, reserved mask types, and wrong soft-key object types"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .soft_key_masks
            .get(&ObjectID(2)),
        Some(&ObjectID(3))
    );

    let list_item_command = |list_id: u16, index: u8, item_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_LIST_ITEM;
        data[1..3].copy_from_slice(&list_id.to_le_bytes());
        data[3] = index;
        data[4..6].copy_from_slice(&item_id.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        list_item_command(4, 0, 5),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .list_items
            .get(&(ObjectID(4), 0)),
        Some(&ObjectID(5))
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        list_item_command(32, 1, 5),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .list_items
            .get(&(ObjectID(32), 1)),
        Some(&ObjectID(5)),
        "Animation frame lists are standard Change List Item targets"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        list_item_command(5, 2, 4),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        list_item_command(4, 2, 0x7777),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        list_item_command(4, 1, 5),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        list_item_command(20, 0, 6),
        0x42,
    ));
    assert!(
        !server.clients()[0]
            .object_state
            .list_items
            .contains_key(&(ObjectID(5), 2))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .list_items
            .contains_key(&(ObjectID(4), 2)),
        "list item mutation must ignore wrong list types and unknown item objects"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .list_items
            .contains_key(&(ObjectID(4), 1)),
        "list item mutation must reject indexes outside the uploaded list"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .list_items
            .contains_key(&(ObjectID(20), 0)),
        "Output List item mutation must reject style/reference metadata that cannot be displayed"
    );

    let change_priority = |alarm_id: u16, priority: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_PRIORITY;
        data[1..3].copy_from_slice(&alarm_id.to_le_bytes());
        data[3] = priority;
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_priority(22, AlarmPriority::Warning as u8),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .priorities
            .get(&ObjectID(22)),
        Some(&(AlarmPriority::Warning as u8))
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_priority(5, AlarmPriority::Critical as u8),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_priority(22, 3), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .priorities
            .contains_key(&ObjectID(5)),
        "priority changes must target Alarm Mask objects"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .priorities
            .get(&ObjectID(22)),
        Some(&(AlarmPriority::Warning as u8)),
        "invalid priority values must not mutate retained priority state"
    );

    let font_attribute_command =
        |object_id: u16, colour: u8, size: u8, font_type: u8, style: u8| {
            let mut data = [0xFFu8; 8];
            data[0] = cmd::CHANGE_FONT_ATTRIBUTES;
            data[1..3].copy_from_slice(&object_id.to_le_bytes());
            data[3] = colour;
            data[4] = size;
            data[5] = font_type;
            data[6] = style;
            data.to_vec()
        };
    let line_attribute_command = |object_id: u16, colour: u8, width: u8, line_art: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_LINE_ATTRIBUTES;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = colour;
        data[4] = width;
        data[5..7].copy_from_slice(&line_art.to_le_bytes());
        data.to_vec()
    };
    let fill_attribute_command = |object_id: u16, fill_type: u8, colour: u8, pattern: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_FILL_ATTRIBUTES;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = fill_type;
        data[4] = colour;
        data[5..7].copy_from_slice(&pattern.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        font_attribute_command(6, 5, 6, 1, 0x0A),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 1)),
        Some(&5)
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.last(),
        Some(&ServerRenderEffect::ChangeFontAttributeValues {
            id: ObjectID(6),
            colour: 5,
            size: 6,
            font_type: 1,
            style: 0x0A,
        })
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        font_attribute_command(6, 9, 64, 7, 0x80),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.last(),
        Some(&ServerRenderEffect::ChangeFontAttributeValues {
            id: ObjectID(6),
            colour: 9,
            size: 64,
            font_type: 7,
            style: 0x80,
        }),
        "proportional Font Attributes commands must admit standard 8..=N height values when bit 7 is set"
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        font_attribute_command(6, 7, 15, 1, 0),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        font_attribute_command(6, 7, 7, 1, 0x80),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        font_attribute_command(6, 7, 20, 3, 0x80),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        line_attribute_command(0x7777, 9, 3, 0x00C0),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        line_attribute_command(7, 9, 3, 0x00C0),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.last(),
        Some(&ServerRenderEffect::ChangeLineAttributeValues {
            id: ObjectID(7),
            colour: 9,
            width: 3,
            line_art: 0x00C0,
        })
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fill_attribute_command(8, 2, 7, 0xFFFF),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.last(),
        Some(&ServerRenderEffect::ChangeFillAttributeValues {
            id: ObjectID(8),
            fill_type: 2,
            colour: 7,
            pattern: ObjectID::NULL,
        })
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fill_attribute_command(8, 4, 7, 0xFFFF),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 2)),
        Some(&64),
        "invalid font-size/type mutations must not replace retained font attributes"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(0x7777), 1)),
        "line-attribute command must ignore unknown LineAttributes objects"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(8), 1)),
        Some(&2),
        "fill-attribute command must reject reserved fill types"
    );
}

#[test]
fn vt_server_child_geometry_and_input_selection_require_uploaded_objects() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let select_input = |object_id: u16, option: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_INPUT_OBJECT_COMMAND;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = option;
        data.to_vec()
    };

    let select_input_response = |object_id: u16, response: u8, error_bits: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_INPUT_OBJECT_COMMAND;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = response;
        data[4] = error_bits;
        machbus::isobus::vt::OutboundFrame::to(data.to_vec(), 0x42)
    };
    let change_soft_key_mask = |data_mask: u16, soft_key_mask: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_SOFT_KEY_MASK;
        data[1] = 1;
        data[2..4].copy_from_slice(&data_mask.to_le_bytes());
        data[4..6].copy_from_slice(&soft_key_mask.to_le_bytes());
        data.to_vec()
    };

    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(4, 0), 0x42)),
        vec![select_input_response(4, 2, 0)],
        "Select Input Object option 0 must return the standard opened-for-edit response"
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID(4)
    );
    assert_eq!(
        server.clients()[0].object_state.open_input_object,
        ObjectID(4),
        "Select Input Object option 0 must retain the open-for-data-input state for input fields"
    );

    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(33, 0xFF), 0x42)),
        vec![select_input_response(33, 0, 0x08)],
        "Select Input Object must report busy when another input field is open"
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID(4),
        "busy Select Input Object response must not replace retained focus"
    );

    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(4, 0xFF), 0x42)),
        vec![select_input_response(4, 1, 0)],
        "Select Input Object option FF must return the standard focus-only response"
    );
    assert_eq!(
        server.clients()[0].object_state.open_input_object,
        ObjectID::NULL
    );

    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(33, 0xFF), 0x42)),
        vec![select_input_response(33, 1, 0)],
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID(33),
        "VT4+ Select Input Object must admit Button focus targets"
    );
    assert_eq!(
        server.clients()[0].object_state.open_input_object,
        ObjectID::NULL,
        "Button focus targets must not retain open-for-input state"
    );

    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(34, 0xFF), 0x42)),
        vec![select_input_response(34, 1, 0)],
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID(34),
        "VT4+ Select Input Object must admit Key focus targets"
    );

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                change_soft_key_mask(2, ObjectID::NULL.raw()),
                0x42,
            ))
            .is_empty()
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(4, 0xFF), 0x42)),
        vec![select_input_response(4, 1, 0)],
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(34, 0xFF), 0x42)),
        vec![select_input_response(34, 0, 0x04)],
        "Select Input Object must treat Keys on a cleared active Soft Key Mask as not visible"
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID(4),
        "off-mask Key selection must not replace retained focus"
    );
    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                change_soft_key_mask(2, 3),
                0x42,
            ))
            .is_empty()
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(34, 0xFF), 0x42)),
        vec![select_input_response(34, 1, 0)],
        "restoring the active Soft Key Mask must make its Key selectable again"
    );

    assert_eq!(
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            select_input(ObjectID::NULL.raw(), 0xFF),
            0x42,
        )),
        vec![select_input_response(ObjectID::NULL.raw(), 0, 0)],
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID::NULL,
        "NULL target with option FF must remove focus"
    );
    assert_eq!(
        server.clients()[0].object_state.open_input_object,
        ObjectID::NULL
    );

    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(4, 0xFF), 0x42)),
        vec![select_input_response(4, 1, 0)],
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID(4)
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(33, 0), 0x42)),
        vec![select_input_response(33, 0, 0x20)]
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(34, 0), 0x42)),
        vec![select_input_response(34, 0, 0x20)]
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(4, 1), 0x42)),
        vec![select_input_response(4, 0, 0x20)]
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(5, 0xFF), 0x42)),
        vec![select_input_response(5, 0, 0x02)]
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            select_input(0x7777, 0xFF),
            0x42,
        )),
        vec![select_input_response(0x7777, 0, 0x02)]
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(24, 0xFF), 0x42)),
        vec![select_input_response(24, 0, 0x04)],
        "selecting an otherwise valid object outside the active mask must report the standard visibility error"
    );
    let mut hide_container = [0xFFu8; 8];
    hide_container[0] = cmd::HIDE_SHOW;
    hide_container[1..3].copy_from_slice(&26u16.to_le_bytes());
    hide_container[3] = 0;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, hide_container.to_vec(), 0x42))
            .is_empty()
    );
    assert_eq!(
        server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, select_input(25, 0xFF), 0x42)),
        vec![select_input_response(25, 0, 0x04)],
        "selecting an object inside a hidden container must report the standard visibility error"
    );
    assert_eq!(
        server.clients()[0].object_state.selected_input_object,
        ObjectID(4),
        "input selection must ignore invalid options, non-selectable objects, invalid open targets, hidden/off-mask targets, and unknown object references"
    );

    let esc_response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::ESC_INPUT),
        0x42,
    ));
    assert_eq!(
        esc_response,
        vec![machbus::isobus::vt::OutboundFrame::to(
            vec![cmd::VT_ESC, 4, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            0x42,
        )],
        "accepted ESC Input must be acknowledged as a VT ESC response for the selected input"
    );
    assert_eq!(server.clients()[0].object_state.input_escape_count, 1);
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.last(),
        Some(&ServerRenderEffect::Esc)
    );

    let mut malformed_esc = fixed_command(cmd::ESC_INPUT);
    malformed_esc[1] = 0;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, malformed_esc, 0x42))
            .is_empty(),
        "malformed ESC Input reserved bytes must not emit a VT ESC response"
    );
    assert_eq!(
        server.clients()[0].object_state.input_escape_count,
        1,
        "malformed ESC Input must not mutate retained server state"
    );

    let child_location = |parent_id: u16, child_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_CHILD_LOCATION;
        data[1..3].copy_from_slice(&parent_id.to_le_bytes());
        data[3..5].copy_from_slice(&child_id.to_le_bytes());
        data[5] = 10;
        data[6] = 11;
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, child_location(1, 2), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .child_locations
            .get(&(ObjectID(1), ObjectID(2))),
        Some(&(10, 11))
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        child_location(1, 0x7777),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        child_location(0x7777, 2),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, child_location(2, 6), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .child_locations
            .contains_key(&(ObjectID(1), ObjectID(0x7777)))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .child_locations
            .contains_key(&(ObjectID(0x7777), ObjectID(2))),
        "child-location mutation must ignore unknown parent or child references"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .child_locations
            .contains_key(&(ObjectID(2), ObjectID(6))),
        "child-location mutation must reject existing objects when the parent does not own the child"
    );

    let child_position = |parent_id: u16, child_id: u16| {
        let mut data = vec![cmd::CHANGE_CHILD_POSITION];
        data.extend(parent_id.to_le_bytes());
        data.extend(child_id.to_le_bytes());
        data.extend(0x0123u16.to_le_bytes());
        data.extend(0x0456u16.to_le_bytes());
        data
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, child_position(1, 2), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .child_positions
            .get(&(ObjectID(1), ObjectID(2))),
        Some(&(0x0123, 0x0456))
    );

    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        child_position(1, 0x7777),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        child_position(0x7777, 2),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, child_position(2, 6), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .child_positions
            .contains_key(&(ObjectID(1), ObjectID(0x7777)))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .child_positions
            .contains_key(&(ObjectID(0x7777), ObjectID(2))),
        "child-position mutation must ignore unknown parent or child references"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .child_positions
            .contains_key(&(ObjectID(2), ObjectID(6))),
        "child-position mutation must reject existing objects when the parent does not own the child"
    );
}
