#[cfg(test)]
mod tests {
    use super::super::error::ErrorCode;
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn etp_cm_frame(src: Address, dst: Address, data: [u8; 8]) -> Frame {
        Frame::new(
            Identifier::encode(Priority::Lowest, PGN_ETP_CM, src, dst),
            data,
            8,
        )
    }

    fn etp_dt_frame(src: Address, dst: Address, data: [u8; 8]) -> Frame {
        Frame::new(
            Identifier::encode(Priority::Lowest, PGN_ETP_DT, src, dst),
            data,
            8,
        )
    }

    fn rts_data(total_bytes: u32) -> [u8; 8] {
        [
            etp_cm::RTS,
            (total_bytes & 0xFF) as u8,
            ((total_bytes >> 8) & 0xFF) as u8,
            ((total_bytes >> 16) & 0xFF) as u8,
            ((total_bytes >> 24) & 0xFF) as u8,
            0x00,
            0xCA,
            0x00,
        ]
    }

    #[test]
    fn rejects_payload_too_small() {
        let mut etp = ExtendedTransportProtocol::new();
        let small = vec![0u8; 1000];
        assert!(
            etp.send(0xCA00, &small, 0x10, 0x20, 0, Priority::Lowest)
                .is_err()
        );
    }

    #[test]
    fn rejects_broadcast() {
        let mut etp = ExtendedTransportProtocol::new();
        let big = vec![0u8; 2000];
        assert!(
            etp.send(0xCA00, &big, 0x10, BROADCAST_ADDRESS, 0, Priority::Lowest)
                .is_err()
        );
    }

