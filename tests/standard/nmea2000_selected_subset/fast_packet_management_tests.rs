#[test]
fn nmea2000_fast_packet_rejects_non_classic_frame_lengths_before_session_mutation() {
    let pgn = PGN_PRODUCT_INFO;
    let source = 0x23;
    let id = Identifier::encode(Priority::Default, pgn, source, BROADCAST_ADDRESS);

    for length in [0, 7, 9, 15] {
        let mut first = Frame::new(
            id,
            [0x00, 20, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF],
            length.min(8),
        );
        first.length = length;
        let mut continuation = Frame::new(
            id,
            [0x01, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77],
            length.min(8),
        );
        continuation.length = length;
        let mut rx = FastPacketProtocol::new();

        assert!(
            rx.process_frame(&first).is_none(),
            "non-classic Fast Packet first frames must not allocate sessions"
        );
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 1);

        assert!(
            rx.process_frame(&continuation).is_none(),
            "non-classic Fast Packet continuation frames must not mutate state"
        );
        assert_eq!(rx.rx_session_count(), 0);
        assert_eq!(rx.stats().dropped_frames, 2);
    }
}

#[test]
fn nmea2000_fast_packet_timeout_boundary_drops_incomplete_receive_sessions() {
    let pgn = PGN_PRODUCT_INFO;
    let source = 0x23;
    let payload: Vec<u8> = (0..20).collect();
    let mut tx = FastPacketProtocol::new();
    let frames = tx.send(pgn, &payload, source).unwrap();

    let mut rx = FastPacketProtocol::new();
    assert!(rx.process_frame(&frames[0]).is_none());
    assert_eq!(rx.rx_session_count(), 1);

    rx.update(TP_TIMEOUT_T1_MS - 1);
    assert_eq!(
        rx.rx_session_count(),
        1,
        "timeout must not fire one millisecond early"
    );

    rx.update(1);
    assert_eq!(rx.rx_session_count(), 0);
    assert_eq!(rx.stats().timeouts, 1);
    assert_eq!(rx.stats().dropped_sessions, 1);

    assert!(
        rx.process_frame(&frames[1]).is_none(),
        "continuation after timeout must not resurrect the dropped session"
    );
    assert_eq!(rx.stats().dropped_frames, 1);
}

#[test]
fn nmea2000_management_rejects_invalid_peer_addresses_before_events_or_pending_state() {
    for address in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let mut manager = N2KManagement::default();
        assert_eq!(
            manager.request_product_info(address).unwrap_err().code,
            ErrorCode::InvalidAddress
        );
        assert_eq!(
            manager.request_config_info(address).unwrap_err().code,
            ErrorCode::InvalidAddress
        );
        assert_eq!(
            manager.send_config_info(address).unwrap_err().code,
            ErrorCode::InvalidAddress
        );
        assert!(manager.pending_requests().is_empty());

        let heartbeats: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
        let heartbeat_log = heartbeats.clone();
        manager
            .on_heartbeat_received
            .subscribe(move |(_, source)| heartbeat_log.borrow_mut().push(*source));

        let heartbeat = N2KHeartbeat::default().encode().unwrap().to_vec();
        manager.handle_message(&Message::new(PGN_HEARTBEAT_N2K, heartbeat, address));
        assert!(heartbeats.borrow().is_empty());
    }
}

#[test]
fn nmea2000_management_rejects_invalid_pdu2_destinations_before_events_or_pending_state() {
    let mut manager = N2KManagement::default();
    manager.request_product_info(0x42).unwrap();

    let products: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let configs: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let heartbeats: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let product_log = products.clone();
    let config_log = configs.clone();
    let heartbeat_log = heartbeats.clone();
    manager
        .on_product_info_received
        .subscribe(move |(_, source)| product_log.borrow_mut().push(*source));
    manager
        .on_config_info_received
        .subscribe(move |(_, source)| config_log.borrow_mut().push(*source));
    manager
        .on_heartbeat_received
        .subscribe(move |(_, source)| heartbeat_log.borrow_mut().push(*source));

    for (pgn, data) in [
        (
            PGN_PRODUCT_INFO,
            N2KProductInfo::default().encode().unwrap(),
        ),
        (PGN_CONFIG_INFO, N2KConfigInfo::default().encode().unwrap()),
        (
            PGN_HEARTBEAT_N2K,
            N2KHeartbeat::default().encode().unwrap().to_vec(),
        ),
    ] {
        manager.handle_message(&Message::with_addressing(
            pgn,
            data,
            0x42,
            0x33,
            Priority::Default,
        ));
        manager.handle_message(&Message::with_addressing(
            pgn,
            vec![0xFF; 8],
            0x42,
            NULL_ADDRESS,
            Priority::Default,
        ));
    }

    assert!(products.borrow().is_empty());
    assert!(configs.borrow().is_empty());
    assert!(heartbeats.borrow().is_empty());
    assert!(
        manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x42),
        "invalid destination metadata must not clear pending management requests"
    );

    manager.handle_message(&Message::new(
        PGN_PRODUCT_INFO,
        N2KProductInfo::default().encode().unwrap(),
        0x42,
    ));
    assert_eq!(products.borrow().as_slice(), &[0x42]);
    assert!(!manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x42));
}

