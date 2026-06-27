#[test]
fn control_audio_termination_builds_and_parses_vt5_reserved_payload() {
    let termination =
        machbus::isobus::vt::render::ControlAudioSignalTermination::new(None).unwrap();

    let payload = termination.to_payload_for_vt_version(5);
    assert_eq!(
        payload,
        [
            cmd::CONTROL_AUDIO_SIGNAL_TERMINATION,
            0x01,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF
        ]
    );

    assert_eq!(
        machbus::isobus::vt::render::ControlAudioSignalTermination::from_payload_for_vt_version(
            &payload, 5,
        )
        .unwrap(),
        termination
    );
}

#[test]
fn control_audio_termination_builds_and_parses_vt6_tan_payload() {
    let termination =
        machbus::isobus::vt::render::ControlAudioSignalTermination::new(Some(0x04)).unwrap();

    let payload = termination.to_payload_for_vt_version(6);
    assert_eq!(
        payload,
        [
            cmd::CONTROL_AUDIO_SIGNAL_TERMINATION,
            0x01,
            0x4F,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF
        ]
    );

    assert_eq!(
        machbus::isobus::vt::render::ControlAudioSignalTermination::from_payload_for_vt_version(
            &payload, 6,
        )
        .unwrap(),
        termination
    );
}

#[test]
fn control_audio_termination_rejects_noncanonical_payloads() {
    let mut bad = [
        cmd::CONTROL_AUDIO_SIGNAL_TERMINATION,
        0x03,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
    ];
    assert!(
        machbus::isobus::vt::render::ControlAudioSignalTermination::from_payload_for_vt_version(
            &bad, 5,
        )
        .is_err(),
        "cause byte must be exactly the terminated bit"
    );

    bad[1] = 0x01;
    bad[2] = 0x40;
    assert!(
        machbus::isobus::vt::render::ControlAudioSignalTermination::from_payload_for_vt_version(
            &bad, 6,
        )
        .is_err(),
        "VT6 low TAN nibble is reserved and must be 0xF"
    );

    bad[2] = 0x4F;
    bad[7] = 0x00;
    assert!(
        machbus::isobus::vt::render::ControlAudioSignalTermination::from_payload_for_vt_version(
            &bad, 6,
        )
        .is_err(),
        "tail bytes stay reserved 0xFF"
    );
}

#[test]
fn control_audio_termination_wraps_and_parses_vt_to_ecu_message() {
    let termination =
        machbus::isobus::vt::render::ControlAudioSignalTermination::new(Some(0x0A)).unwrap();

    let message = termination.to_message(0x80, 0x42, 6).unwrap();
    assert_eq!(message.pgn, PGN_VT_TO_ECU);
    assert_eq!(message.source, 0x80);
    assert_eq!(message.destination, 0x42);
    assert_eq!(
        message.data.as_slice(),
        &[
            cmd::CONTROL_AUDIO_SIGNAL_TERMINATION,
            0x01,
            0xAF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF
        ]
    );
    assert_eq!(
        machbus::isobus::vt::render::ControlAudioSignalTermination::from_message(&message, 6)
            .unwrap(),
        termination
    );
    assert!(
        machbus::isobus::vt::render::ControlAudioSignalTermination::from_message(
            &Message::new(PGN_VT_TO_ECU, message.data.clone(), 0x80),
            6,
        )
        .is_err(),
        "full-message parser requires a concrete ECU destination"
    );
}

#[test]
fn control_audio_termination_response_is_vt6_only_and_checked() {
    let response =
        machbus::isobus::vt::render::ControlAudioSignalTerminationResponse::new(0x03).unwrap();
    let payload = response.to_payload_for_vt_version(6).unwrap();
    assert_eq!(
        payload,
        [
            cmd::CONTROL_AUDIO_SIGNAL_TERMINATION,
            0x01,
            0x3F,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF
        ]
    );

    assert_eq!(
        machbus::isobus::vt::render::ControlAudioSignalTerminationResponse::from_payload_for_vt_version(
            &payload, 6,
        )
        .unwrap(),
        response
    );
    assert!(response.to_payload_for_vt_version(5).is_err());
    assert!(
        machbus::isobus::vt::render::ControlAudioSignalTerminationResponse::from_payload_for_vt_version(
            &payload, 5,
        )
        .is_err()
    );

    let message = response.to_message(0x42, 0x80, 6).unwrap();
    assert_eq!(message.pgn, PGN_ECU_TO_VT);
    assert_eq!(message.source, 0x42);
    assert_eq!(message.destination, 0x80);
    assert_eq!(
        machbus::isobus::vt::render::ControlAudioSignalTerminationResponse::from_message(
            &message, 6,
        )
        .unwrap(),
        response
    );
}
