#[test]
fn soft_key_activation_response_builds_vt5_reserved_payload() {
    let response = machbus::isobus::vt::render::ControlActivationResponse::soft_key(
        ActivationCode::Pressed,
        ObjectID::new(0x1234),
        ObjectID::new(0x0020),
        7,
        None,
    )
    .unwrap();

    let payload = response.to_payload_for_vt_version(5).unwrap();
    assert_eq!(
        payload,
        [cmd::SOFT_KEY_ACTIVATION, 1, 0x34, 0x12, 0x20, 0, 7, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::ControlActivationResponse::from_payload_for_vt_version(
            &payload, 5,
        )
        .unwrap(),
        response
    );
}

#[test]
fn button_activation_response_builds_and_parses_vt6_tan_payload() {
    let response = machbus::isobus::vt::render::ControlActivationResponse::button(
        ActivationCode::Held,
        ObjectID::new(0x2345),
        ObjectID::new(0x0100),
        9,
        Some(0x0A),
    )
    .unwrap();

    let payload = response.to_payload_for_vt_version(6).unwrap();
    assert_eq!(
        payload,
        [cmd::BUTTON_ACTIVATION, 2, 0x45, 0x23, 0, 1, 9, 0xAF]
    );
    assert_eq!(
        machbus::isobus::vt::render::ControlActivationResponse::from_payload_for_vt_version(
            &payload, 6,
        )
        .unwrap(),
        response
    );
}

#[test]
fn activation_response_rejects_noncanonical_shapes() {
    let response = machbus::isobus::vt::render::ControlActivationResponse::button(
        ActivationCode::Released,
        ObjectID::new(1),
        ObjectID::new(2),
        3,
        None,
    )
    .unwrap();
    assert!(
        response.to_payload_for_vt_version(6).is_err(),
        "VT6 activation responses must carry the response TAN"
    );
    assert!(
        machbus::isobus::vt::render::ControlActivationResponse::button(
            ActivationCode::Released,
            ObjectID::NULL,
            ObjectID::new(2),
            3,
            None,
        )
        .is_err(),
        "activation response object IDs must be concrete"
    );

    let mut bad = [cmd::SOFT_KEY_ACTIVATION, 4, 1, 0, 2, 0, 3, 0xFF];
    assert!(
        machbus::isobus::vt::render::ControlActivationResponse::from_payload_for_vt_version(
            &bad, 5,
        )
        .is_err(),
        "activation code is limited to 0..=3"
    );
    bad[1] = 1;
    bad[7] = 0x40;
    assert!(
        machbus::isobus::vt::render::ControlActivationResponse::from_payload_for_vt_version(
            &bad, 6,
        )
        .is_err(),
        "VT6 activation responses reserve the low TAN nibble as 0xF"
    );
}

#[test]
fn activation_response_wraps_and_parses_ecu_to_vt_message() {
    let response = machbus::isobus::vt::render::ControlActivationResponse::soft_key(
        ActivationCode::Aborted,
        ObjectID::new(0x0201),
        ObjectID::new(0x0304),
        0,
        Some(0x05),
    )
    .unwrap();

    let message = response.to_message(0x42, 0x80, 6).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        message.data.as_slice(),
        &[cmd::SOFT_KEY_ACTIVATION, 3, 1, 2, 4, 3, 0, 0x5F]
    );
    assert_eq!(
        machbus::isobus::vt::render::ControlActivationResponse::from_message(&message, 6)
            .unwrap(),
        response
    );
    assert!(
        machbus::isobus::vt::render::ControlActivationResponse::from_message(
            &Message::new(PGN_ECU_TO_VT, message.data.clone(), 0x42),
            6,
        )
        .is_err(),
        "full response parser requires a concrete VT destination"
    );
}

