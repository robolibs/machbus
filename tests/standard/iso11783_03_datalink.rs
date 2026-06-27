use machbus::isobus::group_function::{
    GroupFunctionError, GroupFunctionMsg, GroupFunctionResponder, GroupFunctionSupport,
    GroupFunctionType,
};
use machbus::j1939::{
    AckControl, Acknowledgment, ProprietaryMsg, Request2Msg, Request2Responder, TimeDate,
    TransferMsg, is_proprietary_pgn, pgn_request, proprietary_b_pgn,
};
use machbus::net::Message;
use machbus::net::pgn_defs::{
    PGN_ACKNOWLEDGMENT, PGN_ADDRESS_CLAIMED, PGN_DM1, PGN_ETP_CM, PGN_ETP_DT, PGN_PROPRIETARY_A,
    PGN_PROPRIETARY_A2, PGN_REQUEST, PGN_REQUEST2, PGN_TIME_DATE, PGN_TP_CM, PGN_TP_DT,
    PGN_TRANSFER,
};
use machbus::net::tp::TP_T_HOLD_MS;
use machbus::net::{
    BROADCAST_ADDRESS, ETP_MAX_DATA_LENGTH, ErrorCode, ExtendedTransportProtocol, Frame,
    Identifier, NULL_ADDRESS, Priority, SessionState, TP_MAX_DATA_LENGTH, TP_MAX_PACKETS_PER_CTS,
    TP_TIMEOUT_T1_MS, TP_TIMEOUT_T3_MS, TP_TIMEOUT_T4_MS, TpSessionState, TransportAbortReason,
    TransportProtocol,
};

fn tp_cm_frame(source: u8, destination: u8, data: [u8; 8]) -> Frame {
    Frame::new(
        Identifier::encode(Priority::Default, PGN_TP_CM, source, destination),
        data,
        8,
    )
}

fn etp_cm_frame(source: u8, destination: u8, data: [u8; 8]) -> Frame {
    Frame::new(
        Identifier::encode(Priority::Default, PGN_ETP_CM, source, destination),
        data,
        8,
    )
}

fn tp_dt_frame(source: u8, destination: u8, sequence: u8, data: [u8; 7]) -> Frame {
    let mut payload = [0xFF; 8];
    payload[0] = sequence;
    payload[1..].copy_from_slice(&data);
    Frame::new(
        Identifier::encode(Priority::Default, PGN_TP_DT, source, destination),
        payload,
        8,
    )
}

fn etp_dt_frame(source: u8, destination: u8, sequence: u32, data: [u8; 4]) -> Frame {
    let mut payload = [0xFF; 8];
    payload[0] = (sequence & 0xFF) as u8;
    payload[1] = ((sequence >> 8) & 0xFF) as u8;
    payload[2] = ((sequence >> 16) & 0xFF) as u8;
    payload[3] = ((sequence >> 24) & 0xFF) as u8;
    payload[4..].copy_from_slice(&data);
    Frame::new(
        Identifier::encode(Priority::Default, PGN_ETP_DT, source, destination),
        payload,
        8,
    )
}

#[test]
fn datalink_priority_public_decoder_rejects_noncanonical_bytes() {
    let valid = [
        Priority::Highest,
        Priority::High,
        Priority::AboveNormal,
        Priority::Normal,
        Priority::BelowNormal,
        Priority::Low,
        Priority::Default,
        Priority::Lowest,
    ];
    for priority in valid {
        assert_eq!(Priority::try_from_u8(priority.as_u8()), Some(priority));
    }
    for raw in [8, 9, 0x0F, 0x10, 0x80, 0xFE, 0xFF] {
        assert_eq!(Priority::try_from_u8(raw), None);
    }
}

#[test]
fn datalink_acknowledgment_control_public_decoder_rejects_noncanonical_bytes() {
    for (raw, control) in [
        (0, AckControl::PositiveAck),
        (1, AckControl::NegativeAck),
        (2, AckControl::AccessDenied),
        (3, AckControl::CannotRespond),
    ] {
        assert_eq!(AckControl::try_from_u8(raw), Some(control));
        assert_eq!(AckControl::from_u8(raw), control);
    }
    for raw in [4, 5, 0x10, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(
            AckControl::try_from_u8(raw),
            None,
            "ACK control public strict decoder must reject non-canonical bytes"
        );
    }
}

