#[test]
fn vt_bus_message_tan_constructors_encode_vt6_low_reserved_nibble() {
    let soft = machbus::isobus::vt::render::VtBusMessage::soft_key_activation_with_transfer_sequence_number(
        ActivationCode::Pressed,
        ObjectID::new(0x1234),
        ObjectID::new(0x0102),
        7,
        0x0A,
    )
    .unwrap();
    assert_eq!(soft.kind, VtBusMessageKind::SoftKeyActivation);
    assert_eq!(
        soft.as_bytes(),
        &[cmd::SOFT_KEY_ACTIVATION, 1, 0x34, 0x12, 0x02, 0x01, 7, 0xAF]
    );

    let button = machbus::isobus::vt::render::VtBusMessage::button_activation_with_transfer_sequence_number(
        ActivationCode::Released,
        ObjectID::new(0x2201),
        ObjectID::new(0x3302),
        3,
        0x02,
    )
    .unwrap();
    assert_eq!(button.kind, VtBusMessageKind::ButtonActivation);
    assert_eq!(
        button.as_bytes(),
        &[cmd::BUTTON_ACTIVATION, 0, 0x01, 0x22, 0x02, 0x33, 3, 0x2F]
    );

    let numeric = machbus::isobus::vt::render::VtBusMessage::numeric_value_change_with_transfer_sequence_number(
        ObjectID::new(0x0040),
        0xAABB_CCDD,
        0x0B,
    )
    .unwrap();
    assert_eq!(numeric.kind, VtBusMessageKind::NumericValueChange);
    assert_eq!(
        numeric.as_bytes(),
        &[cmd::NUMERIC_VALUE_CHANGE, 0x40, 0, 0xBF, 0xDD, 0xCC, 0xBB, 0xAA]
    );

    assert!(
        machbus::isobus::vt::render::VtBusMessage::numeric_value_change_with_transfer_sequence_number(
            ObjectID::new(1),
            0,
            0x10,
        )
        .is_err(),
        "VT6 TAN is a four-bit field"
    );
}

#[test]
fn vt_bus_message_parses_payload_and_full_message_roundtrip() {
    let message = machbus::isobus::vt::render::VtBusMessage::soft_key_activation_with_transfer_sequence_number(
        ActivationCode::Held,
        ObjectID::new(0x1001),
        ObjectID::new(0x2002),
        4,
        0x0C,
    )
    .unwrap();

    let parsed = machbus::isobus::vt::render::VtBusMessage::from_payload(message.as_bytes()).unwrap();
    assert_eq!(parsed, message);

    let framed = message.try_to_message(0x80, 0x42).unwrap();
    assert_eq!(framed.pgn, PGN_VT_TO_ECU);
    assert_eq!(framed.source, 0x80);
    assert_eq!(framed.destination, 0x42);
    assert_eq!(
        machbus::isobus::vt::render::VtBusMessage::from_message(&framed).unwrap(),
        message
    );

    let mut wrong_pgn = framed.clone();
    wrong_pgn.pgn = PGN_ECU_TO_VT;
    assert!(machbus::isobus::vt::render::VtBusMessage::from_message(&wrong_pgn).is_err());

    let mut wrong_destination = framed;
    wrong_destination.destination = BROADCAST_ADDRESS;
    assert!(
        machbus::isobus::vt::render::VtBusMessage::from_message(&wrong_destination).is_err(),
        "VT-to-ECU event notifications are destination-specific"
    );
}

#[test]
fn vt_bus_message_string_payload_parser_checks_length_utf8_and_object_id() {
    let message =
        machbus::isobus::vt::render::VtBusMessage::string_value_change(ObjectID::new(9), "ä")
            .unwrap();
    assert_eq!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(message.as_bytes()).unwrap(),
        message
    );

    let mut bad_len = message.as_bytes().to_vec();
    bad_len[3] = 3;
    bad_len[4] = 0;
    assert!(machbus::isobus::vt::render::VtBusMessage::from_payload(&bad_len).is_err());

    let invalid_utf8 = [cmd::STRING_VALUE_CHANGE, 9, 0, 1, 0, 0xFF];
    assert!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(&invalid_utf8).is_err(),
        "string value change payload bytes must be valid UTF-8"
    );

    let null_id = [cmd::STRING_VALUE_CHANGE, 0xFF, 0xFF, 0, 0];
    assert!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(&null_id).is_err(),
        "string value change needs a concrete object ID"
    );
}

