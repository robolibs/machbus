#[test]
fn file_client_gates_requests_by_advertised_server_capabilities_without_allocating_tans() {
    let properties = FileServerProperties {
        supports_directories: false,
        supports_volume_management: false,
        supports_file_attributes: false,
        supports_move_file: false,
        supports_delete_file: false,
        ..FileServerProperties::default()
    };

    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client_with_properties(&mut client, 0x80, properties);

    for err in [
        client.try_get_current_directory().unwrap_err(),
        client.try_change_directory("logs").unwrap_err(),
        client
            .try_open_file("logs", OpenFlags::OpenDir.bit())
            .unwrap_err(),
        client.try_move_file("old.txt", "new.txt").unwrap_err(),
        client.try_delete_file("old.txt").unwrap_err(),
        client.try_get_file_attributes("old.txt").unwrap_err(),
        client
            .try_set_file_attributes("old.txt", FileAttributes::Hidden.bit())
            .unwrap_err(),
        client.try_initialize_volume("ISOBUS", 0, 0).unwrap_err(),
    ] {
        assert_eq!(
            err.code,
            ErrorCode::InvalidState,
            "unsupported operations must fail as local capability-state errors"
        );
    }

    let regular_open = client
        .try_open_file("plain.txt", OpenFlags::Read.bit())
        .expect("regular file opens remain legal when optional capabilities are disabled");
    assert_eq!(regular_open.data[0], FSFunction::OpenFile.as_u8());
    assert_eq!(
        regular_open.data[1], 1,
        "capability-gated rejected requests must not allocate or consume TANs"
    );
}

#[test]
fn file_server_volume_name_validation_rejects_non_ascii_across_client_and_server_paths() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    for err in [
        client.try_request_volume_status("CAFÉ").unwrap_err(),
        client.try_initialize_volume("CAFÉ", 0, 0).unwrap_err(),
    ] {
        assert_eq!(
            err.code,
            ErrorCode::InvalidData,
            "client volume-name requests must use one byte per protocol character"
        );
    }

    let valid_status = client
        .try_request_volume_status("ISOBUS")
        .expect("ASCII volume names remain encodable");
    assert_eq!(valid_status.data[0], FSFunction::VolumeStatus.as_u8());
    assert_eq!(&valid_status.data[5..], b"ISOBUS");

    let mut server = FileServer::new(FileServerConfig::default());
    assert!(
        server.set_volume_name("CAFÉ").is_err(),
        "server volume label setter must reject non-ASCII names before status emission"
    );
    assert_eq!(server.volume_name(), "ISOBUS");

    let non_ascii_init = server.handle_client_message(&fs_request(
        initialize_volume_request(0x58, 0, 0, "CAFÉ"),
        0x42,
    ));
    assert_response(
        &non_ascii_init[0].data,
        FSFunction::InitializeVolume,
        0x58,
        FSError::MalformedRequest,
    );
    assert_eq!(
        server.volume_name(),
        "ISOBUS",
        "rejected non-ASCII InitializeVolume names must not replace the server volume label"
    );
}

#[test]
fn file_client_transaction_numbers_wrap_without_using_reserved_tan() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let tans: Vec<u8> = (0..260)
        .map(|_| {
            let outbound = client.request_server_status();
            assert_eq!(outbound.pgn, PGN_FILE_CLIENT_TO_SERVER);
            assert_eq!(outbound.dest, Some(0x80));
            assert_eq!(outbound.data[0], FSFunction::FileServerStatus.as_u8());
            outbound.data[1]
        })
        .collect();

    assert!(
        tans.iter().all(|tan| *tan != INVALID_TAN),
        "client-generated transaction numbers must never use the reserved TAN"
    );
    assert_eq!(tans[0], 1);
    assert_eq!(tans[253], 254);
    assert_eq!(
        tans[254], 0,
        "after 0xFE the client must wrap to 0x00 instead of emitting reserved 0xFF"
    );
    assert_eq!(tans[255], 1);
}

#[test]
fn file_client_explicit_status_and_property_requests_reject_disconnected_state_without_tan() {
    let mut client = FileClient::new(FileClientConfig::default());

    for err in [
        client.try_request_server_properties().unwrap_err(),
        client.try_request_server_status().unwrap_err(),
    ] {
        assert!(
            matches!(err.code, ErrorCode::InvalidState | ErrorCode::NotConnected),
            "disconnected explicit server requests must fail before wire emission"
        );
    }

    let connect = client
        .connect_to_server(0x80)
        .expect("failed explicit refresh requests must not consume the first TAN");
    assert_eq!(
        connect.data[1], 0,
        "local rejection before connection must not allocate a transaction number"
    );

    let tan = connect.data[1];
    let mut properties_response = vec![FSFunction::GetFileServerProperties.as_u8(), tan, 0x00];
    properties_response.extend_from_slice(&FileServerProperties::default().encode());
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        properties_response,
        0x80,
    ));
    assert!(client.is_connected());

    let status = client
        .try_request_server_status()
        .expect("connected client can explicitly request FileServerStatus");
    assert_eq!(status.dest, Some(0x80));
    assert_eq!(status.data[0], FSFunction::FileServerStatus.as_u8());
    assert_eq!(
        status.data[1], 1,
        "first connected refresh after handshake should receive the next TAN"
    );
}

