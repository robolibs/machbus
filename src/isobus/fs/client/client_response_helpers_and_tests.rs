fn decode_response_error(response: &[u8]) -> Result<FSError, FSError> {
    let Some(&raw_error) = response.get(2) else {
        return Err(FSError::MalformedRequest);
    };
    if !fs_error_byte_is_valid(raw_error) {
        return Err(FSError::MalformedRequest);
    }
    Ok(FSError::from_u8(raw_error))
}

fn is_valid_one_byte_file_path(path: &str) -> bool {
    path.is_ascii() && path.len() <= u8::MAX as usize && is_valid_fs_path(path, true, false)
}

fn resolve_client_directory_response_path(
    current_directory: &str,
    requested_path: &str,
) -> Option<String> {
    if requested_path == "." {
        return Some(current_directory.to_string());
    }
    if requested_path == ".." {
        return Some(parent_directory(current_directory));
    }
    if requested_path == "\\" {
        return Some("\\".to_string());
    }
    if requested_path.is_empty() || !is_valid_fs_path(requested_path, true, false) {
        return None;
    }

    let body = requested_path.trim_matches('\\');
    if body.is_empty() {
        return Some("\\".to_string());
    }
    if requested_path.starts_with('\\') {
        return Some(format!("\\{body}\\"));
    }
    if current_directory == "\\" {
        Some(format!("\\{body}\\"))
    } else {
        Some(format!("{}{}\\", current_directory, body))
    }
}

fn parent_directory(current_directory: &str) -> String {
    if current_directory == "\\" {
        return "\\".to_string();
    }
    let trimmed = current_directory.trim_end_matches('\\');
    if let Some(idx) = trimmed.rfind('\\') {
        current_directory[..idx + 1].to_string()
    } else {
        "\\".to_string()
    }
}

fn parse_volume_status_request_name(data: &[u8]) -> Option<String> {
    if data.len() < 5 || FSFunction::from_u8(data[0]) != Some(FSFunction::VolumeStatus) {
        return None;
    }
    let name_len = u16::from_le_bytes([data[3], data[4]]) as usize;
    let end = 5usize.checked_add(name_len)?;
    if !fs_payload_len_is_canonical(data, end) {
        return None;
    }
    let name_bytes = &data[5..end];
    if !name_bytes.is_ascii() {
        return None;
    }
    let name = core::str::from_utf8(name_bytes).ok()?;
    if !name.is_empty() && !is_valid_volume_name(name) {
        return None;
    }
    Some(name.to_string())
}

fn parse_volume_status_fields(data: &[u8]) -> Option<(VolumeState, Option<&str>)> {
    if data.len() < 4 || FSFunction::from_u8(data[0]) != Some(FSFunction::VolumeStatus) {
        return None;
    }
    let state = VolumeState::try_from_u8(data[2])?;
    if data[3] > 250 {
        return None;
    }
    if fs_payload_len_is_canonical(data, 4) {
        return Some((state, None));
    }
    if data.len() < 6 {
        return None;
    }
    let name_len = u16::from_le_bytes([data[4], data[5]]) as usize;
    if data.len() == 6 + name_len {
        if !data[6..].is_ascii() {
            return None;
        }
        let name = core::str::from_utf8(&data[6..]).ok()?;
        if !name.is_empty() && !is_valid_volume_name(name) {
            return None;
        }
        Some((state, Some(name)))
    } else {
        None
    }
}

