//! ISO 11783-13 File Server basic types: TAN, FileHandle, function
//! codes, classic `FileServerProperties`/`VolumeState`, file entry,
//! status, CCM, path utilities, DOS date/time helpers.
//!
//! Mirrors the C++ `machbus::isobus::fs::types.hpp`. The duplicate
//! `FileServerProperties` / `VolumeState` definitions in
//! `properties.hpp` are ported under different names — see
//! [`super::properties`] and `book/src/reference/behavior-differences.md`.

use alloc::{string::String, vec::Vec};

use super::error_codes::{FileAttributes, has_attribute};

/// Transaction Number (TAN). Used for request/response matching.
/// `0xFF` is reserved as the [`INVALID_TAN`] sentinel; values wrap
/// `0..=0xFE` per the ISO spec.
pub type TAN = u8;
pub const INVALID_TAN: TAN = 0xFF;

/// Server-assigned file handle. `0x00` and `0xFF` are reserved.
pub type FileHandle = u8;
pub const INVALID_FILE_HANDLE: FileHandle = 0xFF;
pub const RESERVED_FILE_HANDLE_0: FileHandle = 0x00;
/// Highest File Server count value this implementation will accept or
/// advertise for one-byte open-file/client counters.
pub const FS_SUPPORTED_COUNT_MAX: u8 = 250;
pub const FS_CLASSIC_PROPERTIES_VERSION: u8 = 1;

#[must_use]
pub const fn fs_count_is_supported(count: u8) -> bool {
    count <= FS_SUPPORTED_COUNT_MAX
}

/// FS function codes (ISO 11783-13 §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FSFunction {
    #[default]
    GetCurrentDirectory = 0x00,
    ChangeDirectory = 0x01,
    OpenFile = 0x02,
    SeekFile = 0x03,
    ReadFile = 0x04,
    WriteFile = 0x05,
    CloseFile = 0x06,
    MoveFile = 0x10,
    DeleteFile = 0x11,
    GetFileAttributes = 0x12,
    SetFileAttributes = 0x13,
    GetFileDateTime = 0x14,
    MakeDirectory = 0x15,
    RemoveDirectory = 0x16,
    CopyFile = 0x17,
    GetFileSize = 0x18,
    GetFreeSpace = 0x19,
    InitializeVolume = 0x20,
    FileServerStatus = 0x30,
    GetFileServerProperties = 0x31,
    VolumeStatus = 0x40,
}

impl FSFunction {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::GetCurrentDirectory),
            0x01 => Some(Self::ChangeDirectory),
            0x02 => Some(Self::OpenFile),
            0x03 => Some(Self::SeekFile),
            0x04 => Some(Self::ReadFile),
            0x05 => Some(Self::WriteFile),
            0x06 => Some(Self::CloseFile),
            0x10 => Some(Self::MoveFile),
            0x11 => Some(Self::DeleteFile),
            0x12 => Some(Self::GetFileAttributes),
            0x13 => Some(Self::SetFileAttributes),
            0x14 => Some(Self::GetFileDateTime),
            0x15 => Some(Self::MakeDirectory),
            0x16 => Some(Self::RemoveDirectory),
            0x17 => Some(Self::CopyFile),
            0x18 => Some(Self::GetFileSize),
            0x19 => Some(Self::GetFreeSpace),
            0x20 => Some(Self::InitializeVolume),
            0x30 => Some(Self::FileServerStatus),
            0x31 => Some(Self::GetFileServerProperties),
            0x40 => Some(Self::VolumeStatus),
            _ => None,
        }
    }
}

/// Classic volume state (ISO 11783-13 §7.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum VolumeState {
    #[default]
    Present = 0,
    InUse = 1,
    PreparingForRemoval = 2,
    Removed = 3,
}

impl VolumeState {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::InUse,
            2 => Self::PreparingForRemoval,
            3 => Self::Removed,
            _ => Self::Present,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Present),
            1 => Some(Self::InUse),
            2 => Some(Self::PreparingForRemoval),
            3 => Some(Self::Removed),
            _ => None,
        }
    }
}

