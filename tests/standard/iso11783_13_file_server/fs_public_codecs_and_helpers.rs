use machbus::isobus::file_transfer::{FileOperation, FileTransferError};
use machbus::isobus::fs::{
    FS_CLASSIC_PROPERTIES_VERSION, FS_SUPPORTED_COUNT_MAX, FS_V2_PROPERTIES_VERSION, FSError,
    FSFunction, FileAttributes, FileClient, FileClientConfig, FileServer, FileServerConfig,
    FileServerProperties, FileServerPropertiesV2, FileServerStatus, INVALID_FILE_HANDLE,
    INVALID_TAN, OpenFlags, RESERVED_FILE_HANDLE_0, VolumeState, VolumeStateV2, VolumeStatus,
    is_valid_fs_path, pack_dos_date, pack_dos_time,
};
use machbus::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use machbus::net::pgn_defs::{PGN_FILE_CLIENT_TO_SERVER, PGN_FILE_SERVER_TO_CLIENT, PGN_REQUEST};
use machbus::net::{ErrorCode, Message, Priority};
use std::cell::RefCell;
use std::rc::Rc;

type ClientAttributeLog = Rc<RefCell<Vec<(u8, Result<u8, FSError>)>>>;

#[test]
fn file_server_public_error_decoder_rejects_noncanonical_bytes() {
    let valid = [
        FSError::Success,
        FSError::AccessDenied,
        FSError::InvalidAccess,
        FSError::TooManyOpen,
        FSError::NotFound,
        FSError::WrongType,
        FSError::MaxHandles,
        FSError::InvalidHandle,
        FSError::InvalidSourceName,
        FSError::InvalidDestName,
        FSError::NoSpace,
        FSError::WriteFail,
        FSError::MediaNotPresent,
        FSError::NotInitialized,
        FSError::NotSupported,
        FSError::InvalidLength,
        FSError::OutOfMemory,
        FSError::OtherError,
        FSError::EndOfFile,
        FSError::TANError,
        FSError::MalformedRequest,
    ];
    for error in valid {
        assert_eq!(FSError::try_from_u8(error.as_u8()), Some(error));
    }
    for raw in [14, 19, 21, 41, 48, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(FSError::try_from_u8(raw), None);
        assert_eq!(
            FSError::from_u8(raw),
            FSError::OtherError,
            "legacy lossy decoder may only be used after strict wire validation"
        );
    }
}

#[test]
fn file_server_legacy_transfer_error_decoder_rejects_noncanonical_bytes() {
    let valid = [
        FileTransferError::NoError,
        FileTransferError::FileNotFound,
        FileTransferError::AccessDenied,
        FileTransferError::DiskFull,
        FileTransferError::InvalidFilename,
        FileTransferError::ServerBusy,
        FileTransferError::InvalidHandle,
        FileTransferError::EndOfFile,
        FileTransferError::VolumeNotMounted,
        FileTransferError::IoError,
        FileTransferError::InvalidSeekPosition,
        FileTransferError::InvalidParameter,
        FileTransferError::FileAlreadyOpen,
        FileTransferError::DirectoryNotEmpty,
        FileTransferError::Unknown,
    ];
    for error in valid {
        assert_eq!(FileTransferError::try_from_u8(error.as_u8()), Some(error));
    }
    for raw in [0x0E, 0x0F, 0x10, 0x7F, 0x80, 0xFE] {
        assert_eq!(FileTransferError::try_from_u8(raw), None);
    }
}

#[test]
fn file_server_legacy_operation_decoder_rejects_noncanonical_bytes() {
    let valid = [
        FileOperation::Read,
        FileOperation::Write,
        FileOperation::Delete,
        FileOperation::List,
        FileOperation::GetAttributes,
        FileOperation::SetAttributes,
        FileOperation::OpenFile,
        FileOperation::CloseFile,
        FileOperation::ReadData,
        FileOperation::WriteData,
        FileOperation::SeekFile,
        FileOperation::GetCurrentDir,
        FileOperation::ChangeCurrentDir,
        FileOperation::MakeDir,
        FileOperation::RemoveDir,
        FileOperation::MoveFile,
        FileOperation::CopyFile,
        FileOperation::GetFileSize,
        FileOperation::GetFreeSpace,
        FileOperation::GetVolumeInfo,
        FileOperation::GetServerStatus,
    ];
    for operation in valid {
        assert_eq!(
            FileOperation::try_from_u8(operation.as_u8()),
            Some(operation)
        );
        assert_eq!(FileOperation::from_u8(operation.as_u8()), Some(operation));
    }
    for raw in [0x00, 0x07, 0x0F, 0x15, 0x24, 0x32, 0x42, 0x51, 0x61, 0xFE] {
        assert_eq!(
            FileOperation::try_from_u8(raw),
            None,
            "legacy file-operation public decoder must reject reserved operation bytes"
        );
        assert_eq!(FileOperation::from_u8(raw), None);
    }
}

fn fs_request(data: Vec<u8>, source: u8) -> Message {
    Message::new(PGN_FILE_CLIENT_TO_SERVER, data, source)
}

fn open_request(tan: u8, path: &str, flags: u8) -> Vec<u8> {
    let mut request = vec![FSFunction::OpenFile.as_u8(), tan, path.len() as u8, flags];
    request.extend_from_slice(path.as_bytes());
    request
}

fn read_request(tan: u8, handle: u8, count: u16) -> Vec<u8> {
    let mut request = vec![0xFF; 8];
    request[0] = FSFunction::ReadFile.as_u8();
    request[1] = tan;
    request[2] = handle;
    request[3..5].copy_from_slice(&count.to_le_bytes());
    request
}

fn read_dir_request(tan: u8, handle: u8, count: u16, report_hidden: u8) -> Vec<u8> {
    let mut request = read_request(tan, handle, count);
    request[5] = report_hidden;
    request
}

fn write_request(tan: u8, handle: u8, data: &[u8]) -> Vec<u8> {
    let mut request = vec![FSFunction::WriteFile.as_u8(), tan, handle];
    request.extend_from_slice(&(data.len() as u16).to_le_bytes());
    request.extend_from_slice(data);
    request
}

fn fs_count(data: &[u8]) -> u16 {
    u16::from_le_bytes([data[3], data[4]])
}

fn fs_date_time(data: &[u8]) -> (u16, u16) {
    (
        u16::from_le_bytes([data[3], data[4]]),
        u16::from_le_bytes([data[5], data[6]]),
    )
}

fn directory_entry_names(data: &[u8]) -> Vec<String> {
    let mut names = Vec::new();
    let mut offset = 5;
    while offset < data.len() {
        let Some(&name_len) = data.get(offset) else {
            break;
        };
        let name_len = name_len as usize;
        let name_start = offset + 1;
        let name_end = name_start + name_len;
        if name_len == 0 || name_end > data.len() {
            break;
        }
        names.push(String::from_utf8_lossy(&data[name_start..name_end]).into_owned());
        offset = name_end + 1 + 2 + 2 + 4;
    }
    names
}