    #[test]
    fn payload_boundaries_emit_valid_etp_rts_shape() {
        let mut etp = ExtendedTransportProtocol::new();
        assert!(
            etp.send(
                0xCA00,
                &vec![0u8; TP_MAX_DATA_LENGTH as usize],
                0x10,
                0x20,
                0,
                Priority::Lowest,
            )
            .is_err()
        );

        let min_etp = TP_MAX_DATA_LENGTH + 1;
        let frames = etp
            .send(
                0xCA00,
                &vec![0u8; min_etp as usize],
                0x10,
                0x20,
                0,
                Priority::Lowest,
            )
            .expect("ETP minimum payload must be accepted");
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].pgn(), PGN_ETP_CM);
        assert_eq!(frames[0].data, rts_data(min_etp));
    }

    #[test]
    fn receive_rts_rejects_payload_over_protocol_max_without_allocation() {
        let mut rx = ExtendedTransportProtocol::new();
        let rts = etp_cm_frame(0x10, 0x20, rts_data(ETP_MAX_DATA_LENGTH + 1));

        let responses = rx.process_frame(&rts, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::UnexpectedDataSize.as_u8()
        );
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_protocol_max_rts_is_rejected_by_lower_allocation_cap() {
        let mut rx = ExtendedTransportProtocol::with_max_receive_bytes(TP_MAX_DATA_LENGTH + 1);
        let rts = etp_cm_frame(0x10, 0x20, rts_data(ETP_MAX_DATA_LENGTH));

        let responses = rx.process_frame(&rts, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::ResourcesUnavailable.as_u8()
        );
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().resource_rejections, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_profile_validates_protocol_max_without_allocating() {
        let rx = ExtendedTransportProtocol::with_max_receive_bytes(ETP_MAX_DATA_LENGTH);
        let profile = rx
            .receive_profile_for_advertised_size(ETP_MAX_DATA_LENGTH)
            .expect("protocol maximum should be admissible under an explicit max profile");

        assert_eq!(profile.total_bytes, ETP_MAX_DATA_LENGTH);
        assert_eq!(profile.total_packets, 0xFF_FFFF);
        assert_eq!(profile.max_receive_bytes, ETP_MAX_DATA_LENGTH);

        let capped = ExtendedTransportProtocol::with_max_receive_bytes(TP_MAX_DATA_LENGTH + 1);
        assert_eq!(
            capped.receive_profile_for_advertised_size(ETP_MAX_DATA_LENGTH),
            Err(TransportAbortReason::ResourcesUnavailable)
        );
        assert_eq!(
            rx.receive_profile_for_advertised_size(TP_MAX_DATA_LENGTH),
            Err(TransportAbortReason::UnexpectedDataSize)
        );
        assert_eq!(
            rx.receive_profile_for_advertised_size(ETP_MAX_DATA_LENGTH + 1),
            Err(TransportAbortReason::UnexpectedDataSize)
        );
    }

    #[test]
    fn send_respects_session_cap() {
        let mut etp = ExtendedTransportProtocol::with_max_sessions(0);
        let err = etp
            .send(0xCA00, &vec![0u8; 2000], 0x10, 0x20, 0, Priority::Lowest)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::NoResources);
        assert!(etp.active_sessions().is_empty());
        assert_eq!(etp.stats().resource_rejections, 1);
    }

    #[test]
    fn send_rejects_invalid_target_pgn_before_wire_truncation() {
        let mut etp = ExtendedTransportProtocol::new();
        let err = etp
            .send(0x40000, &vec![0u8; 2000], 0x10, 0x20, 0, Priority::Lowest)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));
        assert!(etp.active_sessions().is_empty());
    }

    #[test]
    fn send_rejects_invalid_endpoint_addresses_before_state_mutation() {
        for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut etp = ExtendedTransportProtocol::new();
            let err = etp
                .send(0xEF00, &vec![0u8; 2000], source, 0x20, 0, Priority::Lowest)
                .unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidAddress);
            assert!(etp.active_sessions().is_empty());
            assert!(etp.stats().is_empty());
        }

        for destination in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut etp = ExtendedTransportProtocol::new();
            let err = etp
                .send(
                    0xEF00,
                    &vec![0u8; 2000],
                    0x10,
                    destination,
                    0,
                    Priority::Lowest,
                )
                .unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidAddress);
            assert!(etp.active_sessions().is_empty());
            assert!(etp.stats().is_empty());
        }
    }

    #[test]
    fn receive_rts_respects_session_cap() {
        let mut rx = ExtendedTransportProtocol::with_max_sessions(1);
        let first = etp_cm_frame(
            0x10,
            0x20,
            [etp_cm::RTS, 0xD0, 0x07, 0, 0, 0x00, 0xCA, 0x00],
        );
        let second = etp_cm_frame(
            0x11,
            0x20,
            [etp_cm::RTS, 0xD0, 0x07, 0, 0, 0x00, 0xCA, 0x00],
        );

        assert_eq!(rx.process_frame(&first, 0).len(), 1);
        assert_eq!(rx.active_sessions().len(), 1);

        let responses = rx.process_frame(&second, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::ResourcesUnavailable.as_u8()
        );
        assert_eq!(rx.active_sessions().len(), 1);
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().resource_rejections, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_cm_rejects_invalid_target_pgn_high_bits() {
        let mut etp = ExtendedTransportProtocol::new();
        let invalid = etp_cm_frame(
            0x10,
            0x20,
            [etp_cm::RTS, 0xD0, 0x07, 0, 0, 0x00, 0x00, 0x04],
        );

        assert!(etp.process_frame(&invalid, 0).is_empty());
        assert!(etp.active_sessions().is_empty());
        assert_eq!(etp.stats().dropped_frames, 1);
    }

    #[test]
    fn receive_rejects_invalid_endpoint_addresses_before_session_mutation() {
        for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut etp = ExtendedTransportProtocol::new();
            let invalid = etp_cm_frame(
                source,
                0x20,
                [etp_cm::RTS, 0xD0, 0x07, 0x00, 0x00, 0x00, 0xEF, 0x00],
            );

            assert!(etp.process_frame(&invalid, 0).is_empty());
            assert!(etp.active_sessions().is_empty());
            assert_eq!(etp.stats().dropped_frames, 1);
        }

        for destination in [NULL_ADDRESS, BROADCAST_ADDRESS] {
            let mut etp = ExtendedTransportProtocol::new();
            let invalid = etp_cm_frame(
                0x10,
                destination,
                [etp_cm::RTS, 0xD0, 0x07, 0x00, 0x00, 0x00, 0xEF, 0x00],
            );

            assert!(etp.process_frame(&invalid, 0).is_empty());
            assert!(etp.active_sessions().is_empty());
            assert_eq!(etp.stats().dropped_frames, 1);
        }
    }

    #[test]
    fn receive_duplicate_rts_aborts_existing_session() {
        let mut rx = ExtendedTransportProtocol::new();
        let rts = etp_cm_frame(
            0x10,
            0x20,
            [etp_cm::RTS, 0xD0, 0x07, 0, 0, 0x00, 0xCA, 0x00],
        );

        assert_eq!(rx.process_frame(&rts, 0).len(), 1);
        let responses = rx.process_frame(&rts, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::AlreadyInSession.as_u8()
        );
        assert_eq!(rx.active_sessions().len(), 1);
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_rts_respects_allocation_cap() {
        let mut rx = ExtendedTransportProtocol::with_max_receive_bytes(1_999);
        let rts = etp_cm_frame(
            0x10,
            0x20,
            [etp_cm::RTS, 0xD0, 0x07, 0, 0, 0x00, 0xCA, 0x00],
        );

        let responses = rx.process_frame(&rts, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::ResourcesUnavailable.as_u8()
        );
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().resource_rejections, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_rts_rejects_tp_sized_payload() {
        let mut rx = ExtendedTransportProtocol::new();
        let rts = etp_cm_frame(0x10, 0x20, [etp_cm::RTS, 100, 0, 0, 0, 0x00, 0xCA, 0x00]);

        let responses = rx.process_frame(&rts, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::UnexpectedDataSize.as_u8()
        );
        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }

    #[test]
    fn receive_dt_before_dpo_aborts() {
        let payload: Vec<u8> = (0..2000u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let mut rx = ExtendedTransportProtocol::new();

        let rts = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        assert_eq!(rx.process_frame(&rts, 0).len(), 1);

        let dt = etp_dt_frame(0x10, 0x20, [1, 0, 1, 2, 3, 4, 5, 6]);
        let responses = rx.process_frame(&dt, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
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
        let mut rx = ExtendedTransportProtocol::new();
        let dt = etp_dt_frame(0x10, 0x20, [1, 0, 1, 2, 3, 4, 5, 6]);

        assert!(rx.process_frame(&dt, 0).is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    #[test]
    fn short_etp_frame_is_counted_as_drop() {
        let mut rx = ExtendedTransportProtocol::new();
        let id = Identifier::encode(Priority::Lowest, PGN_ETP_CM, 0x10, 0x20);
        let short = Frame::new(id, rts_data(TP_MAX_DATA_LENGTH + 1), 7);

        assert!(rx.process_frame(&short, 0).is_empty());
        assert_eq!(rx.stats().dropped_frames, 1);
    }

    #[test]
    fn receive_dpo_offset_mismatch_aborts() {
        let payload: Vec<u8> = (0..2000u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let mut rx = ExtendedTransportProtocol::new();

        let rts = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        assert_eq!(rx.process_frame(&rts, 0).len(), 1);

        let bad_dpo = etp_cm_frame(0x10, 0x20, [etp_cm::DPO, 1, 1, 0, 0, 0x00, 0xCA, 0x00]);
        let responses = rx.process_frame(&bad_dpo, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
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
    fn transmit_invalid_cts_next_packet_aborts() {
        let payload: Vec<u8> = (0..2000u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let _ = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();

        let invalid_cts = etp_cm_frame(0x20, 0x10, [etp_cm::CTS, 1, 0, 0, 0, 0x00, 0xCA, 0x00]);
        let responses = tx.process_frame(&invalid_cts, 0);
        assert_eq!(responses.len(), 1);
        assert_eq!(responses[0].data[0], etp_cm::ABORT);
        assert_eq!(
            responses[0].data[1],
            TransportAbortReason::BadSequence.as_u8()
        );
        assert!(tx.active_sessions().is_empty());
        assert_eq!(tx.stats().dropped_frames, 1);
        assert_eq!(tx.stats().dropped_sessions, 1);
        assert_eq!(tx.stats().aborts_sent, 1);
    }

    /// End-to-end RTS → CTS → DPO+DT × N → … → EOMA. Drives both
    /// sides to completion via an explicit pump.
    #[test]
    fn etp_round_trip_completes() {
        // ETP minimum payload is > 1785 bytes (TP territory below that).
        // 2000 bytes spans ~18 windows of 16 packets.
        let payload: Vec<u8> = (0..2000u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let mut rx = ExtendedTransportProtocol::new();

        let received = Rc::new(RefCell::new(None::<TransportSession>));
        let r = received.clone();
        rx.on_complete
            .subscribe(move |s| *r.borrow_mut() = Some(s.clone()));

        // RTS → CTS.
        let rts = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        let mut to_tx = rx.process_frame(&rts, 0);
        assert_eq!(to_tx.len(), 1);
        assert_eq!(to_tx[0].data[0], etp_cm::CTS);

        // Pump until receiver completes. Each turn:
        //   1. feed `to_tx` (CTS or EOMA) into the sender,
        //   2. drain sender's pending data frames,
        //   3. feed those into the receiver, capture its responses.
        for turn in 0..50 {
            assert!(turn < 49, "round-trip did not converge");

            for f in to_tx.drain(..) {
                let _ = tx.process_frame(&f, 0);
            }
            let dt = tx.get_pending_data_frames();
            if dt.is_empty() {
                break;
            }

            for f in &dt {
                to_tx.extend(rx.process_frame(f, 0));
            }
            if received.borrow().is_some() {
                break;
            }
        }

        let got = received.borrow().clone().expect("rx received the message");
        assert_eq!(got.data, payload);
        assert_eq!(got.pgn, 0xCA00);
        assert_eq!(got.source_address, 0x10);
    }

    #[test]
    fn etp_timeout_aborts() {
        let mut tx = ExtendedTransportProtocol::new();
        let aborts = Rc::new(RefCell::new(0u32));
        let a = aborts.clone();
        tx.on_abort.subscribe(move |_| *a.borrow_mut() += 1);

        let big = vec![0u8; 2000];
        let _ = tx
            .send(0xCA00, &big, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap();
        let frames = tx.update(ETP_TIMEOUT_T1_MS + 1);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data[0], etp_cm::ABORT);
        assert_eq!(*aborts.borrow(), 1);
        assert_eq!(tx.stats().timeouts, 1);
        assert_eq!(tx.stats().dropped_sessions, 1);
        assert_eq!(tx.stats().aborts_sent, 1);
        tx.clear_stats();
        assert!(tx.stats().is_empty());
    }

    #[test]
    fn abort_received_drops_matching_session_and_updates_stats() {
        let payload: Vec<u8> = (0..2000u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let mut rx = ExtendedTransportProtocol::new();

        let rts = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        let _ = rx.process_frame(&rts, 0);
        assert_eq!(rx.active_sessions().len(), 1);

        let abort = etp_cm_frame(
            0x10,
            0x20,
            [
                etp_cm::ABORT,
                TransportAbortReason::ResourcesUnavailable.as_u8(),
                0xFF,
                0xFF,
                0xFF,
                0x00,
                0xCA,
                0x00,
            ],
        );
        let _ = rx.process_frame(&abort, 0);

        assert!(rx.active_sessions().is_empty());
        assert_eq!(rx.stats().aborts_received, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
    }

    #[test]
    fn etp_bad_sequence_aborts() {
        let payload: Vec<u8> = (0..2500u32).map(|n| (n & 0xFF) as u8).collect();
        let mut tx = ExtendedTransportProtocol::new();
        let mut rx = ExtendedTransportProtocol::new();

        let rts = tx
            .send(0xCA00, &payload, 0x10, 0x20, 0, Priority::Lowest)
            .unwrap()[0];
        let cts = rx.process_frame(&rts, 0)[0];
        let _ = tx.process_frame(&cts, 0);
        let pending = tx.get_pending_data_frames();
        // pending = [DPO, DT1, DT2, ..., DT16]; drop DT1, send DT2 first.
        // First feed DPO so rx is ready.
        let _ = rx.process_frame(&pending[0], 0);
        let resp = rx.process_frame(&pending[2], 0); // skip DT1
        assert!(resp.iter().any(|f| f.data[0] == etp_cm::ABORT
            && f.data[1] == TransportAbortReason::BadSequence.as_u8()));
        assert_eq!(rx.stats().dropped_frames, 1);
        assert_eq!(rx.stats().dropped_sessions, 1);
        assert_eq!(rx.stats().aborts_sent, 1);
    }
}