impl Default for FileClient {
    fn default() -> Self {
        Self::new(FileClientConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::error::ErrorCode;
    use crate::net::pgn_defs::PGN_FILE_SERVER_TO_CLIENT;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn server_msg(data: Vec<u8>, src: Address) -> Message {
        Message::new(PGN_FILE_SERVER_TO_CLIENT, data, src)
    }

    #[test]
    fn connect_emits_get_properties_request() {
        let mut c = FileClient::new(FileClientConfig::default());
        let frame = c.connect_to_server(0x80).unwrap();
        assert_eq!(frame.dest, Some(0x80));
        assert_eq!(frame.data[0], FSFunction::GetFileServerProperties.as_u8());
        assert_eq!(c.state(), ClientState::WaitingForStatus);
    }

    #[test]
    fn fallible_connect_rejects_duplicate_without_restarting_fsm() {
        let mut c = FileClient::new(FileClientConfig::default());
        let first = c.try_connect_to_server(0x80).unwrap();
        let err = c.try_connect_to_server(0x81).unwrap_err();

        assert_eq!(err.code, ErrorCode::InvalidState);
        assert_eq!(c.server_address(), 0x80);
        assert_eq!(c.state(), ClientState::WaitingForStatus);
        assert_eq!(c.pending_requests.len(), 1);
        assert!(c.pending_requests.contains_key(&first.data[1]));
        assert!(c.connect_to_server(0x81).is_none());
    }

    #[test]
    fn properties_response_transitions_to_connected() {
        let mut c = FileClient::new(FileClientConfig::default());
        let req = c.connect_to_server(0x80).unwrap();
        let tan = req.data[1];
        // Build response: [func, tan, error=0, props bytes]
        let props = FileServerProperties::default();
        let mut response = vec![FSFunction::GetFileServerProperties.as_u8(), tan, 0];
        response.extend_from_slice(&props.encode());
        c.handle_server_response(&server_msg(response, 0x80));
        assert!(c.is_connected());
        assert!(c.server_properties().is_some());
    }

    #[test]
    fn server_version_is_exposed_for_negotiation() {
        let mut c = FileClient::new(FileClientConfig::default());
        // Before any properties response, the version is unknown.
        assert_eq!(c.server_version(), None);
        assert!(!c.server_supports_version(1));

        let req = c.connect_to_server(0x80).unwrap();
        let tan = req.data[1];
        let props = FileServerProperties {
            version_number: 3,
            ..FileServerProperties::default()
        };
        let mut response = vec![FSFunction::GetFileServerProperties.as_u8(), tan, 0];
        response.extend_from_slice(&props.encode());
        c.handle_server_response(&server_msg(response, 0x80));

        assert_eq!(c.server_version(), Some(3));
        assert!(c.server_supports_version(2));
        assert!(c.server_supports_version(3));
        assert!(!c.server_supports_version(4));
    }

    fn force_connected(c: &mut FileClient) {
        let req = c.connect_to_server(0x80).unwrap();
        let tan = req.data[1];
        let props = FileServerProperties::default();
        let mut response = vec![FSFunction::GetFileServerProperties.as_u8(), tan, 0];
        response.extend_from_slice(&props.encode());
        c.handle_server_response(&server_msg(response, 0x80));
        assert!(c.is_connected());
    }

    #[test]
    fn open_file_response_records_handle() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        type OpenLog = Vec<(TAN, Result<FileHandle, FSError>)>;
        let log: Rc<RefCell<OpenLog>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_open_response
            .subscribe(move |&(t, r)| lc.borrow_mut().push((t, r)));

        let frame = c.open_file("doc.txt", OpenFlags::Read.bit()).unwrap();
        let tan = frame.data[1];
        // Server replies with handle 7.
        let response = vec![FSFunction::OpenFile.as_u8(), tan, 0, 7];
        c.handle_server_response(&server_msg(response, 0x80));

        assert_eq!(*log.borrow(), vec![(tan, Ok(7))]);
        assert!(c.open_files().contains_key(&7));
        assert_eq!(c.open_files().get(&7).unwrap().path, "doc.txt");
    }

    #[test]
    fn open_file_error_propagates() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        type OpenLog = Vec<(TAN, Result<FileHandle, FSError>)>;
        let log: Rc<RefCell<OpenLog>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_open_response
            .subscribe(move |&(t, r)| lc.borrow_mut().push((t, r)));

        let frame = c.open_file("x.txt", OpenFlags::Read.bit()).unwrap();
        let tan = frame.data[1];
        // Server replies NotFound.
        let response = vec![
            FSFunction::OpenFile.as_u8(),
            tan,
            FSError::NotFound.as_u8(),
            0,
        ];
        c.handle_server_response(&server_msg(response, 0x80));

        assert_eq!(*log.borrow(), vec![(tan, Err(FSError::NotFound))]);
        assert!(c.open_files().is_empty());
    }

    #[test]
    fn malformed_counted_responses_do_not_update_client_state() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        c.open_files.insert(
            5,
            OpenFileInfo {
                handle: 5,
                path: "f".to_string(),
                ..Default::default()
            },
        );

        type ReadLog = Vec<Result<Vec<u8>, FSError>>;
        let read_log: Rc<RefCell<ReadLog>> = Rc::new(RefCell::new(Vec::new()));
        let read_log_cb = read_log.clone();
        c.on_read_response
            .subscribe(move |(_, r)| read_log_cb.borrow_mut().push(r.clone()));

        let read = c.read_file(5, 8).unwrap();
        let read_tan = read.data[1];
        c.handle_server_response(&server_msg(
            vec![
                FSFunction::ReadFile.as_u8(),
                read_tan,
                FSError::Success.as_u8(),
                4,
                b'a',
                b'b',
            ],
            0x80,
        ));
        assert_eq!(*read_log.borrow(), vec![Err(FSError::MalformedRequest)]);
        assert_eq!(c.open_files().get(&5).unwrap().position, 0);