#[test]
fn file_client_keeps_pending_request_after_mismatched_response_function() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let open = client
        .open_file("doc.txt", OpenFlags::Read.bit())
        .expect("connected client can emit OpenFile");
    let open_tan = open.data[1];

    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::CloseFile.as_u8(),
            open_tan,
            FSError::Success.as_u8(),
        ],
        0x80,
    ));
    assert!(
        client.open_files().is_empty(),
        "wrong response function must not create an open-file record"
    );

    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::OpenFile.as_u8(),
            open_tan,
            FSError::Success.as_u8(),
            7,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert!(
        client.open_files().contains_key(&7),
        "the valid matching response must still satisfy the original pending TAN"
    );
}

#[test]
fn file_client_rejects_successful_open_responses_with_reserved_handles() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    for reserved in [RESERVED_FILE_HANDLE_0, INVALID_FILE_HANDLE] {
        let open = client
            .open_file("doc.txt", OpenFlags::Read.bit())
            .expect("connected client can emit OpenFile");
        let open_tan = open.data[1];
        client.handle_server_response(&Message::new(
            PGN_FILE_SERVER_TO_CLIENT,
            vec![
                FSFunction::OpenFile.as_u8(),
                open_tan,
                FSError::Success.as_u8(),
                reserved,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        assert!(
            !client.open_files().contains_key(&reserved),
            "reserved/invalid file handles must not become usable local state"
        );
        assert!(
            client.open_files().is_empty(),
            "failed OpenFile response validation must not create any handle"
        );
    }
}

#[test]
fn file_client_request_timeout_keeps_boundary_and_drops_late_successes() {
    let mut client = FileClient::new(
        FileClientConfig::default()
            .with_ccm_interval(10_000)
            .with_request_timeout(1_000),
    );
    connect_file_client(&mut client, 0x80);

    type OpenLog = Vec<(u8, Result<u8, FSError>)>;
    let open_log: Rc<RefCell<OpenLog>> = Rc::new(RefCell::new(Vec::new()));
    let open_log_cb = open_log.clone();
    client
        .on_open_response
        .subscribe(move |&(tan, result)| open_log_cb.borrow_mut().push((tan, result)));

    let boundary_request = client
        .try_open_file("boundary.txt", OpenFlags::Read.bit())
        .unwrap();
    let boundary_tan = boundary_request.data[1];
    assert!(client.update(1_000).is_empty());
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::OpenFile.as_u8(),
            boundary_tan,
            FSError::Success.as_u8(),
            0x21,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(
        open_log.borrow().as_slice(),
        &[(boundary_tan, Ok(0x21))],
        "a response exactly on the request-timeout boundary must still be accepted"
    );
    assert!(
        client.open_files().contains_key(&0x21),
        "accepted boundary response must update the open-file table"
    );

    let expired_request = client
        .try_open_file("late.txt", OpenFlags::Read.bit())
        .unwrap();
    let expired_tan = expired_request.data[1];
    assert!(client.update(1_001).is_empty());
    assert!(
        client.is_connected(),
        "request expiry alone must not disconnect before the server-status timeout"
    );
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::OpenFile.as_u8(),
            expired_tan,
            FSError::Success.as_u8(),
            0x22,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(
        open_log.borrow().as_slice(),
        &[(boundary_tan, Ok(0x21))],
        "late success responses for expired TANs must not emit ordinary operation events"
    );
    assert!(
        !client.open_files().contains_key(&0x22),
        "late success responses for expired TANs must not allocate handles"
    );
}

#[test]
fn file_client_rejects_malformed_status_broadcasts_without_keepalive_or_cache_update() {
    let mut client = FileClient::new(
        FileClientConfig::default()
            .with_ccm_interval(10_000)
            .with_request_timeout(10_000),
    );
    connect_file_client(&mut client, 0x80);
    assert!(client.server_status().is_none());

    let mut malformed_busy = FileServerStatus {
        busy: true,
        number_of_open_files: 2,
    }
    .encode()
    .to_vec();
    malformed_busy[0] = 0x02;
    client.update(500);
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        malformed_busy,
        0x80,
    ));
    assert!(
        client.server_status().is_none(),
        "malformed busy bits must not update the File Server status cache"
    );

    let mut malformed_tail = FileServerStatus {
        busy: false,
        number_of_open_files: 1,
    }
    .encode()
    .to_vec();
    malformed_tail[2] = 0x00;
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        malformed_tail,
        0x80,
    ));
    assert!(
        client.server_status().is_none(),
        "non-canonical reserved tail bytes must not update the status cache"
    );

    let out_of_range_open_count = FileServerStatus {
        busy: false,
        number_of_open_files: 251,
    }
    .encode()
    .to_vec();
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        out_of_range_open_count,
        0x80,
    ));
    assert!(
        client.server_status().is_none(),
        "out-of-range open-file counts must not update the status cache"
    );

    client.update(5_500);
    assert!(
        !client.is_connected(),
        "malformed status broadcasts must not refresh the server-status timeout"
    );

    let mut client = FileClient::new(
        FileClientConfig::default()
            .with_ccm_interval(10_000)
            .with_request_timeout(10_000),
    );
    connect_file_client(&mut client, 0x80);
    client.update(500);
    let valid = FileServerStatus {
        busy: true,
        number_of_open_files: 3,
    }
    .encode()
    .to_vec();
    client.handle_server_response(&Message::new(PGN_FILE_SERVER_TO_CLIENT, valid, 0x80));
    assert_eq!(
        client.server_status(),
        Some(FileServerStatus {
            busy: true,
            number_of_open_files: 3
        })
    );
    client.update(5_000);
    assert!(
        client.is_connected(),
        "valid status broadcasts should refresh the server-status timeout"
    );
}