fn etp_cts(source: u8, destination: u8, count: u8, next_packet: u32, pgn: u32) -> Frame {
    etp_cm_frame(
        source,
        destination,
        [
            machbus::net::etp::etp_cm::CTS,
            count,
            (next_packet & 0xFF) as u8,
            ((next_packet >> 8) & 0xFF) as u8,
            ((next_packet >> 16) & 0xFF) as u8,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    )
}

fn etp_eoma(source: u8, destination: u8, total_bytes: u32, pgn: u32) -> Frame {
    etp_cm_frame(
        source,
        destination,
        [
            machbus::net::etp::etp_cm::EOMA,
            (total_bytes & 0xFF) as u8,
            ((total_bytes >> 8) & 0xFF) as u8,
            ((total_bytes >> 16) & 0xFF) as u8,
            ((total_bytes >> 24) & 0xFF) as u8,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    )
}

fn etp_dpo(source: u8, destination: u8, count: u8, packet_offset: u32, pgn: u32) -> Frame {
    etp_cm_frame(
        source,
        destination,
        [
            machbus::net::etp::etp_cm::DPO,
            count,
            (packet_offset & 0xFF) as u8,
            ((packet_offset >> 8) & 0xFF) as u8,
            ((packet_offset >> 16) & 0xFF) as u8,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    )
}

fn tp_cts(source: u8, destination: u8, count: u8, next_seq: u8, pgn: u32) -> Frame {
    tp_cm_frame(
        source,
        destination,
        [
            machbus::net::tp::tp_cm::CTS,
            count,
            next_seq,
            0xFF,
            0xFF,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    )
}

fn tp_eoma(source: u8, destination: u8, total_bytes: u16, packets: u8, pgn: u32) -> Frame {
    tp_cm_frame(
        source,
        destination,
        [
            machbus::net::tp::tp_cm::EOMA,
            (total_bytes & 0xFF) as u8,
            (total_bytes >> 8) as u8,
            packets,
            0xFF,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    )
}

#[test]
fn datalink_pdu1_identifier_preserves_destination_but_normalizes_pgn() {
    let id = Identifier::encode(Priority::Default, PGN_REQUEST, 0x80, 0x42);

    assert_eq!(id.pgn(), PGN_REQUEST);
    assert_eq!(id.source(), 0x80);
    assert_eq!(id.destination(), 0x42);
    assert_eq!(id.pdu_format(), 0xEA);
    assert_eq!(id.pdu_specific(), 0x42);
    assert!(!id.is_pdu2());
}

#[test]
fn datalink_pdu2_identifier_uses_group_extension_not_destination_argument() {
    let id = Identifier::encode(Priority::Default, PGN_DM1, 0x80, 0x42);

    assert_eq!(id.pgn(), PGN_DM1);
    assert_eq!(id.destination(), BROADCAST_ADDRESS);
    assert_eq!(id.pdu_format(), 0xFE);
    assert_eq!(id.pdu_specific(), 0xCA);
    assert!(id.is_broadcast());
}

#[test]
fn datalink_identifier_try_encode_rejects_out_of_range_pgn_before_wire_use() {
    let id = Identifier::try_encode(Priority::Default, PGN_REQUEST, 0x80, 0x42).unwrap();
    assert_eq!(id.pgn(), PGN_REQUEST);
    assert_eq!(id.destination(), 0x42);

    let err = Identifier::try_encode(Priority::Default, 0x40000, 0x80, 0x42)
        .expect_err("PGNs outside the 18-bit identifier field must be rejected");
    assert_eq!(err.code, machbus::net::ErrorCode::InvalidData);
    assert!(err.message.contains("PGN"));
}

#[test]
fn datalink_request_rejects_prefix_compatible_garbage() {
    let canonical = pgn_request::encode_request(PGN_ADDRESS_CLAIMED).unwrap();
    assert_eq!(
        pgn_request::decode_request(&canonical),
        Some(PGN_ADDRESS_CLAIMED)
    );

    assert!(pgn_request::decode_request(&canonical[..2]).is_none());
    assert!(pgn_request::decode_request(&[canonical[0], canonical[1], canonical[2], 0]).is_none());
    assert!(
        pgn_request::decode_request(&[
            canonical[0],
            canonical[1],
            canonical[2],
            0,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ])
        .is_none()
    );
}

#[test]
fn datalink_acknowledgement_rejects_invalid_acknowledged_pgn() {
    assert!(Acknowledgment::ack(0x40_000, 0x42).encode().is_err());
}

#[test]
fn datalink_utility_control_pgns_reject_non_canonical_payloads() {
    let request = pgn_request::encode_request(PGN_ADDRESS_CLAIMED).unwrap();
    assert_eq!(request, [0x00, 0xEE, 0x00]);
    assert_eq!(
        pgn_request::decode_request(&request),
        Some(PGN_ADDRESS_CLAIMED)
    );
    assert!(pgn_request::encode_request(0x40_000).is_err());
    assert!(pgn_request::decode_request(&[0x00, 0xEE]).is_none());
    assert!(pgn_request::decode_request(&[0x00, 0xEE, 0x00, 0x00]).is_none());
    assert!(pgn_request::decode_request(&[0x00, 0xEE, 0x04]).is_none());

    for (raw_control, control) in [
        (0x00, AckControl::PositiveAck),
        (0x01, AckControl::NegativeAck),
        (0x02, AckControl::AccessDenied),
        (0x03, AckControl::CannotRespond),
    ] {
        let ack = Acknowledgment {
            control,
            group_function: 0xFF,
            acknowledged_pgn: PGN_REQUEST,
            address: 0x42,
        };
        let bytes = ack.encode().unwrap();
        assert_eq!(
            bytes,
            [raw_control, 0xFF, 0xFF, 0xFF, 0x42, 0x00, 0xEA, 0x00]
        );
        assert_eq!(Acknowledgment::decode(&bytes), Some(ack));
    }

    let mut bad_control = Acknowledgment::ack(PGN_REQUEST, 0x42).encode().unwrap();
    bad_control[0] = 0x04;
    assert!(Acknowledgment::decode(&bad_control).is_none());

    let mut bad_padding = Acknowledgment::ack(PGN_REQUEST, 0x42).encode().unwrap();
    bad_padding[2] = 0x00;
    assert!(Acknowledgment::decode(&bad_padding).is_none());
    bad_padding[2] = 0xFF;
    bad_padding[3] = 0x00;
    assert!(Acknowledgment::decode(&bad_padding).is_none());

    assert!(Acknowledgment::ack(0x40_000, 0x42).encode().is_err());
    assert!(Acknowledgment::decode(&[0x00, 0xFF, 0xFF, 0xFF, 0x42, 0x00, 0xEA, 0x04]).is_none());
}

#[test]
fn datalink_utility_full_message_helpers_reject_invalid_envelopes() {
    let request = pgn_request::encode_request(PGN_ADDRESS_CLAIMED).unwrap();
    assert_eq!(
        pgn_request::requested_pgn(&Message::new(PGN_REQUEST, request.to_vec(), 0x80)),
        Some(PGN_ADDRESS_CLAIMED)
    );
    for msg in [
        Message::new(PGN_TIME_DATE, request.to_vec(), 0x80),
        Message::new(PGN_REQUEST, request.to_vec(), NULL_ADDRESS),
        Message::new(PGN_REQUEST, request.to_vec(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_REQUEST,
            request.to_vec(),
            0x80,
            NULL_ADDRESS,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            pgn_request::requested_pgn(&msg),
            None,
            "Request full-message helper must bind PGN source and destination before decoding"
        );
    }

    let ack = Acknowledgment::ack(PGN_REQUEST, 0x42).encode().unwrap();
    assert_eq!(
        Acknowledgment::from_message(&Message::new(PGN_ACKNOWLEDGMENT, ack.to_vec(), 0x80)),
        Some(Acknowledgment::ack(PGN_REQUEST, 0x42))
    );
    for msg in [
        Message::new(PGN_TIME_DATE, ack.to_vec(), 0x80),
        Message::new(PGN_ACKNOWLEDGMENT, ack.to_vec(), NULL_ADDRESS),
        Message::new(PGN_ACKNOWLEDGMENT, ack.to_vec(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_ACKNOWLEDGMENT,
            ack.to_vec(),
            0x80,
            NULL_ADDRESS,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            Acknowledgment::from_message(&msg),
            None,
            "acknowledgement full-message helper must bind PGN source and destination before decoding"
        );
    }

    let request = Request2Msg {
        requested_pgn: PGN_TIME_DATE,
        extended_id: vec![0x01],
        use_transfer: true,
    };
    let request_bytes = request.encode().unwrap();
    assert_eq!(
        Request2Msg::from_message(&Message::new(PGN_REQUEST2, request_bytes.to_vec(), 0x81)),
        Some(request.clone())
    );
    for msg in [
        Message::new(PGN_TRANSFER, request_bytes.to_vec(), 0x81),
        Message::new(PGN_REQUEST2, request_bytes.to_vec(), NULL_ADDRESS),
        Message::new(PGN_REQUEST2, request_bytes.to_vec(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_REQUEST2,
            request_bytes.to_vec(),
            0x81,
            NULL_ADDRESS,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            Request2Msg::from_message(&msg),
            None,
            "Request2 full-message helper must bind PGN source and destination before decoding"
        );
    }

    let transfer = TransferMsg {
        original_pgn: PGN_TIME_DATE,
        data: vec![0xAA, 0xBB],
    };
    let transfer_bytes = transfer.encode().unwrap();
    assert_eq!(
        TransferMsg::from_message(&Message::new(PGN_TRANSFER, transfer_bytes.clone(), 0x82)),
        Some(transfer.clone())
    );
    for msg in [
        Message::new(PGN_REQUEST2, transfer_bytes.clone(), 0x82),
        Message::new(PGN_TRANSFER, transfer_bytes.clone(), NULL_ADDRESS),
        Message::new(PGN_TRANSFER, transfer_bytes.clone(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_TRANSFER,
            transfer_bytes.clone(),
            0x82,
            NULL_ADDRESS,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            TransferMsg::from_message(&msg),
            None,
            "Transfer full-message helper must bind PGN source and destination before decoding"
        );
    }
}

#[test]
fn datalink_time_date_full_message_helper_rejects_invalid_envelopes() {
    let time_date = TimeDate {
        seconds: Some(12),
        minutes: Some(34),
        hours: Some(9),
        day: Some(15),
        month: Some(6),
        year: Some(2026),
        utc_offset_min: Some(0),
        utc_offset_hours: Some(2),
        timestamp_us: 0,
    };
    let encoded = time_date.encode();
    assert_eq!(
        TimeDate::decode(&Message::new(PGN_TIME_DATE, encoded.to_vec(), 0x80)).map(|decoded| {
            TimeDate {
                timestamp_us: 0,
                ..decoded
            }
        }),
        Some(time_date)
    );

    for msg in [
        Message::new(PGN_REQUEST, encoded.to_vec(), 0x80),
        Message::new(PGN_TIME_DATE, encoded.to_vec(), NULL_ADDRESS),
        Message::new(PGN_TIME_DATE, encoded.to_vec(), BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_TIME_DATE,
            encoded.to_vec(),
            0x80,
            0x42,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            TimeDate::decode(&msg),
            None,
            "Time/Date full-message helper must bind PGN source and PDU2 broadcast destination before decoding"
        );
    }
}

#[test]
fn datalink_proprietary_message_helper_rejects_non_proprietary_pgns_and_bad_sources() {
    let proprietary_a = Message::with_addressing(
        PGN_PROPRIETARY_A,
        vec![1, 2, 3, 4],
        0x80,
        0x42,
        Priority::Default,
    );
    let decoded_a = ProprietaryMsg::from_message(&proprietary_a).unwrap();
    assert!(decoded_a.is_proprietary_a());
    assert_eq!(decoded_a.destination, 0x42);
    assert_eq!(decoded_a.data, vec![1, 2, 3, 4]);

    let proprietary_a2 = Message::with_addressing(
        PGN_PROPRIETARY_A2,
        vec![0xAA],
        0x81,
        0x43,
        Priority::Default,
    );
    assert!(
        ProprietaryMsg::from_message(&proprietary_a2)
            .unwrap()
            .is_proprietary_a2()
    );

    let proprietary_b = Message::new(proprietary_b_pgn(0x42), vec![0xCC], 0x82);
    let decoded_b = ProprietaryMsg::from_message(&proprietary_b).unwrap();
    assert!(decoded_b.is_proprietary_b());
    assert_eq!(decoded_b.group_extension(), 0x42);

    assert!(is_proprietary_pgn(PGN_PROPRIETARY_A));
    assert!(is_proprietary_pgn(PGN_PROPRIETARY_A2));
    assert!(is_proprietary_pgn(proprietary_b_pgn(0xFF)));
    assert!(!is_proprietary_pgn(PGN_TIME_DATE));

    for msg in [
        Message::new(PGN_TIME_DATE, vec![1, 2, 3, 4], 0x80),
        Message::new(PGN_PROPRIETARY_A, vec![1, 2, 3, 4], NULL_ADDRESS),
        Message::new(PGN_PROPRIETARY_A, vec![1, 2, 3, 4], BROADCAST_ADDRESS),
        Message::with_addressing(
            PGN_PROPRIETARY_A,
            vec![1, 2, 3, 4],
            0x80,
            NULL_ADDRESS,
            Priority::Default,
        ),
        Message::with_addressing(
            proprietary_b_pgn(0x42),
            vec![0xCC],
            0x82,
            0x40,
            Priority::Default,
        ),
    ] {
        assert_eq!(
            ProprietaryMsg::from_message(&msg),
            None,
            "proprietary helper must reject non-proprietary PGNs invalid sources and non-broadcast Proprietary B envelopes"
        );
    }
}

#[test]
fn datalink_request2_responder_is_source_scoped_and_transfer_wrapped() {
    let direct = Request2Msg {
        requested_pgn: PGN_REQUEST,
        extended_id: Vec::new(),
        use_transfer: false,
    };
    let direct_bytes = direct.encode().unwrap();
    assert_eq!(
        direct_bytes,
        [0x00, 0xEA, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(Request2Msg::decode(&direct_bytes), Some(direct.clone()));
    assert!(Request2Msg::decode(&direct_bytes[..7]).is_none());
    assert!(Request2Msg::decode(&[direct_bytes.as_slice(), &[0xFF]].concat()).is_none());

    let transfer_request = Request2Msg {
        requested_pgn: PGN_TIME_DATE,
        extended_id: vec![0x01, 0x02, 0x03],
        use_transfer: true,
    };
    let transfer_request_bytes = transfer_request.encode().unwrap();
    assert_eq!(
        transfer_request_bytes,
        [0xE6, 0xFE, 0x00, 0x01, 0x01, 0x02, 0x03, 0xFF]
    );
    assert_eq!(
        Request2Msg::decode(&transfer_request_bytes),
        Some(transfer_request.clone())
    );

    let mut bad_control = transfer_request_bytes;
    bad_control[3] = 0x02;
    assert!(Request2Msg::decode(&bad_control).is_none());

    let mut bad_extended_id_hole = transfer_request_bytes;
    bad_extended_id_hole[5] = 0xFF;
    assert!(Request2Msg::decode(&bad_extended_id_hole).is_none());

    let mut bad_tail = transfer_request_bytes;
    bad_tail[7] = 0x00;
    assert!(Request2Msg::decode(&bad_tail).is_none());

    assert!(
        Request2Msg {
            requested_pgn: PGN_REQUEST,
            extended_id: vec![0, 1, 2, 3],
            use_transfer: false,
        }
        .encode()
        .is_err()
    );
    assert!(
        Request2Msg {
            requested_pgn: 0x40_000,
            extended_id: Vec::new(),
            use_transfer: false,
        }
        .encode()
        .is_err()
    );
    assert!(Request2Msg::decode(&[0x00, 0xEA, 0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]).is_none());

    let response_payload = vec![0x11, 0x22, 0x33, 0x44];
    let responder = Request2Responder::new()
        .with_response(PGN_REQUEST, [0xA5, 0x5A])
        .unwrap()
        .with_response(PGN_TIME_DATE, response_payload.clone())
        .unwrap();

    let direct_reply = responder.handle_request(0x80, &direct).unwrap();
    assert_eq!(direct_reply.pgn, PGN_REQUEST);
    assert_eq!(direct_reply.destination, 0x80);
    assert_eq!(direct_reply.data, vec![0xA5, 0x5A]);

    assert!(responder.handle_request(NULL_ADDRESS, &direct).is_none());
    assert!(
        responder
            .handle_request(BROADCAST_ADDRESS, &direct)
            .is_none()
    );
    assert!(
        responder
            .handle_message(&Message::new(PGN_TRANSFER, direct_bytes.to_vec(), 0x80))
            .is_none()
    );
    assert!(
        responder
            .handle_message(&Message::new(PGN_REQUEST2, vec![0x00, 0xEA], 0x80))
            .is_none()
    );

    let transfer_reply = responder.handle_request(0x81, &transfer_request).unwrap();
    assert_eq!(transfer_reply.pgn, PGN_TRANSFER);
    assert_eq!(transfer_reply.destination, 0x81);
    let wrapped = TransferMsg::decode(&transfer_reply.data).unwrap();
    assert_eq!(wrapped.original_pgn, PGN_TIME_DATE);
    assert_eq!(wrapped.data, response_payload);

    let transfer_payload = TransferMsg {
        original_pgn: PGN_TIME_DATE,
        data: vec![0xAA, 0xBB, 0xCC],
    }
    .encode()
    .unwrap();
    assert_eq!(transfer_payload, vec![0xE6, 0xFE, 0x00, 0xAA, 0xBB, 0xCC]);
    assert_eq!(
        TransferMsg::decode(&transfer_payload),
        Some(TransferMsg {
            original_pgn: PGN_TIME_DATE,
            data: vec![0xAA, 0xBB, 0xCC],
        })
    );
    assert!(TransferMsg::decode(&transfer_payload[..2]).is_none());
    assert!(TransferMsg::decode(&[0x00, 0xEA, 0x04]).is_none());
    assert!(
        TransferMsg {
            original_pgn: 0x40_000,
            data: vec![],
        }
        .encode()
        .is_err()
    );
}

#[test]
fn datalink_group_function_carrier_and_responder_policy_are_canonical() {
    let request = GroupFunctionMsg {
        function_type: GroupFunctionType::Request,
        target_pgn: PGN_TIME_DATE,
        parameters: vec![0x01, 0x02],
    };
    let encoded = request.encode().unwrap();
    assert_eq!(
        encoded,
        [
            GroupFunctionType::Request.as_u8(),
            (PGN_TIME_DATE & 0xFF) as u8,
            ((PGN_TIME_DATE >> 8) & 0xFF) as u8,
            ((PGN_TIME_DATE >> 16) & 0xFF) as u8,
            0x01,
            0x02,
            0xFF,
            0xFF,
        ]
    );
    assert_eq!(GroupFunctionMsg::decode(&encoded), Some(request.clone()));

    let carrier = Message::with_addressing(
        PGN_ACKNOWLEDGMENT,
        encoded.to_vec(),
        0x80,
        0x42,
        Priority::Default,
    );
    assert_eq!(carrier.pgn, PGN_ACKNOWLEDGMENT);
    assert_eq!(GroupFunctionMsg::decode(&carrier.data), Some(request));

    assert_eq!(
        GroupFunctionMsg::decode(&[
            GroupFunctionType::Command.as_u8(),
            0x00,
            0xEA,
            0x00,
            0x01,
            0xFF,
            0x02,
            0xFF,
        ]),
        None,
        "non-0xFF data after parameter padding begins must be rejected"
    );
    assert_eq!(
        GroupFunctionMsg::decode(&[0xFF, 0x00, 0xEA, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]),
        None,
        "reserved group-function type bytes must not decode"
    );
    assert_eq!(
        GroupFunctionMsg::decode(&[
            GroupFunctionType::Request.as_u8(),
            0x00,
            0x00,
            0x04,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ]),
        None,
        "target PGN high bits outside the 18-bit range must be rejected"
    );

    let bad_target = GroupFunctionMsg {
        target_pgn: 0x04_0000,
        ..Default::default()
    };
    let bad_target_error = bad_target.encode().unwrap_err();
    assert_eq!(bad_target_error.code, ErrorCode::InvalidData);

    let responder =
        GroupFunctionResponder::new().supporting(GroupFunctionSupport::request(PGN_TIME_DATE));
    assert_eq!(
        responder.response_for(&GroupFunctionMsg {
            function_type: GroupFunctionType::Request,
            target_pgn: PGN_TIME_DATE,
            parameters: Vec::new(),
        }),
        Some(GroupFunctionMsg::acknowledge(
            PGN_TIME_DATE,
            GroupFunctionError::NoError
        ))
    );
    assert_eq!(
        responder.response_for(&GroupFunctionMsg {
            function_type: GroupFunctionType::Command,
            target_pgn: PGN_TIME_DATE,
            parameters: Vec::new(),
        }),
        Some(GroupFunctionMsg::acknowledge(
            PGN_TIME_DATE,
            GroupFunctionError::UnsupportedFunction
        ))
    );
    assert_eq!(
        responder.response_for(&GroupFunctionMsg {
            function_type: GroupFunctionType::Request,
            target_pgn: PGN_DM1,
            parameters: Vec::new(),
        }),
        Some(GroupFunctionMsg::acknowledge(
            PGN_DM1,
            GroupFunctionError::UnsupportedPgn
        ))
    );
    for function_type in [GroupFunctionType::Acknowledge, GroupFunctionType::ReadReply] {
        assert_eq!(
            responder.response_for(&GroupFunctionMsg {
                function_type,
                target_pgn: PGN_TIME_DATE,
                parameters: Vec::new(),
            }),
            None,
            "response-like group-function frames must not create acknowledge loops"
        );
    }

    let unsupported_ack =
        GroupFunctionMsg::acknowledge(PGN_TIME_DATE, GroupFunctionError::UnsupportedFunction);
    assert_eq!(
        unsupported_ack.encode().unwrap(),
        [
            GroupFunctionType::Acknowledge.as_u8(),
            (PGN_TIME_DATE & 0xFF) as u8,
            ((PGN_TIME_DATE >> 8) & 0xFF) as u8,
            ((PGN_TIME_DATE >> 16) & 0xFF) as u8,
            GroupFunctionError::UnsupportedFunction.as_u8(),
            0xFF,
            0xFF,
            0xFF,
        ]
    );
}

#[test]
fn datalink_group_function_public_error_decoder_rejects_noncanonical_bytes() {
    for (raw, error) in [
        (0, GroupFunctionError::NoError),
        (1, GroupFunctionError::UnsupportedPgn),
        (2, GroupFunctionError::UnsupportedFunction),
        (3, GroupFunctionError::InvalidParameter),
        (4, GroupFunctionError::PermissionDenied),
        (5, GroupFunctionError::Busy),
    ] {
        assert_eq!(GroupFunctionError::try_from_u8(raw), Some(error));
        assert_eq!(GroupFunctionError::from_u8(raw), error);
    }

    for reserved in [6, 7, 0x08, 0x10, 0x40, 0xFE, 0xFF] {
        assert_eq!(GroupFunctionError::try_from_u8(reserved), None);
    }

    let ack = GroupFunctionMsg::acknowledge(PGN_TIME_DATE, GroupFunctionError::PermissionDenied);
    assert_eq!(
        GroupFunctionMsg::decode(&ack.encode().unwrap())
            .and_then(|decoded| decoded.parameters.first().copied())
            .and_then(GroupFunctionError::try_from_u8),
        Some(GroupFunctionError::PermissionDenied)
    );
}

#[test]
fn datalink_tp_send_rejects_unusable_envelopes_before_session_mutation() {
    let payload = vec![0xA5; 20];
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let mut tp = TransportProtocol::new();
        assert!(
            tp.send(0xEF00, &payload, source, 0x20, 0, Priority::Default)
                .is_err()
        );
        assert!(tp.active_sessions().is_empty());
        assert!(tp.stats().is_empty());
    }

    let mut null_destination = TransportProtocol::new();
    assert!(
        null_destination
            .send(0xEF00, &payload, 0x10, NULL_ADDRESS, 0, Priority::Default)
            .is_err()
    );
    assert!(null_destination.active_sessions().is_empty());
    assert!(null_destination.stats().is_empty());

    let mut single_frame_payload = TransportProtocol::new();
    assert!(
        single_frame_payload
            .send(0xEF00, &[0xAA; 8], 0x10, 0x20, 0, Priority::Default)
            .is_err()
    );
    assert!(single_frame_payload.active_sessions().is_empty());
}

#[test]
fn datalink_tp_rts_packet_window_and_bam_reserved_byte_are_canonical() {
    for (requested_window, expected_window) in [
        (0, 1),
        (1, 1),
        (TP_MAX_PACKETS_PER_CTS as u8, TP_MAX_PACKETS_PER_CTS as u8),
        (
            TP_MAX_PACKETS_PER_CTS as u8 + 1,
            TP_MAX_PACKETS_PER_CTS as u8,
        ),
    ] {
        let mut tp = TransportProtocol::with_advertised_packets_per_cts(requested_window);
        assert_eq!(tp.advertised_packets_per_cts(), expected_window);

        let rts = tp
            .send(0xEF00, &[0xA5; 20], 0x10, 0x20, 0, Priority::Default)
            .expect("valid TP RTS transfer should start");
        assert_eq!(rts.len(), 1);
        assert_eq!(rts[0].pgn(), PGN_TP_CM);
        assert_eq!(rts[0].data[0], machbus::net::tp::tp_cm::RTS);
        assert_eq!(rts[0].data[3], 3);
        assert_eq!(rts[0].data[4], expected_window);
        assert_eq!(&rts[0].data[5..8], &[0x00, 0xEF, 0x00]);
    }

    let mut bam_sender = TransportProtocol::with_advertised_packets_per_cts(1);
    let bam = bam_sender
        .send(
            0xEF00,
            &[0x5A; 20],
            0x10,
            BROADCAST_ADDRESS,
            0,
            Priority::Default,
        )
        .expect("valid TP BAM transfer should start");
    assert_eq!(bam.len(), 1);
    assert_eq!(bam[0].pgn(), PGN_TP_CM);
    assert_eq!(bam[0].data[0], machbus::net::tp::tp_cm::BAM);
    assert_eq!(bam[0].data[3], 3);
    assert_eq!(bam[0].data[4], 0xFF);
    assert_eq!(&bam[0].data[5..8], &[0x00, 0xEF, 0x00]);
}

#[test]
fn datalink_tp_receive_rejects_bad_rts_without_allocating_partial_session() {
    let mut capped = TransportProtocol::with_max_receive_bytes(19);
    let too_large_for_local_cap = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );
    let responses = capped.process_frame(&too_large_for_local_cap, 0);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(
        responses[0].data[1],
        TransportAbortReason::ResourcesUnavailable.as_u8()
    );
    assert!(capped.active_sessions().is_empty());
    assert_eq!(capped.stats().resource_rejections, 1);

    let mut invalid_source = TransportProtocol::new();
    let invalid = tp_cm_frame(
        NULL_ADDRESS,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );
    assert!(invalid_source.process_frame(&invalid, 0).is_empty());
    assert!(invalid_source.active_sessions().is_empty());
    assert_eq!(invalid_source.stats().dropped_frames, 1);
}

#[test]
fn datalink_tp_etp_reject_invalid_endpoint_frames_without_session_mutation() {
    let mut tp = TransportProtocol::new();
    let tp_rts = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );
    assert_eq!(tp.process_frame(&tp_rts, 0).len(), 1);
    let tp_session_before = tp.active_sessions()[0].clone();

    assert!(
        tp.process_frame(
            &tp_dt_frame(NULL_ADDRESS, 0x20, 1, [1, 2, 3, 4, 5, 6, 7]),
            0,
        )
        .is_empty()
    );
    assert!(
        tp.process_frame(
            &tp_cm_frame(
                0x10,
                NULL_ADDRESS,
                [machbus::net::tp::tp_cm::CTS, 1, 1, 0xFF, 0xFF, 0, 0xEF, 0]
            ),
            0,
        )
        .is_empty()
    );
    assert_eq!(tp.active_sessions().len(), 1);
    let tp_session_after = &tp.active_sessions()[0];
    assert_eq!(tp_session_after.state, tp_session_before.state);
    assert_eq!(tp_session_after.pgn, tp_session_before.pgn);
    assert_eq!(tp_session_after.total_bytes, tp_session_before.total_bytes);
    assert_eq!(
        tp_session_after.bytes_transferred,
        tp_session_before.bytes_transferred
    );
    assert_eq!(
        tp_session_after.source_address,
        tp_session_before.source_address
    );
    assert_eq!(
        tp_session_after.destination_address,
        tp_session_before.destination_address
    );
    assert_eq!(tp.stats().dropped_frames, 2);

    let mut etp = ExtendedTransportProtocol::new();
    let etp_rts = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::RTS,
            ((TP_MAX_DATA_LENGTH + 1) & 0xFF) as u8,
            (((TP_MAX_DATA_LENGTH + 1) >> 8) & 0xFF) as u8,
            (((TP_MAX_DATA_LENGTH + 1) >> 16) & 0xFF) as u8,
            (((TP_MAX_DATA_LENGTH + 1) >> 24) & 0xFF) as u8,
            0x00,
            0xCA,
            0x00,
        ],
    );
    assert_eq!(etp.process_frame(&etp_rts, 0).len(), 1);
    let etp_session_before = etp.active_sessions()[0].clone();

    assert!(
        etp.process_frame(&etp_dt_frame(NULL_ADDRESS, 0x20, 1, [1, 2, 3, 4]), 0,)
            .is_empty()
    );
    assert!(
        etp.process_frame(
            &etp_cm_frame(
                0x10,
                BROADCAST_ADDRESS,
                [machbus::net::etp::etp_cm::CTS, 1, 1, 0, 0, 0x00, 0xCA, 0x00,],
            ),
            0,
        )
        .is_empty()
    );
    assert_eq!(etp.active_sessions().len(), 1);
    let etp_session_after = &etp.active_sessions()[0];
    assert_eq!(etp_session_after.state, etp_session_before.state);
    assert_eq!(etp_session_after.pgn, etp_session_before.pgn);
    assert_eq!(
        etp_session_after.total_bytes,
        etp_session_before.total_bytes
    );
    assert_eq!(
        etp_session_after.bytes_transferred,
        etp_session_before.bytes_transferred
    );
    assert_eq!(
        etp_session_after.source_address,
        etp_session_before.source_address
    );
    assert_eq!(
        etp_session_after.destination_address,
        etp_session_before.destination_address
    );
    assert_eq!(etp.stats().dropped_frames, 2);
}

#[test]
fn datalink_tp_rejects_concurrent_sessions_that_share_dt_endpoint_path() {
    let payload = vec![0xA5; 20];

    let mut sender = TransportProtocol::new();
    sender
        .send(0xEF00, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    assert!(
        sender
            .send(0xEF01, &payload, 0x10, 0x20, 0, Priority::Default)
            .is_err(),
        "a second TP transmit session on the same DT source/destination path would be ambiguous"
    );
    assert_eq!(sender.active_sessions().len(), 1);

    let mut receiver = TransportProtocol::new();
    let first_rts = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );
    let second_rts_same_dt_path = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x01, 0xEF, 0x00],
    );
    assert_eq!(receiver.process_frame(&first_rts, 0).len(), 1);
    let responses = receiver.process_frame(&second_rts_same_dt_path, 0);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(
        responses[0].data[1],
        TransportAbortReason::AlreadyInSession.as_u8()
    );
    assert_eq!(receiver.active_sessions().len(), 1);

    let bam_same_source_while_specific_session_is_active = tp_cm_frame(
        0x10,
        BROADCAST_ADDRESS,
        [
            machbus::net::tp::tp_cm::BAM,
            20,
            0,
            3,
            0xFF,
            0x02,
            0xEF,
            0x00,
        ],
    );
    assert!(
        receiver
            .process_frame(&bam_same_source_while_specific_session_is_active, 0)
            .is_empty(),
        "a BAM from the same source would share the same TP.DT stream and must be dropped"
    );
    assert_eq!(receiver.active_sessions().len(), 1);
    assert_eq!(receiver.stats().dropped_frames, 2);
}

#[test]
fn datalink_transport_abort_public_decoder_rejects_noncanonical_bytes() {
    let valid = [
        TransportAbortReason::None,
        TransportAbortReason::AlreadyInSession,
        TransportAbortReason::ResourcesUnavailable,
        TransportAbortReason::Timeout,
        TransportAbortReason::ConnectionModeError,
        TransportAbortReason::MaxRetransmitsExceeded,
        TransportAbortReason::UnexpectedPgn,
        TransportAbortReason::BadSequence,
        TransportAbortReason::DuplicateSequence,
        TransportAbortReason::UnexpectedDataSize,
    ];
    for reason in valid {
        assert_eq!(
            TransportAbortReason::try_from_u8(reason.as_u8()),
            Some(reason)
        );
    }
    for raw in [10, 11, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(TransportAbortReason::try_from_u8(raw), None);
    }

    let pgn = 0xEF00;
    let mut tp = TransportProtocol::new();
    let tp_rts = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );
    assert_eq!(tp.process_frame(&tp_rts, 0).len(), 1);
    assert_eq!(tp.active_sessions()[0].state, SessionState::WaitingForData);
    let invalid_tp_abort = tp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::tp::tp_cm::ABORT,
            10,
            0xFF,
            0xFF,
            0xFF,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    );
    assert!(tp.process_frame(&invalid_tp_abort, 0).is_empty());
    assert_eq!(tp.active_sessions().len(), 1);
    assert_eq!(tp.active_sessions()[0].state, SessionState::WaitingForData);
    assert_eq!(tp.stats().aborts_received, 0);
    assert_eq!(tp.stats().dropped_frames, 1);

    let etp_pgn = 0xCA00;
    let mut etp = ExtendedTransportProtocol::new();
    let etp_rts = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::RTS,
            0xD0,
            0x07,
            0,
            0,
            (etp_pgn & 0xFF) as u8,
            ((etp_pgn >> 8) & 0xFF) as u8,
            ((etp_pgn >> 16) & 0xFF) as u8,
        ],
    );
    assert_eq!(etp.process_frame(&etp_rts, 0).len(), 1);
    assert_eq!(etp.active_sessions()[0].state, SessionState::WaitingForData);
    let invalid_etp_abort = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::ABORT,
            10,
            0xFF,
            0xFF,
            0xFF,
            (etp_pgn & 0xFF) as u8,
            ((etp_pgn >> 8) & 0xFF) as u8,
            ((etp_pgn >> 16) & 0xFF) as u8,
        ],
    );
    assert!(etp.process_frame(&invalid_etp_abort, 0).is_empty());
    assert_eq!(etp.active_sessions().len(), 1);
    assert_eq!(etp.active_sessions()[0].state, SessionState::WaitingForData);
    assert_eq!(etp.stats().aborts_received, 0);
    assert_eq!(etp.stats().dropped_frames, 1);
}