#[test]
fn nmea2000_management_malformed_responses_do_not_clear_pending_requests() {
    let mut manager = N2KManagement::default();
    let products: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let product_log = products.clone();
    manager
        .on_product_info_received
        .subscribe(move |(_, source)| product_log.borrow_mut().push(*source));

    manager.request_product_info(0x42).unwrap();
    manager.handle_message(&Message::new(PGN_PRODUCT_INFO, vec![0x00; 133], 0x42));
    assert!(manager.has_pending_request_to(0x42));
    assert!(products.borrow().is_empty());

    manager.handle_message(&Message::new(
        PGN_PRODUCT_INFO,
        N2KProductInfo::default().encode().unwrap(),
        0x42,
    ));
    assert!(!manager.has_pending_request_to(0x42));
    assert_eq!(products.borrow().as_slice(), &[0x42]);

    let configs: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let config_log = configs.clone();
    manager
        .on_config_info_received
        .subscribe(move |(_, source)| config_log.borrow_mut().push(*source));

    manager.request_config_info(0x43).unwrap();
    manager.handle_message(&Message::new(PGN_CONFIG_INFO, vec![0x00, 0x47], 0x43));
    assert!(manager.has_pending_request_to(0x43));
    assert!(configs.borrow().is_empty());

    manager.handle_message(&Message::new(
        PGN_CONFIG_INFO,
        N2KConfigInfo::default().encode().unwrap(),
        0x43,
    ));
    assert!(!manager.has_pending_request_to(0x43));
    assert_eq!(configs.borrow().as_slice(), &[0x43]);
}

#[test]
fn nmea2000_management_tracks_distinct_requested_pgns_for_same_peer() {
    let mut manager = N2KManagement::default();

    let product_request = manager.request_product_info(0x44).unwrap();
    let config_request = manager.request_config_info(0x44).unwrap();
    assert_eq!(product_request.dest, Some(0x44));
    assert_eq!(config_request.dest, Some(0x44));
    assert!(manager.has_pending_request_to(0x44));
    assert!(manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x44));
    assert!(manager.has_pending_request_for(PGN_CONFIG_INFO, 0x44));
    assert_eq!(manager.pending_requests().len(), 2);

    manager.handle_message(&Message::new(
        PGN_PRODUCT_INFO,
        N2KProductInfo::default().encode().unwrap(),
        0x44,
    ));
    assert!(!manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x44));
    assert!(manager.has_pending_request_for(PGN_CONFIG_INFO, 0x44));
    assert!(manager.has_pending_request_to(0x44));

    manager.handle_message(&Message::new(
        PGN_CONFIG_INFO,
        N2KConfigInfo::default().encode().unwrap(),
        0x44,
    ));
    assert!(!manager.has_pending_request_to(0x44));
}