fn directory_entry_date_time(data: &[u8], expected_name: &str) -> Option<(u16, u16)> {
    let mut offset = 5;
    while offset < data.len() {
        let name_len = *data.get(offset)? as usize;
        let name_start = offset + 1;
        let name_end = name_start + name_len;
        let attrs_index = name_end;
        let date_index = attrs_index + 1;
        let time_index = date_index + 2;
        let size_index = time_index + 2;
        if name_len == 0 || size_index + 4 > data.len() {
            return None;
        }
        let name = String::from_utf8_lossy(&data[name_start..name_end]);
        if name == expected_name {
            return Some((
                u16::from_le_bytes([data[date_index], data[date_index + 1]]),
                u16::from_le_bytes([data[time_index], data[time_index + 1]]),
            ));
        }
        offset = size_index + 4;
    }
    None
}

fn date_time_request(tan: u8, path: &str) -> Vec<u8> {
    let mut request = vec![FSFunction::GetFileDateTime.as_u8(), tan];
    request.extend_from_slice(&(path.len() as u16).to_le_bytes());
    request.extend_from_slice(path.as_bytes());
    request
}

fn volume_status_request(tan: u8, mode: u8, volume_name: &str) -> Vec<u8> {
    let mut request = vec![FSFunction::VolumeStatus.as_u8(), tan, mode];
    request.extend_from_slice(&(volume_name.len() as u16).to_le_bytes());
    request.extend_from_slice(volume_name.as_bytes());
    request
}

fn initialize_volume_request(tan: u8, space: u32, flags: u8, volume_name: &str) -> Vec<u8> {
    let mut request = vec![FSFunction::InitializeVolume.as_u8(), tan];
    request.extend_from_slice(&space.to_le_bytes());
    request.push(flags);
    request.extend_from_slice(&(volume_name.len() as u16).to_le_bytes());
    request.extend_from_slice(volume_name.as_bytes());
    request
}

fn assert_response(data: &[u8], function: FSFunction, tan: u8, error: FSError) {
    assert_eq!(data[0], function.as_u8());
    assert_eq!(data[1], tan);
    assert_eq!(data[2], error.as_u8());
}

fn connect_file_client(client: &mut FileClient, server: u8) {
    connect_file_client_with_properties(client, server, FileServerProperties::default());
}

fn connect_file_client_with_properties(
    client: &mut FileClient,
    server: u8,
    properties: FileServerProperties,
) {
    let request = client.connect_to_server(server).unwrap();
    let tan = request.data[1];
    let mut properties_response = vec![FSFunction::GetFileServerProperties.as_u8(), tan, 0x00];
    properties_response.extend_from_slice(&properties.encode());
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        properties_response,
        server,
    ));
    assert!(client.is_connected());
}

#[test]
fn file_server_and_client_reject_wrong_pgn_or_invalid_source_before_state_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    let open = open_request(0x01, "log.txt", OpenFlags::Read.bit());

    assert!(
        server
            .handle_client_message(&Message::new(PGN_REQUEST, open.clone(), 0x42))
            .is_empty()
    );
    assert!(server.clients().is_empty());

    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert!(
            server
                .handle_client_message(&Message::new(
                    PGN_FILE_CLIENT_TO_SERVER,
                    open.clone(),
                    source
                ))
                .is_empty()
        );
        assert!(
            server.clients().is_empty(),
            "unusable source addresses must not create File Server client state"
        );
    }
    assert!(
        server
            .handle_client_message(&Message::with_addressing(
                PGN_FILE_CLIENT_TO_SERVER,
                open.clone(),
                0x42,
                NULL_ADDRESS,
                Priority::Default,
            ))
            .is_empty()
    );
    assert!(
        server.clients().is_empty(),
        "null-destination requests must not create File Server client state"
    );

    let mut client = FileClient::new(FileClientConfig::default());
    let request = client.connect_to_server(0x80).unwrap();
    let tan = request.data[1];
    let mut properties_response = vec![FSFunction::GetFileServerProperties.as_u8(), tan, 0x00];
    properties_response.extend_from_slice(&FileServerProperties::default().encode());

    client.handle_server_response(&Message::new(
        PGN_REQUEST,
        properties_response.clone(),
        0x80,
    ));
    assert!(
        !client.is_connected(),
        "wrong-PGN response must not consume the pending properties request"
    );
    assert!(client.server_properties().is_none());

    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        client.handle_server_response(&Message::new(
            PGN_FILE_SERVER_TO_CLIENT,
            properties_response.clone(),
            source,
        ));
        assert!(
            !client.is_connected(),
            "invalid-source response must not consume the pending properties request"
        );
        assert!(client.server_properties().is_none());
    }

    client.handle_server_response(&Message::with_addressing(
        PGN_FILE_SERVER_TO_CLIENT,
        properties_response.clone(),
        0x80,
        NULL_ADDRESS,
        Priority::Default,
    ));
    assert!(
        !client.is_connected(),
        "null-destination response must not consume the pending properties request"
    );
    assert!(client.server_properties().is_none());

    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        properties_response,
        0x80,
    ));
    assert!(client.is_connected());
    assert!(client.server_properties().is_some());
}

#[test]
fn file_server_function_codes_reject_reserved_values() {
    for function in [
        FSFunction::GetCurrentDirectory,
        FSFunction::ChangeDirectory,
        FSFunction::OpenFile,
        FSFunction::SeekFile,
        FSFunction::ReadFile,
        FSFunction::WriteFile,
        FSFunction::CloseFile,
        FSFunction::MoveFile,
        FSFunction::DeleteFile,
        FSFunction::GetFileAttributes,
        FSFunction::SetFileAttributes,
        FSFunction::GetFileDateTime,
        FSFunction::MakeDirectory,
        FSFunction::RemoveDirectory,
        FSFunction::CopyFile,
        FSFunction::GetFileSize,
        FSFunction::GetFreeSpace,
        FSFunction::InitializeVolume,
        FSFunction::FileServerStatus,
        FSFunction::GetFileServerProperties,
        FSFunction::VolumeStatus,
    ] {
        assert_eq!(FSFunction::try_from_u8(function.as_u8()), Some(function));
        assert_eq!(FSFunction::from_u8(function.as_u8()), Some(function));
    }

    for raw in [0x07, 0x0F, 0x1A, 0x21, 0x32, 0x41, 0xFF] {
        assert_eq!(FSFunction::try_from_u8(raw), None);
        assert_eq!(FSFunction::from_u8(raw), None);
    }
    assert_eq!(INVALID_FILE_HANDLE, 0xFF);
    assert_eq!(RESERVED_FILE_HANDLE_0, 0x00);
}

