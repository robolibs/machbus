impl Default for FileServer {
    fn default() -> Self {
        Self::new(FileServerConfig::default())
    }
}

// ─── Public CCM helper ────────────────────────────────────────────────

/// Convenience: encode a Client Connection Maintenance payload (C.1.3).
///
/// The CCM carries no TAN — 4.10 has it establish the connection before the
/// client sends anything TAN-bearing.
#[must_use]
pub fn encode_ccm(version: u8) -> [u8; 8] {
    CCMMessage { version }.encode()
}

// ─── Helpers ──────────────────────────────────────────────────────────

fn encode_error_response(function: u8, tan: TAN, error: FSError) -> Vec<u8> {
    let mut response = vec![0xFFu8; 8];
    response[0] = function;
    response[1] = tan;
    response[2] = error.as_u8();
    response
}

fn success_response(function: FSFunction, tan: TAN) -> Vec<u8> {
    let mut response = vec![0xFFu8; 8];
    response[0] = function.as_u8();
    response[1] = tan;
    response[2] = FSError::Success.as_u8();
    response
}

fn fs_file_size_to_wire(len: usize) -> u32 {
    u32::try_from(len).unwrap_or(u32::MAX)
}

/// B.11 Space is counted in 512-byte blocks.
fn fs_space_blocks(bytes: u64) -> u32 {
    u32::try_from(bytes / 512).unwrap_or(u32::MAX)
}

fn fs_payload_len_is_canonical(data: &[u8], used: usize) -> bool {
    used <= data.len()
        && (data.len() == used
            || (used <= 8 && data.len() == 8 && data[used..].iter().all(|&b| b == 0xFF)))
}

fn normalize_preloaded_file_path(path: &str) -> Result<String> {
    let normalized = normalize_iso_path(path, "\\", false, false)?;
    if normalized.ends_with('\\') {
        return Err(invalid_fs_path_error(
            "FileServer: file path must not end with a directory separator",
        ));
    }
    Ok(normalized)
}

fn normalize_client_path(
    path: &str,
    current_directory: &str,
    is_directory: bool,
) -> Result<String> {
    let normalized = normalize_iso_path(path, current_directory, is_directory, false)?;
    if is_directory {
        Ok(ensure_directory_suffix(normalized))
    } else if normalized.ends_with('\\') {
        Err(invalid_fs_path_error(
            "FileServer: file path must not end with a directory separator",
        ))
    } else {
        Ok(normalized)
    }
}

fn normalize_directory_path(path: &str, current_directory: &str) -> Result<String> {
    let normalized = normalize_iso_path(path, current_directory, true, false)?;
    let normalized = ensure_directory_suffix(normalized);
    if normalized.len() > FS_WIRE_STRING_MAX_LEN {
        return Err(invalid_fs_path_error(
            "FileServer: directory path exceeds one-byte wire length",
        ));
    }
    Ok(normalized)
}

fn normalize_directory_listing_request(
    path: &str,
    current_directory: &str,
) -> Result<(String, String)> {
    if !has_wildcards(path) {
        return Ok((
            normalize_directory_path(path, current_directory)?,
            "*".to_string(),
        ));
    }

    let normalized = normalize_iso_path(path, current_directory, true, true)?;
    let Some(separator_index) = normalized.rfind('\\') else {
        return Err(invalid_fs_path_error(
            "FileServer: wildcard directory listing path must contain a directory base",
        ));
    };
    let pattern = &normalized[separator_index + 1..];
    if pattern.is_empty()
        || !has_wildcards(pattern)
        || !is_valid_fs_path(pattern, false, true)
        || has_wildcards(&normalized[..separator_index])
    {
        return Err(invalid_fs_path_error(
            "FileServer: wildcard directory listing may only use wildcards in the final name component",
        ));
    }
    let directory = if separator_index == 0 {
        "\\".to_string()
    } else {
        format!("{}\\", &normalized[..separator_index])
    };
    if directory.len() > FS_WIRE_STRING_MAX_LEN {
        return Err(invalid_fs_path_error(
            "FileServer: wildcard directory listing base exceeds one-byte wire length",
        ));
    }
    Ok((directory, pattern.to_string()))
}

/// Read a B.12 Path Name Length (2 bytes, little-endian) at `count_index` and
/// return the raw path bytes that follow it.
///
/// Every one of these fields used to be read as a single byte, so a conformant
/// request had its path truncated and the residual length byte prepended to
/// the name.
fn counted_path_bytes(request: &[u8], count_index: usize, path_start: usize) -> Option<&[u8]> {
    let low = *request.get(count_index)?;
    let high = *request.get(count_index + 1)?;
    let path_len = u16::from_le_bytes([low, high]) as usize;
    let used = path_start.checked_add(path_len)?;
    if !fs_payload_len_is_canonical(request, used) {
        return None;
    }
    request.get(path_start..used)
}

fn parse_counted_file_path(
    request: &[u8],
    count_index: usize,
    current_directory: &str,
) -> Option<String> {
    parse_counted_file_path_with_count_at(request, count_index, count_index + 2, current_directory)
}

/// Like [`parse_counted_file_path`], but accepts a directory (including the
/// volume root) as well. C.4.3 Delete File names either kind.
fn parse_counted_file_or_directory_path(
    request: &[u8],
    count_index: usize,
    current_directory: &str,
) -> Option<String> {
    if let Some(path) = parse_counted_file_path(request, count_index, current_directory) {
        return Some(path);
    }
    let requested_path = decode_wire_path(counted_path_bytes(request, count_index, count_index + 2)?)?;
    normalize_directory_path(&requested_path, current_directory).ok()
}

fn parse_counted_file_path_with_count_at(
    request: &[u8],
    count_index: usize,
    path_start: usize,
    current_directory: &str,
) -> Option<String> {
    let requested_path = decode_wire_path(counted_path_bytes(request, count_index, path_start)?)?;
    normalize_client_path(&requested_path, current_directory, false).ok()
}

