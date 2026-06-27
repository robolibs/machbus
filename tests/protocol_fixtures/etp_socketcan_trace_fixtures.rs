#[test]
fn fixture_tp_malformed_cm_dt_corpus_is_bounded_and_deterministic() {
    let cm_to_receiver = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
    let dt_to_receiver = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x90);
    let cm_to_sender = Identifier::encode(Priority::Default, PGN_TP_CM, 0x90, 0x80);

    let mut rx = TransportProtocol::new();

    let unknown_cm = Frame::new(
        cm_to_receiver,
        parse_named_hex_frame(TP_MALFORMED_CM_DT_CORPUS_HEX, "unknown_cm"),
        8,
    );
    assert!(rx.process_frame(&unknown_cm, 0).is_empty());
    assert!(rx.active_sessions().is_empty());

    let dest_specific_bam = Frame::new(
        cm_to_receiver,
        parse_named_hex_frame(TP_MALFORMED_CM_DT_CORPUS_HEX, "bam_dest_specific"),
        8,
    );
    assert!(rx.process_frame(&dest_specific_bam, 0).is_empty());
    assert!(rx.active_sessions().is_empty());

    let rts_zero_cts = Frame::new(
        cm_to_receiver,
        parse_named_hex_frame(TP_MALFORMED_CM_DT_CORPUS_HEX, "rts_zero_cts"),
        8,
    );
    let abort = rx.process_frame(&rts_zero_cts, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].source(), 0x90);
    assert_eq!(abort[0].destination(), 0x80);
    assert_eq!(abort[0].data, *TP_ABORT_UNEXPECTED_SIZE_PGN_EF00);
    assert!(rx.active_sessions().is_empty());

    let orphan_dt_zero = Frame::new(
        dt_to_receiver,
        parse_named_hex_frame(TP_MALFORMED_CM_DT_CORPUS_HEX, "dt_zero"),
        8,
    );
    assert!(rx.process_frame(&orphan_dt_zero, 0).is_empty());
    assert!(rx.active_sessions().is_empty());

    let rts = Frame::new(cm_to_receiver, *TP_RTS_20B_PGN_EF00, 8);
    let cts = rx.process_frame(&rts, 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].data, *TP_CTS_20B_PGN_EF00);
    let abort = rx.process_frame(&orphan_dt_zero, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].source(), 0x90);
    assert_eq!(abort[0].destination(), 0x80);
    assert_eq!(abort[0].data, *TP_ABORT_BAD_SEQUENCE_PGN_EF00);
    assert!(rx.active_sessions().is_empty());

    let mut tx = TransportProtocol::new();
    let rts = tx
        .send(
            PGN_PROPRIETARY_A,
            &incrementing_payload(20),
            0x80,
            0x90,
            0,
            Priority::Default,
        )
        .expect("TP send must start");
    assert_eq!(rts[0].data, *TP_RTS_20B_PGN_EF00);
    let cts_next0 = Frame::new(
        cm_to_sender,
        parse_named_hex_frame(TP_MALFORMED_CM_DT_CORPUS_HEX, "cts_next0"),
        8,
    );
    let abort = tx.process_frame(&cts_next0, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].source(), 0x80);
    assert_eq!(abort[0].destination(), 0x90);
    assert_eq!(abort[0].data, *TP_ABORT_BAD_SEQUENCE_PGN_EF00);
    assert!(tx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_rts_cts_dpo_and_first_dt_frame_are_stable() {
    let payload = incrementing_payload(2000);
    let mut tx = ExtendedTransportProtocol::new();
    let mut rx = ExtendedTransportProtocol::new();

    let rts = tx
        .send(PGN_TRANSFER, &payload, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send must start");
    assert_eq!(rts.len(), 1);
    assert_eq!(rts[0].pgn(), PGN_ETP_CM);
    assert_eq!(rts[0].source(), 0x10);
    assert_eq!(rts[0].destination(), 0x20);
    assert_eq!(rts[0].data, *ETP_RTS_2000B_PGN_CA00);

    let cts = rx.process_frame(&rts[0], 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].pgn(), PGN_ETP_CM);
    assert_eq!(cts[0].source(), 0x20);
    assert_eq!(cts[0].destination(), 0x10);
    assert_eq!(cts[0].data, *ETP_CTS_2000B_PGN_CA00);

    assert!(tx.process_frame(&cts[0], 0).is_empty());
    let frames = tx.get_pending_data_frames();
    assert_eq!(frames.len(), 17);

    let dpo = frames[0];
    assert_eq!(dpo.pgn(), PGN_ETP_CM);
    assert_eq!(dpo.source(), 0x10);
    assert_eq!(dpo.destination(), 0x20);
    assert_eq!(dpo.data, *ETP_DPO_2000B_PGN_CA00);

    let first_dt = frames[1];
    assert_eq!(first_dt.pgn(), PGN_ETP_DT);
    assert_eq!(first_dt.source(), 0x10);
    assert_eq!(first_dt.destination(), 0x20);
    assert_eq!(first_dt.data, *ETP_DT_SEQ1_2000B_PAYLOAD);
}

#[test]
fn fixture_etp_rejects_invalid_endpoint_addresses_before_session_mutation() {
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let rts = Frame::new(
            Identifier::encode(Priority::Default, PGN_ETP_CM, source, 0x20),
            *ETP_RTS_2000B_PGN_CA00,
            8,
        );
        let mut rx = ExtendedTransportProtocol::new();

        assert!(rx.process_frame(&rts, 0).is_empty());
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    for destination in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let rts = Frame::new(
            Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, destination),
            *ETP_RTS_2000B_PGN_CA00,
            8,
        );
        let mut rx = ExtendedTransportProtocol::new();

        assert!(rx.process_frame(&rts, 0).is_empty());
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }
}