#[test]
fn vt_bus_message_select_input_parser_checks_state_and_reserved_tail() {
    let cleared = machbus::isobus::vt::render::VtBusMessage::select_input_object(
        ObjectID::NULL,
        false,
        false,
    );
    assert_eq!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(cleared.as_bytes()).unwrap(),
        cleared
    );

    let selected_null = [cmd::SELECT_INPUT_OBJECT, 0xFF, 0xFF, 1, 0, 0xFF, 0xFF, 0xFF];
    assert!(machbus::isobus::vt::render::VtBusMessage::from_payload(&selected_null).is_err());

    let open_without_selected = [cmd::SELECT_INPUT_OBJECT, 1, 0, 0, 1, 0xFF, 0xFF, 0xFF];
    assert!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(&open_without_selected).is_err()
    );

    let bad_bool = [cmd::SELECT_INPUT_OBJECT, 1, 0, 2, 0, 0xFF, 0xFF, 0xFF];
    assert!(machbus::isobus::vt::render::VtBusMessage::from_payload(&bad_bool).is_err());

    let bad_tail = [cmd::SELECT_INPUT_OBJECT, 1, 0, 1, 0, 0, 0xFF, 0xFF];
    assert!(machbus::isobus::vt::render::VtBusMessage::from_payload(&bad_tail).is_err());
}

#[test]
fn vt_bus_message_pointing_parser_accepts_legacy_and_vt6_shapes() {
    let legacy = machbus::isobus::vt::render::VtBusMessage::pointing_event(
        10,
        20,
        ActivationCode::Held,
        ObjectID::new(0x1234),
        None,
    )
    .unwrap();
    assert_eq!(legacy.as_bytes(), &[cmd::POINTING_EVENT, 10, 0, 20, 0, 2, 0xFF, 0xFF]);
    assert_eq!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(legacy.as_bytes()).unwrap(),
        legacy
    );

    let vt6 = machbus::isobus::vt::render::VtBusMessage::pointing_event(
        100,
        200,
        ActivationCode::Pressed,
        ObjectID::new(0x3004),
        Some(0x07),
    )
    .unwrap();
    assert_eq!(vt6.as_bytes(), &[cmd::POINTING_EVENT, 100, 0, 200, 0, 0x71, 4, 0x30]);
    assert_eq!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(vt6.as_bytes()).unwrap(),
        vt6
    );

    let vt6_null_parent = [cmd::POINTING_EVENT, 1, 0, 2, 0, 0x11, 0xFF, 0xFF];
    assert!(machbus::isobus::vt::render::VtBusMessage::from_payload(&vt6_null_parent).is_err());

    let legacy_aborted = [cmd::POINTING_EVENT, 1, 0, 2, 0, 3, 0xFF, 0xFF];
    assert!(machbus::isobus::vt::render::VtBusMessage::from_payload(&legacy_aborted).is_err());
}

#[test]
fn vt_bus_message_user_layout_parser_checks_null_second_shape() {
    let hide_one = machbus::isobus::vt::render::VtBusMessage::user_layout_hide_show(
        (ObjectID::new(0x2222), false),
        None,
        Some(0x03),
    )
    .unwrap();
    assert_eq!(
        hide_one.as_bytes(),
        &[cmd::USER_LAYOUT_HIDE_SHOW, 0x22, 0x22, 0, 0xFF, 0xFF, 0, 0x3F]
    );
    assert_eq!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(hide_one.as_bytes()).unwrap(),
        hide_one
    );

    let null_second_shown = [
        cmd::USER_LAYOUT_HIDE_SHOW,
        1,
        0,
        1,
        0xFF,
        0xFF,
        1,
        0xFF,
    ];
    assert!(
        machbus::isobus::vt::render::VtBusMessage::from_payload(&null_second_shown).is_err(),
        "NULL second record is allowed only with shown=false"
    );

    let bad_tan_low_nibble = [cmd::USER_LAYOUT_HIDE_SHOW, 1, 0, 1, 2, 0, 0, 0x30];
    assert!(machbus::isobus::vt::render::VtBusMessage::from_payload(&bad_tan_low_nibble).is_err());
}
