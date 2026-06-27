#[test]
fn graphics_context_response_payload_and_message_roundtrip_are_checked() {
    let errors = machbus::isobus::vt::render::GraphicsContextErrorFlags::new(
        machbus::isobus::vt::render::GraphicsContextErrorFlags::INVALID_OBJECT_ID
            | machbus::isobus::vt::render::GraphicsContextErrorFlags::INVALID_PARAMETER,
    )
    .unwrap();
    let response = machbus::isobus::vt::render::GraphicsContextResponse::new(
        ObjectID(0x1234),
        0x0D,
        errors,
    );
    assert_eq!(
        response.to_payload(),
        [cmd::GRAPHICS_CONTEXT, 0x34, 0x12, 0x0D, 0x05, 0xFF, 0xFF, 0xFF]
    );

    let parsed =
        machbus::isobus::vt::render::GraphicsContextResponse::from_payload(&response.to_payload())
            .unwrap();
    assert_eq!(parsed, response);
    assert!(parsed.errors.has_errors());

    let message = response.to_message(0x80, 0x42).unwrap();
    assert_eq!(message.pgn, PGN_VT_TO_ECU);
    assert_eq!(message.source, 0x80);
    assert_eq!(message.destination, 0x42);
    assert_eq!(
        machbus::isobus::vt::render::GraphicsContextResponse::from_message(&message).unwrap(),
        response
    );

    assert!(
        machbus::isobus::vt::render::GraphicsContextResponse::with_error_bits(0x1234.into(), 1, 0x20)
            .is_err(),
        "bits 5..7 are reserved in the F.57 error field"
    );

    let mut bad_tail = response.to_payload();
    bad_tail[7] = 0;
    assert!(
        machbus::isobus::vt::render::GraphicsContextResponse::from_payload(&bad_tail).is_err()
    );

    assert!(
        machbus::isobus::vt::render::GraphicsContextResponse::from_payload(&response.to_payload()[..7])
            .is_err()
    );

    let mut wrong_command = response.to_payload();
    wrong_command[0] = cmd::GET_ATTRIBUTE_VALUE;
    assert!(
        machbus::isobus::vt::render::GraphicsContextResponse::from_payload(&wrong_command).is_err()
    );

    assert!(
        response.to_message(NULL_ADDRESS, 0x42).is_err(),
        "full VT-to-ECU response envelopes need a usable VT source"
    );
    assert!(
        response.to_message(0x80, BROADCAST_ADDRESS).is_err(),
        "full VT-to-ECU response envelopes need a destination ECU"
    );
}