#[test]
fn datalink_tp_cm_rejects_non_canonical_reserved_bytes_before_state_mutation() {
    let pgn = 0xEF00;
    let payload = vec![0xA5; 20];

    let mut tx_waiting_for_cts = TransportProtocol::new();
    tx_waiting_for_cts
        .send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    let mut bad_cts = tp_cts(0x20, 0x10, 3, 1, pgn);
    bad_cts.data[3] = 0x00;
    assert!(tx_waiting_for_cts.process_frame(&bad_cts, 0).is_empty());
    assert_eq!(
        tx_waiting_for_cts.active_sessions()[0].state,
        SessionState::WaitingForCTS
    );
    assert_eq!(tx_waiting_for_cts.stats().dropped_frames, 1);

    let mut tx_waiting_for_eoma = TransportProtocol::new();
    tx_waiting_for_eoma
        .send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    assert!(
        tx_waiting_for_eoma
            .process_frame(&tp_cts(0x20, 0x10, 3, 1, pgn), 0)
            .is_empty()
    );
    assert_eq!(tx_waiting_for_eoma.get_pending_data_frames().len(), 3);
    let mut bad_eoma = tp_eoma(0x20, 0x10, 20, 3, pgn);
    bad_eoma.data[4] = 0x00;
    assert!(tx_waiting_for_eoma.process_frame(&bad_eoma, 0).is_empty());
    assert_eq!(
        tx_waiting_for_eoma.active_sessions()[0].state,
        SessionState::WaitingForEndOfMsg
    );

    let mut bam_receiver = TransportProtocol::new();
    let bad_bam = tp_cm_frame(
        0x10,
        BROADCAST_ADDRESS,
        [
            machbus::net::tp::tp_cm::BAM,
            20,
            0,
            3,
            0x00,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    );
    assert!(bam_receiver.process_frame(&bad_bam, 0).is_empty());
    assert!(bam_receiver.active_sessions().is_empty());
    assert_eq!(bam_receiver.stats().dropped_frames, 1);

    let mut rx_session = TransportProtocol::new();
    let rts = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );
    assert_eq!(rx_session.process_frame(&rts, 0).len(), 1);
    assert_eq!(rx_session.active_sessions().len(), 1);
    let bad_abort = tp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::tp::tp_cm::ABORT,
            TransportAbortReason::Timeout.as_u8(),
            0x00,
            0xFF,
            0xFF,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    );
    assert!(rx_session.process_frame(&bad_abort, 0).is_empty());
    assert_eq!(
        rx_session.active_sessions()[0].state,
        SessionState::WaitingForData
    );
}