#[test]
fn fixture_etp_cts_window_variation_uses_golden_dpo_and_next_cts() {
    let payload = incrementing_payload(2000);
    let mut tx = ExtendedTransportProtocol::new();
    let mut rx = ExtendedTransportProtocol::new();

    let rts = tx
        .send(PGN_TRANSFER, &payload, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send must start");
    assert_eq!(rx.process_frame(&rts[0], 0).len(), 1);

    let cts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x20, 0x10);
    let cts = Frame::new(cts_id, *ETP_CTS_3_PACKETS_2000B_PGN_CA00, 8);
    assert!(tx.process_frame(&cts, 0).is_empty());
    let pending = tx.get_pending_data_frames();
    assert_eq!(pending.len(), 4);
    assert_eq!(pending[0].data, *ETP_DPO_3_PACKETS_2000B_PGN_CA00);
    assert_eq!(pending[1].data, *ETP_DT_SEQ1_2000B_PAYLOAD);
    assert_eq!(pending[3].data, *ETP_DT_SEQ3_2000B_PAYLOAD);

    assert!(rx.process_frame(&pending[0], 0).is_empty());
    assert!(rx.process_frame(&pending[1], 0).is_empty());
    assert!(rx.process_frame(&pending[2], 0).is_empty());
    let next_cts = rx.process_frame(&pending[3], 0);
    assert_eq!(next_cts.len(), 1);
    assert_eq!(next_cts[0].pgn(), PGN_ETP_CM);
    assert_eq!(next_cts[0].source(), 0x20);
    assert_eq!(next_cts[0].destination(), 0x10);
    assert_eq!(next_cts[0].data, *ETP_CTS_NEXT4_2000B_PGN_CA00);
}

#[test]
fn fixture_etp_duplicate_cts_retransmits_same_golden_window() {
    let payload = incrementing_payload(2000);
    let mut tx = ExtendedTransportProtocol::new();

    let rts = tx
        .send(PGN_TRANSFER, &payload, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send must start");
    assert_eq!(rts[0].data, *ETP_RTS_2000B_PGN_CA00);

    let cts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x20, 0x10);
    let cts = Frame::new(cts_id, *ETP_CTS_2000B_PGN_CA00, 8);
    assert!(tx.process_frame(&cts, 0).is_empty());
    let first_window = tx.get_pending_data_frames();
    assert_eq!(first_window.len(), 17);
    assert_eq!(first_window[0].data, *ETP_DPO_2000B_PGN_CA00);
    assert_eq!(first_window[1].data, *ETP_DT_SEQ1_2000B_PAYLOAD);

    // If the receiver's first CTS is retried because the DPO/DT burst was
    // lost, the sender rewinds to the requested packet and retransmits the
    // exact same fixture-backed first window.
    assert!(tx.process_frame(&cts, 0).is_empty());
    let retransmitted = tx.get_pending_data_frames();
    assert_eq!(retransmitted.len(), 17);
    assert_eq!(retransmitted[0].data, *ETP_DPO_2000B_PGN_CA00);
    assert_eq!(retransmitted[1].data, *ETP_DT_SEQ1_2000B_PAYLOAD);
    assert_eq!(retransmitted[16].data, first_window[16].data);
    assert_eq!(tx.active_sessions().len(), 1);
}

#[test]
fn fixture_etp_cts_hold_then_resume_uses_golden_dpo_and_keeps_sender_alive() {
    let payload = incrementing_payload(2000);
    let cts_hold_expected = parse_hex_bytes(ETP_CTS_HOLD_PGN_CA00_HEX.trim());
    let mut tx = ExtendedTransportProtocol::new();

    let rts = tx
        .send(PGN_TRANSFER, &payload, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send must start");
    assert_eq!(rts[0].data, *ETP_RTS_2000B_PGN_CA00);

    let cts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x20, 0x10);
    let cts_hold_bytes: [u8; 8] = cts_hold_expected
        .as_slice()
        .try_into()
        .expect("hold fixture is one ETP CM frame");
    let cts_hold = Frame::new(cts_id, cts_hold_bytes, 8);
    assert_eq!(cts_hold.data.as_slice(), cts_hold_expected.as_slice());

    assert!(tx.process_frame(&cts_hold, 0).is_empty());
    assert_eq!(tx.active_sessions().len(), 1);
    assert_eq!(tx.active_sessions()[0].state, SessionState::WaitingForCTS);
    assert!(tx.get_pending_data_frames().is_empty());
    assert!(tx.update(ETP_TIMEOUT_T1_MS - 1).is_empty());
    assert_eq!(tx.active_sessions().len(), 1);

    let cts_resume = Frame::new(cts_id, *ETP_CTS_3_PACKETS_2000B_PGN_CA00, 8);
    assert!(tx.process_frame(&cts_resume, 0).is_empty());
    let pending = tx.get_pending_data_frames();
    assert_eq!(pending.len(), 4);
    assert_eq!(pending[0].data, *ETP_DPO_3_PACKETS_2000B_PGN_CA00);
    assert_eq!(pending[1].data, *ETP_DT_SEQ1_2000B_PAYLOAD);
    assert_eq!(pending[3].data, *ETP_DT_SEQ3_2000B_PAYLOAD);
}

