#[test]
fn user_layout_hide_show_response_round_trips_vt6_tan_payload() {
    let response = machbus::isobus::vt::render::UserLayoutHideShowResponse::new(
        machbus::isobus::vt::render::UserLayoutHideShowRecord::new(ObjectID::new(2), true),
        Some(machbus::isobus::vt::render::UserLayoutHideShowRecord::new(
            ObjectID::new(20),
            false,
        )),
        Some(0x03),
    )
    .unwrap();

    let payload = response.to_payload_for_vt_version(6);
    assert_eq!(
        payload,
        [cmd::USER_LAYOUT_HIDE_SHOW, 2, 0, 1, 20, 0, 0, 0x3F]
    );

    assert_eq!(
        machbus::isobus::vt::render::UserLayoutHideShowResponse::from_payload_for_vt_version(
            &payload, 6
        )
        .unwrap(),
        response
    );
}

#[test]
fn user_layout_hide_show_response_uses_vt5_reserved_tail_without_tan() {
    let response = machbus::isobus::vt::render::UserLayoutHideShowResponse::new(
        machbus::isobus::vt::render::UserLayoutHideShowRecord::new(ObjectID::new(30), false),
        None,
        Some(0x04),
    )
    .unwrap();

    let payload = response.to_payload_for_vt_version(5);
    assert_eq!(
        payload,
        [
            cmd::USER_LAYOUT_HIDE_SHOW,
            30,
            0,
            0,
            0xFF,
            0xFF,
            0,
            0xFF
        ]
    );

    let parsed =
        machbus::isobus::vt::render::UserLayoutHideShowResponse::from_payload_for_vt_version(
            &payload, 5,
        )
        .unwrap();
    assert_eq!(parsed.first.object_id, ObjectID::new(30));
    assert!(!parsed.first.shown);
    assert_eq!(parsed.second, None);
    assert_eq!(parsed.transfer_sequence_number, None);
}

#[test]
fn user_layout_hide_show_response_rejects_reserved_status_and_null_second_state() {
    let mut bad_status = [cmd::USER_LAYOUT_HIDE_SHOW, 2, 0, 0x02, 0xFF, 0xFF, 0, 0xFF];
    assert!(
        machbus::isobus::vt::render::UserLayoutHideShowResponse::from_payload_for_vt_version(
            &bad_status,
            5,
        )
        .is_err()
    );

    bad_status[3] = 1;
    bad_status[6] = 1;
    assert!(
        machbus::isobus::vt::render::UserLayoutHideShowResponse::from_payload_for_vt_version(
            &bad_status,
            5,
        )
        .is_err(),
        "NULL second-object status bit must stay hidden"
    );
}

#[test]
fn user_layout_hide_show_response_wraps_and_parses_ecu_to_vt_message() {
    let response = machbus::isobus::vt::render::UserLayoutHideShowResponse::new(
        machbus::isobus::vt::render::UserLayoutHideShowRecord::new(ObjectID::new(40), true),
        None,
        Some(0x0A),
    )
    .unwrap();

    let message = response.to_message(0x42, 0x80, 6).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        message.data.as_slice(),
        &[cmd::USER_LAYOUT_HIDE_SHOW, 40, 0, 1, 0xFF, 0xFF, 0, 0xAF]
    );
    assert_eq!(
        machbus::isobus::vt::render::UserLayoutHideShowResponse::from_message(&message, 6)
            .unwrap(),
        response
    );

    assert!(
        machbus::isobus::vt::render::UserLayoutHideShowResponse::from_message(
            &Message::new(PGN_ECU_TO_VT, message.data.clone(), 0x42),
            6,
        )
        .is_err(),
        "full response parser requires a concrete VT destination"
    );
}