#[test]
fn datalink_tp_ignores_eoma_until_transmit_data_is_fully_sent_and_ack_matches() {
    let pgn = 0xEF00;
    let payload = vec![0xA5; 20];
    let mut tp = TransportProtocol::new();
    let rts = tp
        .send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    assert_eq!(rts.len(), 1);
    assert_eq!(tp.active_sessions()[0].state, SessionState::WaitingForCTS);

    let premature_eoma = tp_eoma(0x20, 0x10, 20, 3, pgn);
    assert!(tp.process_frame(&premature_eoma, 0).is_empty());
    assert_eq!(tp.active_sessions().len(), 1);
    assert_eq!(tp.active_sessions()[0].state, SessionState::WaitingForCTS);
    assert_eq!(tp.stats().dropped_frames, 1);

    assert!(
        tp.process_frame(&tp_cts(0x20, 0x10, 3, 1, pgn), 0)
            .is_empty()
    );
    let data_frames = tp.get_pending_data_frames();
    assert_eq!(data_frames.len(), 3);
    assert_eq!(
        tp.active_sessions()[0].state,
        SessionState::WaitingForEndOfMsg
    );

    let mismatched_total = tp_eoma(0x20, 0x10, 21, 3, pgn);
    assert!(tp.process_frame(&mismatched_total, 0).is_empty());
    assert_eq!(tp.active_sessions().len(), 1);
    assert_eq!(
        tp.active_sessions()[0].state,
        SessionState::WaitingForEndOfMsg
    );

    let valid_eoma = tp_eoma(0x20, 0x10, 20, 3, pgn);
    assert!(tp.process_frame(&valid_eoma, 0).is_empty());
    assert!(tp.active_sessions().is_empty());
}