#[test]
fn file_client_rejects_invalid_current_directory_responses_before_state_mutation() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);
    assert_eq!(client.current_directory(), "\\");

    for invalid in ["", "../secret", "\\logs\\..\\secret", "bad:name"] {
        let request = client.get_current_directory().unwrap();
        let tan = request.data[1];
        let mut response = vec![
            FSFunction::GetCurrentDirectory.as_u8(),
            tan,
            FSError::Success.as_u8(),
            invalid.len() as u8,
        ];
        response.extend_from_slice(invalid.as_bytes());
        while response.len() < 8 {
            response.push(0xFF);
        }
        client.handle_server_response(&Message::new(PGN_FILE_SERVER_TO_CLIENT, response, 0x80));
        assert_eq!(
            client.current_directory(),
            "\\",
            "invalid current-directory response {invalid:?} must not mutate client state"
        );
    }

    let request = client.get_current_directory().unwrap();
    let tan = request.data[1];
    let mut response = vec![
        FSFunction::GetCurrentDirectory.as_u8(),
        tan,
        FSError::Success.as_u8(),
        6,
    ];
    response.extend_from_slice(b"\\logs\\");
    client.handle_server_response(&Message::new(PGN_FILE_SERVER_TO_CLIENT, response, 0x80));
    assert_eq!(client.current_directory(), "\\logs\\");
}

#[test]
fn file_client_rejects_non_ascii_current_directory_response_before_state_mutation() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);
    assert_eq!(client.current_directory(), "\\");

    type DirectoryLog = Vec<(u8, Result<String, FSError>)>;
    let directory_log: Rc<RefCell<DirectoryLog>> = Rc::new(RefCell::new(Vec::new()));
    let directory_log_cb = directory_log.clone();
    client
        .on_current_directory_response
        .subscribe(move |event| directory_log_cb.borrow_mut().push(event.clone()));

    let request = client.get_current_directory().unwrap();
    let tan = request.data[1];
    let non_ascii_directory = vec![
        FSFunction::GetCurrentDirectory.as_u8(),
        tan,
        FSError::Success.as_u8(),
        4,
        b'\\',
        0xC3,
        0xBF,
        b'\\',
    ];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        non_ascii_directory,
        0x80,
    ));

    assert_eq!(
        client.current_directory(),
        "\\",
        "non-ASCII current-directory response bytes must not mutate the cached directory"
    );
    assert_eq!(directory_log.borrow().len(), 1);
    assert!(matches!(
        directory_log.borrow()[0],
        (_, Err(FSError::InvalidSourceName))
    ));

    let valid_request = client.get_current_directory().unwrap();
    let valid_tan = valid_request.data[1];
    let mut valid_response = vec![
        FSFunction::GetCurrentDirectory.as_u8(),
        valid_tan,
        FSError::Success.as_u8(),
        6,
    ];
    valid_response.extend_from_slice(b"\\logs\\");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        valid_response,
        0x80,
    ));

    assert_eq!(client.current_directory(), "\\logs\\");
    assert_eq!(
        directory_log.borrow().last(),
        Some(&(valid_tan, Ok("\\logs\\".to_string()))),
        "a later canonical current-directory response should still update the cache"
    );
}

#[test]
fn file_client_resolves_change_directory_success_without_storing_relative_tokens() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    for (request_path, expected_directory) in [
        ("logs", "\\logs\\"),
        (".", "\\logs\\"),
        ("day", "\\logs\\day\\"),
        ("..", "\\logs\\"),
        ("\\", "\\"),
        ("\\TASKDATA", "\\TASKDATA\\"),
    ] {
        let request = client
            .change_directory(request_path)
            .expect("connected client can emit ChangeDirectory");
        let tan = request.data[1];
        client.handle_server_response(&Message::new(
            PGN_FILE_SERVER_TO_CLIENT,
            vec![
                FSFunction::ChangeDirectory.as_u8(),
                tan,
                FSError::Success.as_u8(),
                0xFF,
                0xFF,
                0xFF,
                0xFF,
                0xFF,
            ],
            0x80,
        ));
        assert_eq!(
            client.current_directory(),
            expected_directory,
            "successful ChangeDirectory {request_path:?} must resolve to a canonical directory"
        );
    }

    let before_error = client.current_directory().to_string();
    let request = client.change_directory("errors").unwrap();
    let tan = request.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            tan,
            FSError::NotFound.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(
        client.current_directory(),
        before_error,
        "unsuccessful ChangeDirectory must not mutate the cached current directory"
    );
}