#[test]
fn pointing_event_response_builds_vt3_vt5_and_vt6_payloads() {
    let legacy = machbus::isobus::vt::render::PointingEventResponse::new(
        11,
        12,
        ActivationCode::Pressed,
        None,
        None,
    )
    .unwrap();
    let vt3_payload = legacy.to_payload_for_vt_version(3).unwrap();
    assert_eq!(
        vt3_payload,
        [cmd::POINTING_EVENT, 11, 0, 12, 0, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::PointingEventResponse::from_payload_for_vt_version(
            &vt3_payload,
            3,
        )
        .unwrap(),
        legacy
    );

    let vt5_payload = legacy.to_payload_for_vt_version(5).unwrap();
    assert_eq!(
        vt5_payload,
        [cmd::POINTING_EVENT, 11, 0, 12, 0, 1, 0xFF, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::PointingEventResponse::from_payload_for_vt_version(
            &vt5_payload,
            5,
        )
        .unwrap(),
        legacy
    );

    let vt6 = machbus::isobus::vt::render::PointingEventResponse::new(
        300,
        200,
        ActivationCode::Held,
        Some(ObjectID::new(0x0770)),
        Some(0x04),
    )
    .unwrap();
    let vt6_payload = vt6.to_payload_for_vt_version(6).unwrap();
    assert_eq!(
        vt6_payload,
        [cmd::POINTING_EVENT, 0x2C, 1, 0xC8, 0, 0x42, 0x70, 0x07]
    );
    assert_eq!(
        machbus::isobus::vt::render::PointingEventResponse::from_payload_for_vt_version(
            &vt6_payload,
            6,
        )
        .unwrap(),
        vt6
    );
}

#[test]
fn pointing_event_response_rejects_invalid_touch_and_vt6_context() {
    assert!(
        machbus::isobus::vt::render::PointingEventResponse::new(
            1,
            2,
            ActivationCode::Aborted,
            None,
            None,
        )
        .is_err(),
        "pointing events do not use the activation Aborted state"
    );

    let vt6_missing_context = machbus::isobus::vt::render::PointingEventResponse::new(
        1,
        2,
        ActivationCode::Pressed,
        None,
        None,
    )
    .unwrap();
    assert!(
        vt6_missing_context.to_payload_for_vt_version(6).is_err(),
        "VT6 pointing responses need TAN and parent mask"
    );

    let bad_reserved = [cmd::POINTING_EVENT, 1, 0, 2, 0, 1, 0, 0];
    assert!(
        machbus::isobus::vt::render::PointingEventResponse::from_payload_for_vt_version(
            &bad_reserved,
            5,
        )
        .is_err(),
        "VT5 pointing response tail bytes stay reserved"
    );

    let bad_touch = [cmd::POINTING_EVENT, 1, 0, 2, 0, 0x43, 1, 0];
    assert!(
        machbus::isobus::vt::render::PointingEventResponse::from_payload_for_vt_version(
            &bad_touch, 6,
        )
        .is_err(),
        "VT6 pointing response low nibble is a touch state, not reserved data"
    );
}

#[test]
fn pointing_event_response_wraps_and_parses_ecu_to_vt_message() {
    let response = machbus::isobus::vt::render::PointingEventResponse::new(
        7,
        8,
        ActivationCode::Released,
        Some(ObjectID::new(0x090A)),
        Some(0x0B),
    )
    .unwrap();

    let message = response.to_message(0x42, 0x80, 6).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        message.data.as_slice(),
        &[cmd::POINTING_EVENT, 7, 0, 8, 0, 0xB0, 0x0A, 0x09]
    );
    assert_eq!(
        machbus::isobus::vt::render::PointingEventResponse::from_message(&message, 6).unwrap(),
        response
    );
    assert!(
        machbus::isobus::vt::render::PointingEventResponse::from_message(
            &Message::new(PGN_ECU_TO_VT, message.data.clone(), 0x42),
            6,
        )
        .is_err(),
        "full response parser requires a concrete VT destination"
    );
}

#[test]
fn select_input_object_response_builds_versioned_payloads() {
    let response = machbus::isobus::vt::render::SelectInputObjectResponse::new(
        ObjectID::new(0x1234),
        true,
        true,
        Some(0x06),
    )
    .unwrap();

    let vt4_payload = response.to_payload_for_vt_version(4).unwrap();
    assert_eq!(
        vt4_payload,
        [cmd::SELECT_INPUT_OBJECT, 0x34, 0x12, 1, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    let vt4_parsed =
        machbus::isobus::vt::render::SelectInputObjectResponse::from_payload_for_vt_version(
            &vt4_payload,
            4,
        )
        .unwrap();
    assert_eq!(vt4_parsed.object_id, ObjectID::new(0x1234));
    assert!(vt4_parsed.selected);
    assert!(!vt4_parsed.open_for_input);
    assert_eq!(vt4_parsed.transfer_sequence_number, None);

    let vt5_payload = response.to_payload_for_vt_version(5).unwrap();
    assert_eq!(
        vt5_payload,
        [cmd::SELECT_INPUT_OBJECT, 0x34, 0x12, 1, 1, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::SelectInputObjectResponse::from_payload_for_vt_version(
            &vt5_payload,
            5,
        )
        .unwrap(),
        machbus::isobus::vt::render::SelectInputObjectResponse::new(
            ObjectID::new(0x1234),
            true,
            true,
            None,
        )
        .unwrap()
    );

    let vt6_payload = response.to_payload_for_vt_version(6).unwrap();
    assert_eq!(
        vt6_payload,
        [cmd::SELECT_INPUT_OBJECT, 0x34, 0x12, 1, 1, 0xFF, 0xFF, 0x6F]
    );
    assert_eq!(
        machbus::isobus::vt::render::SelectInputObjectResponse::from_payload_for_vt_version(
            &vt6_payload,
            6,
        )
        .unwrap(),
        response
    );
}

#[test]
fn select_input_object_response_rejects_bad_flags_and_context() {
    assert!(
        machbus::isobus::vt::render::SelectInputObjectResponse::new(
            ObjectID::NULL,
            true,
            false,
            None,
        )
        .is_err(),
        "select-input response object ID must be concrete"
    );
    assert!(
        machbus::isobus::vt::render::SelectInputObjectResponse::new(
            ObjectID::new(1),
            false,
            true,
            None,
        )
        .is_err(),
        "open-for-input implies selected"
    );

    let bad_selection = [cmd::SELECT_INPUT_OBJECT, 1, 0, 2, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(
        machbus::isobus::vt::render::SelectInputObjectResponse::from_payload_for_vt_version(
            &bad_selection,
            4,
        )
        .is_err()
    );

    let bad_open_mask = [cmd::SELECT_INPUT_OBJECT, 1, 0, 1, 0x02, 0xFF, 0xFF, 0xFF];
    assert!(
        machbus::isobus::vt::render::SelectInputObjectResponse::from_payload_for_vt_version(
            &bad_open_mask,
            5,
        )
        .is_err(),
        "open bitmask only admits bit 0"
    );

    let missing_tan =
        machbus::isobus::vt::render::SelectInputObjectResponse::new(
            ObjectID::new(1),
            true,
            false,
            None,
        )
        .unwrap();
    assert!(
        missing_tan.to_payload_for_vt_version(6).is_err(),
        "VT6 select-input responses must carry a TAN"
    );
}

#[test]
fn select_input_object_response_wraps_and_parses_ecu_to_vt_message() {
    let response = machbus::isobus::vt::render::SelectInputObjectResponse::new(
        ObjectID::new(0x0040),
        false,
        false,
        Some(0x0C),
    )
    .unwrap();

    let message = response.to_message(0x42, 0x80, 6).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        message.data.as_slice(),
        &[cmd::SELECT_INPUT_OBJECT, 0x40, 0, 0, 0, 0xFF, 0xFF, 0xCF]
    );
    assert_eq!(
        machbus::isobus::vt::render::SelectInputObjectResponse::from_message(&message, 6)
            .unwrap(),
        response
    );
    assert!(
        machbus::isobus::vt::render::SelectInputObjectResponse::from_message(
            &Message::new(PGN_ECU_TO_VT, message.data.clone(), 0x42),
            6,
        )
        .is_err(),
        "full response parser requires a concrete VT destination"
    );
}

#[test]
fn vt_esc_response_builds_and_parses_versioned_payloads() {
    let vt5 =
        machbus::isobus::vt::render::VtEscResponse::new(ObjectID::new(0xCAFE), None).unwrap();
    let vt5_payload = vt5.to_payload_for_vt_version(5).unwrap();
    assert_eq!(
        vt5_payload,
        [cmd::VT_ESC, 0xFE, 0xCA, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::VtEscResponse::from_payload_for_vt_version(
            &vt5_payload,
            5,
        )
        .unwrap(),
        vt5
    );

    let vt6 =
        machbus::isobus::vt::render::VtEscResponse::new(ObjectID::new(0xCAFE), Some(0x0A))
            .unwrap();
    let vt6_payload = vt6.to_payload_for_vt_version(6).unwrap();
    assert_eq!(
        vt6_payload,
        [cmd::VT_ESC, 0xFE, 0xCA, 0xFF, 0xFF, 0xFF, 0xFF, 0xAF]
    );
    assert_eq!(
        machbus::isobus::vt::render::VtEscResponse::from_payload_for_vt_version(
            &vt6_payload,
            6,
        )
        .unwrap(),
        vt6
    );
}

#[test]
fn vt_esc_response_rejects_noncanonical_payloads() {
    let missing_tan =
        machbus::isobus::vt::render::VtEscResponse::new(ObjectID::new(1), None).unwrap();
    assert!(
        missing_tan.to_payload_for_vt_version(6).is_err(),
        "VT6 ESC responses must carry a TAN"
    );

    let bad_reserved = [cmd::VT_ESC, 1, 0, 0x10, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(
        machbus::isobus::vt::render::VtEscResponse::from_payload_for_vt_version(
            &bad_reserved,
            5,
        )
        .is_err(),
        "H.11 bytes 4 through 7 are reserved in the ECU response"
    );

    let bad_tan = [cmd::VT_ESC, 1, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xA0];
    assert!(
        machbus::isobus::vt::render::VtEscResponse::from_payload_for_vt_version(&bad_tan, 6)
            .is_err(),
        "VT6 ESC response low TAN nibble is reserved 0xF"
    );
}

#[test]
fn vt_esc_response_wraps_and_parses_ecu_to_vt_message() {
    let response =
        machbus::isobus::vt::render::VtEscResponse::new(ObjectID::NULL, Some(0x02)).unwrap();

    let message = response.to_message(0x42, 0x80, 6).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        message.data.as_slice(),
        &[cmd::VT_ESC, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x2F]
    );
    assert_eq!(
        machbus::isobus::vt::render::VtEscResponse::from_message(&message, 6).unwrap(),
        response
    );
    assert!(
        machbus::isobus::vt::render::VtEscResponse::from_message(
            &Message::new(PGN_ECU_TO_VT, message.data.clone(), 0x42),
            6,
        )
        .is_err(),
        "full response parser requires a concrete VT destination"
    );
}