/// Classic File-Server Properties block (ISO 11783-13 §7.5.2).
///
/// Note: `properties.hpp` defines a v2 `FileServerProperties`
/// with extra fields. Both layouts coexist; pick the one that
/// matches your VT version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileServerProperties {
    pub version_number: u8,
    pub max_simultaneous_files: u8,
    pub supports_directories: bool,
    pub supports_volume_management: bool,
    pub supports_file_attributes: bool,
    pub supports_move_file: bool,
    pub supports_delete_file: bool,
}

impl Default for FileServerProperties {
    fn default() -> Self {
        Self {
            version_number: 1,
            max_simultaneous_files: 16,
            supports_directories: true,
            supports_volume_management: true,
            supports_file_attributes: true,
            supports_move_file: true,
            supports_delete_file: true,
        }
    }
}

impl FileServerProperties {
    #[must_use]
    pub const fn normalized_for_wire(mut self) -> Self {
        self.version_number = FS_CLASSIC_PROPERTIES_VERSION;
        if self.max_simultaneous_files > FS_SUPPORTED_COUNT_MAX {
            self.max_simultaneous_files = FS_SUPPORTED_COUNT_MAX;
        }
        self
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.version_number;
        data[1] = self.max_simultaneous_files.min(FS_SUPPORTED_COUNT_MAX);
        let mut caps = 0u8;
        if self.supports_directories {
            caps |= 1 << 0;
        }
        if self.supports_volume_management {
            caps |= 1 << 1;
        }
        if self.supports_file_attributes {
            caps |= 1 << 2;
        }
        if self.supports_move_file {
            caps |= 1 << 3;
        }
        if self.supports_delete_file {
            caps |= 1 << 4;
        }
        data[2] = caps;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 3 || data.len() > 8 || data[3..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        if data[0] != FS_CLASSIC_PROPERTIES_VERSION || !fs_count_is_supported(data[1]) {
            return None;
        }
        let caps = data[2];
        if caps & !0x1F != 0 {
            return None;
        }
        Some(Self {
            version_number: data[0],
            max_simultaneous_files: data[1],
            supports_directories: caps & (1 << 0) != 0,
            supports_volume_management: caps & (1 << 1) != 0,
            supports_file_attributes: caps & (1 << 2) != 0,
            supports_move_file: caps & (1 << 3) != 0,
            supports_delete_file: caps & (1 << 4) != 0,
        })
    }
}

/// Directory listing entry.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileEntry {
    pub name: String,
    pub size: u32,
    pub attributes: u8,
    /// DOS-format date.
    pub date: u16,
    /// DOS-format time.
    pub time: u16,
}

impl FileEntry {
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        has_attribute(self.attributes, FileAttributes::Directory)
    }

    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        has_attribute(self.attributes, FileAttributes::ReadOnly)
    }
}

/// TAN cache entry for idempotent retry-handling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TANResponse {
    pub tan: TAN,
    pub response_data: Vec<u8>,
    pub timestamp_ms: u32,
}

impl Default for TANResponse {
    fn default() -> Self {
        Self {
            tan: INVALID_TAN,
            response_data: Vec::new(),
            timestamp_ms: 0,
        }
    }
}

impl TANResponse {
    #[must_use]
    pub fn is_expired(&self, current_time_ms: u32, timeout_ms: u32) -> bool {
        current_time_ms.saturating_sub(self.timestamp_ms) > timeout_ms
    }
}

/// File-Server Status (ISO 11783-13 §7.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileServerStatus {
    pub busy: bool,
    pub number_of_open_files: u8,
}

impl FileServerStatus {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = u8::from(self.busy);
        data[1] = self.number_of_open_files;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 || data.len() > 8 || data[2..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        if data[0] & !0x01 != 0 || !fs_count_is_supported(data[1]) {
            return None;
        }
        Some(Self {
            busy: data[0] & 0x01 != 0,
            number_of_open_files: data[1],
        })
    }
}