fn parse_initialize_volume_request(request: &[u8]) -> core::result::Result<Option<String>, ()> {
    if fs_payload_len_is_canonical(request, 2) {
        return Ok(None);
    }
    if request.len() < 9 {
        return Err(());
    }
    let flags = request[6];
    if flags & INITIALIZE_VOLUME_FLAGS_RESERVED_MASK != 0 {
        return Err(());
    }
    let name_len = u16::from_le_bytes([request[7], request[8]]) as usize;
    let used = 9usize.checked_add(name_len).ok_or(())?;
    if !fs_payload_len_is_canonical(request, used) {
        return Err(());
    }
    if name_len == 0 {
        return Ok(None);
    }
    let name = core::str::from_utf8(&request[9..used]).map_err(|_| ())?;
    if !is_valid_volume_name(name) {
        return Err(());
    }
    Ok(Some(name.to_string()))
}

fn parse_two_counted_file_paths(
    request: &[u8],
    current_directory: &str,
) -> Option<(String, String)> {
    if request.len() < 6 {
        return None;
    }
    let source_len = u16::from_le_bytes([request[2], request[3]]) as usize;
    let dest_len = u16::from_le_bytes([request[4], request[5]]) as usize;
    let source_start = 6usize;
    let dest_start = source_start.checked_add(source_len)?;
    let used = dest_start.checked_add(dest_len)?;
    if !fs_payload_len_is_canonical(request, used) {
        return None;
    }
    let source = decode_wire_path(&request[source_start..dest_start])?;
    let dest = decode_wire_path(&request[dest_start..used])?;
    let source = normalize_client_path(&source, current_directory, false).ok()?;
    let dest = normalize_client_path(&dest, current_directory, false).ok()?;
    Some((source, dest))
}

fn parse_counted_file_path_u16(request: &[u8], current_directory: &str) -> Option<Result<String>> {
    if request.len() < 4 {
        return None;
    }
    let path_len = u16::from_le_bytes([request[2], request[3]]) as usize;
    let used = 4usize.checked_add(path_len)?;
    if !fs_payload_len_is_canonical(request, used) {
        return None;
    }
    let requested_path = decode_wire_path(&request[4..used])?;
    Some(normalize_iso_path(
        &requested_path,
        current_directory,
        true,
        false,
    ))
}

fn decode_wire_path(path_bytes: &[u8]) -> Option<String> {
    if !path_bytes.is_ascii() {
        return None;
    }
    Some(
        core::str::from_utf8(path_bytes)
            .expect("ASCII File Server path bytes are valid UTF-8")
            .to_owned(),
    )
}

fn normalize_iso_path(
    path: &str,
    current_directory: &str,
    allow_root: bool,
    allow_wildcards: bool,
) -> Result<String> {
    let path = path.replace('/', "\\");
    if !is_valid_fs_path(&path, allow_root, allow_wildcards) {
        return Err(invalid_fs_path_error(
            "FileServer: path must be a valid ISO 11783-13 backslash path",
        ));
    }

    if path == "\\" || path == "\\\\" {
        return if allow_root {
            Ok("\\".to_string())
        } else {
            Err(invalid_fs_path_error(
                "FileServer: file path cannot be root",
            ))
        };
    }

    let body = path.trim_matches('\\');
    if body.is_empty() {
        return if allow_root {
            Ok("\\".to_string())
        } else {
            Err(invalid_fs_path_error(
                "FileServer: file path cannot be root",
            ))
        };
    }

    if is_absolute_path(&path) {
        Ok(format!("\\{body}"))
    } else {
        let base = normalize_directory_base(current_directory)?;
        if base == "\\" {
            Ok(format!("\\{body}"))
        } else {
            Ok(format!("{base}{body}"))
        }
    }
}

fn normalize_directory_base(path: &str) -> Result<String> {
    let path = path.replace('/', "\\");
    if path == "\\" || path == "\\\\" {
        return Ok("\\".to_string());
    }
    if !is_valid_fs_path(&path, true, false) {
        return Err(invalid_fs_path_error(
            "FileServer: current directory must be a valid ISO 11783-13 path",
        ));
    }
    let body = path.trim_matches('\\');
    if body.is_empty() {
        Ok("\\".to_string())
    } else {
        Ok(format!("\\{body}\\"))
    }
}

fn ensure_directory_suffix(mut path: String) -> String {
    if path != "\\" && !path.ends_with('\\') {
        path.push('\\');
    }
    path
}

fn file_parent_directory_path(path: &str) -> String {
    let normalized = path.replace('/', "\\");
    let trimmed = normalized.trim_end_matches('\\');
    let Some(index) = trimmed.rfind('\\') else {
        return "\\".to_string();
    };
    if index == 0 {
        "\\".to_string()
    } else {
        format!("{}\\", &trimmed[..index])
    }
}

fn open_mode_writes(flags: u8) -> bool {
    matches!(
        get_access_mode(flags),
        mode if mode == OpenFlags::Write.bit() || mode == OpenFlags::ReadWrite.bit()
    ) || has_flag(flags, OpenFlags::Append)
}

fn default_file_date_time() -> (u16, u16) {
    (pack_dos_date(2025, 1, 1), pack_dos_time(12, 0, 0))
}

fn encode_directory_entry(entry: &FileEntry) -> Option<Vec<u8>> {
    let name_bytes = entry.name.as_bytes();
    let name_len = u8::try_from(name_bytes.len()).ok()?;
    if name_len == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(10 + usize::from(name_len));
    out.push(name_len);
    out.extend_from_slice(name_bytes);
    out.push(entry.attributes);
    out.extend_from_slice(&entry.date.to_le_bytes());
    out.extend_from_slice(&entry.time.to_le_bytes());
    out.extend_from_slice(&entry.size.to_le_bytes());
    Some(out)
}

fn invalid_fs_path_error(message: &'static str) -> Error {
    Error::invalid_data(message)
}

fn wildcard_match(s: &str, pattern: &str) -> bool {
    let sb = s.as_bytes();
    let pb = pattern.as_bytes();
    let (mut si, mut pi) = (0usize, 0usize);
    let mut star_idx: Option<usize> = None;
    let mut match_idx = 0usize;

    while si < sb.len() {
        if pi < pb.len() && (pb[pi] == b'?' || pb[pi] == sb[si]) {
            si += 1;
            pi += 1;
        } else if pi < pb.len() && pb[pi] == b'*' {
            star_idx = Some(pi);
            match_idx = si;
            pi += 1;
        } else if let Some(s_idx) = star_idx {
            pi = s_idx + 1;
            match_idx += 1;
            si = match_idx;
        } else {
            return false;
        }
    }
    while pi < pb.len() && pb[pi] == b'*' {
        pi += 1;
    }
    pi == pb.len()
}