#[test]
fn datalink_tp_accepts_bounded_retransmit_cts_while_waiting_for_eoma() {
    let pgn = 0xEF00;
    let payload = vec![0x5A; 20];
    let mut tp = TransportProtocol::with_max_retransmits(1);
    tp.send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    assert!(
        tp.process_frame(&tp_cts(0x20, 0x10, 3, 1, pgn), 0)
            .is_empty()
    );
    assert_eq!(tp.get_pending_data_frames().len(), 3);
    assert_eq!(
        tp.active_sessions()[0].state,
        SessionState::WaitingForEndOfMsg
    );

    assert!(
        tp.process_frame(&tp_cts(0x20, 0x10, 3, 1, pgn), 0)
            .is_empty()
    );
    assert_eq!(tp.active_sessions()[0].state, SessionState::SendingData);
    assert_eq!(tp.active_sessions()[0].retransmit_count, 1);
    assert_eq!(tp.get_pending_data_frames().len(), 3);
    assert_eq!(
        tp.active_sessions()[0].state,
        SessionState::WaitingForEndOfMsg
    );

    let responses = tp.process_frame(&tp_cts(0x20, 0x10, 3, 1, pgn), 0);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(
        responses[0].data[1],
        TransportAbortReason::MaxRetransmitsExceeded.as_u8()
    );
    assert!(tp.active_sessions().is_empty());
    assert_eq!(tp.stats().aborts_sent, 1);
    assert_eq!(tp.stats().dropped_sessions, 1);
}