#[test]
fn fixture_etp_receiver_abort_cancels_sender_with_golden_reason() {
    let payload = incrementing_payload(2000);
    let mut tx = ExtendedTransportProtocol::new();
    let aborts = Rc::new(RefCell::new(Vec::new()));
    let observed = Rc::clone(&aborts);
    tx.on_abort
        .subscribe(move |event| observed.borrow_mut().push(event.reason));

    let rts = tx
        .send(PGN_TRANSFER, &payload, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send must start");
    assert_eq!(rts[0].data, *ETP_RTS_2000B_PGN_CA00);

    let abort_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x20, 0x10);
    let abort = Frame::new(abort_id, *ETP_ABORT_NO_RESOURCES_PGN_CA00, 8);
    assert!(tx.process_frame(&abort, 0).is_empty());

    assert!(tx.active_sessions().is_empty());
    assert_eq!(
        aborts.borrow().as_slice(),
        &[machbus::net::TransportAbortReason::ResourcesUnavailable]
    );
}

#[test]
fn fixture_etp_full_stream_reassembles_and_emits_golden_eoma() {
    let payload = incrementing_payload(2000);
    let mut tx = ExtendedTransportProtocol::new();
    let mut rx = ExtendedTransportProtocol::new();
    let received = Rc::new(RefCell::new(None::<Vec<u8>>));
    let observed = Rc::clone(&received);
    rx.on_complete
        .subscribe(move |session| *observed.borrow_mut() = Some(session.data.clone()));

    let rts = tx
        .send(PGN_TRANSFER, &payload, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send must start");
    assert_eq!(rts[0].data, *ETP_RTS_2000B_PGN_CA00);

    let mut to_tx = rx.process_frame(&rts[0], 0);
    assert_eq!(to_tx.len(), 1);
    assert_eq!(to_tx[0].data, *ETP_CTS_2000B_PGN_CA00);

    let mut eoma = None;
    for _ in 0..32 {
        for frame in to_tx.drain(..) {
            assert!(tx.process_frame(&frame, 0).is_empty());
        }

        let pending = tx.get_pending_data_frames();
        assert!(
            !pending.is_empty(),
            "ETP sender stalled before receiver emitted EoMA"
        );

        for frame in &pending {
            for response in rx.process_frame(frame, 0) {
                if response.data[0] == 0x17 {
                    eoma = Some(response);
                } else {
                    to_tx.push(response);
                }
            }
        }

        if eoma.is_some() {
            break;
        }
    }

    let eoma = eoma.expect("receiver must emit ETP EoMA");
    assert_eq!(eoma.pgn(), PGN_ETP_CM);
    assert_eq!(eoma.source(), 0x20);
    assert_eq!(eoma.destination(), 0x10);
    assert_eq!(eoma.data, *ETP_EOMA_2000B_PGN_CA00);
    assert_eq!(received.borrow().as_ref(), Some(&payload));
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_profile_max_4096b_stream_reassembles_with_golden_edges() {
    const PROFILE_MAX_BYTES: usize = 4096;

    let payload = incrementing_payload(PROFILE_MAX_BYTES);
    let rts_expected = parse_hex_bytes(ETP_RTS_4096B_PGN_CA00_HEX.trim());
    let cts_expected = parse_hex_bytes(ETP_CTS_4096B_PGN_CA00_HEX.trim());
    let first_dpo_expected = parse_hex_bytes(ETP_DPO_FIRST_4096B_PGN_CA00_HEX.trim());
    let first_dt_expected = parse_hex_bytes(ETP_DT_SEQ1_4096B_PAYLOAD_HEX.trim());
    let last_dpo_expected = parse_hex_bytes(ETP_DPO_LAST_4096B_PGN_CA00_HEX.trim());
    let last_dt_expected = parse_hex_bytes(ETP_DT_SEQ10_LAST_4096B_PAYLOAD_HEX.trim());
    let eoma_expected = parse_hex_bytes(ETP_EOMA_4096B_PGN_CA00_HEX.trim());

    let mut tx = ExtendedTransportProtocol::new();
    let mut rx = ExtendedTransportProtocol::with_max_receive_bytes(PROFILE_MAX_BYTES as u32);
    let received = Rc::new(RefCell::new(None::<Vec<u8>>));
    let observed = Rc::clone(&received);
    rx.on_complete
        .subscribe(move |session| *observed.borrow_mut() = Some(session.data.clone()));

    let rts = tx
        .send(PGN_TRANSFER, &payload, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send must start for the profile maximum");
    assert_eq!(rts[0].data.as_slice(), rts_expected.as_slice());

    let mut to_tx = rx.process_frame(&rts[0], 0);
    assert_eq!(to_tx.len(), 1);
    assert_eq!(to_tx[0].data.as_slice(), cts_expected.as_slice());

    let mut first_window_checked = false;
    let mut last_window_checked = false;
    let mut eoma = None;
    for _ in 0..48 {
        for frame in to_tx.drain(..) {
            assert!(tx.process_frame(&frame, 0).is_empty());
        }

        let pending = tx.get_pending_data_frames();
        assert!(
            !pending.is_empty(),
            "ETP sender stalled before receiver emitted EoMA"
        );

        if !first_window_checked {
            assert_eq!(pending[0].data.as_slice(), first_dpo_expected.as_slice());
            assert_eq!(pending[1].data.as_slice(), first_dt_expected.as_slice());
            first_window_checked = true;
        }
        if pending[0].data.as_slice() == last_dpo_expected.as_slice() {
            assert_eq!(pending.len(), 11, "last 64 bytes fit in 10 DT frames");
            assert_eq!(pending[10].data.as_slice(), last_dt_expected.as_slice());
            last_window_checked = true;
        }

        for frame in &pending {
            for response in rx.process_frame(frame, 0) {
                if response.data[0] == 0x17 {
                    eoma = Some(response);
                } else {
                    to_tx.push(response);
                }
            }
        }

        if eoma.is_some() {
            break;
        }
    }

    let eoma = eoma.expect("receiver must emit ETP EoMA");
    assert_eq!(eoma.source(), 0x20);
    assert_eq!(eoma.destination(), 0x10);
    assert_eq!(eoma.data.as_slice(), eoma_expected.as_slice());
    assert!(first_window_checked, "first DPO/DT window was checked");
    assert!(last_window_checked, "last DPO/DT window was checked");
    assert_eq!(received.borrow().as_ref(), Some(&payload));
    assert!(rx.active_sessions().is_empty());
    assert!(tx.process_frame(&eoma, 0).is_empty());
    assert!(tx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_two_peer_sessions_interleave_across_windows_to_eoma() {
    let payload_a = incrementing_payload(2000);
    let payload_b: Vec<u8> = (0..2000)
        .map(|n| 0xFFu8.wrapping_sub((n & 0xFF) as u8))
        .collect();
    let mut tx_a = ExtendedTransportProtocol::new();
    let mut tx_b = ExtendedTransportProtocol::new();
    let mut rx = ExtendedTransportProtocol::new();
    let received = Rc::new(RefCell::new(Vec::<(u8, Vec<u8>)>::new()));
    let observed = Rc::clone(&received);
    rx.on_complete.subscribe(move |session| {
        observed
            .borrow_mut()
            .push((session.source_address, session.data.clone()))
    });

    let rts_a = tx_a
        .send(PGN_TRANSFER, &payload_a, 0x10, 0x20, 0, Priority::Default)
        .expect("ETP send A must start");
    let rts_b = tx_b
        .send(PGN_TRANSFER, &payload_b, 0x11, 0x20, 0, Priority::Default)
        .expect("ETP send B must start");
    assert_eq!(rts_a[0].data, *ETP_RTS_2000B_PGN_CA00);
    assert_eq!(rts_b[0].data, *ETP_RTS_2000B_PGN_CA00);

    let mut to_a = rx.process_frame(&rts_a[0], 0);
    let mut to_b = rx.process_frame(&rts_b[0], 0);
    assert_eq!(to_a.len(), 1);
    assert_eq!(to_a[0].source(), 0x20);
    assert_eq!(to_a[0].destination(), 0x10);
    assert_eq!(to_a[0].data, *ETP_CTS_2000B_PGN_CA00);
    assert_eq!(to_b.len(), 1);
    assert_eq!(to_b[0].source(), 0x20);
    assert_eq!(to_b[0].destination(), 0x11);
    assert_eq!(to_b[0].data, *ETP_CTS_2000B_PGN_CA00);

    let mut eoma_a = false;
    let mut eoma_b = false;
    for _ in 0..40 {
        for frame in to_a.drain(..) {
            assert!(tx_a.process_frame(&frame, 0).is_empty());
        }
        for frame in to_b.drain(..) {
            assert!(tx_b.process_frame(&frame, 0).is_empty());
        }

        let pending_a = tx_a.get_pending_data_frames();
        let pending_b = tx_b.get_pending_data_frames();
        if pending_a.is_empty() && pending_b.is_empty() && eoma_a && eoma_b {
            break;
        }

        let max_len = pending_a.len().max(pending_b.len());
        for i in 0..max_len {
            if let Some(frame) = pending_a.get(i) {
                for response in rx.process_frame(frame, 0) {
                    assert_eq!(response.source(), 0x20);
                    assert_eq!(response.destination(), 0x10);
                    if response.data[0] == 0x17 {
                        assert_eq!(response.data, *ETP_EOMA_2000B_PGN_CA00);
                        eoma_a = true;
                    }
                    to_a.push(response);
                }
            }
            if let Some(frame) = pending_b.get(i) {
                for response in rx.process_frame(frame, 0) {
                    assert_eq!(response.source(), 0x20);
                    assert_eq!(response.destination(), 0x11);
                    if response.data[0] == 0x17 {
                        assert_eq!(response.data, *ETP_EOMA_2000B_PGN_CA00);
                        eoma_b = true;
                    }
                    to_b.push(response);
                }
            }
        }
    }

    assert!(eoma_a, "peer A must complete to EoMA");
    assert!(eoma_b, "peer B must complete to EoMA");
    assert!(rx.active_sessions().is_empty());
    assert!(tx_a.active_sessions().is_empty());
    assert!(tx_b.active_sessions().is_empty());

    let received = received.borrow();
    assert_eq!(received.len(), 2);
    assert!(
        received
            .iter()
            .any(|(source, data)| *source == 0x10 && data == &payload_a)
    );
    assert!(
        received
            .iter()
            .any(|(source, data)| *source == 0x11 && data == &payload_b)
    );
}

#[test]
fn fixture_etp_receive_timeout_emits_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let rts = Frame::new(rts_id, *ETP_RTS_2000B_PGN_CA00, 8);

    let mut rx = ExtendedTransportProtocol::new();
    let cts = rx.process_frame(&rts, 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].data, *ETP_CTS_2000B_PGN_CA00);

    let abort = rx.update(ETP_TIMEOUT_T1_MS + 1);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_ETP_CM);
    assert_eq!(abort[0].source(), 0x20);
    assert_eq!(abort[0].destination(), 0x10);
    assert_eq!(abort[0].data, *ETP_ABORT_TIMEOUT_PGN_CA00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_receive_cap_aborts_with_golden_no_resources() {
    let rts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let rts = Frame::new(rts_id, *ETP_RTS_2000B_PGN_CA00, 8);

    let mut rx = ExtendedTransportProtocol::with_max_receive_bytes(1_999);
    let abort = rx.process_frame(&rts, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_ETP_CM);
    assert_eq!(abort[0].source(), 0x20);
    assert_eq!(abort[0].destination(), 0x10);
    assert_eq!(abort[0].data, *ETP_ABORT_NO_RESOURCES_PGN_CA00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_wrong_peer_dpo_is_ignored_without_dropping_session() {
    let rts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let rts = Frame::new(rts_id, *ETP_RTS_2000B_PGN_CA00, 8);
    let wrong_src_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x11, 0x20);
    let wrong_dst_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x21);
    let good_dpo_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let good_dt_id = Identifier::encode(Priority::Default, PGN_ETP_DT, 0x10, 0x20);
    let wrong_src = Frame::new(wrong_src_id, *ETP_DPO_2000B_PGN_CA00, 8);
    let wrong_dst = Frame::new(wrong_dst_id, *ETP_DPO_2000B_PGN_CA00, 8);
    let good_dpo = Frame::new(good_dpo_id, *ETP_DPO_2000B_PGN_CA00, 8);
    let good_dt = Frame::new(good_dt_id, *ETP_DT_SEQ1_2000B_PAYLOAD, 8);

    let mut rx = ExtendedTransportProtocol::new();
    assert_eq!(rx.process_frame(&rts, 0).len(), 1);

    assert!(rx.process_frame(&wrong_src, 0).is_empty());
    assert_eq!(rx.active_sessions().len(), 1);
    assert!(rx.process_frame(&wrong_dst, 0).is_empty());
    assert_eq!(rx.active_sessions().len(), 1);

    assert!(rx.process_frame(&good_dpo, 0).is_empty());
    assert!(rx.process_frame(&good_dt, 0).is_empty());
    assert_eq!(rx.active_sessions().len(), 1);
}

#[test]
fn fixture_etp_bad_dpo_offset_aborts_with_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let rts = Frame::new(rts_id, *ETP_RTS_2000B_PGN_CA00, 8);
    let bad_dpo_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let bad_dpo = Frame::new(bad_dpo_id, *ETP_DPO_BAD_OFFSET_2000B_PGN_CA00, 8);

    let mut rx = ExtendedTransportProtocol::new();
    let cts = rx.process_frame(&rts, 0);
    assert_eq!(cts.len(), 1);
    assert_eq!(cts[0].data, *ETP_CTS_2000B_PGN_CA00);

    let abort = rx.process_frame(&bad_dpo, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_ETP_CM);
    assert_eq!(abort[0].source(), 0x20);
    assert_eq!(abort[0].destination(), 0x10);
    assert_eq!(abort[0].data, *ETP_ABORT_BAD_SEQUENCE_PGN_CA00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_tp_sized_rts_aborts_with_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let too_small = Frame::new(rts_id, *ETP_RTS_TP_SIZED_PGN_CA00, 8);

    let mut rx = ExtendedTransportProtocol::new();
    let abort = rx.process_frame(&too_small, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_ETP_CM);
    assert_eq!(abort[0].source(), 0x20);
    assert_eq!(abort[0].destination(), 0x10);
    assert_eq!(abort[0].data, *ETP_ABORT_UNEXPECTED_SIZE_PGN_CA00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_over_max_rts_aborts_with_golden_abort() {
    let rts_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let too_large = Frame::new(rts_id, *ETP_RTS_OVER_MAX_PGN_CA00, 8);

    let mut rx = ExtendedTransportProtocol::new();
    let abort = rx.process_frame(&too_large, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].pgn(), PGN_ETP_CM);
    assert_eq!(abort[0].source(), 0x20);
    assert_eq!(abort[0].destination(), 0x10);
    assert_eq!(abort[0].data, *ETP_ABORT_UNEXPECTED_SIZE_PGN_CA00);
    assert!(rx.active_sessions().is_empty());
}

#[test]
fn fixture_etp_malformed_cm_dt_corpus_is_bounded_and_deterministic() {
    let cm_to_receiver = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
    let dt_to_receiver = Identifier::encode(Priority::Default, PGN_ETP_DT, 0x10, 0x20);
    let cm_to_sender = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x20, 0x10);

    let mut rx = ExtendedTransportProtocol::new();
    let unknown_cm = Frame::new(
        cm_to_receiver,
        parse_named_hex_frame(ETP_MALFORMED_CM_DT_CORPUS_HEX, "unknown_cm"),
        8,
    );
    assert!(rx.process_frame(&unknown_cm, 0).is_empty());
    assert!(rx.active_sessions().is_empty());

    let orphan_dt_zero = Frame::new(
        dt_to_receiver,
        parse_named_hex_frame(ETP_MALFORMED_CM_DT_CORPUS_HEX, "dt_zero"),
        8,
    );
    assert!(rx.process_frame(&orphan_dt_zero, 0).is_empty());
    assert!(rx.active_sessions().is_empty());

    let dpo_zero = Frame::new(
        cm_to_receiver,
        parse_named_hex_frame(ETP_MALFORMED_CM_DT_CORPUS_HEX, "dpo_zero"),
        8,
    );
    let dpo_over_window = Frame::new(
        cm_to_receiver,
        parse_named_hex_frame(ETP_MALFORMED_CM_DT_CORPUS_HEX, "dpo_over_window"),
        8,
    );

    for bad_dpo in [dpo_zero, dpo_over_window] {
        let mut rx = ExtendedTransportProtocol::new();
        let rts = Frame::new(cm_to_receiver, *ETP_RTS_2000B_PGN_CA00, 8);
        let cts = rx.process_frame(&rts, 0);
        assert_eq!(cts.len(), 1);
        assert_eq!(cts[0].data, *ETP_CTS_2000B_PGN_CA00);

        let abort = rx.process_frame(&bad_dpo, 0);
        assert_eq!(abort.len(), 1);
        assert_eq!(abort[0].source(), 0x20);
        assert_eq!(abort[0].destination(), 0x10);
        assert_eq!(abort[0].data, *ETP_ABORT_BAD_SEQUENCE_PGN_CA00);
        assert!(rx.active_sessions().is_empty());
    }

    let mut rx = ExtendedTransportProtocol::new();
    let rts = Frame::new(cm_to_receiver, *ETP_RTS_2000B_PGN_CA00, 8);
    assert_eq!(rx.process_frame(&rts, 0).len(), 1);
    let dpo = Frame::new(cm_to_receiver, *ETP_DPO_2000B_PGN_CA00, 8);
    assert!(rx.process_frame(&dpo, 0).is_empty());
    let dt_zero = Frame::new(
        dt_to_receiver,
        parse_named_hex_frame(ETP_MALFORMED_CM_DT_CORPUS_HEX, "dt_zero"),
        8,
    );
    let abort = rx.process_frame(&dt_zero, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].source(), 0x20);
    assert_eq!(abort[0].destination(), 0x10);
    assert_eq!(abort[0].data, *ETP_ABORT_BAD_SEQUENCE_PGN_CA00);
    assert!(rx.active_sessions().is_empty());

    let mut tx = ExtendedTransportProtocol::new();
    let rts = tx
        .send(
            PGN_TRANSFER,
            &incrementing_payload(2000),
            0x10,
            0x20,
            0,
            Priority::Default,
        )
        .expect("ETP send must start");
    assert_eq!(rts[0].data, *ETP_RTS_2000B_PGN_CA00);
    let cts_next0 = Frame::new(
        cm_to_sender,
        parse_named_hex_frame(ETP_MALFORMED_CM_DT_CORPUS_HEX, "cts_next0"),
        8,
    );
    let abort = tx.process_frame(&cts_next0, 0);
    assert_eq!(abort.len(), 1);
    assert_eq!(abort[0].source(), 0x10);
    assert_eq!(abort[0].destination(), 0x20);
    assert_eq!(abort[0].data, *ETP_ABORT_BAD_SEQUENCE_PGN_CA00);
    assert!(tx.active_sessions().is_empty());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn proptest_tp_rx_arbitrary_cm_dt_frames_are_bounded(cm in any::<[u8; 8]>(), dt in any::<[u8; 8]>()) {
        let mut tp = TransportProtocol::with_max_sessions(2);
        tp.set_max_receive_bytes(64);
        let cm_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x80, 0x90);
        let dt_id = Identifier::encode(Priority::Default, PGN_TP_DT, 0x80, 0x90);

        for frame in [Frame::new(cm_id, cm, 8), Frame::new(dt_id, dt, 8)] {
            let responses = tp.process_frame(&frame, 0);
            prop_assert!(tp.active_sessions().len() <= tp.max_sessions());
            for response in responses {
                prop_assert_eq!(response.length, 8);
                prop_assert_eq!(response.pgn(), PGN_TP_CM);
            }
        }
    }

    #[test]
    fn proptest_tp_tx_arbitrary_cm_frames_are_bounded(cm in any::<[u8; 8]>()) {
        let mut tp = TransportProtocol::new();
        let initial = tp
            .send(
                PGN_PROPRIETARY_A,
                &incrementing_payload(64),
                0x80,
                0x90,
                0,
                Priority::Default,
            )
            .expect("TP send must start");
        prop_assert_eq!(initial[0].data, [0x10, 0x40, 0x00, 0x0A, 0x10, 0x00, 0xEF, 0x00]);

        let cm_id = Identifier::encode(Priority::Default, PGN_TP_CM, 0x90, 0x80);
        let responses = tp.process_frame(&Frame::new(cm_id, cm, 8), 0);
        prop_assert!(tp.active_sessions().len() <= tp.max_sessions());
        for response in responses {
            prop_assert_eq!(response.length, 8);
            prop_assert_eq!(response.pgn(), PGN_TP_CM);
        }

        let pending = tp.get_pending_data_frames();
        prop_assert!(pending.len() <= TP_MAX_PACKETS_PER_CTS as usize);
        for frame in pending {
            prop_assert_eq!(frame.length, 8);
            prop_assert_eq!(frame.pgn(), PGN_TP_DT);
        }
    }

    #[test]
    fn proptest_etp_rx_arbitrary_cm_dt_frames_are_bounded(cm in any::<[u8; 8]>(), dt in any::<[u8; 8]>()) {
        let mut etp = ExtendedTransportProtocol::with_max_sessions(2);
        etp.set_max_receive_bytes(4096);
        let cm_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x10, 0x20);
        let dt_id = Identifier::encode(Priority::Default, PGN_ETP_DT, 0x10, 0x20);

        for frame in [Frame::new(cm_id, cm, 8), Frame::new(dt_id, dt, 8)] {
            let responses = etp.process_frame(&frame, 0);
            prop_assert!(etp.active_sessions().len() <= etp.max_sessions());
            for response in responses {
                prop_assert_eq!(response.length, 8);
                prop_assert_eq!(response.pgn(), PGN_ETP_CM);
            }
        }
    }

    #[test]
    fn proptest_etp_tx_arbitrary_cm_frames_are_bounded(cm in any::<[u8; 8]>()) {
        let mut etp = ExtendedTransportProtocol::new();
        let initial = etp
            .send(
                PGN_TRANSFER,
                &incrementing_payload(2000),
                0x10,
                0x20,
                0,
                Priority::Default,
            )
            .expect("ETP send must start");
        prop_assert_eq!(initial[0].data, *ETP_RTS_2000B_PGN_CA00);

        let cm_id = Identifier::encode(Priority::Default, PGN_ETP_CM, 0x20, 0x10);
        let responses = etp.process_frame(&Frame::new(cm_id, cm, 8), 0);
        prop_assert!(etp.active_sessions().len() <= etp.max_sessions());
        for response in responses {
            prop_assert_eq!(response.length, 8);
            prop_assert_eq!(response.pgn(), PGN_ETP_CM);
        }

        let pending = etp.get_pending_data_frames();
        prop_assert!(pending.len() <= TP_MAX_PACKETS_PER_CTS as usize + 1);
        if let Some((first, rest)) = pending.split_first() {
            prop_assert_eq!(first.length, 8);
            prop_assert_eq!(first.pgn(), PGN_ETP_CM);
            for frame in rest {
                prop_assert_eq!(frame.length, 8);
                prop_assert_eq!(frame.pgn(), PGN_ETP_DT);
            }
        }
    }
}

fn incrementing_payload(len: usize) -> Vec<u8> {
    (0..len).map(|n| (n & 0xFF) as u8).collect()
}

fn sc_step(step_id: u16) -> SequenceStep {
    SequenceStep {
        step_id,
        description: format!("step {step_id}"),
        duration_ms: 0,
        completed: false,
    }
}

fn temp_fixture_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!("machbus_{prefix}_{nanos}"));
    fs::create_dir_all(&path).unwrap();
    path
}