#[test]
fn file_server_volume_state_decoders_reject_noncanonical_bytes() {
    for state in [
        VolumeState::Present,
        VolumeState::InUse,
        VolumeState::PreparingForRemoval,
        VolumeState::Removed,
    ] {
        assert_eq!(VolumeState::try_from_u8(state.as_u8()), Some(state));
        assert_eq!(VolumeState::from_u8(state.as_u8()), state);
    }
    for raw in [0x04, 0x05, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(VolumeState::try_from_u8(raw), None);
    }

    for state in [
        VolumeStateV2::Mounted,
        VolumeStateV2::NotMounted,
        VolumeStateV2::PrepareForRemoval,
        VolumeStateV2::Maintenance,
    ] {
        assert_eq!(VolumeStateV2::try_from_u8(state.as_u8()), Some(state));
        assert_eq!(VolumeStateV2::from_u8(state.as_u8()), state);
    }
    for raw in [0x04, 0x05, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(VolumeStateV2::try_from_u8(raw), None);
    }
}

#[test]
fn file_server_ccm_requires_canonical_keepalive_payload_before_connection() {
    let mut server = FileServer::new(FileServerConfig::default());

    let malformed_ccm = vec![0xFF, 0x20, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(
        server
            .handle_client_message(&fs_request(malformed_ccm, 0x42))
            .is_empty(),
        "malformed CCM keepalive frames must not get an error response"
    );
    assert!(
        server.clients().is_empty(),
        "malformed CCM keepalive frames must not establish a client connection"
    );

    assert!(
        server
            .handle_client_message(&fs_request(vec![0xFF, INVALID_TAN], 0x42))
            .is_empty(),
        "reserved TAN is legal only as the CCM sentinel path and remains silent"
    );
    assert!(
        server.clients().is_empty(),
        "reserved-TAN CCM must not create a connection"
    );

    let valid_ccm = vec![0xFF, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(
        server
            .handle_client_message(&fs_request(valid_ccm, 0x42))
            .is_empty()
    );
    assert_eq!(server.clients().len(), 1);
    assert_eq!(server.clients().get(&0x42).unwrap().client_address, 0x42);
}

#[test]
fn file_server_non_ccm_requests_do_not_suppress_first_ccm_connection_event() {
    let mut server = FileServer::new(FileServerConfig::default());
    let connected = Rc::new(RefCell::new(Vec::new()));
    let connected_log = Rc::clone(&connected);
    server
        .on_client_connected
        .subscribe(move |addr| connected_log.borrow_mut().push(*addr));

    let malformed_open = vec![
        FSFunction::OpenFile.as_u8(),
        0x10,
        1,
        OpenFlags::Read.bit(),
        b'a',
        0x00,
    ];
    let response = server.handle_client_message(&fs_request(malformed_open, 0x42));
    assert_response(
        &response[0].data,
        FSFunction::OpenFile,
        0x10,
        FSError::MalformedRequest,
    );
    assert!(
        connected.borrow().is_empty(),
        "non-CCM requests must not create a connected File Server client"
    );
    assert!(
        server
            .clients()
            .get(&0x42)
            .is_some_and(|client| !client.ccm_seen),
        "pre-CCM request bookkeeping must remain distinguishable from a live CCM connection"
    );

    let valid_ccm = vec![0xFF, 0x20, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
    assert!(
        server
            .handle_client_message(&fs_request(valid_ccm, 0x42))
            .is_empty()
    );
    assert_eq!(
        connected.borrow().as_slice(),
        &[0x42],
        "first canonical CCM after earlier request bookkeeping must still emit the connection event"
    );
    assert!(server.clients().get(&0x42).unwrap().ccm_seen);
}

#[test]
fn file_server_properties_and_paths_reject_malformed_inputs() {
    let properties = FileServerProperties::default();
    assert_eq!(
        FileServerProperties::decode(&properties.encode()),
        Some(properties)
    );

    let mut bad_reserved_caps = properties.encode();
    bad_reserved_caps[2] |= 0xE0;
    assert_eq!(FileServerProperties::decode(&bad_reserved_caps), None);

    let mut bad_tail = properties.encode();
    bad_tail[7] = 0;
    assert_eq!(FileServerProperties::decode(&bad_tail), None);
    let mut bad_count = properties.encode();
    bad_count[1] = FS_SUPPORTED_COUNT_MAX + 1;
    assert_eq!(FileServerProperties::decode(&bad_count), None);
    assert_eq!(VolumeState::try_from_u8(4), None);

    let mut bad_status = machbus::isobus::fs::FileServerStatus {
        busy: true,
        number_of_open_files: 1,
    }
    .encode();
    bad_status[0] |= 0xFE;
    assert_eq!(
        machbus::isobus::fs::FileServerStatus::decode(&bad_status),
        None
    );
    let mut bad_status_count = FileServerStatus {
        busy: true,
        number_of_open_files: FS_SUPPORTED_COUNT_MAX,
    }
    .encode();
    assert!(FileServerStatus::decode(&bad_status_count).is_some());
    bad_status_count[1] = FS_SUPPORTED_COUNT_MAX + 1;
    assert_eq!(FileServerStatus::decode(&bad_status_count), None);

    assert!(is_valid_fs_path("\\TASKDATA\\XML", true, false));
    assert!(!is_valid_fs_path("../TASKDATA", true, false));
    assert!(!is_valid_fs_path("\\TASKDATA\\..\\SECRET", true, false));
}

#[test]
fn file_server_property_count_ranges_are_validated_before_advertisement_or_connection() {
    let oversized_config = FileServerConfig::default()
        .with_max_files_per_client(u8::MAX)
        .with_max_files_total(u8::MAX);
    let server = FileServer::new(oversized_config);
    assert_eq!(
        server.get_properties().max_simultaneous_files,
        FS_SUPPORTED_COUNT_MAX,
        "server properties must not advertise reserved count values"
    );
    assert_eq!(
        server.get_properties().version_number,
        FS_CLASSIC_PROPERTIES_VERSION,
        "server properties must advertise the supported classic properties layout"
    );

    let mut oversized_classic = FileServerProperties::default().encode();
    oversized_classic[1] = FS_SUPPORTED_COUNT_MAX + 1;
    assert_eq!(FileServerProperties::decode(&oversized_classic), None);
    let mut unsupported_classic_version = FileServerProperties::default().encode();
    unsupported_classic_version[0] = FS_CLASSIC_PROPERTIES_VERSION + 1;
    assert_eq!(
        FileServerProperties::decode(&unsupported_classic_version),
        None
    );

    let mut client = FileClient::new(FileClientConfig::default());
    let request = client.connect_to_server(0x80).unwrap();
    let tan = request.data[1];
    let mut invalid_properties_response =
        vec![FSFunction::GetFileServerProperties.as_u8(), tan, 0x00];
    invalid_properties_response.extend_from_slice(&oversized_classic);
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        invalid_properties_response,
        0x80,
    ));
    assert!(
        !client.is_connected(),
        "invalid advertised property counts must not complete the FS client handshake"
    );
    assert!(client.server_properties().is_none());

    let mut v2 = FileServerPropertiesV2::default().encode();
    v2[2] = FS_SUPPORTED_COUNT_MAX + 1;
    assert_eq!(FileServerPropertiesV2::decode(&v2), None);
    let mut v2_clients = FileServerPropertiesV2::default().encode();
    v2_clients[4] = FS_SUPPORTED_COUNT_MAX + 1;
    assert_eq!(FileServerPropertiesV2::decode(&v2_clients), None);
    let mut unsupported_v2_version = FileServerPropertiesV2::default().encode();
    unsupported_v2_version[1] = FS_V2_PROPERTIES_VERSION + 1;
    assert_eq!(
        FileServerPropertiesV2::decode(&unsupported_v2_version),
        None
    );
}

#[test]
fn file_server_v2_properties_and_volume_status_reject_reserved_bits() {
    let properties = FileServerPropertiesV2 {
        version_number: 2,
        max_open_files: 16,
        supports_volumes: true,
        supports_long_filenames: true,
        max_simultaneous_clients: 4,
    };
    let property_bytes = properties.encode();
    assert_eq!(
        FileServerPropertiesV2::decode(&property_bytes),
        Some(properties)
    );

    for reserved_caps in [0x04, 0x08, 0x80, 0xFC] {
        let mut malformed_properties = property_bytes;
        malformed_properties[3] |= reserved_caps;
        assert_eq!(
            FileServerPropertiesV2::decode(&malformed_properties),
            None,
            "v2 file-server property capabilities must not accept reserved bits"
        );
    }

    let volume_status = VolumeStatus {
        name: "ISOFS".into(),
        state: VolumeStateV2::Mounted,
        total_bytes: 1_000_000,
        free_bytes: 500_000,
        removable: true,
    };
    let volume_bytes = volume_status.encode().unwrap();
    assert_eq!(
        VolumeStatus::decode(&volume_bytes),
        Some(volume_status.clone())
    );

    for reserved_removable_bits in [0x02, 0x04, 0x80, 0xFE] {
        let mut malformed_volume = volume_bytes.clone();
        malformed_volume[2] |= reserved_removable_bits;
        assert_eq!(
            VolumeStatus::decode(&malformed_volume),
            None,
            "volume-status removable byte must not accept reserved bits"
        );
    }

    for malformed_name in ["ISO/FS", "ISO\\FS", "ISO*FS"] {
        assert!(
            VolumeStatus {
                name: malformed_name.into(),
                ..volume_status.clone()
            }
            .encode()
            .is_err(),
            "v2 volume-status names must not encode invalid volume-label grammar"
        );

        let mut malformed_volume = volume_bytes.clone();
        malformed_volume[11] = malformed_name.len() as u8;
        malformed_volume.truncate(12);
        malformed_volume.extend_from_slice(malformed_name.as_bytes());
        assert_eq!(
            VolumeStatus::decode(&malformed_volume),
            None,
            "v2 volume-status names must reject invalid volume-label grammar"
        );
    }

    assert!(
        VolumeStatus {
            free_bytes: volume_status.total_bytes + 1,
            ..volume_status.clone()
        }
        .encode()
        .is_err(),
        "v2 volume-status must not encode impossible capacity accounting"
    );
    let mut impossible_capacity = volume_bytes.clone();
    impossible_capacity[7..11].copy_from_slice(&(volume_status.total_bytes + 1).to_le_bytes());
    assert_eq!(
        VolumeStatus::decode(&impossible_capacity),
        None,
        "v2 volume-status must reject free space greater than total space"
    );

    for valid_boundary in [
        VolumeStatus {
            total_bytes: 0,
            free_bytes: 0,
            ..volume_status.clone()
        },
        VolumeStatus {
            name: "FIELD".into(),
            total_bytes: 1,
            free_bytes: 1,
            ..volume_status.clone()
        },
    ] {
        let encoded = valid_boundary.encode().unwrap();
        assert_eq!(
            VolumeStatus::decode(&encoded),
            Some(valid_boundary),
            "v2 volume-status accepts free space equal to total space"
        );
    }

    let mut non_ascii_name = volume_bytes;
    non_ascii_name[11] = 1;
    non_ascii_name.truncate(13);
    non_ascii_name[12] = 0x80;
    assert_eq!(
        VolumeStatus::decode(&non_ascii_name),
        None,
        "v2 volume-status names must reject non-ASCII bytes"
    );
}

#[test]
fn file_server_operation_cycle_preserves_tan_and_owner_scoped_handles() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("log.txt", b"abc".to_vec(), 0).unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x10, "log.txt", OpenFlags::ReadWrite.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x10, FSError::Success);
    let handle = open[0].data[3];
    assert_ne!(handle, INVALID_FILE_HANDLE);

    let other_client_read =
        server.handle_client_message(&fs_request(read_request(0x11, handle, 1), 0x43));
    assert_response(
        &other_client_read[0].data,
        FSFunction::ReadFile,
        0x11,
        FSError::InvalidHandle,
    );

    let write = server.handle_client_message(&fs_request(write_request(0x12, handle, b"XY"), 0x42));
    assert_response(
        &write[0].data,
        FSFunction::WriteFile,
        0x12,
        FSError::Success,
    );
    assert_eq!(fs_count(&write[0].data), 2);

    let seek = server.handle_client_message(&fs_request(
        vec![FSFunction::SeekFile.as_u8(), 0x13, handle, 0, 0, 0, 0],
        0x42,
    ));
    assert_response(&seek[0].data, FSFunction::SeekFile, 0x13, FSError::Success);

    let read = server.handle_client_message(&fs_request(read_request(0x14, handle, 3), 0x42));
    assert_response(&read[0].data, FSFunction::ReadFile, 0x14, FSError::Success);
    assert_eq!(fs_count(&read[0].data), 3);
    assert_eq!(&read[0].data[5..8], b"XYc");

    let status = server.handle_client_message(&fs_request(
        vec![FSFunction::FileServerStatus.as_u8(), 0x15],
        0x42,
    ));
    assert_response(
        &status[0].data,
        FSFunction::FileServerStatus,
        0x15,
        FSError::Success,
    );
    assert_eq!(status[0].data[4], 1, "one owner-scoped handle remains open");
}

