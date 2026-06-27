#[test]
fn numeric_value_response_builds_vt5_reserved_payload() {
    let response = machbus::isobus::vt::render::NumericValueChangeResponse::new(
        ObjectID::new(0x1234),
        [0x11, 0x22, 0, 0],
        None,
    )
    .unwrap();

    let payload = response.to_payload_for_vt_version(5).unwrap();
    assert_eq!(
        payload,
        [
            cmd::NUMERIC_VALUE_CHANGE,
            0x34,
            0x12,
            0xFF,
            0x11,
            0x22,
            0,
            0
        ]
    );
    assert_eq!(
        machbus::isobus::vt::render::NumericValueChangeResponse::from_payload_for_vt_version(
            &payload, 5,
        )
        .unwrap(),
        response
    );
}

#[test]
fn numeric_value_response_builds_vt6_tan_payload_and_preserves_bytes() {
    let response = machbus::isobus::vt::render::NumericValueChangeResponse::from_u32(
        ObjectID::new(0x1234),
        0xAABB_CCDD,
        Some(0x06),
    )
    .unwrap();

    let payload = response.to_payload_for_vt_version(6).unwrap();
    assert_eq!(
        payload,
        [
            cmd::NUMERIC_VALUE_CHANGE,
            0x34,
            0x12,
            0x6F,
            0xDD,
            0xCC,
            0xBB,
            0xAA
        ]
    );
    assert_eq!(response.value_u32(), 0xAABB_CCDD);
    assert_eq!(
        machbus::isobus::vt::render::NumericValueChangeResponse::from_payload_for_vt_version(
            &payload, 6,
        )
        .unwrap(),
        response
    );
}

#[test]
fn numeric_value_response_rejects_bad_tan_and_null_object() {
    assert!(
        machbus::isobus::vt::render::NumericValueChangeResponse::new(
            ObjectID::NULL,
            [0, 0, 0, 0],
            None,
        )
        .is_err()
    );

    let missing_tan = machbus::isobus::vt::render::NumericValueChangeResponse::new(
        ObjectID::new(1),
        [1, 0, 0, 0],
        None,
    )
    .unwrap();
    assert!(
        missing_tan.to_payload_for_vt_version(6).is_err(),
        "VT6 numeric-value responses must carry a TAN"
    );

    let bad_reserved = [cmd::NUMERIC_VALUE_CHANGE, 1, 0, 0x00, 1, 0, 0, 0];
    assert!(
        machbus::isobus::vt::render::NumericValueChangeResponse::from_payload_for_vt_version(
            &bad_reserved,
            5,
        )
        .is_err(),
        "VT5/prior byte 4 is reserved 0xFF"
    );

    let bad_tan = [cmd::NUMERIC_VALUE_CHANGE, 1, 0, 0x60, 1, 0, 0, 0];
    assert!(
        machbus::isobus::vt::render::NumericValueChangeResponse::from_payload_for_vt_version(
            &bad_tan, 6,
        )
        .is_err(),
        "VT6 low TAN nibble is reserved 0xF"
    );
}

#[test]
fn numeric_value_response_wraps_and_parses_ecu_to_vt_message() {
    let response = machbus::isobus::vt::render::NumericValueChangeResponse::from_u32(
        ObjectID::new(0x0040),
        7,
        Some(0x0C),
    )
    .unwrap();

    let message = response.to_message(0x42, 0x80, 6).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        message.data.as_slice(),
        &[cmd::NUMERIC_VALUE_CHANGE, 0x40, 0, 0xCF, 7, 0, 0, 0]
    );
    assert_eq!(
        machbus::isobus::vt::render::NumericValueChangeResponse::from_message(&message, 6)
            .unwrap(),
        response
    );
    assert!(
        machbus::isobus::vt::render::NumericValueChangeResponse::from_message(
            &Message::new(PGN_ECU_TO_VT, message.data.clone(), 0x42),
            6,
        )
        .is_err(),
        "full response parser requires a concrete VT destination"
    );
}

#[test]
fn string_value_response_builds_and_parses_fixed_payload() {
    let response =
        machbus::isobus::vt::render::StringValueChangeResponse::new(ObjectID::new(0x1234))
            .unwrap();

    let payload = response.to_payload();
    assert_eq!(
        payload,
        [
            cmd::STRING_VALUE_CHANGE,
            0xFF,
            0xFF,
            0x34,
            0x12,
            0xFF,
            0xFF,
            0xFF
        ]
    );
    assert_eq!(
        machbus::isobus::vt::render::StringValueChangeResponse::from_payload(&payload).unwrap(),
        response
    );
}

#[test]
fn string_value_response_rejects_reserved_bytes_and_null_object() {
    assert!(
        machbus::isobus::vt::render::StringValueChangeResponse::new(ObjectID::NULL).is_err()
    );

    let mut bad = [
        cmd::STRING_VALUE_CHANGE,
        0x00,
        0xFF,
        1,
        0,
        0xFF,
        0xFF,
        0xFF,
    ];
    assert!(
        machbus::isobus::vt::render::StringValueChangeResponse::from_payload(&bad).is_err(),
        "bytes 2 and 3 are reserved"
    );

    bad[1] = 0xFF;
    bad[7] = 0x00;
    assert!(
        machbus::isobus::vt::render::StringValueChangeResponse::from_payload(&bad).is_err(),
        "bytes 6 through 8 are reserved"
    );
}

#[test]
fn string_value_response_wraps_and_parses_ecu_to_vt_message() {
    let response =
        machbus::isobus::vt::render::StringValueChangeResponse::new(ObjectID::new(0xCAFE))
            .unwrap();

    let message = response.to_message(0x42, 0x80).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        message.data.as_slice(),
        &[
            cmd::STRING_VALUE_CHANGE,
            0xFF,
            0xFF,
            0xFE,
            0xCA,
            0xFF,
            0xFF,
            0xFF
        ]
    );
    assert_eq!(
        machbus::isobus::vt::render::StringValueChangeResponse::from_message(&message).unwrap(),
        response
    );
    assert!(
        machbus::isobus::vt::render::StringValueChangeResponse::from_message(&Message::new(
            PGN_ECU_TO_VT,
            message.data.clone(),
            0x42,
        ))
        .is_err(),
        "full response parser requires a concrete VT destination"
    );
}