fn parse_named_hex_frame(corpus: &str, name: &str) -> [u8; 8] {
    let bytes = parse_named_hex_bytes(corpus, name);
    bytes
        .as_slice()
        .try_into()
        .expect("named fixture must contain one classic CAN payload")
}

fn parse_named_hex_bytes(corpus: &str, name: &str) -> Vec<u8> {
    for line in corpus.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((label, hex)) = line.split_once('=') else {
            panic!("malformed named hex fixture line: {line}");
        };
        if label.trim() == name {
            return parse_hex_bytes(hex.trim());
        }
    }
    panic!("missing named hex fixture: {name}");
}

fn parse_named_text_value<'a>(corpus: &'a str, name: &str) -> &'a str {
    for line in corpus.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((label, value)) = line.split_once('=') else {
            panic!("malformed named text fixture line: {line}");
        };
        if label.trim() == name {
            return value.trim();
        }
    }
    panic!("missing named text fixture: {name}");
}

fn parse_candump_fixture_line(line: &str) -> Option<(u32, Vec<u8>, bool)> {
    parse_hash_candump_fixture_line(line).or_else(|| parse_bracketed_candump_fixture_line(line))
}

fn parse_hash_candump_fixture_line(line: &str) -> Option<(u32, Vec<u8>, bool)> {
    let token = line.split_whitespace().find(|part| part.contains('#'))?;
    let (id_hex, payload_hex) = token.split_once('#')?;
    if id_hex.is_empty() || payload_hex.contains('#') {
        return None;
    }
    let raw_id = u32::from_str_radix(id_hex, 16).ok()?;
    let data = parse_candump_payload_hex(payload_hex)?;
    if data.len() > 8 {
        return None;
    }
    let extended = match id_hex.len() {
        1..=3 if raw_id <= 0x7FF => false,
        8 if raw_id <= 0x1FFF_FFFF => true,
        _ => return None,
    };
    Some((raw_id, data, extended))
}