        type DirLog = Vec<Result<String, FSError>>;
        let dir_log: Rc<RefCell<DirLog>> = Rc::new(RefCell::new(Vec::new()));
        let dir_log_cb = dir_log.clone();
        c.on_current_directory_response
            .subscribe(move |(_, r)| dir_log_cb.borrow_mut().push(r.clone()));

        let dir = c.get_current_directory().unwrap();
        let dir_tan = dir.data[1];
        c.handle_server_response(&server_msg(
            vec![
                FSFunction::GetCurrentDirectory.as_u8(),
                dir_tan,
                FSError::Success.as_u8(),
                3,
                b'\\',
            ],
            0x80,
        ));
        assert_eq!(*dir_log.borrow(), vec![Err(FSError::MalformedRequest)]);
        assert_eq!(c.current_directory(), "\\");
    }

    #[test]
    fn read_and_write_responses_saturate_local_file_position() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        c.open_files.insert(
            5,
            OpenFileInfo {
                handle: 5,
                path: "f".to_string(),
                position: u32::MAX - 1,
                ..Default::default()
            },
        );

        let read = c.read_file(5, 8).unwrap();
        let read_tan = read.data[1];
        c.handle_server_response(&server_msg(
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
        assert_eq!(c.open_files().get(&5).unwrap().position, u32::MAX);

        let write = c.write_file(5, b"abc").unwrap();
        let write_tan = write.data[1];
        c.handle_server_response(&server_msg(
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
        assert_eq!(c.open_files().get(&5).unwrap().position, u32::MAX);
    }

    #[test]
    fn malformed_status_and_mismatched_responses_are_ignored() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);

        let volume_log: Rc<RefCell<Vec<VolumeState>>> = Rc::new(RefCell::new(Vec::new()));
        let volume_log_cb = volume_log.clone();
        c.on_volume_status
            .subscribe(move |&v| volume_log_cb.borrow_mut().push(v));
        c.handle_server_response(&server_msg(
            vec![
                FSFunction::VolumeStatus.as_u8(),
                INVALID_TAN,
                VolumeState::Removed.as_u8(),
                0,
                0,
            ],
            0x80,
        ));
        assert!(volume_log.borrow().is_empty());

        let open = c.open_file("doc.txt", OpenFlags::Read.bit()).unwrap();
        let tan = open.data[1];
        c.handle_server_response(&server_msg(
            vec![FSFunction::CloseFile.as_u8(), tan, FSError::Success.as_u8()],
            0x80,
        ));
        assert!(c.open_files().is_empty());
    }

    #[test]
    fn read_eof_returns_empty_vec_ok() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        // Pre-track an open handle.
        c.open_files.insert(
            5,
            OpenFileInfo {
                handle: 5,
                path: "f".to_string(),
                ..Default::default()
            },
        );
        type ReadLog = Vec<Result<Vec<u8>, FSError>>;
        let log: Rc<RefCell<ReadLog>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_read_response
            .subscribe(move |(_, r)| lc.borrow_mut().push(r.clone()));

        let frame = c.read_file(5, 8).unwrap();
        let tan = frame.data[1];
        // EOF response: error=EndOfFile, count=0.
        let response = vec![
            FSFunction::ReadFile.as_u8(),
            tan,
            FSError::EndOfFile.as_u8(),
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ];
        c.handle_server_response(&server_msg(response, 0x80));
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(log.borrow()[0], Ok(Vec::new()));
    }

    #[test]
    fn ccm_emitted_at_cadence_when_connected() {
        let mut c = FileClient::new(FileClientConfig::default().with_ccm_interval(100));
        force_connected(&mut c);
        // First update under cadence: only timer increments.
        let out = c.update(50);
        assert!(out.is_empty());
        // Crosses cadence threshold.
        let out = c.update(60);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], CCM_FUNCTION_CODE);
    }

    #[test]
    fn server_status_timeout_disconnects() {
        let mut c = FileClient::new(FileClientConfig::default().with_request_timeout(10_000));
        force_connected(&mut c);
        // Push past server_status_timeout (default 6000).
        c.update(7000);
        assert_eq!(c.state(), ClientState::Disconnected);
    }

    #[test]
    fn disconnect_clears_state_and_emits_close_frames() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        c.open_files.insert(
            5,
            OpenFileInfo {
                handle: 5,
                path: "f".to_string(),
                ..Default::default()
            },
        );
        let frames = c.disconnect();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data[0], FSFunction::CloseFile.as_u8());
        assert_eq!(c.state(), ClientState::Disconnected);
        assert!(c.open_files().is_empty());
    }

    #[test]
    fn read_file_unknown_handle_returns_none() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        assert!(c.read_file(99, 4).is_none());
    }

    #[test]
    fn fallible_directory_queries_reject_disconnected_state_without_allocating_tans() {
        let mut c = FileClient::new(FileClientConfig::default());

        let before = c.next_tan;
        assert_eq!(
            c.try_get_current_directory().unwrap_err().code,
            ErrorCode::NotConnected
        );
        assert_eq!(
            c.try_change_directory("\\logs").unwrap_err().code,
            ErrorCode::NotConnected
        );
        assert_eq!(c.next_tan, before);
        assert!(c.pending_requests.is_empty());
    }

    #[test]
    fn fallible_open_and_change_directory_reject_bad_paths_without_tracking() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        let before = c.next_tan;
        let too_long = "x".repeat(usize::from(u8::MAX) + 1);

        let open_err = c
            .try_open_file(&too_long, OpenFlags::Read.bit())
            .unwrap_err();
        assert_eq!(open_err.code, ErrorCode::InvalidData);
        let traversal_err = c
            .try_open_file("..\\secret.txt", OpenFlags::Read.bit())
            .unwrap_err();
        assert_eq!(traversal_err.code, ErrorCode::InvalidData);
        let chdir_err = c.try_change_directory("safe\\..").unwrap_err();
        assert_eq!(chdir_err.code, ErrorCode::InvalidData);

        assert_eq!(c.next_tan, before);
        assert!(c.pending_requests.is_empty());
    }

    #[test]
    fn fallible_handle_operations_report_unknown_handle_or_oversized_write() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        let before = c.next_tan;

        let read_err = c.try_read_file(99, 4).unwrap_err();
        assert_eq!(read_err.code, ErrorCode::InvalidData);
        assert!(read_err.message.contains("unknown FS file handle 99"));
        let close_err = c.try_close_file(99).unwrap_err();
        assert_eq!(close_err.code, ErrorCode::InvalidData);
        let seek_err = c.try_seek_file(99, 0).unwrap_err();
        assert_eq!(seek_err.code, ErrorCode::InvalidData);
        let write_unknown = c.try_write_file(99, b"abc").unwrap_err();
        assert_eq!(write_unknown.code, ErrorCode::InvalidData);

        c.open_files.insert(
            5,
            OpenFileInfo {
                handle: 5,
                path: "f".to_string(),
                ..Default::default()
            },
        );
        let oversized_payload = vec![0xAA; usize::from(u16::MAX) + 1];
        let write_oversized = c.try_write_file(5, &oversized_payload).unwrap_err();
        assert_eq!(write_oversized.code, ErrorCode::InvalidData);
        assert!(write_oversized.message.contains("exceeds 65535 bytes"));

        assert_eq!(c.next_tan, before);
        assert!(c.pending_requests.is_empty());
    }

    #[test]
    fn client_rejects_paths_and_payloads_that_cannot_encode_in_wire_count_fields() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        let too_long = "x".repeat(usize::from(u8::MAX) + 1);
        assert!(c.open_file(&too_long, OpenFlags::Read.bit()).is_none());
        assert!(
            c.open_file("..\\secret.txt", OpenFlags::Read.bit())
                .is_none()
        );
        assert!(c.change_directory("safe\\..").is_none());

        c.open_files.insert(
            5,
            OpenFileInfo {
                handle: 5,
                path: "f".to_string(),
                ..Default::default()
            },
        );
        let oversized_payload = vec![0xAA; usize::from(u16::MAX) + 1];
        assert!(c.write_file(5, &oversized_payload).is_none());
    }

    #[test]
    fn volume_status_broadcast_emits_event() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);
        let log: Rc<RefCell<Vec<VolumeState>>> = Rc::new(RefCell::new(Vec::new()));
        let lc = log.clone();
        c.on_volume_status
            .subscribe(move |&v| lc.borrow_mut().push(v));
        // VolumeStatus broadcast: tan=0xFF, byte 2 = state=Removed.
        let response = vec![
            FSFunction::VolumeStatus.as_u8(),
            0xFF,
            VolumeState::Removed.as_u8(),
            0,
        ];
        c.handle_server_response(&server_msg(response, 0x80));
        assert_eq!(*log.borrow(), vec![VolumeState::Removed]);
    }

    #[test]
    fn status_broadcast_updates_cached_server_status() {
        let mut c = FileClient::new(FileClientConfig::default());
        force_connected(&mut c);

        let status = FileServerStatus {
            busy: true,
            number_of_open_files: 2,
        };
        c.handle_server_response(&server_msg(status.encode().to_vec(), 0x80));

        assert_eq!(c.server_status(), Some(status));
    }
}