#[test]
fn datalink_tp_dt_sequence_errors_abort_without_partial_delivery() {
    let pgn = 0xEF00;
    let rts = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );

    let mut skip_first_sequence = TransportProtocol::new();
    assert_eq!(skip_first_sequence.process_frame(&rts, 0).len(), 1);
    let aborts = skip_first_sequence.process_frame(
        &tp_dt_frame(0x10, 0x20, 2, [0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5]),
        0,
    );
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(aborts[0].data[1], TransportAbortReason::BadSequence.as_u8());
    assert_eq!(&aborts[0].data[5..8], &[0x00, 0xEF, 0x00]);
    assert!(skip_first_sequence.active_sessions().is_empty());
    assert_eq!(skip_first_sequence.stats().aborts_sent, 1);
    assert_eq!(skip_first_sequence.stats().dropped_sessions, 1);

    let mut duplicate_sequence = TransportProtocol::new();
    assert_eq!(duplicate_sequence.process_frame(&rts, 0).len(), 1);
    assert!(
        duplicate_sequence
            .process_frame(&tp_dt_frame(0x10, 0x20, 1, [1, 2, 3, 4, 5, 6, 7]), 0,)
            .is_empty()
    );
    assert_eq!(duplicate_sequence.active_sessions()[0].last_sequence, 1);
    assert_eq!(duplicate_sequence.active_sessions()[0].bytes_transferred, 7);

    let aborts = duplicate_sequence
        .process_frame(&tp_dt_frame(0x10, 0x20, 1, [8, 9, 10, 11, 12, 13, 14]), 0);
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(
        aborts[0].data[1],
        TransportAbortReason::DuplicateSequence.as_u8()
    );
    assert_eq!(&aborts[0].data[5..8], &[0x00, 0xEF, 0x00]);
    assert!(duplicate_sequence.active_sessions().is_empty());

    let mut wrong_source = TransportProtocol::new();
    assert_eq!(wrong_source.process_frame(&rts, 0).len(), 1);
    let before = wrong_source.active_sessions()[0].clone();
    assert!(
        wrong_source
            .process_frame(&tp_dt_frame(0x11, 0x20, 1, [1, 2, 3, 4, 5, 6, 7]), 0,)
            .is_empty()
    );
    assert_eq!(wrong_source.active_sessions().len(), 1);
    assert_eq!(wrong_source.active_sessions()[0].state, before.state);
    assert_eq!(wrong_source.active_sessions()[0].pgn, pgn);
    assert_eq!(
        wrong_source.active_sessions()[0].bytes_transferred,
        before.bytes_transferred
    );
    assert_eq!(wrong_source.stats().dropped_frames, 1);
}