#[test]
fn file_server_rejects_reserved_handles_for_file_operations_without_state_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("reserved.txt", b"abc".to_vec(), 0).unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x18, "reserved.txt", OpenFlags::ReadWrite.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x18, FSError::Success);
    let valid_handle = open[0].data[3];
    assert_ne!(valid_handle, RESERVED_FILE_HANDLE_0);
    assert_ne!(valid_handle, INVALID_FILE_HANDLE);

    let write =
        server.handle_client_message(&fs_request(write_request(0x19, valid_handle, b"XY"), 0x42));
    assert_response(
        &write[0].data,
        FSFunction::WriteFile,
        0x19,
        FSError::Success,
    );

    for (base_tan, reserved_handle) in [(0x1A, RESERVED_FILE_HANDLE_0), (0x20, INVALID_FILE_HANDLE)]
    {
        let read = server.handle_client_message(&fs_request(
            read_request(base_tan, reserved_handle, 1),
            0x42,
        ));
        assert_response(
            &read[0].data,
            FSFunction::ReadFile,
            base_tan,
            FSError::InvalidHandle,
        );

        let seek = server.handle_client_message(&fs_request(
            vec![
                FSFunction::SeekFile.as_u8(),
                base_tan + 1,
                reserved_handle,
                0,
                0,
                0,
                0,
            ],
            0x42,
        ));
        assert_response(
            &seek[0].data,
            FSFunction::SeekFile,
            base_tan + 1,
            FSError::InvalidHandle,
        );

        let write = server.handle_client_message(&fs_request(
            write_request(base_tan + 2, reserved_handle, b"bad"),
            0x42,
        ));
        assert_response(
            &write[0].data,
            FSFunction::WriteFile,
            base_tan + 2,
            FSError::InvalidHandle,
        );

        let close = server.handle_client_message(&fs_request(
            vec![FSFunction::CloseFile.as_u8(), base_tan + 3, reserved_handle],
            0x42,
        ));
        assert_response(
            &close[0].data,
            FSFunction::CloseFile,
            base_tan + 3,
            FSError::InvalidHandle,
        );
    }

    assert_eq!(
        server.open_files().len(),
        1,
        "reserved handles must not close or allocate any owner-scoped handle"
    );
    assert_eq!(server.open_files()[0].handle, valid_handle);

    let rewind = server.handle_client_message(&fs_request(
        vec![FSFunction::SeekFile.as_u8(), 0x26, valid_handle, 0, 0, 0, 0],
        0x42,
    ));
    assert_response(
        &rewind[0].data,
        FSFunction::SeekFile,
        0x26,
        FSError::Success,
    );

    let read_back =
        server.handle_client_message(&fs_request(read_request(0x27, valid_handle, 5), 0x42));
    assert_response(
        &read_back[0].data,
        FSFunction::ReadFile,
        0x27,
        FSError::Success,
    );
    assert_eq!(fs_count(&read_back[0].data), 3);
    assert_eq!(
        &read_back[0].data[5..8],
        b"XYc",
        "reserved-handle write attempts must not alter file contents"
    );
}