#[test]
fn file_client_rejects_read_write_success_counts_that_exceed_request() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let open = client
        .open_file("doc.txt", OpenFlags::ReadWrite.bit())
        .expect("connected client can emit OpenFile");
    let open_tan = open.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::OpenFile.as_u8(),
            open_tan,
            FSError::Success.as_u8(),
            7,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.open_files().get(&7).unwrap().position, 0);

    let read = client.read_file(7, 2).unwrap();
    let read_tan = read.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::ReadFile.as_u8(),
            read_tan,
            FSError::Success.as_u8(),
            3,
            0,
            b'a',
            b'b',
            b'c',
        ],
        0x80,
    ));
    assert_eq!(
        client.open_files().get(&7).unwrap().position,
        0,
        "ReadFile success count larger than requested must not advance local position"
    );

    let read = client.read_file(7, 2).unwrap();
    let read_tan = read.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::ReadFile.as_u8(),
            read_tan,
            FSError::Success.as_u8(),
            2,
            0,
            b'a',
            b'b',
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.open_files().get(&7).unwrap().position, 2);

    let write = client.write_file(7, b"xy").unwrap();
    let write_tan = write.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::WriteFile.as_u8(),
            write_tan,
            FSError::Success.as_u8(),
            3,
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(
        client.open_files().get(&7).unwrap().position,
        2,
        "WriteFile success count larger than requested must not advance local position"
    );

    let write = client.write_file(7, b"xy").unwrap();
    let write_tan = write.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::WriteFile.as_u8(),
            write_tan,
            FSError::Success.as_u8(),
            1,
            0,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.open_files().get(&7).unwrap().position, 3);
}

#[test]
fn file_client_rejects_malformed_close_and_seek_successes_before_state_mutation() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let open = client
        .open_file("doc.txt", OpenFlags::ReadWrite.bit())
        .expect("connected client can emit OpenFile");
    let open_tan = open.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::OpenFile.as_u8(),
            open_tan,
            FSError::Success.as_u8(),
            7,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.open_files().get(&7).unwrap().position, 0);

    let seek = client.seek_file(7, 123).unwrap();
    let seek_tan = seek.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::SeekFile.as_u8(),
            seek_tan,
            FSError::Success.as_u8(),
            0x00,
        ],
        0x80,
    ));
    assert_eq!(
        client.open_files().get(&7).unwrap().position,
        0,
        "malformed SeekFile success must not apply the requested position"
    );

    let seek = client.seek_file(7, 123).unwrap();
    let seek_tan = seek.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::SeekFile.as_u8(),
            seek_tan,
            FSError::Success.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.open_files().get(&7).unwrap().position, 123);

    let close = client.close_file(7).unwrap();
    let close_tan = close.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::CloseFile.as_u8(),
            close_tan,
            FSError::Success.as_u8(),
            0x00,
        ],
        0x80,
    ));
    assert!(
        client.open_files().contains_key(&7),
        "malformed CloseFile success must not drop the local file handle"
    );

    let close = client.close_file(7).unwrap();
    let close_tan = close.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::CloseFile.as_u8(),
            close_tan,
            FSError::Success.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert!(!client.open_files().contains_key(&7));
}

#[test]
fn file_client_removed_volume_status_clears_media_dependent_state() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let change = client
        .change_directory("jobs")
        .expect("connected client can emit ChangeDirectory");
    let change_tan = change.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            change_tan,
            FSError::Success.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.current_directory(), "\\jobs\\");

    let open = client
        .open_file("task.txt", OpenFlags::Read.bit())
        .expect("connected client can emit OpenFile");
    let open_tan = open.data[1];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::OpenFile.as_u8(),
            open_tan,
            FSError::Success.as_u8(),
            7,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert!(client.open_files().contains_key(&7));

    let volume_log: Rc<RefCell<Vec<VolumeState>>> = Rc::new(RefCell::new(Vec::new()));
    let volume_log_cb = volume_log.clone();
    client
        .on_volume_status
        .subscribe(move |&state| volume_log_cb.borrow_mut().push(state));
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::VolumeStatus.as_u8(),
            INVALID_TAN,
            VolumeState::Removed.as_u8(),
            0,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));

    assert_eq!(*volume_log.borrow(), vec![VolumeState::Removed]);
    assert!(
        client.open_files().is_empty(),
        "removed-volume broadcast must invalidate stale client file handles"
    );
    assert_eq!(
        client.current_directory(),
        "\\",
        "removed-volume broadcast must reset stale client current directory"
    );
    assert!(
        client.is_connected(),
        "media removal invalidates media state without disconnecting the server session"
    );
}

#[test]
fn file_client_volume_status_requests_validate_and_parse_counted_responses() {
    let mut client = FileClient::new(FileClientConfig::default());
    assert!(
        client.try_request_volume_status("").is_err(),
        "VolumeStatus requests need a known server address"
    );
    connect_file_client(&mut client, 0x80);
    assert!(
        client.try_request_volume_status("\\").is_err(),
        "VolumeStatus names use the volume-name field, not a file-system path"
    );

    let log: Rc<RefCell<Vec<VolumeState>>> = Rc::new(RefCell::new(Vec::new()));
    let lc = log.clone();
    client
        .on_volume_status
        .subscribe(move |&state| lc.borrow_mut().push(state));

    let status = client.try_request_volume_status("").unwrap();
    assert_eq!(status.dest, Some(0x80));
    assert_eq!(status.data, volume_status_request(status.data[1], 0x00, ""));

    let open = client
        .open_file("cache.txt", OpenFlags::Read.bit())
        .unwrap();
    let handle = 0x22;
    let mut open_response = vec![0xFF; 8];
    open_response[0] = FSFunction::OpenFile.as_u8();
    open_response[1] = open.data[1];
    open_response[2] = FSError::Success.as_u8();
    open_response[3] = handle;
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        open_response,
        0x80,
    ));
    assert!(client.open_files().contains_key(&handle));

    let mut malformed = vec![
        FSFunction::VolumeStatus.as_u8(),
        status.data[1],
        VolumeState::Present.as_u8(),
        1,
        6,
        0,
    ];
    malformed.extend_from_slice(b"ISO");
    client.handle_server_response(&Message::new(PGN_FILE_SERVER_TO_CLIENT, malformed, 0x80));
    assert!(log.borrow().is_empty());

    let status = client.try_request_volume_status("").unwrap();
    let mut removed = vec![
        FSFunction::VolumeStatus.as_u8(),
        status.data[1],
        VolumeState::Removed.as_u8(),
        1,
        6,
        0,
    ];
    removed.extend_from_slice(b"ISOBUS");
    client.handle_server_response(&Message::new(PGN_FILE_SERVER_TO_CLIENT, removed, 0x80));
    assert_eq!(*log.borrow(), vec![VolumeState::Removed]);
    assert!(client.open_files().is_empty());
    assert_eq!(client.current_directory(), "\\");
    assert!(
        client
            .update(7_000)
            .iter()
            .all(|out| out.data[0] != FSFunction::VolumeStatus.as_u8()),
        "a valid VolumeStatus response or broadcast clears the pending volume request"
    );
}