#[test]
fn datalink_tp_timeout_windows_abort_without_leaking_partial_sessions() {
    let pgn = 0xEF00;
    let payload = vec![0xA5; 20];

    let mut tx_waiting_for_cts = TransportProtocol::new();
    tx_waiting_for_cts
        .send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    assert!(tx_waiting_for_cts.update(TP_TIMEOUT_T3_MS - 1).is_empty());
    assert_eq!(
        tx_waiting_for_cts.active_sessions()[0].state,
        SessionState::WaitingForCTS
    );
    let aborts = tx_waiting_for_cts.update(1);
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(aborts[0].data[1], TransportAbortReason::Timeout.as_u8());
    assert!(tx_waiting_for_cts.active_sessions().is_empty());
    assert_eq!(tx_waiting_for_cts.stats().timeouts, 1);
    assert_eq!(tx_waiting_for_cts.stats().dropped_sessions, 1);

    let mut rx_waiting_for_data = TransportProtocol::new();
    let rts = tp_cm_frame(
        0x10,
        0x20,
        [machbus::net::tp::tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
    );
    assert_eq!(rx_waiting_for_data.process_frame(&rts, 0).len(), 1);
    assert_eq!(
        rx_waiting_for_data.active_sessions()[0].state,
        SessionState::WaitingForData
    );
    assert!(rx_waiting_for_data.update(TP_TIMEOUT_T1_MS - 1).is_empty());
    assert_eq!(
        rx_waiting_for_data.active_sessions()[0].state,
        SessionState::WaitingForData
    );
    let aborts = rx_waiting_for_data.update(1);
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(aborts[0].data[1], TransportAbortReason::Timeout.as_u8());
    assert!(rx_waiting_for_data.active_sessions().is_empty());
    assert_eq!(rx_waiting_for_data.stats().timeouts, 1);
}

#[test]
fn datalink_tp_auxiliary_timer_sessions_validate_endpoints_and_all_timeout_windows() {
    let pgn = PGN_PROPRIETARY_A;
    let mut invalid = TransportProtocol::new();
    for (source, destination) in [
        (NULL_ADDRESS, 0x90),
        (BROADCAST_ADDRESS, 0x90),
        (0x80, NULL_ADDRESS),
        (0x80, BROADCAST_ADDRESS),
    ] {
        invalid.track_session(source, destination, pgn, TpSessionState::WaitForCts, 0);
    }
    invalid.track_session(0x80, 0x90, 0x04_0000, TpSessionState::WaitForCts, 0);
    invalid.track_session(0x80, 0x90, pgn, TpSessionState::Idle, 0);
    assert!(
        invalid.timer_sessions().is_empty(),
        "auxiliary TP timer sessions must not be created for unusable endpoints invalid PGNs or inactive states"
    );

    let mut paused_receiver = TransportProtocol::new();
    paused_receiver.track_session(0x80, 0x90, pgn, TpSessionState::WaitForCts, 0);
    paused_receiver.set_receiver_paused(0x80, 0x90, pgn, 0);
    assert!(
        paused_receiver.update_sessions(TP_T_HOLD_MS - 1).is_empty(),
        "receiver-paused TP keepalive must not fire one millisecond early"
    );
    let keepalive = paused_receiver.update_sessions(1);
    assert_eq!(keepalive.len(), 1);
    assert_eq!(keepalive[0].pgn(), PGN_TP_CM);
    assert_eq!(keepalive[0].source(), 0x90);
    assert_eq!(keepalive[0].destination(), 0x80);
    assert_eq!(
        keepalive[0].data,
        [
            machbus::net::tp::tp_cm::CTS,
            0,
            0,
            0xFF,
            0xFF,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ]
    );

    let mut wait_for_cts = TransportProtocol::new();
    wait_for_cts.track_session(0x80, 0x90, pgn, TpSessionState::WaitForCts, 0);
    assert!(
        wait_for_cts
            .update_sessions(TP_TIMEOUT_T3_MS - 1)
            .is_empty()
    );
    assert_eq!(
        wait_for_cts.timer_sessions()[0].timer_state,
        TpSessionState::WaitForCts
    );
    let t3_abort = wait_for_cts.update_sessions(1);
    assert_eq!(t3_abort.len(), 1);
    assert_eq!(t3_abort[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(t3_abort[0].data[1], TransportAbortReason::Timeout.as_u8());
    assert_eq!(t3_abort[0].source(), 0x80);
    assert_eq!(t3_abort[0].destination(), 0x90);
    assert_eq!(
        wait_for_cts.timer_sessions()[0].timer_state,
        TpSessionState::TimedOut
    );
    assert_eq!(wait_for_cts.stats().timeouts, 1);
    assert_eq!(wait_for_cts.stats().dropped_sessions, 1);
    assert_eq!(wait_for_cts.stats().aborts_sent, 1);

    let mut sending = TransportProtocol::new();
    sending.track_session(0x80, 0x90, pgn, TpSessionState::Sending, 0);
    assert!(sending.update_sessions(TP_TIMEOUT_T4_MS - 1).is_empty());
    assert_eq!(
        sending.timer_sessions()[0].timer_state,
        TpSessionState::Sending
    );
    let t4_abort = sending.update_sessions(1);
    assert_eq!(t4_abort.len(), 1);
    assert_eq!(t4_abort[0].data[0], machbus::net::tp::tp_cm::ABORT);
    assert_eq!(t4_abort[0].data[1], TransportAbortReason::Timeout.as_u8());
    assert_eq!(t4_abort[0].source(), 0x80);
    assert_eq!(t4_abort[0].destination(), 0x90);
    assert_eq!(
        sending.timer_sessions()[0].timer_state,
        TpSessionState::TimedOut
    );
    assert_eq!(sending.stats().timeouts, 1);
    assert_eq!(sending.stats().dropped_sessions, 1);
    assert_eq!(sending.stats().aborts_sent, 1);
}

#[test]
fn datalink_etp_profiles_accept_only_extended_payload_range_and_local_capacity() {
    let etp = ExtendedTransportProtocol::with_max_receive_bytes(TP_MAX_DATA_LENGTH + 10);

    assert_eq!(
        etp.receive_profile_for_advertised_size(TP_MAX_DATA_LENGTH)
            .unwrap_err(),
        TransportAbortReason::UnexpectedDataSize
    );
    let accepted = etp
        .receive_profile_for_advertised_size(TP_MAX_DATA_LENGTH + 1)
        .unwrap();
    assert_eq!(accepted.total_bytes, TP_MAX_DATA_LENGTH + 1);
    assert_eq!(accepted.total_packets, 256);

    assert_eq!(
        etp.receive_profile_for_advertised_size(TP_MAX_DATA_LENGTH + 11)
            .unwrap_err(),
        TransportAbortReason::ResourcesUnavailable
    );

    let max_profile = ExtendedTransportProtocol::new()
        .receive_profile_for_advertised_size(ETP_MAX_DATA_LENGTH)
        .unwrap();
    assert_eq!(max_profile.total_bytes, ETP_MAX_DATA_LENGTH);
}

#[test]
fn datalink_etp_timeout_window_aborts_extended_session_without_state_leak() {
    let pgn = 0xCA00;
    let payload = vec![0x5A; (TP_MAX_DATA_LENGTH + 1) as usize];
    let mut etp = ExtendedTransportProtocol::new();
    etp.send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();

    assert!(
        etp.update(machbus::net::ETP_TIMEOUT_T1_MS - 1).is_empty(),
        "ETP timeout boundary should not fire one millisecond early"
    );
    assert_eq!(etp.active_sessions()[0].state, SessionState::WaitingForCTS);

    let aborts = etp.update(1);
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::etp::etp_cm::ABORT);
    assert_eq!(aborts[0].data[1], TransportAbortReason::Timeout.as_u8());
    assert!(etp.active_sessions().is_empty());
    assert_eq!(etp.stats().timeouts, 1);
    assert_eq!(etp.stats().dropped_sessions, 1);
}

#[test]
fn datalink_etp_rejects_broadcast_and_tp_sized_sessions_without_state_mutation() {
    let payload = vec![0x5A; (TP_MAX_DATA_LENGTH + 1) as usize];
    let mut broadcast = ExtendedTransportProtocol::new();
    assert!(
        broadcast
            .send(
                0xCA00,
                &payload,
                0x10,
                BROADCAST_ADDRESS,
                0,
                Priority::Default
            )
            .is_err()
    );
    assert!(broadcast.active_sessions().is_empty());
    assert!(broadcast.stats().is_empty());

    let mut tp_sized = ExtendedTransportProtocol::new();
    assert!(
        tp_sized
            .send(
                0xCA00,
                &vec![0x5A; TP_MAX_DATA_LENGTH as usize],
                0x10,
                0x20,
                0,
                Priority::Default
            )
            .is_err()
    );
    assert!(tp_sized.active_sessions().is_empty());

    let mut receiver = ExtendedTransportProtocol::new();
    let tp_sized_rts = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::RTS,
            (TP_MAX_DATA_LENGTH & 0xFF) as u8,
            ((TP_MAX_DATA_LENGTH >> 8) & 0xFF) as u8,
            ((TP_MAX_DATA_LENGTH >> 16) & 0xFF) as u8,
            ((TP_MAX_DATA_LENGTH >> 24) & 0xFF) as u8,
            0x00,
            0xCA,
            0x00,
        ],
    );
    let responses = receiver.process_frame(&tp_sized_rts, 0);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].data[0], machbus::net::etp::etp_cm::ABORT);
    assert_eq!(
        responses[0].data[1],
        TransportAbortReason::UnexpectedDataSize.as_u8()
    );
    assert!(receiver.active_sessions().is_empty());
}

#[test]
fn datalink_etp_rejects_concurrent_sessions_that_share_dt_endpoint_path() {
    let payload = vec![0x5A; (TP_MAX_DATA_LENGTH + 1) as usize];

    let mut sender = ExtendedTransportProtocol::new();
    sender
        .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    assert!(
        sender
            .send(0xCA01, &payload, 0x10, 0x20, 0, Priority::Default)
            .is_err(),
        "a second ETP transmit session on the same DT source/destination path would be ambiguous"
    );
    assert_eq!(sender.active_sessions().len(), 1);

    let mut receiver = ExtendedTransportProtocol::new();
    let first_rts = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::RTS,
            0xD2,
            0x07,
            0,
            0,
            0x00,
            0xCA,
            0x00,
        ],
    );
    let second_rts_same_dt_path = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::RTS,
            0xD2,
            0x07,
            0,
            0,
            0x01,
            0xCA,
            0x00,
        ],
    );
    assert_eq!(receiver.process_frame(&first_rts, 0).len(), 1);
    let responses = receiver.process_frame(&second_rts_same_dt_path, 0);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].data[0], machbus::net::etp::etp_cm::ABORT);
    assert_eq!(
        responses[0].data[1],
        TransportAbortReason::AlreadyInSession.as_u8()
    );
    assert_eq!(receiver.active_sessions().len(), 1);
    assert_eq!(receiver.stats().dropped_frames, 1);
}