#[test]
fn file_server_replays_same_tan_without_reexecuting_side_effects() {
    let mut server = FileServer::new(FileServerConfig {
        tan_cache_timeout_ms: 10,
        ..FileServerConfig::default()
    });
    server.add_file("same.txt", b"abc".to_vec(), 0).unwrap();

    let request = fs_request(open_request(0x22, "same.txt", OpenFlags::Read.bit()), 0x42);
    let first = server.handle_client_message(&request);
    let replay = server.handle_client_message(&request);

    assert_response(&first[0].data, FSFunction::OpenFile, 0x22, FSError::Success);
    assert_eq!(replay[0].data, first[0].data);
    assert_eq!(
        server.open_files().len(),
        1,
        "replayed TAN must not allocate another handle"
    );

    server.update(11);
    let after_cache_expiry = server.handle_client_message(&request);
    assert_response(
        &after_cache_expiry[0].data,
        FSFunction::OpenFile,
        0x22,
        FSError::Success,
    );
    assert_ne!(after_cache_expiry[0].data[3], first[0].data[3]);
    assert_eq!(
        server.open_files().len(),
        2,
        "the same request may execute again only after the TAN cache expires"
    );
}

#[test]
fn file_server_replays_cached_tan_even_when_reused_for_different_operation() {
    let mut server = FileServer::new(FileServerConfig {
        tan_cache_timeout_ms: 10,
        ..FileServerConfig::default()
    });
    server.add_file("cached.txt", b"abc".to_vec(), 0).unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x24, "cached.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x24, FSError::Success);
    let handle = open[0].data[3];
    assert_eq!(server.open_files().len(), 1);

    let conflicting_close_same_tan = vec![
        FSFunction::CloseFile.as_u8(),
        0x24,
        handle,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
    ];
    let replay = server.handle_client_message(&fs_request(conflicting_close_same_tan, 0x42));
    assert_eq!(
        replay[0].data, open[0].data,
        "a live TAN cache entry must be replayed instead of executing a different operation"
    );
    assert_eq!(
        server.open_files().len(),
        1,
        "same-TAN operation mismatch must not close or mutate the cached open handle"
    );

    server.update(11);
    let close_after_expiry = server.handle_client_message(&fs_request(
        vec![
            FSFunction::CloseFile.as_u8(),
            0x24,
            handle,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x42,
    ));
    assert_response(
        &close_after_expiry[0].data,
        FSFunction::CloseFile,
        0x24,
        FSError::Success,
    );
    assert!(
        server.open_files().is_empty(),
        "after the TAN cache expires, the new operation may execute normally"
    );
}

#[test]
fn file_server_tan_cache_is_scoped_by_client_source_address() {
    let mut server = FileServer::new(FileServerConfig {
        tan_cache_timeout_ms: 10,
        ..FileServerConfig::default()
    });
    server.add_file("shared.txt", b"abc".to_vec(), 0).unwrap();

    let first = server.handle_client_message(&fs_request(
        open_request(0x26, "shared.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(&first[0].data, FSFunction::OpenFile, 0x26, FSError::Success);
    assert_eq!(first[0].dest, Some(0x42));
    assert_eq!(server.open_files().len(), 1);

    let second = server.handle_client_message(&fs_request(
        open_request(0x26, "shared.txt", OpenFlags::Read.bit()),
        0x43,
    ));
    assert_response(
        &second[0].data,
        FSFunction::OpenFile,
        0x26,
        FSError::Success,
    );
    assert_eq!(second[0].dest, Some(0x43));
    assert_ne!(
        second[0].data, first[0].data,
        "same TAN from a different client source must not replay another client's cached response"
    );
    assert_eq!(
        server.open_files().len(),
        2,
        "client-scoped TAN caches must allow independent clients to execute the same TAN"
    );
    assert_eq!(server.open_files()[0].owner, 0x42);
    assert_eq!(server.open_files()[1].owner, 0x43);

    let replay_second = server.handle_client_message(&fs_request(
        open_request(0x26, "shared.txt", OpenFlags::Read.bit()),
        0x43,
    ));
    assert_eq!(
        replay_second[0].data, second[0].data,
        "only the matching client's own TAN cache entry should replay"
    );
    assert_eq!(server.open_files().len(), 2);
}

#[test]
fn file_server_rejects_malformed_operation_lengths_without_file_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());

    let mut bad_create = open_request(0x30, "new.txt", OpenFlags::Write | OpenFlags::Create);
    bad_create.push(0x00);
    let bad_create_response = server.handle_client_message(&fs_request(bad_create, 0x42));
    assert_response(
        &bad_create_response[0].data,
        FSFunction::OpenFile,
        0x30,
        FSError::MalformedRequest,
    );

    let missing_after_bad_create = server.handle_client_message(&fs_request(
        open_request(0x31, "new.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &missing_after_bad_create[0].data,
        FSFunction::OpenFile,
        0x31,
        FSError::NotFound,
    );

    let created = server.handle_client_message(&fs_request(
        open_request(0x32, "new.txt", OpenFlags::ReadWrite | OpenFlags::Create),
        0x42,
    ));
    assert_response(
        &created[0].data,
        FSFunction::OpenFile,
        0x32,
        FSError::Success,
    );
    let handle = created[0].data[3];

    let bad_write = server.handle_client_message(&fs_request(
        vec![
            FSFunction::WriteFile.as_u8(),
            0x33,
            handle,
            1,
            0,
            b'X',
            0x00,
        ],
        0x42,
    ));
    assert_response(
        &bad_write[0].data,
        FSFunction::WriteFile,
        0x33,
        FSError::MalformedRequest,
    );

    let read_empty = server.handle_client_message(&fs_request(read_request(0x34, handle, 1), 0x42));
    assert_response(
        &read_empty[0].data,
        FSFunction::ReadFile,
        0x34,
        FSError::EndOfFile,
    );
}

#[test]
fn file_server_rejects_non_ascii_counted_paths_without_file_or_directory_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_directory("logs").unwrap();

    let open_non_ascii = server.handle_client_message(&fs_request(
        vec![
            FSFunction::OpenFile.as_u8(),
            0x36,
            2,
            OpenFlags::ReadWrite | OpenFlags::Create,
            0xC3,
            0xBF,
        ],
        0x42,
    ));
    assert_response(
        &open_non_ascii[0].data,
        FSFunction::OpenFile,
        0x36,
        FSError::InvalidSourceName,
    );
    assert!(
        server.open_files().is_empty(),
        "non-ASCII OpenFile path bytes must not create files or handles"
    );

    let change_logs = server.handle_client_message(&fs_request(
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            0x37,
            4,
            b'l',
            b'o',
            b'g',
            b's',
        ],
        0x42,
    ));
    assert_response(
        &change_logs[0].data,
        FSFunction::ChangeDirectory,
        0x37,
        FSError::Success,
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\logs\\"
    );

    let change_non_ascii = server.handle_client_message(&fs_request(
        vec![FSFunction::ChangeDirectory.as_u8(), 0x38, 2, 0xC3, 0xBF],
        0x42,
    ));
    assert_response(
        &change_non_ascii[0].data,
        FSFunction::ChangeDirectory,
        0x38,
        FSError::InvalidSourceName,
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\logs\\",
        "rejected ChangeDirectory path bytes must not mutate current directory"
    );

    let delete_non_ascii = server.handle_client_message(&fs_request(
        vec![FSFunction::DeleteFile.as_u8(), 0x39, 2, 0xC3, 0xBF],
        0x42,
    ));
    assert_response(
        &delete_non_ascii[0].data,
        FSFunction::DeleteFile,
        0x39,
        FSError::MalformedRequest,
    );

    let date_time_non_ascii = server.handle_client_message(&fs_request(
        vec![FSFunction::GetFileDateTime.as_u8(), 0x3A, 2, 0, 0xC3, 0xBF],
        0x42,
    ));
    assert_response(
        &date_time_non_ascii[0].data,
        FSFunction::GetFileDateTime,
        0x3A,
        FSError::MalformedRequest,
    );
}

#[test]
fn file_server_rejects_read_from_write_only_handle_without_position_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("write.txt", b"seed".to_vec(), 0).unwrap();

    let open = server.handle_client_message(&fs_request(
        open_request(0x40, "write.txt", OpenFlags::Write.bit()),
        0x42,
    ));
    assert_response(&open[0].data, FSFunction::OpenFile, 0x40, FSError::Success);
    let handle = open[0].data[3];
    assert_eq!(server.open_files()[0].position, 0);

    let read = server.handle_client_message(&fs_request(read_request(0x41, handle, 2), 0x42));
    assert_response(
        &read[0].data,
        FSFunction::ReadFile,
        0x41,
        FSError::InvalidAccess,
    );
    assert_eq!(
        server.open_files()[0].position,
        0,
        "rejected ReadFile on a write-only handle must not advance position"
    );

    let write = server.handle_client_message(&fs_request(write_request(0x42, handle, b"OK"), 0x42));
    assert_response(
        &write[0].data,
        FSFunction::WriteFile,
        0x42,
        FSError::Success,
    );
    assert_eq!(server.open_files()[0].position, 2);
}

#[test]
fn file_server_applies_append_and_exclusive_open_semantics() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("append.txt", b"seed".to_vec(), 0).unwrap();

    let exclusive_existing = server.handle_client_message(&fs_request(
        open_request(
            0x50,
            "append.txt",
            OpenFlags::Write.bit() | OpenFlags::Create.bit() | OpenFlags::Exclusive.bit(),
        ),
        0x42,
    ));
    assert_response(
        &exclusive_existing[0].data,
        FSFunction::OpenFile,
        0x50,
        FSError::AccessDenied,
    );
    assert!(server.open_files().is_empty());

    let invalid_append_read = server.handle_client_message(&fs_request(
        open_request(0x51, "append.txt", OpenFlags::Read | OpenFlags::Append),
        0x42,
    ));
    assert_response(
        &invalid_append_read[0].data,
        FSFunction::OpenFile,
        0x51,
        FSError::InvalidAccess,
    );
    assert!(server.open_files().is_empty());

    let append = server.handle_client_message(&fs_request(
        open_request(0x52, "append.txt", OpenFlags::ReadWrite | OpenFlags::Append),
        0x42,
    ));
    assert_response(
        &append[0].data,
        FSFunction::OpenFile,
        0x52,
        FSError::Success,
    );
    let handle = append[0].data[3];
    assert_eq!(
        server.open_files()[0].position,
        4,
        "append open starts at the existing end of file"
    );

    let write = server.handle_client_message(&fs_request(write_request(0x53, handle, b"++"), 0x42));
    assert_response(
        &write[0].data,
        FSFunction::WriteFile,
        0x53,
        FSError::Success,
    );

    let seek = server.handle_client_message(&fs_request(
        vec![FSFunction::SeekFile.as_u8(), 0x54, handle, 0, 0, 0, 0],
        0x42,
    ));
    assert_response(&seek[0].data, FSFunction::SeekFile, 0x54, FSError::Success);

    let read = server.handle_client_message(&fs_request(read_request(0x55, handle, 6), 0x42));
    assert_response(&read[0].data, FSFunction::ReadFile, 0x55, FSError::Success);
    assert_eq!(fs_count(&read[0].data), 6);
    assert_eq!(&read[0].data[5..11], b"seed++");
}