#[test]
fn nmea0183_serial_gnss_rejects_noncanonical_units_and_nonfinite_numbers_before_state_mutation() {
    let mut parser = SerialGNSS::new();
    let positions: Rc<RefCell<Vec<GNSSPosition>>> = Rc::new(RefCell::new(Vec::new()));
    let cogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let sogs: Rc<RefCell<Vec<f64>>> = Rc::new(RefCell::new(Vec::new()));
    let positions_log = positions.clone();
    let cogs_log = cogs.clone();
    let sogs_log = sogs.clone();
    parser
        .on_position
        .subscribe(move |position| positions_log.borrow_mut().push(*position));
    parser
        .on_cog
        .subscribe(move |cog| cogs_log.borrow_mut().push(*cog));
    parser
        .on_sog
        .subscribe(move |sog| sogs_log.borrow_mut().push(*sog));

    parser.feed_bytes(
        nmea0183_with_checksum("$GPGGA,123519,4807.038,N,01131.000,E,1,08,0.9,545.4,M,46.9,M,,")
            .as_bytes(),
    );
    parser.feed_bytes(nmea0183_with_checksum("$GPVTG,054.7,T,034.4,M,005.5,N,010.2,K").as_bytes());
    let baseline = parser
        .latest_position()
        .expect("valid GGA should create the baseline fix");

    for body in [
        "$GPGGA,123520,4807.038,N,01131.000,E,9,08,0.9,545.4,M,46.9,M,,",
        "$GPGGA,123520,4807.038,N,01131.000,E,1,08,0.9,545.4,F,46.9,M,,",
        "$GPGGA,123520,4807.038,N,01131.000,E,1,08,NaN,545.4,M,46.9,M,,",
        "$GPRMC,123520,A,4807.038,N,01131.000,E,inf,084.4,230394,003.1,W",
        "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,310299,003.1,W",
        "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,X",
        "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,,W",
        "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W,N",
        "$GPRMC,123520,A,4807.038,N,01131.000,E,022.4,084.4,230394,003.1,W,X",
        "$GPVTG,055.0,X,034.4,M,005.5,N,010.2,K",
        "$GPVTG,055.0,T,034.4,M,005.5,X,010.2,K",
        "$GPVTG,055.0,T,034.4,M,005.5,N,010.2,X",
        "$GPVTG,055.0,T,034.4,M,005.5,N,010.2,K,N",
        "$GPVTG,055.0,T,034.4,M,005.5,N,010.2,K,X",
        "$GPGSA,X,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1",
        "$GPGSA,A,3,04,05,,09,12,,,24,,,,,NaN,1.3,2.1",
        "$GPGSA,A,3,00,05,,09,12,,,24,,,,,2.5,1.3,2.1",
        "$GPGSA,A,3,256,05,,09,12,,,24,,,,,2.5,1.3,2.1",
        "$GPGSA,A,3,04,05,,09,12,,,24,,,,,2.5,1.3,2.1,999",
        "$GPGLL,4916.45,N,12311.12,W,246060,A,A",
        "$GPGLL,4916.45,N,12311.12,W,225444,A,N",
        "$GPGLL,4916.45,N,12311.12,W,225444,A,X",
        "$GPGSV,3,1,11,07,91,048,42",
        "$GPGSV,3,1,11,07,79,360,42",
        "$GPGSV,3,1,11,07,79,048,100",
        "$GPGSV,3,1,11,07,79,048",
        "$GPGSV,3,1,11,07,79,048,42,999",
    ] {
        parser.feed_bytes(nmea0183_with_checksum(body).as_bytes());
    }

    assert_eq!(positions.borrow().len(), 1);
    assert_eq!(cogs.borrow().len(), 1);
    assert_eq!(sogs.borrow().len(), 1);
    assert_eq!(parser.latest_satellites_in_view(), None);
    assert_eq!(
        parser.latest_position(),
        Some(baseline),
        "bad NMEA 0183 field units and non-finite numeric tokens must not overwrite the last good GNSS fix"
    );
}

#[test]
fn nmea2000_management_request_timeouts_remove_only_expired_pending_entries() {
    let mut manager = N2KManagement::default();
    let timeouts: Rc<RefCell<Vec<(u32, u8)>>> = Rc::new(RefCell::new(Vec::new()));
    let timeout_log = timeouts.clone();
    manager
        .on_request_timeout
        .subscribe(move |event| timeout_log.borrow_mut().push(*event));

    let first_product = manager.request_product_info(0x45).unwrap();
    assert_eq!(first_product.dest, Some(0x45));
    assert_eq!(first_product.data, [0x14, 0xF0, 0x01]);
    assert_eq!(
        manager.request_product_info(0x45).unwrap_err().code,
        ErrorCode::InvalidState,
        "duplicate outstanding requests for the same PGN and peer must be rejected"
    );

    manager.update(N2K_REQUEST_TIMEOUT_MS - 1).unwrap();
    assert!(manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x45));
    assert!(timeouts.borrow().is_empty());

    let config = manager.request_config_info(0x45).unwrap();
    assert_eq!(config.dest, Some(0x45));
    assert_eq!(config.data, [0x16, 0xF0, 0x01]);
    assert_eq!(manager.pending_requests().len(), 2);

    manager.update(1).unwrap();
    assert!(!manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x45));
    assert!(manager.has_pending_request_for(PGN_CONFIG_INFO, 0x45));
    assert_eq!(timeouts.borrow().as_slice(), &[(PGN_PRODUCT_INFO, 0x45)]);

    let retry_product = manager.request_product_info(0x45).unwrap();
    assert_eq!(retry_product.dest, Some(0x45));
    assert!(manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x45));

    manager.handle_message(&Message::new(
        PGN_CONFIG_INFO,
        N2KConfigInfo::default().encode().unwrap(),
        0x45,
    ));
    assert!(!manager.has_pending_request_for(PGN_CONFIG_INFO, 0x45));
    assert!(manager.has_pending_request_for(PGN_PRODUCT_INFO, 0x45));

    manager.update(N2K_REQUEST_TIMEOUT_MS).unwrap();
    assert!(!manager.has_pending_request_to(0x45));
    assert_eq!(
        timeouts.borrow().as_slice(),
        &[(PGN_PRODUCT_INFO, 0x45), (PGN_PRODUCT_INFO, 0x45)]
    );
}