#[cfg(test)]
mod tests {
    use super::super::error_codes::has_attribute;
    use super::*;
    use crate::net::pgn_defs::PGN_FILE_CLIENT_TO_SERVER;

    fn req_msg(data: Vec<u8>, src: Address) -> Message {
        Message::new(PGN_FILE_CLIENT_TO_SERVER, data, src)
    }

    fn read_req(tan: u8, handle: FileHandle, count: u16) -> Vec<u8> {
        let mut data = vec![0xFF; 8];
        data[0] = FSFunction::ReadFile.as_u8();
        data[1] = tan;
        data[2] = handle;
        data[3..5].copy_from_slice(&count.to_le_bytes());
        data
    }

    fn write_req(tan: u8, handle: FileHandle, payload: &[u8]) -> Vec<u8> {
        let mut data = vec![FSFunction::WriteFile.as_u8(), tan, handle];
        data.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        data.extend_from_slice(payload);
        data
    }

    fn response_count(data: &[u8]) -> u16 {
        u16::from_le_bytes([data[3], data[4]])
    }

    #[test]
    fn open_file_with_create_flag() {
        let mut s = FileServer::new(FileServerConfig::default());
        let mut req = vec![FSFunction::OpenFile.as_u8(), 0x01];
        let path = b"new.txt";
        req.push(OpenFlags::Write | OpenFlags::Create);
        req.extend_from_slice(&(path.len() as u16).to_le_bytes());
        req.extend_from_slice(path);
        let out = s.handle_client_message(&req_msg(req, 0x42));
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].data[0], FSFunction::OpenFile.as_u8());
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        let handle = out[0].data[3];
        assert_ne!(handle, INVALID_FILE_HANDLE);
    }

    #[test]
    fn set_volume_name_rejects_unencodable_labels() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.set_volume_name("ISOFS").unwrap();
        assert_eq!(s.volume_name(), "ISOFS");

        for name in ["", "host/path", "bad\\label", "bad:name", "bad\0name"] {
            let err = s
                .set_volume_name(name)
                .expect_err("invalid volume label should be rejected");
            assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
            assert_eq!(s.volume_name(), "ISOFS");
        }
    }

    #[test]
    fn read_existing_file() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("data.bin", b"hello".to_vec(), 0).unwrap();
        // Open.
        let mut req = vec![FSFunction::OpenFile.as_u8(), 0x01];
        let path = b"data.bin";
        req.push(OpenFlags::Read.bit());
        req.extend_from_slice(&(path.len() as u16).to_le_bytes());
        req.extend_from_slice(path);
        let out = s.handle_client_message(&req_msg(req, 0x42));
        let handle = out[0].data[3];
        // Read.
        let read_req = read_req(0x02, handle, 5);
        let out = s.handle_client_message(&req_msg(read_req, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(response_count(&out[0].data), 5);
        assert_eq!(&out[0].data[5..10], b"hello");
    }

    #[test]
    fn open_nonexistent_without_create_errors() {
        let mut s = FileServer::new(FileServerConfig::default());
        let mut req = vec![FSFunction::OpenFile.as_u8(), 0x01];
        let path = b"missing.txt";
        req.push(OpenFlags::Read.bit());
        req.extend_from_slice(&(path.len() as u16).to_le_bytes());
        req.extend_from_slice(path);
        let out = s.handle_client_message(&req_msg(req, 0x42));
        assert_eq!(out[0].data[2], FSError::NotFound.as_u8());
    }

    #[test]
    fn open_file_rejects_invalid_paths_without_creating_entries() {
        let invalid_paths = [
            "..\\secret.txt",
            "dir\\..\\secret.txt",
            "dir\\.\\secret.txt",
            "dir\\\\secret.txt",
            "../secret.txt",
            "c:\\secret.txt",
            "bad|name.txt",
        ];

        for (tan, path) in invalid_paths.into_iter().enumerate() {
            let mut s = FileServer::new(FileServerConfig::default());
            let mut req = vec![FSFunction::OpenFile.as_u8(), tan as u8];
            req.push(OpenFlags::Write | OpenFlags::Create);
            req.extend_from_slice(&(path.len() as u16).to_le_bytes());
            req.extend_from_slice(path.as_bytes());

            let out = s.handle_client_message(&req_msg(req, 0x42));
            assert_eq!(
                out[0].data[2],
                FSError::InvalidSourceName.as_u8(),
                "path {path:?}"
            );
            assert!(!s.files.contains_key(path));
            assert!(s.open_files().is_empty());
        }
    }

    #[test]
    fn malformed_request_lengths_are_rejected_before_file_state_changes() {
        let mut s = FileServer::new(FileServerConfig::default());

        let bad_open = vec![
            FSFunction::OpenFile.as_u8(),
            0x10,
            1,
            OpenFlags::Write | OpenFlags::Create,
            b'a',
            0x00,
        ];
        let out = s.handle_client_message(&req_msg(bad_open, 0x42));
        assert_eq!(out[0].data[2], FSError::MalformedRequest.as_u8());
        assert!(s.open_files().is_empty());
        assert!(!s.files.contains_key("\\a"));

        let padded_open = vec![
            FSFunction::OpenFile.as_u8(),
            0x11,
            OpenFlags::Write | OpenFlags::Create,
            1,
            0,
            b'a',
            0xFF,
            0xFF,
        ];
        let out = s.handle_client_message(&req_msg(padded_open, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        let handle = out[0].data[3];

        let bad_write = vec![
            FSFunction::WriteFile.as_u8(),
            0x12,
            handle,
            1,
            0,
            b'X',
            0x00,
        ];
        let out = s.handle_client_message(&req_msg(bad_write, 0x42));
        assert_eq!(out[0].data[2], FSError::MalformedRequest.as_u8());
        assert_eq!(s.files.get("\\a").unwrap().as_slice(), b"");

        let bad_read = vec![FSFunction::ReadFile.as_u8(), 0x13, handle, 1, 0, 0x00];
        let out = s.handle_client_message(&req_msg(bad_read, 0x42));
        assert_eq!(out[0].data[2], FSError::MalformedRequest.as_u8());

        let bad_change_dir = vec![FSFunction::ChangeDirectory.as_u8(), 0x14, 1, 0, b'\\', 0x00];
        let out = s.handle_client_message(&req_msg(bad_change_dir, 0x42));
        assert_eq!(out[0].data[2], FSError::MalformedRequest.as_u8());
        assert_eq!(s.clients().get(&0x42).unwrap().current_directory, "\\");
    }

    #[test]
    fn malformed_ccm_does_not_connect_client() {
        let mut s = FileServer::new(FileServerConfig::default());
        let bad_ccm = vec![CCM_FUNCTION_CODE, 0x22, 0x00];
        let out = s.handle_client_message(&req_msg(bad_ccm, 0x42));
        assert!(out.is_empty());
        assert!(!s.clients().contains_key(&0x42));
    }

    /// C.3.2.2 — a directory is created through Open File with the directory
    /// access mode and the create flag. ISO 11783-13 has no Make Directory
    /// command; the crate used to invent one at 0x15.
    #[test]
    fn open_file_with_create_directory_flags_creates_the_directory() {
        let mut s = FileServer::new(FileServerConfig::default());
        let path = b"\\newdir";
        let mkdir = |tan: u8, path: &[u8]| {
            let mut req = vec![
                FSFunction::OpenFile.as_u8(),
                tan,
                OpenFlags::OpenDir | OpenFlags::Create,
                path.len() as u8, (path.len() >> 8) as u8,
            ];
            req.extend_from_slice(path);
            req
        };
        assert!(!s.directory_exists("\\newdir"));
        let out = s.handle_client_message(&req_msg(mkdir(0x20, path), 0x42));
        assert_eq!(out[0].data[0], FSFunction::OpenFile.as_u8());
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert!(s.directory_exists("\\newdir"));

        // Opening it again just lists it — the create flag is not exclusive.
        let out = s.handle_client_message(&req_msg(mkdir(0x21, path), 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());

        // A file already occupies the path.
        s.add_file("\\afile", b"x".to_vec(), 0).unwrap();
        let out = s.handle_client_message(&req_msg(mkdir(0x22, b"\\afile"), 0x42));
        assert_eq!(out[0].data[2], FSError::InvalidAccess.as_u8());

        // Without the create flag a missing directory is still NotFound.
        let mut plain = vec![
            FSFunction::OpenFile.as_u8(),
            0x23,
            OpenFlags::OpenDir.bit(),
            7u8,
            0,
        ];
        plain.extend_from_slice(b"\\absent");
        let out = s.handle_client_message(&req_msg(plain, 0x42));
        assert_eq!(out[0].data[2], FSError::NotFound.as_u8());
    }

    /// C.4.3 — Delete File removes directories as well as files.
    #[test]
    fn delete_file_removes_empty_directories_and_rejects_non_empty() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_directory("\\empty").unwrap();
        s.add_directory("\\full").unwrap();
        s.add_file("\\full\\f", b"x".to_vec(), 0).unwrap();
        let rmdir = |tan: u8, path: &[u8]| {
            let mut req = vec![FSFunction::DeleteFile.as_u8(), tan, path.len() as u8, (path.len() >> 8) as u8];
            req.extend_from_slice(path);
            req
        };

        let out = s.handle_client_message(&req_msg(rmdir(0x20, b"\\empty"), 0x42));
        assert_eq!(out[0].data[0], FSFunction::DeleteFile.as_u8());
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert!(!s.directory_exists("\\empty"));

        let out = s.handle_client_message(&req_msg(rmdir(0x21, b"\\full"), 0x42));
        assert_eq!(out[0].data[2], FSError::AccessDenied.as_u8());
        assert!(s.directory_exists("\\full"));

        let out = s.handle_client_message(&req_msg(rmdir(0x22, b"\\nope"), 0x42));
        assert_eq!(out[0].data[2], FSError::NotFound.as_u8());
        let out = s.handle_client_message(&req_msg(rmdir(0x23, b"\\"), 0x42));
        assert_eq!(out[0].data[2], FSError::AccessDenied.as_u8());
    }

    #[test]
    fn preloaded_paths_are_normalized_to_iso_namespace() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_directory("/data").unwrap();
        s.add_file("/data/log.txt", b"abc".to_vec(), 0).unwrap();
        s.add_file("root.txt", b"root".to_vec(), 0).unwrap();

        assert!(s.directory_exists("\\data"));
        assert!(s.directory_exists("/data"));
        assert!(s.files.contains_key("\\data\\log.txt"));
        assert!(s.files.contains_key("\\root.txt"));
        assert!(!s.files.contains_key("/data/log.txt"));
    }

    #[test]
    fn preloaded_paths_reject_traversal_and_file_directories() {
        let mut s = FileServer::new(FileServerConfig::default());
        for path in [
            "..\\secret.txt",
            "dir\\..\\secret.txt",
            "bad|name.txt",
            "\\",
        ] {
            assert!(
                s.add_file(path, Vec::new(), 0).is_err(),
                "file path {path:?} should be rejected"
            );
        }
        assert!(s.add_directory("dir\\..").is_err());
        assert!(s.add_directory("bad|dir").is_err());
    }

    #[test]
    fn directory_paths_reject_values_that_cannot_encode_in_the_count_field() {
        // B.12 gives Path Name Length two bytes; A.2.2.1 caps each individual
        // name component at 255, so the whole path may still be long.
        let mut s = FileServer::new(FileServerConfig::default());
        let component = "a".repeat(255);
        let max_body = core::iter::repeat_n(component.as_str(), 200)
            .collect::<Vec<_>>()
            .join("\\");
        s.add_directory(format!("\\{max_body}")).unwrap();

        let too_long_component = "b".repeat(256);
        assert!(s.add_directory(format!("\\{too_long_component}")).is_err());

        let mut cd = vec![
            FSFunction::ChangeDirectory.as_u8(),
            0x01,
            max_body.len() as u8, (max_body.len() >> 8) as u8,
        ];
        cd.extend_from_slice(max_body.as_bytes());
        let response = s.handle_client_message(&req_msg(cd, 0x42));
        assert_eq!(response[0].data[2], FSError::Success.as_u8());
        let deep_cwd_len = s.clients().get(&0x42).unwrap().current_directory.len();
        assert_eq!(deep_cwd_len, max_body.len() + 2);

        let mut get_cwd = vec![FSFunction::GetCurrentDirectory.as_u8(), 0x02];
        get_cwd.extend_from_slice(&[0xFF; 6]);
        let response = s.handle_client_message(&req_msg(get_cwd, 0x42));
        assert_eq!(response[0].data[2], FSError::Success.as_u8());
        // C.2.2.3 bytes 12,13 — a path this long cannot fit one byte at all.
        assert_eq!(
            u16::from_le_bytes([response[0].data[11], response[0].data[12]]) as usize,
            deep_cwd_len
        );

        // The path is now encodable; what it names still has to exist.
        let missing = vec![FSFunction::ChangeDirectory.as_u8(), 0x03, 1, 0, b'b'];
        let response = s.handle_client_message(&req_msg(missing, 0x42));
        assert_eq!(response[0].data[2], FSError::NotFound.as_u8());
        assert_eq!(
            s.clients().get(&0x42).unwrap().current_directory.len(),
            deep_cwd_len
        );
    }

    #[test]
    fn open_file_resolves_relative_path_against_current_directory() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_directory("\\safe").unwrap();
        s.add_file("\\safe\\log.txt", b"scoped".to_vec(), 0)
            .unwrap();
        s.add_file("\\log.txt", b"root".to_vec(), 0).unwrap();

        let path = b"\\safe";
        let mut cd = vec![FSFunction::ChangeDirectory.as_u8(), 0x01, path.len() as u8, (path.len() >> 8) as u8];
        cd.extend_from_slice(path);
        let out = s.handle_client_message(&req_msg(cd, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());

        let mut open = vec![FSFunction::OpenFile.as_u8(), 0x02];
        open.push(OpenFlags::Read.bit());
        open.extend_from_slice(&(b"log.txt".len() as u16).to_le_bytes());
        open.extend_from_slice(b"log.txt");
        let out = s.handle_client_message(&req_msg(open, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        let handle = out[0].data[3];
        assert_eq!(s.open_files()[0].path, "\\safe\\log.txt");

        let read = read_req(0x03, handle, 6);
        let out = s.handle_client_message(&req_msg(read, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(&out[0].data[5..11], b"scoped");
    }

    #[test]
    fn tan_cache_reuses_response() {
        let mut s = FileServer::new(FileServerConfig::default());
        let req = vec![FSFunction::GetFileServerProperties.as_u8(), 0x05];
        let out1 = s.handle_client_message(&req_msg(req.clone(), 0x42));
        let out2 = s.handle_client_message(&req_msg(req, 0x42));
        assert_eq!(out1[0].data, out2[0].data);
    }

    #[test]
    fn tan_cache_expires_before_reexecuting_request() {
        let config = FileServerConfig {
            tan_cache_timeout_ms: 10,
            ..FileServerConfig::default()
        };
        let mut s = FileServer::new(config);
        s.add_file("data.bin", b"abc".to_vec(), 0).unwrap();

        let mut req = vec![FSFunction::OpenFile.as_u8(), 0x05];
        let path = b"data.bin";
        req.push(OpenFlags::Read.bit());
        req.extend_from_slice(&(path.len() as u16).to_le_bytes());
        req.extend_from_slice(path);

        let out1 = s.handle_client_message(&req_msg(req.clone(), 0x42));
        let handle1 = out1[0].data[3];
        let out2 = s.handle_client_message(&req_msg(req.clone(), 0x42));
        assert_eq!(out2[0].data[3], handle1);
        assert_eq!(s.open_files().len(), 1);

        s.update(11);
        let out3 = s.handle_client_message(&req_msg(req, 0x42));
        assert_eq!(out3[0].data[2], FSError::Success.as_u8());
        assert_ne!(out3[0].data[3], handle1);
        assert_eq!(s.open_files().len(), 2);
    }

    #[test]
    fn ccm_creates_client_record() {
        let mut s = FileServer::new(FileServerConfig::default());
        let _ = s.update(0); // burn 0ms to set up timer
        let req = vec![CCM_FUNCTION_CODE, 0x01];
        s.handle_client_message(&req_msg(req, 0x42));
        assert!(s.clients().contains_key(&0x42));
    }

    #[test]
    fn close_unknown_handle_errors() {
        let mut s = FileServer::new(FileServerConfig::default());
        let req = vec![FSFunction::CloseFile.as_u8(), 0x01, 99];
        let out = s.handle_client_message(&req_msg(req, 0x42));
        assert_eq!(out[0].data[2], FSError::InvalidHandle.as_u8());
    }

    #[test]
    fn write_then_read_round_trip() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("rw.bin", vec![0; 8], 0).unwrap();
        // Open RW.
        let mut req = vec![FSFunction::OpenFile.as_u8(), 0x01];
        let path = b"rw.bin";
        req.push(OpenFlags::ReadWrite.bit());
        req.extend_from_slice(&(path.len() as u16).to_le_bytes());
        req.extend_from_slice(path);
        let out = s.handle_client_message(&req_msg(req, 0x42));
        let handle = out[0].data[3];
        // Write 3 bytes.
        let wreq = write_req(0x02, handle, &[0xAA, 0xBB, 0xCC]);
        let out = s.handle_client_message(&req_msg(wreq, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(response_count(&out[0].data), 3);
        // Seek back to the start (C.3.3.2 mode 0, offset 0).
        let mut seek = vec![FSFunction::SeekFile.as_u8(), 0x03, handle, 0];
        seek.extend_from_slice(&0i32.to_le_bytes());
        let out = s.handle_client_message(&req_msg(seek, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(&out[0].data[4..8], &0u32.to_le_bytes());
        // Read 3 bytes.
        let rreq = read_req(0x04, handle, 3);
        let out = s.handle_client_message(&req_msg(rreq, 0x42));
        assert_eq!(&out[0].data[5..8], &[0xAA, 0xBB, 0xCC]);
    }

    #[test]
    fn write_to_read_only_file_is_denied() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("ro.txt", b"ro".to_vec(), FileAttributes::ReadOnly.bit())
            .unwrap();

        let mut open = vec![FSFunction::OpenFile.as_u8(), 0x01];
        open.push(OpenFlags::ReadWrite.bit());
        open.extend_from_slice(&(b"ro.txt".len() as u16).to_le_bytes());
        open.extend_from_slice(b"ro.txt");
        let handle = s.handle_client_message(&req_msg(open, 0x42))[0].data[3];

        let write = write_req(0x02, handle, b"!");
        let out = s.handle_client_message(&req_msg(write, 0x42));
        assert_eq!(out[0].data[2], FSError::AccessDenied.as_u8());
    }

    /// C.3.3.1: "If the Seek command tries to set the file pointer beyond the
    /// end of file ... or before the start of the file, the response shall
    /// contain an 'Invalid Request Length' error code ... The File pointer
    /// position shall be changed only if the error code 'Success' is
    /// contained in the response message."
    ///
    /// The old handler range-checked nothing, so a seek to `u32::MAX` on an
    /// empty file "succeeded" and parked the pointer past the end.
    #[test]
    fn seek_out_of_range_is_refused_and_leaves_the_pointer_alone() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("rw.bin", b"abcd".to_vec(), 0).unwrap();

        let mut open = vec![FSFunction::OpenFile.as_u8(), 0x01];
        open.push(OpenFlags::ReadWrite.bit());
        open.extend_from_slice(&(b"rw.bin".len() as u16).to_le_bytes());
        open.extend_from_slice(b"rw.bin");
        let handle = s.handle_client_message(&req_msg(open, 0x42))[0].data[3];

        let seek = |tan: u8, mode: u8, offset: i32| {
            let mut req = vec![FSFunction::SeekFile.as_u8(), tan, handle, mode];
            req.extend_from_slice(&offset.to_le_bytes());
            req
        };

        for (tan, mode, offset) in [(0x02, 0u8, i32::MAX), (0x03, 0, -1), (0x04, 2, 1)] {
            let out = s.handle_client_message(&req_msg(seek(tan, mode, offset), 0x42));
            assert_eq!(out[0].data[2], FSError::InvalidLength.as_u8());
            assert_eq!(s.open_files()[0].position, 0);
        }

        // B.17 leaves 3-255 reserved.
        let out = s.handle_client_message(&req_msg(seek(0x05, 3, 0), 0x42));
        assert_eq!(out[0].data[2], FSError::InvalidAccess.as_u8());
        assert_eq!(s.open_files()[0].position, 0);

        // Relative and end-anchored seeks inside the file land where asked and
        // report the resulting position.
        let out = s.handle_client_message(&req_msg(seek(0x06, 2, -1), 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(&out[0].data[4..8], &3u32.to_le_bytes());
        assert_eq!(s.open_files()[0].position, 3);

        let out = s.handle_client_message(&req_msg(seek(0x07, 1, -2), 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(&out[0].data[4..8], &1u32.to_le_bytes());
        assert_eq!(s.open_files()[0].position, 1);

        // Once the pointer is already at the end, moving further reports 45.
        let out = s.handle_client_message(&req_msg(seek(0x08, 2, 0), 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        let out = s.handle_client_message(&req_msg(seek(0x09, 1, 1), 0x42));
        assert_eq!(out[0].data[2], FSError::EndOfFile.as_u8());
        assert_eq!(s.open_files()[0].position, 4);
    }

    #[test]
    fn removed_volume_clears_handles_and_rejects_file_operations() {
        let mut s = FileServer::new(FileServerConfig::default().with_ccm_timeout(60_000));
        s.add_file("media.txt", b"abc".to_vec(), 0).unwrap();

        let mut open = vec![FSFunction::OpenFile.as_u8(), 0x01];
        open.push(OpenFlags::ReadWrite.bit());
        open.extend_from_slice(&(b"media.txt".len() as u16).to_le_bytes());
        open.extend_from_slice(b"media.txt");
        let handle = s.handle_client_message(&req_msg(open, 0x42))[0].data[3];
        assert_eq!(s.open_files().len(), 1);
        assert_eq!(s.clients().get(&0x42).unwrap().open_handles, vec![handle]);

        let preparing = s.prepare_volume_for_removal();
        assert_eq!(preparing[0].data[0], FSFunction::VolumeStatus.as_u8());
        assert_eq!(
            preparing[0].data[2],
            VolumeState::PreparingForRemoval.as_u8()
        );

        let removed = s.update(10_000);
        assert_eq!(s.get_volume_state(), VolumeState::Removed);
        assert!(s.open_files().is_empty());
        assert!(s.clients().get(&0x42).unwrap().open_handles.is_empty());
        assert_eq!(removed[0].data[0], FSFunction::VolumeStatus.as_u8());
        assert_eq!(removed[0].data[2], VolumeState::Removed.as_u8());

        let mut open_after_remove = vec![FSFunction::OpenFile.as_u8(), 0x03];
        open_after_remove.push(OpenFlags::Read.bit());
        open_after_remove.extend_from_slice(&(b"media.txt".len() as u16).to_le_bytes());
        open_after_remove.extend_from_slice(b"media.txt");
        let out = s.handle_client_message(&req_msg(open_after_remove, 0x42));
        assert_eq!(out[0].data[2], FSError::MediaNotPresent.as_u8());

        let read = read_req(0x04, handle, 1);
        let out = s.handle_client_message(&req_msg(read, 0x42));
        assert_eq!(out[0].data[2], FSError::MediaNotPresent.as_u8());

        let write = write_req(0x05, handle, b"!");
        let out = s.handle_client_message(&req_msg(write, 0x42));
        assert_eq!(out[0].data[2], FSError::MediaNotPresent.as_u8());

        let mut seek = vec![FSFunction::SeekFile.as_u8(), 0x06, handle];
        seek.extend_from_slice(&0u32.to_le_bytes());
        let out = s.handle_client_message(&req_msg(seek, 0x42));
        assert_eq!(out[0].data[2], FSError::MediaNotPresent.as_u8());
    }

    #[test]
    fn multi_client_handles_are_owner_scoped_but_share_backing_file() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("shared.bin", vec![0; 2], 0).unwrap();

        let mut open = vec![FSFunction::OpenFile.as_u8(), 0x01];
        open.push(OpenFlags::Read.bit());
        open.extend_from_slice(&(b"shared.bin".len() as u16).to_le_bytes());
        open.extend_from_slice(b"shared.bin");
        let handle_a = s.handle_client_message(&req_msg(open.clone(), 0x42))[0].data[3];

        open[1] = 0x02;
        let handle_b = s.handle_client_message(&req_msg(open, 0x43))[0].data[3];
        assert_ne!(handle_a, handle_b);

        let close_reader_a = vec![FSFunction::CloseFile.as_u8(), 0x03, handle_a];
        let out = s.handle_client_message(&req_msg(close_reader_a, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());

        let close_other = vec![FSFunction::CloseFile.as_u8(), 0x04, handle_b];
        let out = s.handle_client_message(&req_msg(close_other, 0x43));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());

        let mut open_writer = vec![FSFunction::OpenFile.as_u8(), 0x05];
        open_writer.push(OpenFlags::ReadWrite.bit());
        open_writer.extend_from_slice(&(b"shared.bin".len() as u16).to_le_bytes());
        open_writer.extend_from_slice(b"shared.bin");
        let handle_writer = s.handle_client_message(&req_msg(open_writer, 0x42))[0].data[3];

        let write = write_req(0x06, handle_writer, b"OK");
        let out = s.handle_client_message(&req_msg(write, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());

        let close_writer = vec![FSFunction::CloseFile.as_u8(), 0x07, handle_writer];
        let out = s.handle_client_message(&req_msg(close_writer, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());

        let mut open_reader = vec![FSFunction::OpenFile.as_u8(), 0x08];
        open_reader.push(OpenFlags::Read.bit());
        open_reader.extend_from_slice(&(b"shared.bin".len() as u16).to_le_bytes());
        open_reader.extend_from_slice(b"shared.bin");
        let handle_reader = s.handle_client_message(&req_msg(open_reader, 0x43))[0].data[3];

        let mut seek = vec![FSFunction::SeekFile.as_u8(), 0x09, handle_reader];
        seek.extend_from_slice(&0u32.to_le_bytes());
        s.handle_client_message(&req_msg(seek, 0x43));
        let read = read_req(0x0A, handle_reader, 2);
        let out = s.handle_client_message(&req_msg(read, 0x43));
        assert_eq!(&out[0].data[5..7], b"OK");
    }

    #[test]
    fn list_directory_includes_files_and_subdirs() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("\\foo.txt", vec![1, 2, 3], 0).unwrap();
        s.add_directory("\\sub").unwrap();
        let entries = s.list_directory("\\", "*");
        assert!(entries.iter().any(|e| e.name == "foo.txt"));
        let dir_entry = entries.iter().find(|e| e.name == "sub\\").unwrap();
        assert!(has_attribute(
            dir_entry.attributes,
            FileAttributes::Directory
        ));
    }

    #[test]
    fn list_directory_wildcards_only_match_immediate_children() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("\\alpha.txt", vec![1], 0).unwrap();
        s.add_file("\\alpha.bin", vec![2], 0).unwrap();
        s.add_file("\\sub\\nested.txt", vec![3], 0).unwrap();
        s.add_directory("\\sub").unwrap();
        s.add_directory("\\sub\\deep").unwrap();

        let txt = s.list_directory("\\", "*.txt");
        assert_eq!(txt.len(), 1);
        assert_eq!(txt[0].name, "alpha.txt");

        let all = s.list_directory("\\", "*");
        assert!(all.iter().any(|e| e.name == "alpha.txt"));
        assert!(all.iter().any(|e| e.name == "alpha.bin"));
        assert!(all.iter().any(|e| e.name == "sub\\"));
        assert!(!all.iter().any(|e| e.name == "sub\\nested.txt"));

        let sub = s.list_directory("\\sub\\", "*.txt");
        assert_eq!(sub.iter().filter(|e| e.name == "nested.txt").count(), 1);
        assert!(!sub.iter().any(|e| e.name == "deep\\"));
    }

    #[test]
    fn change_directory_rejects_nested_traversal() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_directory("\\safe").unwrap();
        let path = b"safe\\..";
        let mut req = vec![FSFunction::ChangeDirectory.as_u8(), 0x01, path.len() as u8, (path.len() >> 8) as u8];
        req.extend_from_slice(path);
        let out = s.handle_client_message(&req_msg(req, 0x42));
        assert_eq!(out[0].data[2], FSError::InvalidSourceName.as_u8());
        assert_eq!(s.clients().get(&0x42).unwrap().current_directory, "\\");
    }

    #[test]
    fn change_directory_accepts_root_absolute_paths() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_directory("\\safe").unwrap();
        let path = b"\\safe";
        let mut req = vec![FSFunction::ChangeDirectory.as_u8(), 0x01, path.len() as u8, (path.len() >> 8) as u8];
        req.extend_from_slice(path);
        let out = s.handle_client_message(&req_msg(req, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(
            s.clients().get(&0x42).unwrap().current_directory,
            "\\safe\\"
        );
    }

    #[test]
    fn wildcard_match_basic() {
        assert!(wildcard_match("file.txt", "*.txt"));
        assert!(wildcard_match("abc", "?bc"));
        assert!(!wildcard_match("abc", "*.txt"));
        assert!(wildcard_match("anything", "*"));
    }

    #[test]
    fn move_delete_attributes_and_initialize_volume_are_handled() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_directory("\\logs").unwrap();
        s.add_file(
            "\\logs\\old.txt",
            b"abc".to_vec(),
            FileAttributes::Hidden.bit(),
        )
        .unwrap();

        let src = b"\\logs\\old.txt";
        let dst = b"\\logs\\new.txt";
        let mut move_req = vec![
            FSFunction::MoveFile.as_u8(),
            0x10,
            src.len() as u8, (src.len() >> 8) as u8,
            dst.len() as u8,
            (dst.len() >> 8) as u8,
        ];
        move_req.extend_from_slice(src);
        move_req.extend_from_slice(dst);
        let out = s.handle_client_message(&req_msg(move_req, 0x42));
        assert_eq!(out[0].data, success_response(FSFunction::MoveFile, 0x10));
        assert!(!s.files.contains_key("\\logs\\old.txt"));
        assert_eq!(s.files.get("\\logs\\new.txt").unwrap(), b"abc");

        let attrs_path = b"\\logs\\new.txt";
        let mut get_attrs = vec![
            FSFunction::GetFileAttributes.as_u8(),
            0x11,
            attrs_path.len() as u8, (attrs_path.len() >> 8) as u8,
        ];
        get_attrs.extend_from_slice(attrs_path);
        let out = s.handle_client_message(&req_msg(get_attrs, 0x42));
        assert_eq!(out[0].data[2], FSError::Success.as_u8());
        assert_eq!(out[0].data[3], FileAttributes::Hidden.bit());

        let mut set_attrs = vec![
            FSFunction::SetFileAttributes.as_u8(),
            0x12,
            FileAttributes::ReadOnly.bit() | FileAttributes::Hidden.bit(),
            attrs_path.len() as u8, (attrs_path.len() >> 8) as u8,
        ];
        set_attrs.extend_from_slice(attrs_path);
        let out = s.handle_client_message(&req_msg(set_attrs, 0x42));
        assert_eq!(
            out[0].data,
            success_response(FSFunction::SetFileAttributes, 0x12)
        );
        assert_eq!(
            *s.file_attrs.get("\\logs\\new.txt").unwrap(),
            FileAttributes::ReadOnly.bit() | FileAttributes::Hidden.bit()
        );

        let mut delete_read_only =
            vec![FSFunction::DeleteFile.as_u8(), 0x13, attrs_path.len() as u8, (attrs_path.len() >> 8) as u8];
        delete_read_only.extend_from_slice(attrs_path);
        let out = s.handle_client_message(&req_msg(delete_read_only.clone(), 0x42));
        assert_eq!(out[0].data[2], FSError::AccessDenied.as_u8());

        s.file_attrs
            .insert("\\logs\\new.txt".to_string(), FileAttributes::Hidden.bit());
        let out = s.handle_client_message(&req_msg(delete_read_only, 0x43));
        assert_eq!(out[0].data, success_response(FSFunction::DeleteFile, 0x13));
        assert!(!s.files.contains_key("\\logs\\new.txt"));

        s.add_file("other.bin", b"x".to_vec(), 0).unwrap();
        s.add_directory("tmp").unwrap();
        let init = vec![FSFunction::InitializeVolume.as_u8(), 0x14];
        let out = s.handle_client_message(&req_msg(init, 0x42));
        assert_eq!(
            out[0].data,
            success_response(FSFunction::InitializeVolume, 0x14)
        );
        assert!(s.files.is_empty());
        assert_eq!(s.directories, vec!["\\".to_string()]);
        assert!(s.open_files.is_empty());
    }

    #[test]
    fn malformed_or_unsafe_management_requests_do_not_mutate_file_state() {
        let mut s = FileServer::new(FileServerConfig::default());
        s.add_file("a.txt", b"abc".to_vec(), 0).unwrap();
        let initial_files = s.files.clone();
        let initial_attrs = s.file_attrs.clone();

        let malformed_move = vec![FSFunction::MoveFile.as_u8(), 0x21, 5, 0, 5, 0, b'a', b'.'];
        let out = s.handle_client_message(&req_msg(malformed_move, 0x42));
        assert_eq!(out[0].data[2], FSError::MalformedRequest.as_u8());
        assert_eq!(s.files, initial_files);
        assert_eq!(s.file_attrs, initial_attrs);

        let bad_attrs = b"a.txt";
        let mut set_attrs = vec![
            FSFunction::SetFileAttributes.as_u8(),
            0x22,
            FileAttributes::IsVolume.bit(),
            bad_attrs.len() as u8, (bad_attrs.len() >> 8) as u8,
        ];
        set_attrs.extend_from_slice(bad_attrs);
        let out = s.handle_client_message(&req_msg(set_attrs, 0x42));
        assert_eq!(out[0].data[2], FSError::InvalidAccess.as_u8());
        assert_eq!(s.file_attrs, initial_attrs);

        let mut open = vec![FSFunction::OpenFile.as_u8(), 0x23];
        open.push(OpenFlags::Read.bit());
        open.extend_from_slice(&(bad_attrs.len() as u16).to_le_bytes());
        open.extend_from_slice(bad_attrs);
        let handle = s.handle_client_message(&req_msg(open, 0x42))[0].data[3];
        let mut delete_open = vec![FSFunction::DeleteFile.as_u8(), 0x24, bad_attrs.len() as u8, (bad_attrs.len() >> 8) as u8];
        delete_open.extend_from_slice(bad_attrs);
        let out = s.handle_client_message(&req_msg(delete_open, 0x42));
        assert_eq!(out[0].data[2], FSError::AccessDenied.as_u8());
        assert!(s.files.contains_key("\\a.txt"));
        assert!(s.open_files.iter().any(|open| open.handle == handle));
    }

    #[test]
    fn periodic_status_broadcast_emitted() {
        let mut s = FileServer::new(FileServerConfig::default().with_status_interval(100));
        // First tick well under interval.
        let out = s.update(50);
        assert!(out.iter().all(|f| f.dest.is_some() || f.data[0] != 0));
        // Second tick crosses threshold.
        let out = s.update(60);
        assert!(out.iter().any(|f| f.dest.is_none()));
    }

    #[test]
    fn volume_state_transitions_with_open_files() {
        let mut s = FileServer::new(FileServerConfig::default());
        assert_eq!(s.get_volume_state(), VolumeState::Present);
        // Open a file → Present should advance to InUse on next update.
        let mut req = vec![FSFunction::OpenFile.as_u8(), 0x01];
        let path = b"f";
        req.push(OpenFlags::Write | OpenFlags::Create);
        req.extend_from_slice(&(path.len() as u16).to_le_bytes());
        req.extend_from_slice(path);
        s.handle_client_message(&req_msg(req, 0x42));
        let _ = s.update(0);
        assert_eq!(s.get_volume_state(), VolumeState::InUse);
    }
}
