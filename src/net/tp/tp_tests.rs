#[cfg(test)]
mod tests {
    use super::super::error::ErrorCode;
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn payload(n: usize) -> Vec<u8> {
        (0..n).map(|i| (i & 0xFF) as u8).collect()
    }

    fn tp_cm_frame(src: Address, dst: Address, data: [u8; 8]) -> Frame {
        Frame::new(
            Identifier::encode(Priority::Lowest, PGN_TP_CM, src, dst),
            data,
            8,
        )
    }

    fn tp_dt_frame(src: Address, dst: Address, data: [u8; 8]) -> Frame {
        Frame::new(
            Identifier::encode(Priority::Lowest, PGN_TP_DT, src, dst),
            data,
            8,
        )
    }

    #[test]
    fn rejects_payload_too_small() {
        let mut tp = TransportProtocol::new();
        assert!(
            tp.send(0xEF00, &payload(8), 0x10, 0x20, 0, Priority::Lowest)
                .is_err()
        );
    }

    #[test]
    fn rejects_payload_too_large() {
        let mut tp = TransportProtocol::new();
        let big = vec![0u8; (TP_MAX_DATA_LENGTH + 1) as usize];
        assert!(
            tp.send(0xEF00, &big, 0x10, 0x20, 0, Priority::Lowest)
                .is_err()
        );
    }