#[test]
fn file_server_rejects_conflicting_write_opens_without_handle_allocation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("shared.txt", b"abc".to_vec(), 0).unwrap();

    let reader_a = server.handle_client_message(&fs_request(
        open_request(0x56, "shared.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &reader_a[0].data,
        FSFunction::OpenFile,
        0x56,
        FSError::Success,
    );
    let reader_b = server.handle_client_message(&fs_request(
        open_request(0x57, "shared.txt", OpenFlags::Read.bit()),
        0x43,
    ));
    assert_response(
        &reader_b[0].data,
        FSFunction::OpenFile,
        0x57,
        FSError::Success,
    );
    assert_eq!(
        server.open_files().len(),
        2,
        "multiple read-only opens of the same file remain shareable"
    );

    let denied_writer = server.handle_client_message(&fs_request(
        open_request(0x58, "shared.txt", OpenFlags::Write.bit()),
        0x44,
    ));
    assert_response(
        &denied_writer[0].data,
        FSFunction::OpenFile,
        0x58,
        FSError::AccessDenied,
    );
    assert_eq!(
        server.open_files().len(),
        2,
        "conflicting write open must not allocate a handle"
    );

    let close_a = server.handle_client_message(&fs_request(
        vec![FSFunction::CloseFile.as_u8(), 0x59, reader_a[0].data[3]],
        0x42,
    ));
    assert_response(
        &close_a[0].data,
        FSFunction::CloseFile,
        0x59,
        FSError::Success,
    );
    let close_b = server.handle_client_message(&fs_request(
        vec![FSFunction::CloseFile.as_u8(), 0x5A, reader_b[0].data[3]],
        0x43,
    ));
    assert_response(
        &close_b[0].data,
        FSFunction::CloseFile,
        0x5A,
        FSError::Success,
    );

    let writer = server.handle_client_message(&fs_request(
        open_request(0x5B, "shared.txt", OpenFlags::ReadWrite.bit()),
        0x44,
    ));
    assert_response(
        &writer[0].data,
        FSFunction::OpenFile,
        0x5B,
        FSError::Success,
    );
    let denied_reader = server.handle_client_message(&fs_request(
        open_request(0x5C, "shared.txt", OpenFlags::Read.bit()),
        0x42,
    ));
    assert_response(
        &denied_reader[0].data,
        FSFunction::OpenFile,
        0x5C,
        FSError::AccessDenied,
    );
}

#[test]
fn file_server_and_client_reject_reserved_open_flags_before_state_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_file("flags.txt", b"abc".to_vec(), 0).unwrap();

    for (offset, reserved) in [0x20, 0x40, 0x80, 0xE0].into_iter().enumerate() {
        let tan = 0x58 + offset as u8;
        let response = server.handle_client_message(&fs_request(
            open_request(tan, "flags.txt", OpenFlags::Read.bit() | reserved),
            0x42,
        ));
        assert_response(
            &response[0].data,
            FSFunction::OpenFile,
            tan,
            FSError::InvalidAccess,
        );
        assert!(
            server.open_files().is_empty(),
            "reserved OpenFile flag bits must not allocate handles"
        );
        assert_eq!(
            server.clients().get(&0x42).unwrap().open_handles.len(),
            0,
            "reserved OpenFile flag bits must not mutate per-client handle state"
        );
    }

    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);
    let err = client
        .try_open_file("flags.txt", OpenFlags::Read.bit() | 0x80)
        .expect_err("reserved client OpenFile flag bit must be rejected locally");
    assert_eq!(err.code, ErrorCode::InvalidData);
    assert!(client.open_files().is_empty());
}