#[test]
fn file_client_volume_status_responses_clear_only_matching_pending_tan() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let open = client
        .open_file("cache.txt", OpenFlags::Read.bit())
        .expect("connected client can emit OpenFile");
    let handle = 0x24;
    let mut open_response = vec![0xFF; 8];
    open_response[0] = FSFunction::OpenFile.as_u8();
    open_response[1] = open.data[1];
    open_response[2] = FSError::Success.as_u8();
    open_response[3] = handle;
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        open_response,
        0x80,
    ));
    assert!(client.open_files().contains_key(&handle));

    let log: Rc<RefCell<Vec<VolumeState>>> = Rc::new(RefCell::new(Vec::new()));
    let lc = log.clone();
    client
        .on_volume_status
        .subscribe(move |&state| lc.borrow_mut().push(state));

    let first = client.try_request_volume_status("").unwrap();
    let second = client.try_request_volume_status("").unwrap();
    assert_ne!(
        first.data[1], second.data[1],
        "parallel VolumeStatus requests must use distinct TANs"
    );

    let mut first_present = vec![
        FSFunction::VolumeStatus.as_u8(),
        first.data[1],
        VolumeState::Present.as_u8(),
        1,
        6,
        0,
    ];
    first_present.extend_from_slice(b"ISOBUS");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        first_present,
        0x80,
    ));
    assert_eq!(*log.borrow(), vec![VolumeState::Present]);
    assert!(
        client.open_files().contains_key(&handle),
        "a Present response to the first TAN must not clear media-dependent state"
    );

    let mut second_removed = vec![
        FSFunction::VolumeStatus.as_u8(),
        second.data[1],
        VolumeState::Removed.as_u8(),
        1,
        6,
        0,
    ];
    second_removed.extend_from_slice(b"ISOBUS");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        second_removed,
        0x80,
    ));
    assert_eq!(
        *log.borrow(),
        vec![VolumeState::Present, VolumeState::Removed],
        "a later valid VolumeStatus response must still match its own pending TAN"
    );
    assert!(
        client.open_files().is_empty(),
        "Removed response to the second TAN must still clear stale file handles"
    );
    assert_eq!(client.current_directory(), "\\");
}

#[test]
fn file_client_volume_status_response_name_matches_nonempty_request() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let log: Rc<RefCell<Vec<VolumeState>>> = Rc::new(RefCell::new(Vec::new()));
    let lc = log.clone();
    client
        .on_volume_status
        .subscribe(move |&state| lc.borrow_mut().push(state));

    let request = client.try_request_volume_status("FIELD").unwrap();
    let tan = request.data[1];
    assert_eq!(request.data, volume_status_request(tan, 0x00, "FIELD"));

    let mut wrong_name = vec![
        FSFunction::VolumeStatus.as_u8(),
        tan,
        VolumeState::Present.as_u8(),
        1,
        5,
        0,
    ];
    wrong_name.extend_from_slice(b"OTHER");
    client.handle_server_response(&Message::new(PGN_FILE_SERVER_TO_CLIENT, wrong_name, 0x80));
    assert!(
        log.borrow().is_empty(),
        "VolumeStatus responses for a different named volume must not emit events"
    );

    let mut matching_name = vec![
        FSFunction::VolumeStatus.as_u8(),
        tan,
        VolumeState::Present.as_u8(),
        1,
        5,
        0,
    ];
    matching_name.extend_from_slice(b"FIELD");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        matching_name,
        0x80,
    ));
    assert_eq!(
        *log.borrow(),
        vec![VolumeState::Present],
        "a later response with the requested volume name proves the mismatched response did not consume the TAN"
    );
}