fn parse_bracketed_candump_fixture_line(line: &str) -> Option<(u32, Vec<u8>, bool)> {
    let mut parts = line.split_whitespace().peekable();
    if parts
        .peek()
        .is_some_and(|token| token.starts_with('(') && token.ends_with(')') && token.len() > 2)
    {
        let _timestamp = parts.next();
    }

    let _interface = parts.next()?;
    let id_hex = parts.next()?;
    let dlc_token = parts.next()?;
    if !(dlc_token.starts_with('[') && dlc_token.ends_with(']')) {
        return None;
    }
    let dlc = dlc_token[1..dlc_token.len() - 1].parse::<usize>().ok()?;
    if dlc > 8 {
        return None;
    }

    let raw_id = u32::from_str_radix(id_hex, 16).ok()?;
    let extended = match id_hex.len() {
        1..=3 if raw_id <= 0x7FF => false,
        8 if raw_id <= 0x1FFF_FFFF => true,
        _ => return None,
    };

    let mut data = Vec::with_capacity(dlc);
    for token in parts {
        if token.len() != 2 {
            return None;
        }
        data.push(u8::from_str_radix(token, 16).ok()?);
    }
    if data.len() != dlc {
        return None;
    }

    Some((raw_id, data, extended))
}

fn parse_candump_payload_hex(payload_hex: &str) -> Option<Vec<u8>> {
    if !payload_hex.len().is_multiple_of(2) {
        return None;
    }
    let mut out = Vec::with_capacity(payload_hex.len() / 2);
    for chunk in payload_hex.as_bytes().chunks(2) {
        let text = std::str::from_utf8(chunk).ok()?;
        out.push(u8::from_str_radix(text, 16).ok()?);
    }
    Some(out)
}