#[test]
fn file_server_directory_capability_blocks_directory_operations_without_state_mutation() {
    let mut server = FileServer::new(FileServerConfig::default());
    server.add_directory("logs").unwrap();

    let change_logs = server.handle_client_message(&fs_request(
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            0x60,
            4,
            b'l',
            b'o',
            b'g',
            b's',
        ],
        0x42,
    ));
    assert_response(
        &change_logs[0].data,
        FSFunction::ChangeDirectory,
        0x60,
        FSError::Success,
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\logs\\"
    );

    let mut properties = server.get_properties();
    properties.supports_directories = false;
    server.set_properties(properties);

    let cwd = server.handle_client_message(&fs_request(
        vec![
            FSFunction::GetCurrentDirectory.as_u8(),
            0x61,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x42,
    ));
    assert_response(
        &cwd[0].data,
        FSFunction::GetCurrentDirectory,
        0x61,
        FSError::NotSupported,
    );

    let change_root = server.handle_client_message(&fs_request(
        vec![FSFunction::ChangeDirectory.as_u8(), 0x62, 1, b'\\'],
        0x42,
    ));
    assert_response(
        &change_root[0].data,
        FSFunction::ChangeDirectory,
        0x62,
        FSError::NotSupported,
    );
    assert_eq!(
        server.clients().get(&0x42).unwrap().current_directory,
        "\\logs\\",
        "unsupported ChangeDirectory must not mutate server-side current directory"
    );

    let open_dir = server.handle_client_message(&fs_request(
        open_request(0x63, "logs", OpenFlags::OpenDir.bit()),
        0x42,
    ));
    assert_response(
        &open_dir[0].data,
        FSFunction::OpenFile,
        0x63,
        FSError::NotSupported,
    );
    assert!(
        server.open_files().is_empty(),
        "unsupported directory-open must not allocate a handle"
    );
}