#[test]
fn datalink_etp_abort_rejects_non_canonical_reserved_bytes_before_state_mutation() {
    let pgn = 0xCA00;
    let mut receiver = ExtendedTransportProtocol::new();
    let rts = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::RTS,
            0xD0,
            0x07,
            0,
            0,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    );
    assert_eq!(receiver.process_frame(&rts, 0).len(), 1);
    assert_eq!(receiver.active_sessions().len(), 1);
    assert_eq!(
        receiver.active_sessions()[0].state,
        SessionState::WaitingForData
    );

    let bad_abort = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::ABORT,
            TransportAbortReason::Timeout.as_u8(),
            0x00,
            0xFF,
            0xFF,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    );
    assert!(receiver.process_frame(&bad_abort, 0).is_empty());
    assert_eq!(receiver.active_sessions().len(), 1);
    assert_eq!(
        receiver.active_sessions()[0].state,
        SessionState::WaitingForData
    );
    assert_eq!(receiver.stats().aborts_received, 0);
    assert_eq!(receiver.stats().dropped_frames, 1);
}

#[test]
fn datalink_etp_dpo_and_dt_sequence_windows_abort_without_partial_delivery() {
    let pgn = 0xCA00;
    let rts = etp_cm_frame(
        0x10,
        0x20,
        [
            machbus::net::etp::etp_cm::RTS,
            0xD0,
            0x07,
            0,
            0,
            (pgn & 0xFF) as u8,
            ((pgn >> 8) & 0xFF) as u8,
            ((pgn >> 16) & 0xFF) as u8,
        ],
    );

    let mut zero_packet_window = ExtendedTransportProtocol::new();
    assert_eq!(zero_packet_window.process_frame(&rts, 0).len(), 1);
    let aborts = zero_packet_window.process_frame(&etp_dpo(0x10, 0x20, 0, 0, pgn), 0);
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::etp::etp_cm::ABORT);
    assert_eq!(aborts[0].data[1], TransportAbortReason::BadSequence.as_u8());
    assert_eq!(&aborts[0].data[5..8], &[0x00, 0xCA, 0x00]);
    assert!(zero_packet_window.active_sessions().is_empty());
    assert_eq!(zero_packet_window.stats().aborts_sent, 1);
    assert_eq!(zero_packet_window.stats().dropped_sessions, 1);

    let mut wrong_offset = ExtendedTransportProtocol::new();
    assert_eq!(wrong_offset.process_frame(&rts, 0).len(), 1);
    let aborts = wrong_offset.process_frame(&etp_dpo(0x10, 0x20, 1, 1, pgn), 0);
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::etp::etp_cm::ABORT);
    assert_eq!(aborts[0].data[1], TransportAbortReason::BadSequence.as_u8());
    assert!(wrong_offset.active_sessions().is_empty());

    let mut skipped_dt_sequence = ExtendedTransportProtocol::new();
    assert_eq!(skipped_dt_sequence.process_frame(&rts, 0).len(), 1);
    assert!(
        skipped_dt_sequence
            .process_frame(&etp_dpo(0x10, 0x20, 2, 0, pgn), 0)
            .is_empty()
    );
    assert_eq!(
        skipped_dt_sequence.active_sessions()[0].state,
        SessionState::WaitingForData
    );
    let aborts = skipped_dt_sequence.process_frame(&etp_dt_frame(0x10, 0x20, 2, [1, 2, 3, 4]), 0);
    assert_eq!(aborts.len(), 1);
    assert_eq!(aborts[0].data[0], machbus::net::etp::etp_cm::ABORT);
    assert_eq!(aborts[0].data[1], TransportAbortReason::BadSequence.as_u8());
    assert!(skipped_dt_sequence.active_sessions().is_empty());

    let mut wrong_source = ExtendedTransportProtocol::new();
    assert_eq!(wrong_source.process_frame(&rts, 0).len(), 1);
    let before = wrong_source.active_sessions()[0].clone();
    assert!(
        wrong_source
            .process_frame(&etp_dpo(0x11, 0x20, 1, 0, pgn), 0)
            .is_empty()
    );
    assert_eq!(wrong_source.active_sessions().len(), 1);
    assert_eq!(wrong_source.active_sessions()[0].state, before.state);
    assert_eq!(wrong_source.active_sessions()[0].pgn, pgn);
    assert_eq!(
        wrong_source.active_sessions()[0].bytes_transferred,
        before.bytes_transferred
    );
    assert_eq!(wrong_source.stats().dropped_frames, 1);
}

#[test]
fn datalink_etp_ignores_eoma_until_extended_transfer_is_complete_and_size_matches() {
    let pgn = 0xCA00;
    let total_bytes = TP_MAX_DATA_LENGTH + 1;
    let payload = vec![0xA5; total_bytes as usize];
    let mut etp = ExtendedTransportProtocol::new();
    let rts = etp
        .send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();
    assert_eq!(rts.len(), 1);
    assert_eq!(etp.active_sessions()[0].state, SessionState::WaitingForCTS);

    let premature_eoma = etp_eoma(0x20, 0x10, total_bytes, pgn);
    assert!(etp.process_frame(&premature_eoma, 0).is_empty());
    assert_eq!(etp.active_sessions().len(), 1);
    assert_eq!(etp.active_sessions()[0].state, SessionState::WaitingForCTS);
    assert_eq!(etp.stats().dropped_frames, 1);

    while etp.active_sessions()[0].state != SessionState::WaitingForEndOfMsg {
        let session = &etp.active_sessions()[0];
        assert_eq!(session.state, SessionState::WaitingForCTS);
        let next_packet = session.bytes_transferred / 7 + 1;
        let remaining = session.total_packets() - next_packet + 1;
        let count = remaining.min(TP_MAX_PACKETS_PER_CTS) as u8;
        assert!(
            etp.process_frame(&etp_cts(0x20, 0x10, count, next_packet, pgn), 0)
                .is_empty()
        );
        let window = etp.get_pending_data_frames();
        assert_eq!(window.len(), usize::from(count) + 1);
    }

    let mismatched_total = etp_eoma(0x20, 0x10, total_bytes + 1, pgn);
    assert!(etp.process_frame(&mismatched_total, 0).is_empty());
    assert_eq!(etp.active_sessions().len(), 1);
    assert_eq!(
        etp.active_sessions()[0].state,
        SessionState::WaitingForEndOfMsg
    );

    let valid_eoma = etp_eoma(0x20, 0x10, total_bytes, pgn);
    assert!(etp.process_frame(&valid_eoma, 0).is_empty());
    assert!(etp.active_sessions().is_empty());
}

#[test]
fn datalink_etp_treats_duplicate_cts_while_sending_as_idempotent_but_rejects_new_window() {
    let pgn = 0xCA00;
    let payload = vec![0x5A; (TP_MAX_DATA_LENGTH + 100) as usize];
    let mut etp = ExtendedTransportProtocol::new();
    etp.send(pgn, &payload, 0x10, 0x20, 0, Priority::Default)
        .unwrap();

    let first_cts = etp_cts(0x20, 0x10, TP_MAX_PACKETS_PER_CTS as u8, 1, pgn);
    assert!(etp.process_frame(&first_cts, 0).is_empty());
    assert_eq!(etp.active_sessions()[0].state, SessionState::SendingData);

    assert!(etp.process_frame(&first_cts, 0).is_empty());
    assert_eq!(etp.active_sessions()[0].state, SessionState::SendingData);
    assert!(etp.stats().is_empty());

    let shifted_window = etp_cts(0x20, 0x10, TP_MAX_PACKETS_PER_CTS as u8, 2, pgn);
    let responses = etp.process_frame(&shifted_window, 0);
    assert_eq!(responses.len(), 1);
    assert_eq!(responses[0].data[0], machbus::net::etp::etp_cm::ABORT);
    assert_eq!(
        responses[0].data[1],
        TransportAbortReason::ConnectionModeError.as_u8()
    );
    assert!(etp.active_sessions().is_empty());
    assert_eq!(etp.stats().aborts_sent, 1);
    assert_eq!(etp.stats().dropped_sessions, 1);
}