/// Client Connection Maintenance message (sent every 2 s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CCMMessage {
    pub version: u8,
    pub tan: TAN,
}

impl CCMMessage {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.version;
        data[1] = self.tan;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 || data.len() > 8 || data[2..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        Some(Self {
            version: data[0],
            tan: data[1],
        })
    }
}

// ─── Path utilities ────────────────────────────────────────────────────

/// Maximum advertised file-server volume-label length.
///
/// The v2 volume-status payload carries the label length in one byte, so
/// setters must reject labels that cannot be represented instead of letting
/// lower-level encoders truncate them silently.
pub const MAX_VOLUME_NAME_BYTES: usize = u8::MAX as usize;

#[must_use]
pub fn is_valid_path_component(path: &str) -> bool {
    if path.is_empty() || path == "." || path == ".." {
        return false;
    }
    !path.chars().any(|c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    })
}

#[must_use]
pub fn is_valid_volume_name(name: &str) -> bool {
    if name.is_empty() || !name.is_ascii() || name.len() > MAX_VOLUME_NAME_BYTES {
        return false;
    }
    !name.chars().any(|c| {
        c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
    })
}

#[must_use]
pub fn is_valid_fs_path(path: &str, allow_root: bool, allow_wildcards: bool) -> bool {
    if path.is_empty() {
        return false;
    }
    if path == "\\" || path == "\\\\" {
        return allow_root;
    }
    if path.chars().any(|c| {
        c.is_control()
            || matches!(c, '/' | ':' | '"' | '<' | '>' | '|')
            || (!allow_wildcards && matches!(c, '*' | '?'))
    }) {
        return false;
    }

    let mut body = path;
    while let Some(rest) = body.strip_prefix('\\') {
        body = rest;
    }
    while let Some(rest) = body.strip_suffix('\\') {
        body = rest;
    }
    if body.is_empty() {
        return allow_root;
    }
    if body.contains("\\\\") {
        return false;
    }

    body.split('\\').all(|component| {
        !component.is_empty()
            && component != "."
            && component != ".."
            && component.chars().all(|c| {
                !c.is_control()
                    && !matches!(c, '/' | '\\' | ':' | '"' | '<' | '>' | '|')
                    && (allow_wildcards || !matches!(c, '*' | '?'))
            })
    })
}

#[must_use]
pub fn is_absolute_path(path: &str) -> bool {
    path.starts_with('\\')
}

#[must_use]
pub fn has_wildcards(path: &str) -> bool {
    path.contains('*') || path.contains('?')
}

// ─── DOS date/time ─────────────────────────────────────────────────────

#[must_use]
pub const fn pack_dos_date(year: u16, month: u8, day: u8) -> u16 {
    ((year - 1980) << 9) | ((month as u16) << 5) | day as u16
}

#[must_use]
pub const fn pack_dos_time(hour: u8, minute: u8, second: u8) -> u16 {
    ((hour as u16) << 11) | ((minute as u16) << 5) | ((second / 2) as u16)
}

#[must_use]
pub const fn unpack_dos_date(dos_date: u16) -> (u16, u8, u8) {
    let year = ((dos_date >> 9) & 0x7F) + 1980;
    let month = ((dos_date >> 5) & 0x0F) as u8;
    let day = (dos_date & 0x1F) as u8;
    (year, month, day)
}

#[must_use]
pub const fn unpack_dos_time(dos_time: u16) -> (u8, u8, u8) {
    let hour = ((dos_time >> 11) & 0x1F) as u8;
    let minute = ((dos_time >> 5) & 0x3F) as u8;
    let second = ((dos_time & 0x1F) * 2) as u8;
    (hour, minute, second)
}