#[test]
fn file_client_rejects_unencodable_requests_before_transport() {
    let mut client = FileClient::new(FileClientConfig::default());
    let too_long = "x".repeat(usize::from(u8::MAX) + 1);

    for server in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let err = client
            .try_connect_to_server(server)
            .expect_err("unusable file-server addresses must be rejected locally");
        assert_eq!(err.code, ErrorCode::InvalidData);
        assert!(!client.is_connected());
        assert!(client.server_properties().is_none());
    }

    let disconnected_open = client
        .try_open_file("log.txt", OpenFlags::Read.bit())
        .expect_err("file open needs an established file-server connection");
    assert_eq!(disconnected_open.code, ErrorCode::NotConnected);

    let disconnected_directory = client
        .try_get_current_directory()
        .expect_err("directory query needs an established file-server connection");
    assert_eq!(disconnected_directory.code, ErrorCode::NotConnected);

    connect_file_client(&mut client, 0x80);

    let too_long_err = client
        .try_open_file(&too_long, OpenFlags::Read.bit())
        .expect_err("one-byte path length overflow must be rejected locally");
    assert_eq!(too_long_err.code, ErrorCode::InvalidData);

    let traversal_err = client
        .try_open_file("..\\secret.txt", OpenFlags::Read.bit())
        .expect_err("path traversal must be rejected locally");
    assert_eq!(traversal_err.code, ErrorCode::InvalidData);

    assert!(client.open_files().is_empty());
}

#[test]
fn file_client_rejects_non_ascii_path_requests_before_transport() {
    let mut client = FileClient::new(FileClientConfig::default());
    connect_file_client(&mut client, 0x80);

    for err in [
        client
            .try_open_file("naïve.txt", OpenFlags::Read.bit())
            .unwrap_err(),
        client.try_change_directory("café").unwrap_err(),
        client.try_move_file("old.txt", "nü.txt").unwrap_err(),
        client.try_delete_file("résumé.txt").unwrap_err(),
        client.try_get_file_attributes("ångle.txt").unwrap_err(),
        client
            .try_set_file_attributes("måp.txt", FileAttributes::Archive.bit())
            .unwrap_err(),
        client.try_get_file_date_time("día.txt").unwrap_err(),
    ] {
        assert_eq!(
            err.code,
            ErrorCode::InvalidData,
            "non-ASCII path text cannot be encoded as one byte per wire character"
        );
    }

    let valid_open = client
        .try_open_file("plain.txt", OpenFlags::Read.bit())
        .expect("ASCII path requests remain encodable after rejected paths");
    assert_eq!(valid_open.data[0], FSFunction::OpenFile.as_u8());
    assert_eq!(&valid_open.data[4..], b"plain.txt");
}

#[test]
fn file_client_management_requests_validate_and_parse_standard_responses() {
    let mut client = FileClient::new(FileClientConfig::default());
    let disconnected = client
        .try_delete_file("old.txt")
        .expect_err("file management requests require a connected server");
    assert_eq!(disconnected.code, ErrorCode::NotConnected);

    connect_file_client(&mut client, 0x80);
    let too_long = "x".repeat(usize::from(u8::MAX) + 1);

    for err in [
        client.try_move_file(&too_long, "new.txt").unwrap_err(),
        client.try_delete_file(&too_long).unwrap_err(),
        client.try_get_file_attributes("bad:name").unwrap_err(),
        client
            .try_set_file_attributes("old.txt", FileAttributes::Directory.bit())
            .unwrap_err(),
        client.try_initialize_volume("bad\\name", 0, 0).unwrap_err(),
        client.try_initialize_volume("ISOBUS", 0, 0x80).unwrap_err(),
    ] {
        assert_eq!(err.code, ErrorCode::InvalidData);
    }

    let move_req = client
        .try_move_file("old.txt", "new.txt")
        .expect("valid one-byte counted MoveFile request");
    assert_eq!(move_req.data[0], FSFunction::MoveFile.as_u8());
    assert_eq!(move_req.data[2], 7);
    assert_eq!(move_req.data[3], 7);
    assert_eq!(&move_req.data[4..11], b"old.txt");
    assert_eq!(&move_req.data[11..18], b"new.txt");

    let attrs_log: ClientAttributeLog = Rc::new(RefCell::new(Vec::new()));
    let attrs_log_sub = attrs_log.clone();
    client
        .on_file_attributes_response
        .subscribe(move |(tan, result)| attrs_log_sub.borrow_mut().push((*tan, *result)));

    let bad_attrs = client
        .try_get_file_attributes("new.txt")
        .expect("valid GetFileAttributes request");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::GetFileAttributes.as_u8(),
            bad_attrs.data[1],
            FSError::Success.as_u8(),
            FileAttributes::Volume.bit(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(
        attrs_log.borrow().last().unwrap().1,
        Err(FSError::MalformedRequest),
        "reserved/unsupported attribute response bits must be rejected"
    );

    let good_attrs = client
        .try_get_file_attributes("new.txt")
        .expect("valid GetFileAttributes retry");
    let expected_attrs = FileAttributes::Hidden | FileAttributes::Archive;
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::GetFileAttributes.as_u8(),
            good_attrs.data[1],
            FSError::Success.as_u8(),
            expected_attrs,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(
        attrs_log.borrow().last().unwrap().1,
        Ok(expected_attrs),
        "valid file attribute response must preserve the supported bitfield"
    );

    let open = client.open_file("new.txt", OpenFlags::Read.bit()).unwrap();
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

    let cd = client.change_directory("logs").unwrap();
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::ChangeDirectory.as_u8(),
            cd.data[1],
            FSError::Success.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert_eq!(client.current_directory(), "\\logs\\");

    let init = client
        .try_initialize_volume("ISOBUS", 0x0010_0000, 0)
        .expect("valid counted InitializeVolume request");
    assert_eq!(init.data[0], FSFunction::InitializeVolume.as_u8());
    assert_eq!(&init.data[2..6], &0x0010_0000u32.to_le_bytes());
    assert_eq!(init.data[6], 0);
    assert_eq!(&init.data[7..9], &(6u16).to_le_bytes());
    assert_eq!(&init.data[9..15], b"ISOBUS");

    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::InitializeVolume.as_u8(),
            init.data[1],
            FSError::Success.as_u8(),
            0,
        ],
        0x80,
    ));
    assert!(
        client.open_files().contains_key(&7),
        "malformed InitializeVolume success must not clear tracked handles"
    );
    assert_eq!(
        client.current_directory(),
        "\\logs\\",
        "malformed InitializeVolume success must not reset current directory"
    );

    let init = client
        .try_initialize_volume("ISOBUS", 0x0010_0000, 0)
        .expect("valid counted InitializeVolume retry");
    client.handle_server_response(&Message::new(
        PGN_FILE_SERVER_TO_CLIENT,
        vec![
            FSFunction::InitializeVolume.as_u8(),
            init.data[1],
            FSError::Success.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));
    assert!(
        client.open_files().is_empty(),
        "successful InitializeVolume must clear media-dependent client handles"
    );
    assert_eq!(
        client.current_directory(),
        "\\",
        "successful InitializeVolume must reset the media-dependent current directory"
    );
}

