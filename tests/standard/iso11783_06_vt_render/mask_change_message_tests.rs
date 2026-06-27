#[test]
fn active_mask_error_notification_builds_and_parses_payload_and_message() {
    let flags = machbus::isobus::vt::render::MaskErrorFlags::new(
        machbus::isobus::vt::render::MaskErrorFlags::MISSING_OBJECTS
            | machbus::isobus::vt::render::MaskErrorFlags::POOL_BEING_DELETED,
    )
    .unwrap();
    let notification = machbus::isobus::vt::render::ChangeActiveMaskError::new(
        ObjectID::new(0x1234),
        flags,
        ObjectID::new(0x2222),
        ObjectID::new(0x1111),
    )
    .unwrap();

    let payload = notification.to_payload();
    assert_eq!(
        payload,
        [cmd::VT_CHANGE_ACTIVE_MASK, 0x34, 0x12, 0x24, 0x22, 0x22, 0x11, 0x11]
    );
    assert_eq!(
        machbus::isobus::vt::render::ChangeActiveMaskError::from_payload(&payload).unwrap(),
        notification
    );

    let message = notification.to_message(0x80, 0x42).unwrap();
    assert_eq!(message.pgn, PGN_VT_TO_ECU);
    assert_eq!(message.source, 0x80);
    assert_eq!(message.destination, 0x42);
    assert_eq!(
        machbus::isobus::vt::render::ChangeActiveMaskError::from_message(&message).unwrap(),
        notification
    );
}

#[test]
fn active_mask_error_notification_rejects_noncanonical_shapes() {
    assert!(
        machbus::isobus::vt::render::MaskErrorFlags::new(0).is_err(),
        "H.14/H.16 are error notifications, so at least one error bit is required"
    );
    assert!(
        machbus::isobus::vt::render::MaskErrorFlags::new(0x41).is_err(),
        "reserved error-code bits must stay zero"
    );
    assert!(
        machbus::isobus::vt::render::ChangeActiveMaskError::new(
            ObjectID::NULL,
            machbus::isobus::vt::render::MaskErrorFlags::new(
                machbus::isobus::vt::render::MaskErrorFlags::OTHER_ERROR,
            )
            .unwrap(),
            ObjectID::new(1),
            ObjectID::new(2),
        )
        .is_err()
    );

    let bad_command = [cmd::VT_CHANGE_SOFT_KEY_MASK, 1, 0, 4, 2, 0, 3, 0];
    assert!(machbus::isobus::vt::render::ChangeActiveMaskError::from_payload(&bad_command).is_err());

    let valid = machbus::isobus::vt::render::ChangeActiveMaskError::new(
        ObjectID::new(1),
        machbus::isobus::vt::render::MaskErrorFlags::new(
            machbus::isobus::vt::render::MaskErrorFlags::MISSING_OBJECTS,
        )
        .unwrap(),
        ObjectID::new(2),
        ObjectID::new(3),
    )
    .unwrap();
    assert!(valid.to_message(NULL_ADDRESS, 0x42).is_err());
    assert!(valid.to_message(0x80, BROADCAST_ADDRESS).is_err());
}

#[test]
fn active_mask_error_response_builds_and_rejects_reserved_bytes() {
    let response =
        machbus::isobus::vt::render::ChangeActiveMaskResponse::new(ObjectID::new(0x1234))
            .unwrap();
    let payload = response.to_payload();
    assert_eq!(
        payload,
        [cmd::VT_CHANGE_ACTIVE_MASK, 0x34, 0x12, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::ChangeActiveMaskResponse::from_payload(&payload).unwrap(),
        response
    );

    let mut bad_reserved = payload;
    bad_reserved[3] = 0;
    assert!(
        machbus::isobus::vt::render::ChangeActiveMaskResponse::from_payload(&bad_reserved)
            .is_err()
    );

    let message = response.to_message(0x42, 0x80).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(
        machbus::isobus::vt::render::ChangeActiveMaskResponse::from_message(&message).unwrap(),
        response
    );
}

#[test]
fn soft_key_mask_error_notification_builds_and_parses_payload_and_message() {
    let flags = machbus::isobus::vt::render::MaskErrorFlags::new(
        machbus::isobus::vt::render::MaskErrorFlags::OBJECT_HAS_ERRORS,
    )
    .unwrap();
    let notification = machbus::isobus::vt::render::ChangeSoftKeyMaskError::new(
        ObjectID::new(0x1111),
        ObjectID::new(0x2222),
        flags,
    )
    .unwrap();

    let payload = notification.to_payload();
    assert_eq!(
        payload,
        [cmd::VT_CHANGE_SOFT_KEY_MASK, 0x11, 0x11, 0x22, 0x22, 0x08, 0xFF, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskError::from_payload(&payload).unwrap(),
        notification
    );

    let message = notification.to_message(0x80, 0x42).unwrap();
    assert_eq!(message.pgn, PGN_VT_TO_ECU);
    assert_eq!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskError::from_message(&message).unwrap(),
        notification
    );
}

#[test]
fn soft_key_mask_error_notification_rejects_null_ids_and_reserved_tail() {
    let flags = machbus::isobus::vt::render::MaskErrorFlags::new(
        machbus::isobus::vt::render::MaskErrorFlags::OTHER_ERROR,
    )
    .unwrap();
    assert!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskError::new(
            ObjectID::NULL,
            ObjectID::new(2),
            flags,
        )
        .is_err()
    );
    assert!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskError::new(
            ObjectID::new(1),
            ObjectID::NULL,
            flags,
        )
        .is_err()
    );

    let bad_reserved = [cmd::VT_CHANGE_SOFT_KEY_MASK, 1, 0, 2, 0, 0x10, 0, 0xFF];
    assert!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskError::from_payload(&bad_reserved).is_err()
    );
}

#[test]
fn soft_key_mask_error_response_builds_and_rejects_reserved_bytes() {
    let response = machbus::isobus::vt::render::ChangeSoftKeyMaskResponse::new(
        ObjectID::new(0x1111),
        ObjectID::new(0x2222),
    )
    .unwrap();

    let payload = response.to_payload();
    assert_eq!(
        payload,
        [cmd::VT_CHANGE_SOFT_KEY_MASK, 0x11, 0x11, 0x22, 0x22, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskResponse::from_payload(&payload).unwrap(),
        response
    );

    let mut bad_reserved = payload;
    bad_reserved[5] = 0;
    assert!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskResponse::from_payload(&bad_reserved)
            .is_err()
    );

    let message = response.to_message(0x42, 0x80).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(
        machbus::isobus::vt::render::ChangeSoftKeyMaskResponse::from_message(&message).unwrap(),
        response
    );
}