fn assert_geo_rate(tc: &TCGEOInterface, name: &str, pos: Wgs) {
    let expected = parse_named_text_value(ISOBUS_TC_GEO_PRESCRIPTION, name);
    let actual = tc.get_rate_at_position(pos);
    if expected == "none" {
        assert_eq!(actual, None, "{name}");
    } else {
        assert_eq!(actual, Some(expected.parse().unwrap()), "{name}");
    }
}

fn parse_hex_bytes(s: &str) -> Vec<u8> {
    assert!(
        s.len().is_multiple_of(2),
        "hex payload must have an even length"
    );
    s.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let hi = hex_nibble(pair[0]);
            let lo = hex_nibble(pair[1]);
            (hi << 4) | lo
        })
        .collect()
}

fn parse_hex_u64(s: &str) -> u64 {
    let hex = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let mut value = 0u64;
    for byte in hex.bytes() {
        value = (value << 4) | u64::from(hex_nibble(byte));
    }
    value
}

fn hex_nibble(b: u8) -> u8 {
    match b {
        b'0'..=b'9' => b - b'0',
        b'a'..=b'f' => b - b'a' + 10,
        b'A'..=b'F' => b - b'A' + 10,
        _ => panic!("invalid hex digit: {b}"),
    }
}

/// A real capture taken off a live `vcan0` interface by the
/// `socketcan_capture` example via `net::CaptureRecorder` (not synthesized).
const VCAN_LOOPBACK_CAPTURE: &str = include_str!("../fixtures/isobus/vcan_loopback_capture.candump");