    #[test]
    fn send_rejects_invalid_target_pgn_before_wire_truncation() {
        let mut tp = TransportProtocol::new();
        let err = tp
            .send(0x40000, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));
        assert!(tp.active_sessions().is_empty());
    }

    #[test]
    fn send_rejects_invalid_endpoint_addresses_before_state_mutation() {
        for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut tp = TransportProtocol::new();
            let err = tp
                .send(0xEF00, &payload(20), source, 0x20, 0, Priority::Lowest)
                .unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidAddress);
            assert!(tp.active_sessions().is_empty());
            assert!(tp.stats().is_empty());
        }

        let mut tp = TransportProtocol::new();
        let err = tp
            .send(
                0xEF00,
                &payload(20),
                0x10,
                NULL_ADDRESS,
                0,
                Priority::Lowest,
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidAddress);
        assert!(tp.active_sessions().is_empty());
        assert!(tp.stats().is_empty());
    }

    #[test]
    fn payload_boundaries_emit_valid_tp_rts_shapes() {
        for (len, packets) in [(9usize, 2u8), (10, 2), (1784, 255), (1785, 255)] {
            let mut tp = TransportProtocol::new();
            let frames = tp
                .send(0xEF00, &payload(len), 0x10, 0x20, 0, Priority::Lowest)
                .expect("TP boundary payload must be accepted");
            let total_bytes = len as u32;

            assert_eq!(frames.len(), 1);
            assert_eq!(frames[0].pgn(), PGN_TP_CM);
            assert_eq!(frames[0].data[0], tp_cm::RTS);
            assert_eq!(frames[0].data[1], (total_bytes & 0xFF) as u8);
            assert_eq!(frames[0].data[2], ((total_bytes >> 8) & 0xFF) as u8);
            assert_eq!(frames[0].data[3], packets);
            assert_eq!(frames[0].data[4], TP_MAX_PACKETS_PER_CTS as u8);
            assert_eq!(&frames[0].data[5..8], &[0x00, 0xEF, 0x00]);
        }

        let mut tp = TransportProtocol::new();
        assert!(
            tp.send(0xEF00, &payload(1786), 0x10, 0x20, 0, Priority::Lowest)
                .is_err()
        );
    }

    #[test]
    fn send_respects_session_cap() {
        let mut tp = TransportProtocol::with_max_sessions(0);
        let err = tp
            .send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NoResources);
        assert!(tp.active_sessions().is_empty());
        assert_eq!(tp.stats().resource_rejections, 1);
    }

    #[test]
    fn receive_rts_respects_session_cap() {
        let mut tp = TransportProtocol::with_max_sessions(1);
        let first = tp_cm_frame(0x10, 0x20, [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00]);
        let second = tp_cm_frame(0x11, 0x20, [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00]);

        assert_eq!(tp.process_frame(&first, 0).len(), 1);
        assert_eq!(tp.active_sessions().len(), 1);

        let responses = tp.process_frame(&second, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], tp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::ResourcesUnavailable.as_u8()
        );
        assert_eq!(tp.active_sessions().len(), 1);
        assert_eq!(tp.stats().dropped_frames, 1);
        assert_eq!(tp.stats().resource_rejections, 1);
        assert_eq!(tp.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_cm_rejects_invalid_target_pgn_high_bits() {
        let mut tp = TransportProtocol::new();
        let invalid = tp_cm_frame(0x10, 0x20, [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0x00, 0x04]);

        assert!(tp.process_frame(&invalid, 0).is_empty());
        assert!(tp.active_sessions().is_empty());
        assert_eq!(tp.stats().dropped_frames, 1);
    }

    #[test]
    fn receive_rejects_invalid_endpoint_addresses_before_session_mutation() {
        for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut tp = TransportProtocol::new();
            let invalid = tp_cm_frame(source, 0x20, [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00]);

            assert!(tp.process_frame(&invalid, 0).is_empty());
            assert!(tp.active_sessions().is_empty());
            assert_eq!(tp.stats().dropped_frames, 1);
        }

        let mut tp = TransportProtocol::new();
        let null_dest = tp_cm_frame(
            0x10,
            NULL_ADDRESS,
            [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
        );
        assert!(tp.process_frame(&null_dest, 0).is_empty());
        assert!(tp.active_sessions().is_empty());
        assert_eq!(tp.stats().dropped_frames, 1);

        let mut tp = TransportProtocol::new();
        let rts_to_broadcast = tp_cm_frame(
            0x10,
            BROADCAST_ADDRESS,
            [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00],
        );
        assert!(tp.process_frame(&rts_to_broadcast, 0).is_empty());
        assert!(tp.active_sessions().is_empty());
        assert_eq!(tp.stats().dropped_frames, 1);
    }

    #[test]
    fn receive_bam_respects_session_cap() {
        let mut tp = TransportProtocol::with_max_sessions(1);
        let first = tp_cm_frame(
            0x10,
            BROADCAST_ADDRESS,
            [tp_cm::BAM, 20, 0, 3, 0xFF, 0x00, 0xEF, 0x00],
        );
        let second = tp_cm_frame(
            0x11,
            BROADCAST_ADDRESS,
            [tp_cm::BAM, 20, 0, 3, 0xFF, 0x00, 0xEF, 0x00],
        );

        assert!(tp.process_frame(&first, 0).is_empty());
        assert_eq!(tp.active_sessions().len(), 1);
        assert!(tp.process_frame(&second, 0).is_empty());
        assert_eq!(tp.active_sessions().len(), 1);
        assert_eq!(tp.stats().dropped_frames, 1);
        assert_eq!(tp.stats().resource_rejections, 1);
    }

    #[test]
    fn receive_rts_respects_allocation_cap() {
        let mut tp = TransportProtocol::with_max_receive_bytes(19);
        let rts = tp_cm_frame(0x10, 0x20, [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00]);

        let responses = tp.process_frame(&rts, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], tp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::ResourcesUnavailable.as_u8()
        );
        assert!(tp.active_sessions().is_empty());
        assert_eq!(tp.stats().dropped_frames, 1);
        assert_eq!(tp.stats().resource_rejections, 1);
        assert_eq!(tp.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_rts_rejects_malformed_payload_shape() {
        let mut tp = TransportProtocol::new();
        let too_large = tp_cm_frame(
            0x10,
            0x20,
            [tp_cm::RTS, 0xFA, 0x06, 255, 16, 0x00, 0xEF, 0x00],
        );

        let responses = tp.process_frame(&too_large, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], tp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::UnexpectedDataSize.as_u8()
        );
        assert!(tp.active_sessions().is_empty());
        assert_eq!(tp.stats().dropped_frames, 1);
        assert_eq!(tp.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_bam_drops_malformed_payload_shape() {
        let mut tp = TransportProtocol::new();
        let malformed_bam = tp_cm_frame(
            0x10,
            BROADCAST_ADDRESS,
            [tp_cm::BAM, 20, 0, 2, 0xFF, 0x00, 0xEF, 0x00],
        );

        assert!(tp.process_frame(&malformed_bam, 0).is_empty());
        assert!(tp.active_sessions().is_empty());
        assert_eq!(tp.stats().dropped_frames, 1);
    }

    #[test]
    fn zero_sequence_dt_aborts_without_underflow() {
        let mut rx = TransportProtocol::new();
        let rts = tp_cm_frame(0x10, 0x20, [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00]);
        assert_eq!(rx.process_frame(&rts, 0).len(), 1);

        let bad_dt = tp_dt_frame(0x10, 0x20, [0, 1, 2, 3, 4, 5, 6, 7]);
        let responses = rx.process_frame(&bad_dt, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], tp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::BadSequence.as_u8()
        );
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn orphan_dt_is_counted_as_drop() {
        let mut rx = TransportProtocol::new();
        let dt = tp_dt_frame(0x10, 0x20, [1, 0, 1, 2, 3, 4, 5, 6]);

        assert!(rx.process_frame(&dt, 0).is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    #[test]
    fn short_tp_frame_is_counted_as_drop() {
        let mut rx = TransportProtocol::new();
        let id = Identifier::encode(Priority::Lowest, PGN_TP_CM, 0x10, 0x20);
        let short = Frame::new(id, [tp_cm::RTS, 20, 0, 3, 16, 0x00, 0xEF, 0x00], 7);

        assert!(rx.process_frame(&short, 0).is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    #[test]
    fn bam_send_emits_bam_frame() {
        let mut tp = TransportProtocol::new();
        let frames = tp
            .send(
                0xFE00,
                &payload(20),
                0x10,
                BROADCAST_ADDRESS,
                0,
                Priority::Lowest,
            )
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].pgn(), PGN_TP_CM);
        assert_eq!(frames[0].data[0], tp_cm::BAM);
        assert_eq!(frames[0].data[1], 20); // size LSB
        assert_eq!(frames[0].data[3], 3); // ceil(20/7) = 3
    }

    #[test]
    fn rts_send_emits_rts_frame() {
        let mut tp = TransportProtocol::new();
        let frames = tp
            .send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data[0], tp_cm::RTS);
        assert_eq!(frames[0].data[1], 20); // size LSB
        assert_eq!(frames[0].data[3], 3); // total packets
    }

    /// End-to-end RTS / CTS / DT×N / EoMA round-trip across two
    /// `TransportProtocol` instances.
    #[test]
    fn cmdt_round_trip_completes() {
        let payload = payload(20);
        let mut tx = TransportProtocol::new();
        let mut rx = TransportProtocol::new();

        let received = Rc::new(RefCell::new(None::<TransportSession>));
        let r = received.clone();
        rx.on_complete
            .subscribe(move |s| *r.borrow_mut() = Some(s.clone()));

        // 1) sender → RTS
        let frames = tx
            .send(0xEF00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();
        let rts = frames.into_iter().next().unwrap();

        // 2) receiver handles RTS, replies with CTS
        let cts_resp = rx.process_frame(&rts, 0);
        assert_eq!(cts_resp.len(), 1);
        let cts = cts_resp.into_iter().next().unwrap();

        // 3) sender handles CTS — moves to SendingData
        let _ = tx.process_frame(&cts, 0);
        let dt_frames = tx.get_pending_data_frames();
        // 20 bytes at 7B/frame = 3 packets.
        assert_eq!(dt_frames.len(), 3);

        // 4) receiver ingests each DT; final one returns EoMA
        let mut eoma_seen = None;
        for dt in dt_frames {
            for resp in rx.process_frame(&dt, 0) {
                if resp.data[0] == tp_cm::EOMA {
                    eoma_seen = Some(resp);
                }
            }
        }
        assert!(eoma_seen.is_some(), "EoMA must be emitted on completion");

        // 5) sender ingests EoMA → on_complete fires
        let final_complete = Rc::new(RefCell::new(false));
        let f = final_complete.clone();
        tx.on_complete.subscribe(move |_| *f.borrow_mut() = true);
        let _ = tx.process_frame(&eoma_seen.unwrap(), 0);
        assert!(*final_complete.borrow());

        let got = received.borrow().clone().expect("rx received the message");
        assert_eq!(got.data, payload);
        assert_eq!(got.pgn, 0xEF00);
        assert_eq!(got.source_address, 0x10);
    }

    #[test]
    fn duplicate_dt_aborts_with_duplicate_sequence() {
        let mut rx = TransportProtocol::new();
        let mut tx = TransportProtocol::new();
        // Drive RTS → CTS → first DT
        let rts = tx
            .send(0xEF11, &payload(50), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        let cts = rx.process_frame(&rts, 0).into_iter().next().unwrap();
        let _ = tx.process_frame(&cts, 0);
        let dt_frames = tx.get_pending_data_frames();

        // Send first DT, then re-send it (duplicate).
        let _ = rx.process_frame(&dt_frames[0], 0);
        let resp = rx.process_frame(&dt_frames[0], 0);
        assert_eq!(resp.len(), 1);
        assert_eq!(resp[0].data[0], tp_cm::ABORT);
        assert_eq!(
            resp[0].data[1],
            TransportAbortReason::DuplicateSequence.as_u8()
        );
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn out_of_order_dt_aborts_with_bad_sequence() {
        let mut rx = TransportProtocol::new();
        let mut tx = TransportProtocol::new();
        let rts = tx
            .send(0xEF12, &payload(50), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        let cts = rx.process_frame(&rts, 0).into_iter().next().unwrap();
        let _ = tx.process_frame(&cts, 0);
        let dt_frames = tx.get_pending_data_frames();

        // Skip dt[0]; receiver gets dt[1] first → bad sequence.
        let resp = rx.process_frame(&dt_frames[1], 0);
        assert_eq!(resp.len(), 1);
        assert_eq!(resp[0].data[0], tp_cm::ABORT);
        assert_eq!(resp[0].data[1], TransportAbortReason::BadSequence.as_u8());
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn bam_completion_through_update_loop() {
        let payload = payload(20); // 3 packets
        let mut tx = TransportProtocol::new();
        let bam = tx
            .send(
                0xFE00,
                &payload,
                0x10,
                BROADCAST_ADDRESS,
                0,
                Priority::Lowest,
            )
            .unwrap()[0];

        let mut rx = TransportProtocol::new();
        let received = Rc::new(RefCell::new(None::<TransportSession>));
        let r = received.clone();
        rx.on_complete
            .subscribe(move |s| *r.borrow_mut() = Some(s.clone()));

        // RX: ingest BAM CM.
        let _ = rx.process_frame(&bam, 0);

        // TX: drive update() to emit each BAM data frame.
        let mut all_dts = Vec::new();
        for _ in 0..5 {
            all_dts.extend(tx.update(TP_BAM_INTER_PACKET_MS));
            if all_dts.len() >= 3 {
                break;
            }
        }
        assert!(all_dts.len() >= 3);

        // RX: ingest the data.
        for dt in &all_dts {
            let _ = rx.process_frame(dt, 0);
        }
        let got = received
            .borrow()
            .clone()
            .expect("BAM completed at receiver");
        assert_eq!(got.data, payload);
    }

    #[test]
    fn cts_zero_holds_session() {
        let mut tx = TransportProtocol::new();
        let _ = tx
            .send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();
        // Build a CTS hold (num_packets=0) frame and feed it to tx.
        let id = Identifier::encode(Priority::Lowest, PGN_TP_CM, 0x20, 0x10);
        let mut data = [0xFFu8; 8];
        data[0] = tp_cm::CTS;
        data[1] = 0; // hold
        data[2] = 0;
        data[5] = 0x00;
        data[6] = 0xEF;
        data[7] = 0x00;
        let cts_hold = Frame::new(id, data, 8);

        let _ = tx.process_frame(&cts_hold, 0);
        // Session must still be present and in WaitingForCTS.
        assert_eq!(tx.active_sessions().len(), 1);
        assert_eq!(tx.active_sessions()[0].state, SessionState::WaitingForCTS);
    }

    #[test]
    fn cts_while_sending_aborts_connection_mode_error() {
        let mut tx = TransportProtocol::new();
        let _ = tx
            .send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();

        let id = Identifier::encode(Priority::Lowest, PGN_TP_CM, 0x20, 0x10);
        let mut data = [0xFFu8; 8];
        data[0] = tp_cm::CTS;
        data[1] = 3;
        data[2] = 1;
        data[5] = 0x00;
        data[6] = 0xEF;
        data[7] = 0x00;
        let cts = Frame::new(id, data, 8);

        assert!(tx.process_frame(&cts, 0).is_empty());
        assert_eq!(tx.active_sessions()[0].state, SessionState::SendingData);
        assert!(tx.process_frame(&cts, 0).is_empty());
        assert_eq!(tx.active_sessions()[0].state, SessionState::SendingData);

        let mut next_window = cts;
        next_window.data[2] = 2;
        let abort = tx.process_frame(&next_window, 0);
        assert_eq!(abort.len(), 1);
        assert_eq!(abort[0].data[0], tp_cm::ABORT);
        assert_eq!(
            abort[0].data[1],
            TransportAbortReason::ConnectionModeError.as_u8()
        );
        assert!(tx.active_sessions().is_empty());
        assert_eq!(tx.stats().aborts_sent, 1);
        assert_eq!(tx.stats().dropped_sessions, 1);
    }

    #[test]
    fn duplicate_cts_retransmit_cap_aborts() {
        let mut tx = TransportProtocol::with_max_retransmits(1);
        assert_eq!(tx.max_retransmits(), 1);
        let _ = tx
            .send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();

        let id = Identifier::encode(Priority::Lowest, PGN_TP_CM, 0x20, 0x10);
        let mut data = [0xFFu8; 8];
        data[0] = tp_cm::CTS;
        data[1] = 3;
        data[2] = 1;
        data[5] = 0x00;
        data[6] = 0xEF;
        data[7] = 0x00;
        let cts = Frame::new(id, data, 8);

        assert!(tx.process_frame(&cts, 0).is_empty());
        assert_eq!(tx.get_pending_data_frames().len(), 3);

        assert!(tx.process_frame(&cts, 0).is_empty());
        assert_eq!(tx.active_sessions()[0].retransmit_count, 1);
        assert_eq!(tx.get_pending_data_frames().len(), 3);

        let abort = tx.process_frame(&cts, 0);
        assert_eq!(abort.len(), 1);
        assert_eq!(abort[0].data[0], tp_cm::ABORT);
        assert_eq!(
            abort[0].data[1],
            TransportAbortReason::MaxRetransmitsExceeded.as_u8()
        );
        assert!(tx.active_sessions().is_empty());
        assert_eq!(tx.stats().aborts_sent, 1);
        assert_eq!(tx.stats().dropped_sessions, 1);
    }

    #[test]
    fn cmdt_timeout_aborts_with_timeout_frame() {
        let mut tx = TransportProtocol::new();
        let aborts = Rc::new(RefCell::new(0u32));
        let a = aborts.clone();
        tx.on_abort.subscribe(move |_| *a.borrow_mut() += 1);

        let _ = tx
            .send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();
        // Don't reply with CTS — wait T3.
        let frames = tx.update(TP_TIMEOUT_T3_MS + 1);
        // One abort frame to the receiver.
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data[0], tp_cm::ABORT);
        assert_eq!(frames[0].data[1], TransportAbortReason::Timeout.as_u8());
        assert_eq!(*aborts.borrow(), 1);
        assert_eq!(tx.stats().timeouts, 1);
        assert_eq!(tx.stats().dropped_sessions, 1);
        assert_eq!(tx.stats().aborts_sent, 1);
        tx.clear_stats();
        assert!(tx.stats().is_empty());
    }

    #[test]
    fn auxiliary_timer_timeout_updates_stats() {
        let mut tp = TransportProtocol::new();
        tp.track_session(0x10, 0x20, 0xEF00, TpSessionState::WaitForCts, 0);

        let frames = tp.update_sessions(TP_TIMEOUT_T3_MS + 1);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data[0], tp_cm::ABORT);
        assert_eq!(tp.stats().timeouts, 1);
        assert_eq!(tp.stats().dropped_sessions, 1);
        assert_eq!(tp.stats().aborts_sent, 1);
    }

    #[test]
    fn duplicate_session_send_returns_error() {
        let mut tx = TransportProtocol::new();
        tx.send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();
        assert!(
            tx.send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
                .is_err()
        );
    }

    #[test]
    fn abort_received_drops_matching_session() {
        let mut rx = TransportProtocol::new();
        // Receive an RTS first.
        let mut tx = TransportProtocol::new();
        let rts = tx
            .send(0xEF00, &payload(20), 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        let _ = rx.process_frame(&rts, 0);
        assert_eq!(rx.active_sessions().len(), 1);

        // Sender aborts.
        let id = Identifier::encode(Priority::Lowest, PGN_TP_CM, 0x10, 0x20);
        let mut data = [0xFFu8; 8];
        data[0] = tp_cm::ABORT;
        data[1] = TransportAbortReason::ResourcesUnavailable.as_u8();
        data[5] = 0x00;
        data[6] = 0xEF;
        data[7] = 0x00;
        let abort = Frame::new(id, data, 8);

        let _ = rx.process_frame(&abort, 0);
        assert_eq!(rx.active_sessions().len(), 0);
        assert_eq!(rx.stats().aborts_received, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
    }
}