/// Return whether a DOS-format date/time pair stays inside the supported
/// File Server wire ranges.
///
/// A zero date or time is accepted as an unspecified value; non-zero fields
/// must decode to representable calendar/time components before they are
/// stored or surfaced as a successful File Server date/time result.
#[must_use]
pub fn dos_date_time_is_supported(date: u16, time: u16) -> bool {
    let date_ok = if date == 0 {
        true
    } else {
        let (_, month, day) = unpack_dos_date(date);
        (1..=12).contains(&month) && (1..=31).contains(&day)
    };
    let time_ok = if time == 0 {
        true
    } else {
        let (hour, minute, second) = unpack_dos_time(time);
        hour <= 23 && minute <= 59 && second <= 58
    };
    date_ok && time_ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_function_round_trip() {
        for f in [
            FSFunction::GetCurrentDirectory,
            FSFunction::OpenFile,
            FSFunction::ReadFile,
            FSFunction::FileServerStatus,
            FSFunction::VolumeStatus,
        ] {
            assert_eq!(FSFunction::from_u8(f.as_u8()), Some(f));
        }
        assert!(FSFunction::from_u8(0xFF).is_none());
    }

    #[test]
    fn volume_state_round_trip() {
        for s in [
            VolumeState::Present,
            VolumeState::InUse,
            VolumeState::PreparingForRemoval,
            VolumeState::Removed,
        ] {
            assert_eq!(VolumeState::from_u8(s.as_u8()), s);
            assert_eq!(VolumeState::try_from_u8(s.as_u8()), Some(s));
        }
        assert_eq!(VolumeState::try_from_u8(4), None);
    }

    #[test]
    fn file_server_properties_round_trip() {
        let p = FileServerProperties {
            version_number: FS_CLASSIC_PROPERTIES_VERSION,
            max_simultaneous_files: 32,
            supports_directories: true,
            supports_volume_management: false,
            supports_file_attributes: true,
            supports_move_file: false,
            supports_delete_file: true,
        };
        let bytes = p.encode();
        let d = FileServerProperties::decode(&bytes);
        assert_eq!(d, Some(p));
    }

    #[test]
    fn ccm_round_trip() {
        let m = CCMMessage { version: 1, tan: 7 };
        let d = CCMMessage::decode(&m.encode());
        assert_eq!(d, Some(m));
    }

    #[test]
    fn classic_fixed_size_decoders_reject_short_overlong_and_bad_padding() {
        let props = FileServerProperties::default().encode();
        assert!(FileServerProperties::decode(&props[..2]).is_none());
        assert!(FileServerProperties::decode(&[props.as_slice(), &[0xFF]].concat()).is_none());
        let mut bad_props = props;
        bad_props[3] = 0x00;
        assert!(FileServerProperties::decode(&bad_props).is_none());

        let status = FileServerStatus::default().encode();
        assert!(FileServerStatus::decode(&status[..1]).is_none());
        assert!(FileServerStatus::decode(&[status.as_slice(), &[0xFF]].concat()).is_none());
        let mut bad_status = status;
        bad_status[2] = 0x00;
        assert!(FileServerStatus::decode(&bad_status).is_none());

        let ccm = CCMMessage { version: 1, tan: 7 }.encode();
        assert!(CCMMessage::decode(&ccm[..1]).is_none());
        assert!(CCMMessage::decode(&[ccm.as_slice(), &[0xFF]].concat()).is_none());
        let mut bad_ccm = ccm;
        bad_ccm[2] = 0x00;
        assert!(CCMMessage::decode(&bad_ccm).is_none());
    }

    #[test]
    fn path_validation_rejects_special_chars() {
        assert!(is_valid_path_component("file.txt"));
        assert!(!is_valid_path_component(""));
        assert!(!is_valid_path_component("a/b"));
        assert!(!is_valid_path_component("a*b"));
        assert!(!is_valid_path_component("a:b"));
        assert!(!is_valid_path_component("."));
        assert!(!is_valid_path_component(".."));
        assert!(!is_valid_path_component("bad\0name"));
    }

    #[test]
    fn volume_name_validation_rejects_unencodable_labels() {
        assert!(is_valid_volume_name("ISOFS"));
        assert!(is_valid_volume_name(&"A".repeat(MAX_VOLUME_NAME_BYTES)));

        assert!(!is_valid_volume_name(""));
        assert!(!is_valid_volume_name(
            &"A".repeat(MAX_VOLUME_NAME_BYTES + 1)
        ));
        assert!(!is_valid_volume_name("host/path"));
        assert!(!is_valid_volume_name("host\\path"));
        assert!(!is_valid_volume_name("bad:name"));
        assert!(!is_valid_volume_name("bad\0name"));
        assert!(!is_valid_volume_name("CAFÉ"));
    }

    #[test]
    fn full_path_validation_rejects_traversal_and_host_paths() {
        assert!(is_valid_fs_path("file.txt", false, false));
        assert!(is_valid_fs_path("\\dir\\file.txt", false, false));
        assert!(is_valid_fs_path("\\", true, false));
        assert!(is_valid_fs_path("*.txt", false, true));

        assert!(!is_valid_fs_path("\\", false, false));
        assert!(!is_valid_fs_path("..\\secret.txt", false, false));
        assert!(!is_valid_fs_path("dir\\..\\secret.txt", false, false));
        assert!(!is_valid_fs_path("dir\\.\\secret.txt", false, false));
        assert!(!is_valid_fs_path("dir\\\\secret.txt", false, false));
        assert!(!is_valid_fs_path("../secret.txt", false, false));
        assert!(!is_valid_fs_path("c:\\secret.txt", false, false));
        assert!(!is_valid_fs_path("bad|name.txt", false, false));
        assert!(!is_valid_fs_path("bad\0name.txt", false, false));
        assert!(!is_valid_fs_path("*.txt", false, false));
    }

    #[test]
    fn absolute_path_detection() {
        assert!(is_absolute_path("\\"));
        assert!(is_absolute_path("\\foo"));
        assert!(is_absolute_path("\\\\foo"));
        assert!(!is_absolute_path("foo"));
    }

    #[test]
    fn wildcard_detection() {
        assert!(has_wildcards("foo*"));
        assert!(has_wildcards("a?b"));
        assert!(!has_wildcards("plain.txt"));
    }

    #[test]
    fn dos_date_time_round_trip() {
        let date = pack_dos_date(2026, 5, 2);
        let (y, m, d) = unpack_dos_date(date);
        assert_eq!((y, m, d), (2026, 5, 2));
        let time = pack_dos_time(14, 30, 22);
        let (h, mn, s) = unpack_dos_time(time);
        assert_eq!((h, mn, s), (14, 30, 22));
        assert!(dos_date_time_is_supported(date, time));
        assert!(dos_date_time_is_supported(0, 0));
        assert!(!dos_date_time_is_supported((46u16 << 9) | 1, time));
        assert!(!dos_date_time_is_supported(date, 24u16 << 11));
    }

    #[test]
    fn file_entry_attribute_helpers() {
        let dir = FileEntry {
            attributes: FileAttributes::Directory.bit(),
            ..Default::default()
        };
        assert!(dir.is_directory());
        assert!(!dir.is_read_only());
    }

    #[test]
    fn tan_response_expiry() {
        let r = TANResponse {
            tan: 1,
            response_data: vec![],
            timestamp_ms: 100,
        };
        assert!(!r.is_expired(150, 100)); // age 50 < 100
        assert!(r.is_expired(300, 100)); // age 200 > 100
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_fs_classic_decoders_accept_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=64),
        ) {
            if let Some(props) = FileServerProperties::decode(&data) {
                let _ = props.encode();
            }

            if let Some(status) = FileServerStatus::decode(&data) {
                let _ = status.encode();
            }

            if let Some(ccm) = CCMMessage::decode(&data) {
                let _ = ccm.encode();
            }
        }
    }
}
