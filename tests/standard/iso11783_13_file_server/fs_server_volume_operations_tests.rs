#[test]
fn file_server_rejects_initialize_volume_while_files_are_open_without_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("keep.txt", b"keep".to_vec(), 0).unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x90, "keep.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x90, FSError::Success);
    let handle = open[0].data[3];

    let denied = server.handle_client_message(&fs_request(
        vec![FSFunction::InitializeVolume.as_u8(), 0x91],
        0x43,
    ));
    assert_response(
        &denied[0].data,
        FSFunction::InitializeVolume,
        0x91,
        FSError::AccessDenied,
    );
    assert_eq!(
        server.open_files().len(),
        1,
        "denied InitializeVolume must not close existing handles"
    );

    let read = server.handle_client_message(&fs_request(read_request(0x92, handle, 4), 0x42));
    assert_response(&read[0].data, FSFunction::ReadFile, 0x92, FSError::Success);
    assert_eq!(&read[0].data[5..9], b"keep");

    let close = server.handle_client_message(&fs_request(
        vec![FSFunction::CloseFile.as_u8(), 0x93, handle],
        0x42,
    ));
    assert_response(
        &close[0].data,
        FSFunction::CloseFile,
        0x93,
        FSError::Success,
    );

    let initialized = server.handle_client_message(&fs_request(
        vec![FSFunction::InitializeVolume.as_u8(), 0x94],
        0x43,
    ));
    assert_response(
        &initialized[0].data,
        FSFunction::InitializeVolume,
        0x94,
        FSError::Success,
    );
    let missing = server.handle_client_message(&fs_request(
        open_request(0x95, "keep.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &missing[0].data,
        FSFunction::OpenFile,
        0x95,
        FSError::NotFound,
    );
}

#[test]
fn file_server_initialize_volume_accepts_counted_volume_request_and_rejects_reserved_flags() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("old.txt", b"old".to_vec(), 0).unwrap();
    server.add_directory("old").unwrap();

    let invalid = server.handle_client_message(&fs_request(
        initialize_volume_request(0x96, 1024, 0x80, "FIELD"),
        0x42,
    ));
    assert_response(
        &invalid[0].data,
        FSFunction::InitializeVolume,
        0x96,
        FSError::MalformedRequest,
    );
    assert_eq!(server.volume_name(), "ISOBUS");
    let still_openable = server.handle_client_message(&fs_request(
        open_request(0x97, "old.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &still_openable[0].data,
        FSFunction::OpenFile,
        0x97,
        FSError::Success,
    );
    let handle = still_openable[0].data[3];
    let close = server.handle_client_message(&fs_request(
        vec![FSFunction::CloseFile.as_u8(), 0x98, handle],
        0x42,
    ));
    assert_response(
        &close[0].data,
        FSFunction::CloseFile,
        0x98,
        FSError::Success,
    );

    let bad_name = server.handle_client_message(&fs_request(
        initialize_volume_request(0x99, 0, 0x00, "BAD\\NAME"),
        0x42,
    ));
    assert_response(
        &bad_name[0].data,
        FSFunction::InitializeVolume,
        0x99,
        FSError::MalformedRequest,
    );
    assert_eq!(server.volume_name(), "ISOBUS");

    let initialized = server.handle_client_message(&fs_request(
        initialize_volume_request(0x9A, 4096, 0x01, "FIELD"),
        0x42,
    ));
    assert_response(
        &initialized[0].data,
        FSFunction::InitializeVolume,
        0x9A,
        FSError::Success,
    );
    assert_eq!(server.volume_name(), "FIELD");
    assert!(server.open_files().is_empty());
    let old_missing = server.handle_client_message(&fs_request(
        open_request(0x9B, "old.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &old_missing[0].data,
        FSFunction::OpenFile,
        0x9B,
        FSError::NotFound,
    );

    let status = server.handle_client_message(&fs_request(
        volume_status_request(0x9C, 0x00, "FIELD"),
        0x42,
    ));
    assert_eq!(status[0].data[2], VolumeState::Present.as_u8());
    assert_eq!(
        &status[0].data[6..],
        b"FIELD",
        "initialized volume name is used by later VolumeStatus responses"
    );
}

#[test]
fn file_server_reports_wrong_type_for_file_operations_on_directories_without_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_directory("logs").unwrap();
    server.add_file("plain.txt", b"plain".to_vec(), 0).unwrap();

    let open_directory_as_file = server.handle_client_message(&fs_request(
        open_request(0xA0, "logs", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &open_directory_as_file[0].data,
        FSFunction::OpenFile,
        0xA0,
        FSError::WrongType,
    );

    let open_file_as_directory = server.handle_client_message(&fs_request(
        open_request(0xA1, "plain.txt", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &open_file_as_directory[0].data,
        FSFunction::OpenFile,
        0xA1,
        FSError::WrongType,
    );

    let file_path = b"plain.txt";
    let mut change_to_file = vec![
        FSFunction::ChangeDirectory.as_u8(),
        0xA2,
        file_path.len() as u8,
    ];
    change_to_file.extend_from_slice(file_path);
    let denied_change = server.handle_client_message(&fs_request(change_to_file, 0x42));
    assert_response(
        &denied_change[0].data,
        FSFunction::ChangeDirectory,
        0xA2,
        FSError::WrongType,
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\",
        "ChangeDirectory to a regular file must not mutate the current directory"
    );

    let directory_path = b"logs";
    let mut delete_directory = vec![
        FSFunction::DeleteFile.as_u8(),
        0xA3,
        directory_path.len() as u8,
    ];
    delete_directory.extend_from_slice(directory_path);
    let denied_delete = server.handle_client_message(&fs_request(delete_directory, 0x42));
    assert_response(
        &denied_delete[0].data,
        FSFunction::DeleteFile,
        0xA3,
        FSError::WrongType,
    );

    let mut move_directory = vec![
        FSFunction::MoveFile.as_u8(),
        0xA4,
        directory_path.len() as u8,
        7,
    ];
    move_directory.extend_from_slice(directory_path);
    move_directory.extend_from_slice(b"new.txt");
    let denied_move = server.handle_client_message(&fs_request(move_directory, 0x42));
    assert_response(
        &denied_move[0].data,
        FSFunction::MoveFile,
        0xA4,
        FSError::WrongType,
    );

    let mut set_directory_attrs = vec![
        FSFunction::SetFileAttributes.as_u8(),
        0xA5,
        directory_path.len() as u8,
        FileAttributes::Hidden.bit(),
    ];
    set_directory_attrs.extend_from_slice(directory_path);
    let denied_set_attrs = server.handle_client_message(&fs_request(set_directory_attrs, 0x42));
    assert_response(
        &denied_set_attrs[0].data,
        FSFunction::SetFileAttributes,
        0xA5,
        FSError::WrongType,
    );

    let mut get_directory_attrs = vec![
        FSFunction::GetFileAttributes.as_u8(),
        0xA8,
        directory_path.len() as u8,
    ];
    get_directory_attrs.extend_from_slice(directory_path);
    let directory_attrs = server.handle_client_message(&fs_request(get_directory_attrs, 0x42));
    assert_response(
        &directory_attrs[0].data,
        FSFunction::GetFileAttributes,
        0xA8,
        FSError::Success,
    );
    assert_eq!(
        directory_attrs[0].data[3],
        FileAttributes::Directory.bit(),
        "GetFileAttributes reports directory targets without making directory attributes writable"
    );

    let open_directory_after_denials = server.handle_client_message(&fs_request(
        open_request(0xA6, "logs", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &open_directory_after_denials[0].data,
        FSFunction::OpenFile,
        0xA6,
        FSError::Success,
    );

    let open_file_after_denials = server.handle_client_message(&fs_request(
        open_request(0xA7, "plain.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &open_file_after_denials[0].data,
        FSFunction::OpenFile,
        0xA7,
        FSError::Success,
    );
}

#[test]
fn file_server_optional_capabilities_gate_operations_before_file_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("one.txt", b"one".to_vec(), 0).unwrap();
    let mut properties = server.get_properties();
    properties.supports_volume_management = false;
    properties.supports_file_attributes = false;
    properties.supports_move_file = false;
    properties.supports_delete_file = false;
    server.set_properties(properties);

    let source = b"one.txt";
    let destination = b"two.txt";
    let mut move_request = vec![
        FSFunction::MoveFile.as_u8(),
        0xB0,
        source.len() as u8,
        destination.len() as u8,
    ];
    move_request.extend_from_slice(source);
    move_request.extend_from_slice(destination);
    let denied_move = server.handle_client_message(&fs_request(move_request, 0x42));
    assert_response(
        &denied_move[0].data,
        FSFunction::MoveFile,
        0xB0,
        FSError::NotSupported,
    );

    let mut delete_request = vec![FSFunction::DeleteFile.as_u8(), 0xB1, source.len() as u8];
    delete_request.extend_from_slice(source);
    let denied_delete = server.handle_client_message(&fs_request(delete_request, 0x42));
    assert_response(
        &denied_delete[0].data,
        FSFunction::DeleteFile,
        0xB1,
        FSError::NotSupported,
    );

    let mut get_attrs_request = vec![
        FSFunction::GetFileAttributes.as_u8(),
        0xB2,
        source.len() as u8,
    ];
    get_attrs_request.extend_from_slice(source);
    let denied_get_attrs = server.handle_client_message(&fs_request(get_attrs_request, 0x42));
    assert_response(
        &denied_get_attrs[0].data,
        FSFunction::GetFileAttributes,
        0xB2,
        FSError::NotSupported,
    );

    let mut set_attrs_request = vec![
        FSFunction::SetFileAttributes.as_u8(),
        0xB3,
        source.len() as u8,
        FileAttributes::Hidden.bit(),
    ];
    set_attrs_request.extend_from_slice(source);
    let denied_set_attrs = server.handle_client_message(&fs_request(set_attrs_request, 0x42));
    assert_response(
        &denied_set_attrs[0].data,
        FSFunction::SetFileAttributes,
        0xB3,
        FSError::NotSupported,
    );

    let denied_initialize = server.handle_client_message(&fs_request(
        vec![FSFunction::InitializeVolume.as_u8(), 0xB4],
        0x42,
    ));
    assert_response(
        &denied_initialize[0].data,
        FSFunction::InitializeVolume,
        0xB4,
        FSError::NotSupported,
    );

    let original_still_exists = server.handle_client_message(&fs_request(
        open_request(0xB5, "one.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &original_still_exists[0].data,
        FSFunction::OpenFile,
        0xB5,
        FSError::Success,
    );

    let destination_not_created = server.handle_client_message(&fs_request(
        open_request(0xB6, "two.txt", OpenFlags::Read.bit()),
        0x43,
    ));
    assert_response(
        &destination_not_created[0].data,
        FSFunction::OpenFile,
        0xB6,
        FSError::NotFound,
    );
}

#[test]
fn file_server_get_file_date_time_uses_counted_paths_and_rejects_invalid_targets() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("log.txt", b"log".to_vec(), 0).unwrap();
    server.add_directory("jobs").unwrap();

    let file_date_time =
        server.handle_client_message(&fs_request(date_time_request(0xC0, "log.txt"), 0x42));
    assert_response(
        &file_date_time[0].data,
        FSFunction::GetFileDateTime,
        0xC0,
        FSError::Success,
    );
    assert_eq!(
        u16::from_le_bytes([file_date_time[0].data[3], file_date_time[0].data[4]]),
        pack_dos_date(2025, 1, 1)
    );
    assert_eq!(
        u16::from_le_bytes([file_date_time[0].data[5], file_date_time[0].data[6]]),
        pack_dos_time(12, 0, 0)
    );
    assert_eq!(
        file_date_time[0].data[7], 0xFF,
        "fixed GetFileDateTime response tail must stay reserved"
    );

    let directory_date_time =
        server.handle_client_message(&fs_request(date_time_request(0xC1, "jobs"), 0x42));
    assert_response(
        &directory_date_time[0].data,
        FSFunction::GetFileDateTime,
        0xC1,
        FSError::Success,
    );

    let root_denied =
        server.handle_client_message(&fs_request(date_time_request(0xC2, "\\"), 0x42));
    assert_response(
        &root_denied[0].data,
        FSFunction::GetFileDateTime,
        0xC2,
        FSError::AccessDenied,
    );

    let missing =
        server.handle_client_message(&fs_request(date_time_request(0xC3, "missing.txt"), 0x42));
    assert_response(
        &missing[0].data,
        FSFunction::GetFileDateTime,
        0xC3,
        FSError::NotFound,
    );

    let invalid = server.handle_client_message(&fs_request(
        date_time_request(0xC4, "jobs\\..\\secret.txt"),
        0x42,
    ));
    assert_response(
        &invalid[0].data,
        FSFunction::GetFileDateTime,
        0xC4,
        FSError::InvalidSourceName,
    );

    let malformed = server.handle_client_message(&fs_request(
        vec![
            FSFunction::GetFileDateTime.as_u8(),
            0xC5,
            5,
            0,
            b'a',
            b'b',
            b'c',
        ],
        0x42,
    ));
    assert_response(
        &malformed[0].data,
        FSFunction::GetFileDateTime,
        0xC5,
        FSError::MalformedRequest,
    );
}

#[test]
fn file_server_preserves_and_cleans_per_path_date_time_metadata() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("log.txt", b"log".to_vec(), 0).unwrap();
    server.add_directory("jobs").unwrap();

    let file_date = pack_dos_date(2026, 6, 18);
    let file_time = pack_dos_time(9, 30, 12);
    let dir_date = pack_dos_date(2024, 12, 31);
    let dir_time = pack_dos_time(23, 58, 58);
    server
        .set_file_date_time("log.txt", file_date, file_time)
        .unwrap();
    server
        .set_file_date_time("jobs", dir_date, dir_time)
        .unwrap();

    let file_dt =
        server.handle_client_message(&fs_request(date_time_request(0xC6, "log.txt"), 0x42));
    assert_response(
        &file_dt[0].data,
        FSFunction::GetFileDateTime,
        0xC6,
        FSError::Success,
    );
    assert_eq!(fs_date_time(&file_dt[0].data), (file_date, file_time));

    let dir_dt = server.handle_client_message(&fs_request(date_time_request(0xC7, "jobs"), 0x42));
    assert_response(
        &dir_dt[0].data,
        FSFunction::GetFileDateTime,
        0xC7,
        FSError::Success,
    );
    assert_eq!(fs_date_time(&dir_dt[0].data), (dir_date, dir_time));

    let open_dir = server.handle_client_message(&fs_request(
        open_request(0xC8, "\\", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &open_dir[0].data,
        FSFunction::OpenFile,
        0xC8,
        FSError::Success,
    );
    let read_dir = server.handle_client_message(&fs_request(
        read_dir_request(0xC9, open_dir[0].data[3], 4, 1),
        0x42,
    ));
    assert_response(
        &read_dir[0].data,
        FSFunction::ReadFile,
        0xC9,
        FSError::Success,
    );
    assert_eq!(
        directory_entry_date_time(&read_dir[0].data, "log.txt"),
        Some((file_date, file_time))
    );
    assert_eq!(
        directory_entry_date_time(&read_dir[0].data, "jobs\\"),
        Some((dir_date, dir_time))
    );

    let mut move_request = vec![
        FSFunction::MoveFile.as_u8(),
        0xCA,
        b"log.txt".len() as u8,
        b"moved.txt".len() as u8,
    ];
    move_request.extend_from_slice(b"log.txt");
    move_request.extend_from_slice(b"moved.txt");
    let moved = server.handle_client_message(&fs_request(move_request, 0x42));
    assert_response(&moved[0].data, FSFunction::MoveFile, 0xCA, FSError::Success);
    let moved_dt =
        server.handle_client_message(&fs_request(date_time_request(0xCB, "moved.txt"), 0x42));
    assert_response(
        &moved_dt[0].data,
        FSFunction::GetFileDateTime,
        0xCB,
        FSError::Success,
    );
    assert_eq!(fs_date_time(&moved_dt[0].data), (file_date, file_time));
    let old_missing =
        server.handle_client_message(&fs_request(date_time_request(0xCC, "log.txt"), 0x42));
    assert_response(
        &old_missing[0].data,
        FSFunction::GetFileDateTime,
        0xCC,
        FSError::NotFound,
    );

    let mut delete_request = vec![
        FSFunction::DeleteFile.as_u8(),
        0xCD,
        b"moved.txt".len() as u8,
    ];
    delete_request.extend_from_slice(b"moved.txt");
    let deleted = server.handle_client_message(&fs_request(delete_request, 0x42));
    assert_response(
        &deleted[0].data,
        FSFunction::DeleteFile,
        0xCD,
        FSError::Success,
    );
    let deleted_missing =
        server.handle_client_message(&fs_request(date_time_request(0xCE, "moved.txt"), 0x42));
    assert_response(
        &deleted_missing[0].data,
        FSFunction::GetFileDateTime,
        0xCE,
        FSError::NotFound,
    );

    assert!(
        server
            .set_file_date_time("jobs", pack_dos_date(2025, 0, 1), dir_time)
            .is_err(),
        "out-of-range DOS date fields must not enter metadata"
    );
}

#[test]
fn file_server_removed_volume_resets_and_blocks_current_directory_state() {
    let mut server = FileServer::new(FileServerConfig::default().with_ccm_timeout(60_000));
    server.add_directory("jobs").unwrap();
    server
        .add_file("jobs\\task.txt", b"task".to_vec(), 0)
        .unwrap();

    let change_to_jobs = server.handle_client_message(&fs_request(
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            0xD0,
            b"jobs".len() as u8,
            b'j',
            b'o',
            b'b',
            b's',
        ],
        0x42,
    ));
    assert_response(
        &change_to_jobs[0].data,
        FSFunction::ChangeDirectory,
        0xD0,
        FSError::Success,
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\jobs\\"
    );

    let preparing = server.prepare_volume_for_removal();
    assert_eq!(preparing[0].data[0], FSFunction::VolumeStatus.as_u8());
    assert_eq!(
        preparing[0].data[2],
        VolumeState::PreparingForRemoval.as_u8()
    );
    let removed = server.update(10_000);
    assert_eq!(server.get_volume_state(), VolumeState::Removed);
    assert!(
        removed
            .iter()
            .any(|frame| frame.data[0] == FSFunction::VolumeStatus.as_u8()
                && frame.data[2] == VolumeState::Removed.as_u8())
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\",
        "media removal clears stale per-client directory state"
    );

    let get_cwd = server.handle_client_message(&fs_request(
        vec![FSFunction::GetCurrentDirectory.as_u8(), 0xD1],
        0x42,
    ));
    assert_response(
        &get_cwd[0].data,
        FSFunction::GetCurrentDirectory,
        0xD1,
        FSError::MediaNotPresent,
    );

    let denied_change = server.handle_client_message(&fs_request(
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            0xD2,
            b"jobs".len() as u8,
            b'j',
            b'o',
            b'b',
            b's',
        ],
        0x42,
    ));
    assert_response(
        &denied_change[0].data,
        FSFunction::ChangeDirectory,
        0xD2,
        FSError::MediaNotPresent,
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\",
        "rejected ChangeDirectory while media is absent must not recreate stale cwd"
    );

    let present = server.reinsert_volume().unwrap();
    assert_eq!(present.data[0], FSFunction::VolumeStatus.as_u8());
    assert_eq!(present.data[2], VolumeState::Present.as_u8());

    let open_relative = server.handle_client_message(&fs_request(
        open_request(0xD3, "task.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &open_relative[0].data,
        FSFunction::OpenFile,
        0xD3,
        FSError::NotFound,
    );
    let open_absolute = server.handle_client_message(&fs_request(
        open_request(0xD4, "\\jobs\\task.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &open_absolute[0].data,
        FSFunction::OpenFile,
        0xD4,
        FSError::Success,
    );
}

#[test]
fn file_server_volume_status_requests_drive_removal_state_without_ccm() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("open.txt", b"abc".to_vec(), 0).unwrap();

    let status =
        server.handle_client_message(&fs_request(volume_status_request(0xD5, 0x00, ""), 0x42));
    assert_eq!(status.len(), 1);
    assert_eq!(status[0].dest, Some(0x42));
    assert_eq!(status[0].data[0], FSFunction::VolumeStatus.as_u8());
    assert_eq!(status[0].data[1], 0xD5);
    assert_eq!(status[0].data[2], VolumeState::Present.as_u8());
    assert_eq!(
        u16::from_le_bytes([status[0].data[4], status[0].data[5]]) as usize,
        "ISOBUS".len(),
        "VolumeStatus responses carry a counted volume name"
    );

    let invalid_mode =
        server.handle_client_message(&fs_request(volume_status_request(0xD6, 0x04, ""), 0x42));
    assert_eq!(invalid_mode[0].dest, Some(0x42));
    assert_eq!(invalid_mode[0].data[0], FSFunction::VolumeStatus.as_u8());
    assert_eq!(
        invalid_mode[0].data[2], 0xFF,
        "reserved VolumeStatus request bits are rejected without changing state"
    );
    assert_eq!(server.get_volume_state(), VolumeState::Present);

    let open = server.handle_client_message(&fs_request(
        open_request(0xD7, "open.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0xD7, FSError::Success);
    let handle = open[0].data[3];
    server.update(0);
    assert_eq!(server.get_volume_state(), VolumeState::InUse);

    let preparing =
        server.handle_client_message(&fs_request(volume_status_request(0xD8, 0x02, ""), 0x43));
    assert_eq!(preparing.len(), 1);
    assert_eq!(
        preparing[0].dest, None,
        "a VolumeStatus state change is announced globally"
    );
    assert_eq!(preparing[0].data[1], INVALID_TAN);
    assert_eq!(
        preparing[0].data[2],
        VolumeState::PreparingForRemoval.as_u8()
    );

    let maintain =
        server.handle_client_message(&fs_request(volume_status_request(0xD9, 0x01, ""), 0x42));
    assert_eq!(maintain[0].dest, Some(0x42));
    assert_eq!(
        maintain[0].data[2],
        VolumeState::PreparingForRemoval.as_u8()
    );

    let close = server.handle_client_message(&fs_request(
        vec![FSFunction::CloseFile.as_u8(), 0xDA, handle],
        0x42,
    ));
    assert_response(
        &close[0].data,
        FSFunction::CloseFile,
        0xDA,
        FSError::Success,
    );
    let held = server.update(1_000);
    assert_eq!(server.get_volume_state(), VolumeState::PreparingForRemoval);
    assert!(
        held.iter()
            .all(|frame| frame.data[0] != FSFunction::VolumeStatus.as_u8()),
        "a maintained removal window is not completed just because files closed"
    );

    let released =
        server.handle_client_message(&fs_request(volume_status_request(0xDB, 0x00, ""), 0x42));
    assert_eq!(server.get_volume_state(), VolumeState::Removed);
    assert_eq!(released[0].dest, None);
    assert_eq!(released[0].data[2], VolumeState::Removed.as_u8());
}

#[test]
fn file_server_removed_volume_rejects_path_mutations_before_media_state_changes() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("one.txt", b"one".to_vec(), 0).unwrap();

    let removed = server.set_volume_removed();
    assert_eq!(removed[0].data[0], FSFunction::VolumeStatus.as_u8());
    assert_eq!(removed[0].data[2], VolumeState::Removed.as_u8());

    let mut move_request = vec![
        FSFunction::MoveFile.as_u8(),
        0xD5,
        b"one.txt".len() as u8,
        b"two.txt".len() as u8,
    ];
    move_request.extend_from_slice(b"one.txt");
    move_request.extend_from_slice(b"two.txt");
    let move_denied = server.handle_client_message(&fs_request(move_request, 0x42));
    assert_response(
        &move_denied[0].data,
        FSFunction::MoveFile,
        0xD5,
        FSError::MediaNotPresent,
    );

    let mut delete_request = vec![FSFunction::DeleteFile.as_u8(), 0xD6, b"one.txt".len() as u8];
    delete_request.extend_from_slice(b"one.txt");
    let delete_denied = server.handle_client_message(&fs_request(delete_request, 0x42));
    assert_response(
        &delete_denied[0].data,
        FSFunction::DeleteFile,
        0xD6,
        FSError::MediaNotPresent,
    );

    let mut get_attrs_request = vec![
        FSFunction::GetFileAttributes.as_u8(),
        0xD7,
        b"one.txt".len() as u8,
    ];
    get_attrs_request.extend_from_slice(b"one.txt");
    let get_attrs_denied = server.handle_client_message(&fs_request(get_attrs_request, 0x42));
    assert_response(
        &get_attrs_denied[0].data,
        FSFunction::GetFileAttributes,
        0xD7,
        FSError::MediaNotPresent,
    );

    let mut set_attrs_request = vec![
        FSFunction::SetFileAttributes.as_u8(),
        0xD8,
        b"one.txt".len() as u8,
        FileAttributes::ReadOnly.bit(),
    ];
    set_attrs_request.extend_from_slice(b"one.txt");
    let set_attrs_denied = server.handle_client_message(&fs_request(set_attrs_request, 0x42));
    assert_response(
        &set_attrs_denied[0].data,
        FSFunction::SetFileAttributes,
        0xD8,
        FSError::MediaNotPresent,
    );

    let date_denied =
        server.handle_client_message(&fs_request(date_time_request(0xD9, "one.txt"), 0x42));
    assert_response(
        &date_denied[0].data,
        FSFunction::GetFileDateTime,
        0xD9,
        FSError::MediaNotPresent,
    );

    let init_denied = server.handle_client_message(&fs_request(
        vec![FSFunction::InitializeVolume.as_u8(), 0xDD],
        0x42,
    ));
    assert_response(
        &init_denied[0].data,
        FSFunction::InitializeVolume,
        0xDD,
        FSError::MediaNotPresent,
    );

    server.reinsert_volume().unwrap();
    let original_still_exists = server.handle_client_message(&fs_request(
        open_request(0xDA, "one.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &original_still_exists[0].data,
        FSFunction::OpenFile,
        0xDA,
        FSError::Success,
    );
    let destination_not_created = server.handle_client_message(&fs_request(
        open_request(0xDB, "two.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &destination_not_created[0].data,
        FSFunction::OpenFile,
        0xDB,
        FSError::NotFound,
    );
    let mut get_attrs_after = vec![
        FSFunction::GetFileAttributes.as_u8(),
        0xDC,
        b"one.txt".len() as u8,
    ];
    get_attrs_after.extend_from_slice(b"one.txt");
    let attrs_after = server.handle_client_message(&fs_request(get_attrs_after, 0x42));
    assert_response(
        &attrs_after[0].data,
        FSFunction::GetFileAttributes,
        0xDC,
        FSError::Success,
    );
    assert_eq!(
        attrs_after[0].data[3], 0,
        "rejected SetFileAttributes while media is absent must not mutate attributes"
    );
}

#[test]
fn file_client_get_file_date_time_validates_requests_and_fixed_success_responses() {
    let mut client = FileClient::new(FileClientConfig::default());
    assert!(
        client.try_get_file_date_time("log.txt").is_err(),
        "date/time requests require an established FS server session"
    );
    connect_file_client(&mut client, 0x80);

    assert!(
        client.try_get_file_date_time("bad\\..\\name").is_err(),
        "invalid paths must not allocate a transport request"
    );

    type DateTimeLog = Vec<(u8, Result<(u16, u16), FSError>)>;
    let log: Rc<RefCell<DateTimeLog>> = Rc::new(RefCell::new(Vec::new()));
    let log_cb = log.clone();
    client
        .on_file_date_time_response
        .subscribe(move |&(tan, result)| log_cb.borrow_mut().push((tan, result)));
    let error_log: Rc<RefCell<Vec<FSError>>> = Rc::new(RefCell::new(Vec::new()));
    let error_log_cb = error_log.clone();
    client
        .on_error
        .subscribe(move |&error| error_log_cb.borrow_mut().push(error));

    let request = client.get_file_date_time("log.txt").unwrap();
    assert_eq!(request.data[0], FSFunction::GetFileDateTime.as_u8());
    assert_eq!(
        u16::from_le_bytes([request.data[2], request.data[3]]),
        "log.txt".len() as u16
    );
    let tan = request.data[1];
    let mut malformed_success = vec![0xFFu8; 8];
    malformed_success[0] = FSFunction::GetFileDateTime.as_u8();
    malformed_success[1] = tan;
    malformed_success[2] = FSError::Success.as_u8();
    malformed_success[7] = 0x00;
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        malformed_success,
        0x80,
    ));
    assert_eq!(
        log.borrow().as_slice(),
        &[(tan, Err(FSError::MalformedRequest))],
        "malformed fixed responses must be reported without synthesizing a date/time"
    );
    assert!(
        error_log.borrow().is_empty(),
        "malformed responses must not be promoted into protocol error events"
    );

    let request = client.get_file_date_time("log.txt").unwrap();
    let tan = request.data[1];
    let reserved_error_response = vec![FSFunction::GetFileDateTime.as_u8(), tan, 0x7F];
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        reserved_error_response,
        0x80,
    ));
    assert_eq!(
        log.borrow().last(),
        Some(&(tan, Err(FSError::MalformedRequest))),
        "reserved File Server error bytes must be rejected as malformed responses"
    );
    assert!(
        error_log.borrow().is_empty(),
        "reserved error bytes must not be normalized into OtherError"
    );

    let request = client.get_file_date_time("log.txt").unwrap();
    let tan = request.data[1];
    let invalid_month_date = (45u16 << 9) | 1;
    let mut invalid_date_response = vec![0xFFu8; 8];
    invalid_date_response[0] = FSFunction::GetFileDateTime.as_u8();
    invalid_date_response[1] = tan;
    invalid_date_response[2] = FSError::Success.as_u8();
    invalid_date_response[3..5].copy_from_slice(&invalid_month_date.to_le_bytes());
    invalid_date_response[5..7].copy_from_slice(&pack_dos_time(12, 0, 0).to_le_bytes());
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        invalid_date_response,
        0x80,
    ));
    assert_eq!(
        log.borrow().last(),
        Some(&(tan, Err(FSError::MalformedRequest))),
        "out-of-range DOS date fields must not be surfaced as successful date/time results"
    );
    assert!(
        error_log.borrow().is_empty(),
        "malformed date/time fields must not be promoted into protocol error events"
    );

    let request = client.get_file_date_time("log.txt").unwrap();
    let tan = request.data[1];
    let date = pack_dos_date(2025, 1, 1);
    let time = pack_dos_time(12, 0, 0);
    let mut response = vec![0xFFu8; 8];
    response[0] = FSFunction::GetFileDateTime.as_u8();
    response[1] = tan;
    response[2] = FSError::Success.as_u8();
    response[3..5].copy_from_slice(&date.to_le_bytes());
    response[5..7].copy_from_slice(&time.to_le_bytes());
    client.handle_server_response(&Message::new(PGN_FILE_SERVER_TO_CLIENT, response, 0x80));
    assert_eq!(
        log.borrow().last(),
        Some(&(tan, Ok((date, time)))),
        "valid GetFileDateTime responses must surface the received date/time fields"
    );
}