#[test]
fn file_client_rejects_malformed_volume_status_responses_without_media_state_or_pending_loss() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    let change = client
        .change_directory("jobs")
        .expect("connected client can emit ChangeDirectory");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            change.data[1],
            FSError::Success.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.current_directory(), "\\jobs\\");

    let open = client
        .open_file("task.txt", OpenFlags::Read.bit())
        .expect("connected client can emit OpenFile");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::OpenFile.as_u8(),
            open.data[1],
            FSError::Success.as_u8(),
            7,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert!(client.open_files().contains_key(&7));

    let log: Rc<RefCell<Vec<VolumeState>>> = Rc::new(RefCell::new(Vec::new()));
    let lc = log.clone();
    client
        .on_volume_status
        .subscribe(move |&state| lc.borrow_mut().push(state));

    let status = client.try_request_volume_status("").unwrap();
    let tan = status.data[1];
    let malformed_name = vec![
        FSFunction::VolumeStatus.as_u8(),
        tan,
        VolumeState::Removed.as_u8(),
        1,
        1,
        0,
        b'\\',
    ];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        malformed_name,
        0x80,
    ));
    assert!(
        log.borrow().is_empty(),
        "invalid counted volume names must not emit VolumeStatus events"
    );
    assert!(
        client.open_files().contains_key(&7),
        "invalid VolumeStatus responses must not clear open handles"
    );
    assert_eq!(
        client.current_directory(),
        "\\jobs\\",
        "invalid VolumeStatus responses must not reset the current directory"
    );

    let non_ascii_name = vec![
        FSFunction::VolumeStatus.as_u8(),
        tan,
        VolumeState::Removed.as_u8(),
        1,
        2,
        0,
        0xC3,
        0xBF,
    ];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        non_ascii_name,
        0x80,
    ));
    assert!(
        log.borrow().is_empty(),
        "non-ASCII counted volume names must not emit VolumeStatus events"
    );
    assert!(
        client.open_files().contains_key(&7),
        "non-ASCII VolumeStatus responses must not clear open handles"
    );
    assert_eq!(
        client.current_directory(),
        "\\jobs\\",
        "non-ASCII VolumeStatus responses must not reset the current directory"
    );

    let mut invalid_removal_time = vec![
        FSFunction::VolumeStatus.as_u8(),
        tan,
        VolumeState::Removed.as_u8(),
        251,
        6,
        0,
    ];
    invalid_removal_time.extend_from_slice(b"ISOBUS");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        invalid_removal_time,
        0x80,
    ));
    assert!(
        log.borrow().is_empty(),
        "out-of-range VolumeStatus removal-time values must not emit events"
    );
    assert!(
        client.open_files().contains_key(&7),
        "out-of-range VolumeStatus removal-time values must not clear open handles"
    );
    assert_eq!(
        client.current_directory(),
        "\\jobs\\",
        "out-of-range VolumeStatus removal-time values must not reset current directory"
    );

    let mut valid_removed = vec![
        FSFunction::VolumeStatus.as_u8(),
        tan,
        VolumeState::Removed.as_u8(),
        1,
        6,
        0,
    ];
    valid_removed.extend_from_slice(b"ISOBUS");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        valid_removed,
        0x80,
    ));
    assert_eq!(
        *log.borrow(),
        vec![VolumeState::Removed],
        "a valid response with the same TAN proves the malformed response did not consume the pending request"
    );
    assert!(client.open_files().is_empty());
    assert_eq!(client.current_directory(), "\\");
}

#[test]
fn file_server_rejects_attribute_changes_while_file_is_open_without_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server
        .add_file("log.txt", b"abc".to_vec(), FileAttributes::Archive.bit())
        .unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x40, "log.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x40, FSError::Success);

    let path = b"log.txt";
    let mut set_attrs = vec![
        FSFunction::SetFileAttributes.as_u8(),
        0x41,
        path.len() as u8,
        FileAttributes::Hidden.bit(),
    ];
    set_attrs.extend_from_slice(path);
    let denied = server.handle_client_message(&fs_request(set_attrs, 0x43));
    assert_response(
        &denied[0].data,
        FSFunction::SetFileAttributes,
        0x41,
        FSError::AccessDenied,
    );

    let mut get_attrs = vec![
        FSFunction::GetFileAttributes.as_u8(),
        0x42,
        path.len() as u8,
    ];
    get_attrs.extend_from_slice(path);
    let attrs = server.handle_client_message(&fs_request(get_attrs, 0x42));
    assert_response(
        &attrs[0].data,
        FSFunction::GetFileAttributes,
        0x42,
        FSError::Success,
    );
    assert_eq!(
        attrs[0].data[3],
        FileAttributes::Archive.bit(),
        "denied SetFileAttributes must not mutate the existing attributes"
    );
}

#[test]
fn file_server_rejects_seek_on_directory_handles_without_position_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_directory("logs").unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x50, "logs", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x50, FSError::Success);
    let handle = open[0].data[3];
    assert_eq!(
        server.open_files()[0].position,
        0,
        "OpenDir handles start at the beginning of the directory listing"
    );

    let denied = server.handle_client_message(&fs_request(
        vec![FSFunction::SeekFile.as_u8(), 0x51, handle, 5, 0, 0, 0],
        0x42,
    ));
    assert_response(
        &denied[0].data,
        FSFunction::SeekFile,
        0x51,
        FSError::InvalidHandle,
    );
    assert_eq!(
        server.open_files()[0].position,
        0,
        "SeekFile on an OpenDir handle must not mutate directory-handle position"
    );
}