#[test]
fn fixture_vcan_loopback_capture_is_verifiable() {
    use machbus::net::CaptureLog;
    let log = CaptureLog::parse(VCAN_LOOPBACK_CAPTURE);
    // Three ISOBUS-shaped frames captured off the virtual bus.
    assert_eq!(log.len(), 3);
    assert!(
        log.contains_can_id(0x18EE_FF80),
        "address-claim-shaped frame"
    );
    assert!(log.contains_can_id(0x18FE_CA80), "DM1-shaped frame");
    assert!(log.contains_can_id(0x18FE_E680), "time/date-shaped frame");
    assert_eq!(log.count_can_id(0x18FE_E680), 1);
    // Every captured frame parsed to a full 8-byte payload.
    assert!(log.frames.iter().all(|f| f.data.len() == 8));
}

/// Reduced live `vcan0` capture of the real machbus address-claim stack
/// (`socketcan_address_claim`), recorded via `net::CaptureRecorder`.
const VCAN_ADDRESS_CLAIM_CAPTURE: &str = include_str!("../fixtures/traces/vcan_address_claim.candump");

#[test]
fn fixture_vcan_address_claim_capture_is_verifiable() {
    use machbus::net::CaptureLog;
    let log = CaptureLog::parse(VCAN_ADDRESS_CLAIM_CAPTURE);
    // The capture holds the triggering PGN Request and the Address Claimed.
    assert!(
        log.contains_can_id(0x18EA_FFFE),
        "PGN Request for Address Claimed"
    );
    assert!(
        log.contains_can_id(0x18EE_FF80),
        "Address Claimed (PGN 0xEE00) from preferred source 0x80"
    );
    // The Address Claimed frame's source address (low id byte) is 0x80.
    let claimed = log
        .frames
        .iter()
        .find(|f| f.can_id == 0x18EE_FF80)
        .expect("address-claimed frame present");
    assert_eq!(claimed.can_id & 0xFF, 0x80);
    assert_eq!(claimed.data.len(), 8); // 8-byte NAME
}

/// Reduced live `vcan0` capture of the real machbus DM1 emission
/// (`socketcan_address_claim` after raising SPN 100 / FMI 1).
const VCAN_DM1_CAPTURE: &str = include_str!("../fixtures/traces/vcan_dm1.candump");

#[test]
fn fixture_vcan_dm1_capture_is_verifiable() {
    use machbus::net::CaptureLog;
    let log = CaptureLog::parse(VCAN_DM1_CAPTURE);
    // Address Claimed context plus the DM1 frame (PGN 0xFECA) from 0x80.
    assert!(log.contains_can_id(0x18EE_FF80), "address-claim context");
    let dm1 = log
        .frames
        .iter()
        .find(|f| f.can_id == 0x18FE_CA80)
        .expect("DM1 frame (PGN 0xFECA) from source 0x80 present");
    // DTC body begins at data[2]; SPN low byte is 100 (0x64), FMI is 1.
    assert_eq!(dm1.data[2], 0x64, "SPN 100 low byte");
    assert_eq!(dm1.data[4] & 0x1F, 0x01, "FMI 1");
}