#[test]
fn file_server_reads_directory_entries_with_standard_count_and_hidden_filter() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("visible.txt", b"ok".to_vec(), 0).unwrap();
    server
        .add_file(
            "hidden.txt",
            b"secret".to_vec(),
            FileAttributes::Hidden.bit(),
        )
        .unwrap();
    server.add_directory("jobs").unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x58, "\\", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x58, FSError::Success);
    let handle = open[0].data[3];

    let visible_only =
        server.handle_client_message(&fs_request(read_dir_request(0x59, handle, 3, 0), 0x42));
    assert_response(
        &visible_only[0].data,
        FSFunction::ReadFile,
        0x59,
        FSError::Success,
    );
    assert_eq!(fs_count(&visible_only[0].data), 2);
    let names = directory_entry_names(&visible_only[0].data);
    assert!(names.iter().any(|name| name == "visible.txt"));
    assert!(names.iter().any(|name| name == "jobs\\"));
    assert!(!names.iter().any(|name| name == "hidden.txt"));
    assert_eq!(
        server.open_files()[0].position,
        2,
        "directory-handle position advances by entries, not encoded bytes"
    );

    let eof = server.handle_client_message(&fs_request(read_dir_request(0x5A, handle, 1, 0), 0x42));
    assert_response(&eof[0].data, FSFunction::ReadFile, 0x5A, FSError::EndOfFile);

    let open_again = server.handle_client_message(&fs_request(
        open_request(0x5B, "\\", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &open_again[0].data,
        FSFunction::OpenFile,
        0x5B,
        FSError::Success,
    );
    let second_handle = open_again[0].data[3];
    let with_hidden = server.handle_client_message(&fs_request(
        read_dir_request(0x5C, second_handle, 4, 1),
        0x42,
    ));
    assert_response(
        &with_hidden[0].data,
        FSFunction::ReadFile,
        0x5C,
        FSError::Success,
    );
    assert_eq!(fs_count(&with_hidden[0].data), 3);
    assert!(
        directory_entry_names(&with_hidden[0].data)
            .iter()
            .any(|name| name == "hidden.txt")
    );

    let before = server.open_files()[1].position;
    let bad_hidden_flag = server.handle_client_message(&fs_request(
        read_dir_request(0x5D, second_handle, 1, 2),
        0x42,
    ));
    assert_response(
        &bad_hidden_flag[0].data,
        FSFunction::ReadFile,
        0x5D,
        FSError::InvalidAccess,
    );
    assert_eq!(
        server.open_files()[1].position,
        before,
        "reserved directory-listing flag values must not consume entries"
    );
}

#[test]
fn file_server_open_dir_applies_final_component_wildcard_pattern() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("alpha.txt", b"a".to_vec(), 0).unwrap();
    server.add_file("beta.bin", b"b".to_vec(), 0).unwrap();
    server
        .add_file("hidden.txt", b"h".to_vec(), FileAttributes::Hidden.bit())
        .unwrap();
    server.add_directory("jobs").unwrap();
    server.add_file("jobs\\task.txt", b"t".to_vec(), 0).unwrap();
    server.add_file("jobs\\task.bin", b"b".to_vec(), 0).unwrap();

    let open_txt = server.handle_client_message(&fs_request(
        open_request(0x5E, "*.txt", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &open_txt[0].data,
        FSFunction::OpenFile,
        0x5E,
        FSError::Success,
    );
    let txt_handle = open_txt[0].data[3];
    let txt_entries =
        server.handle_client_message(&fs_request(read_dir_request(0x5F, txt_handle, 8, 0), 0x42));
    assert_response(
        &txt_entries[0].data,
        FSFunction::ReadFile,
        0x5F,
        FSError::Success,
    );
    let txt_names = directory_entry_names(&txt_entries[0].data);
    assert_eq!(txt_names, vec!["alpha.txt"]);

    let open_jobs_txt = server.handle_client_message(&fs_request(
        open_request(0x60, "jobs\\*.txt", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &open_jobs_txt[0].data,
        FSFunction::OpenFile,
        0x60,
        FSError::Success,
    );
    let jobs_handle = open_jobs_txt[0].data[3];
    let jobs_entries =
        server.handle_client_message(&fs_request(read_dir_request(0x61, jobs_handle, 8, 0), 0x42));
    assert_response(
        &jobs_entries[0].data,
        FSFunction::ReadFile,
        0x61,
        FSError::Success,
    );
    assert_eq!(
        directory_entry_names(&jobs_entries[0].data),
        vec!["task.txt"]
    );

    let before_bad_open_count = server.open_files().len();
    let bad_parent_pattern = server.handle_client_message(&fs_request(
        open_request(0x62, "jo*\\*.txt", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &bad_parent_pattern[0].data,
        FSFunction::OpenFile,
        0x62,
        FSError::InvalidSourceName,
    );
    assert_eq!(
        server.open_files().len(),
        before_bad_open_count,
        "invalid wildcard directory path must not allocate an OpenDir handle"
    );
}

#[test]
fn file_server_rejects_reserved_tan_before_client_or_file_state_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());

    let rejected = server.handle_client_message(&fs_request(
        open_request(
            INVALID_TAN,
            "new.txt",
            OpenFlags::ReadWrite | OpenFlags::Create,
        ),
        0x42,
    ));
    assert_response(
        &rejected[0].data,
        FSFunction::OpenFile,
        INVALID_TAN,
        FSError::TANError,
    );
    assert!(
        server.clients().is_empty(),
        "reserved-TAN requests must not create File Server client state"
    );
    assert!(
        server.open_files().is_empty(),
        "reserved-TAN OpenFile must not allocate a handle"
    );

    let not_found = server.handle_client_message(&fs_request(
        open_request(0x60, "new.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &not_found[0].data,
        FSFunction::OpenFile,
        0x60,
        FSError::NotFound,
    );
}

#[test]
fn file_server_rejects_move_of_read_only_source_without_file_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server
        .add_file(
            "locked.txt",
            b"fixed".to_vec(),
            FileAttributes::ReadOnly.bit(),
        )
        .unwrap();

    let source = b"locked.txt";
    let destination = b"moved.txt";
    let mut request = vec![
        FSFunction::MoveFile.as_u8(),
        0x70,
        source.len() as u8,
        destination.len() as u8,
    ];
    request.extend_from_slice(source);
    request.extend_from_slice(destination);

    let denied = server.handle_client_message(&fs_request(request, 0x42));
    assert_response(
        &denied[0].data,
        FSFunction::MoveFile,
        0x70,
        FSError::AccessDenied,
    );

    let reopen_source = server.handle_client_message(&fs_request(
        open_request(0x71, "locked.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &reopen_source[0].data,
        FSFunction::OpenFile,
        0x71,
        FSError::Success,
    );

    let destination_missing = server.handle_client_message(&fs_request(
        open_request(0x72, "moved.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &destination_missing[0].data,
        FSFunction::OpenFile,
        0x72,
        FSError::NotFound,
    );
}

#[test]
fn file_server_requires_existing_parent_directories_for_create_and_move() {
    let mut server = FileServer::new(FileServerConfig::default());
    server
        .add_file("source.txt", b"source".to_vec(), 0)
        .unwrap();
    server.add_file("plain", b"not a dir".to_vec(), 0).unwrap();

    let create_missing_parent = server.handle_client_message(&fs_request(
        open_request(
            0x73,
            "missing\\created.txt",
            OpenFlags::ReadWrite | OpenFlags::Create,
        ),
        0x42,
    ));
    assert_response(
        &create_missing_parent[0].data,
        FSFunction::OpenFile,
        0x73,
        FSError::NotFound,
    );
    assert!(
        server.open_files().is_empty(),
        "failed create under a missing parent must not allocate a handle"
    );

    let create_under_file = server.handle_client_message(&fs_request(
        open_request(
            0x74,
            "plain\\created.txt",
            OpenFlags::ReadWrite | OpenFlags::Create,
        ),
        0x42,
    ));
    assert_response(
        &create_under_file[0].data,
        FSFunction::OpenFile,
        0x74,
        FSError::WrongType,
    );

    let missing_created = server.handle_client_message(&fs_request(
        open_request(0x75, "missing\\created.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &missing_created[0].data,
        FSFunction::OpenFile,
        0x75,
        FSError::NotFound,
    );

    let source = b"source.txt";
    let missing_dest = b"missing\\moved.txt";
    let mut move_missing_parent = vec![
        FSFunction::MoveFile.as_u8(),
        0x76,
        source.len() as u8,
        missing_dest.len() as u8,
    ];
    move_missing_parent.extend_from_slice(source);
    move_missing_parent.extend_from_slice(missing_dest);
    let move_denied = server.handle_client_message(&fs_request(move_missing_parent, 0x42));
    assert_response(
        &move_denied[0].data,
        FSFunction::MoveFile,
        0x76,
        FSError::InvalidDestName,
    );

    let source_still_there = server.handle_client_message(&fs_request(
        open_request(0x77, "source.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &source_still_there[0].data,
        FSFunction::OpenFile,
        0x77,
        FSError::Success,
    );
    let source_handle = source_still_there[0].data[3];
    let close_source = server.handle_client_message(&fs_request(
        vec![FSFunction::CloseFile.as_u8(), 0x78, source_handle],
        0x42,
    ));
    assert_response(
        &close_source[0].data,
        FSFunction::CloseFile,
        0x78,
        FSError::Success,
    );

    server.add_directory("missing").unwrap();
    let create_with_parent = server.handle_client_message(&fs_request(
        open_request(
            0x79,
            "missing\\created.txt",
            OpenFlags::ReadWrite | OpenFlags::Create,
        ),
        0x42,
    ));
    assert_response(
        &create_with_parent[0].data,
        FSFunction::OpenFile,
        0x79,
        FSError::Success,
    );
}

#[test]
fn file_server_enforces_advertised_open_file_capacity_before_creation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("one.txt", b"1".to_vec(), 0).unwrap();
    let mut properties = server.get_properties();
    properties.max_simultaneous_files = 1;
    server.set_properties(properties);

    let first = server.handle_client_message(&fs_request(
        open_request(0x80, "one.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(&first[0].data, FSFunction::OpenFile, 0x80, FSError::Success);
    assert_eq!(server.open_files().len(), 1);

    let over_capacity = server.handle_client_message(&fs_request(
        open_request(
            0x81,
            "created.txt",
            OpenFlags::ReadWrite | OpenFlags::Create,
        ),
        0x43,
    ));
    assert_response(
        &over_capacity[0].data,
        FSFunction::OpenFile,
        0x81,
        FSError::MaxHandles,
    );
    assert_eq!(
        server.open_files().len(),
        1,
        "over-capacity OpenFile must not allocate another handle"
    );

    let first_handle = first[0].data[3];
    let close = server.handle_client_message(&fs_request(
        vec![FSFunction::CloseFile.as_u8(), 0x82, first_handle],
        0x42,
    ));
    assert_response(
        &close[0].data,
        FSFunction::CloseFile,
        0x82,
        FSError::Success,
    );

    let created_missing = server.handle_client_message(&fs_request(
        open_request(0x83, "created.txt", OpenFlags::Read.bit()),
        0x44,
    ));
    assert_response(
        &created_missing[0].data,
        FSFunction::OpenFile,
        0x83,
        FSError::NotFound,
    );
}

