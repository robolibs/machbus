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

#[must_use]
pub const fn fs_count_is_supported(count: u8) -> bool {
    count <= FS_SUPPORTED_COUNT_MAX
}

/// FS command byte (ISO 11783-13:2022 Annex C).
///
/// Bits 7-4 are the command group (B.1: 0000 connection management, 0001
/// directory handling, 0010 file access, 0011 file handling, 0100 volume
/// handling) and bits 3-0 the function within it. A response carries the same
/// byte as its request.
///
/// Not one of these bytes used to match the standard: the crate opened a file
/// with 0x02, which a conformant server reads as Volume Status, and read a
/// conformant client's Open File (0x20) as its own Initialize Volume. Nothing
/// interoperated in either direction.
///
/// Five former variants have no command byte in any edition. Directory
/// creation goes through Open File with the create-directory flags (C.3.2.2),
/// directory removal through Delete File, and free space is reported inside
/// the Get Current Directory Response (C.2.2.3); Copy File and Get File Size
/// have no ISO equivalent at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FSFunction {
    /// Server to client. The same byte from a client is a Client Connection
    /// Maintenance message (C.1.3); the direction tells them apart.
    #[default]
    FileServerStatus = 0x00,
    GetFileServerProperties = 0x01,
    VolumeStatus = 0x02,
    GetCurrentDirectory = 0x10,
    ChangeDirectory = 0x11,
    OpenFile = 0x20,
    SeekFile = 0x21,
    ReadFile = 0x22,
    WriteFile = 0x23,
    CloseFile = 0x24,
    MoveFile = 0x30,
    DeleteFile = 0x31,
    GetFileAttributes = 0x32,
    SetFileAttributes = 0x33,
    GetFileDateTime = 0x34,
    InitializeVolume = 0x40,
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
            0x00 => Some(Self::FileServerStatus),
            0x01 => Some(Self::GetFileServerProperties),
            0x02 => Some(Self::VolumeStatus),
            0x10 => Some(Self::GetCurrentDirectory),
            0x11 => Some(Self::ChangeDirectory),
            0x20 => Some(Self::OpenFile),
            0x21 => Some(Self::SeekFile),
            0x22 => Some(Self::ReadFile),
            0x23 => Some(Self::WriteFile),
            0x24 => Some(Self::CloseFile),
            0x30 => Some(Self::MoveFile),
            0x31 => Some(Self::DeleteFile),
            0x32 => Some(Self::GetFileAttributes),
            0x33 => Some(Self::SetFileAttributes),
            0x34 => Some(Self::GetFileDateTime),
            0x40 => Some(Self::InitializeVolume),
            _ => None,
        }
    }
}

/// The Client Connection Maintenance command byte (C.1.3 byte 1: command
/// group 0000, function 0000). It shares its value with the File Server Status
/// broadcast, which only ever travels the other way.
pub const CCM_FUNCTION_CODE: u8 = FSFunction::FileServerStatus.as_u8();

/// The B.5 Version Number this implementation reports: 4, third published
/// edition. B.5 also forbids rejecting a peer over the version it reports, so
/// this is advertised but never used as an acceptance test.
pub const FS_VERSION_NUMBER: u8 = 4;

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

/// Get File Server Properties Response (ISO 11783-13:2022 C.1.5).
///
/// Byte 1 command, byte 2 TAN, byte 3 version number (B.5), byte 4 maximum
/// simultaneously open files (B.6), byte 5 capabilities (B.7), bytes 6-8
/// reserved 0xFF. There is no error-code byte in this response.
///
/// Two things used to be wrong beyond the layout. The decoder rejected any
/// version but 1, which B.5 forbids in as many words — "shall not reject
/// communication or the request based on the reported Version Number" — so a
/// client could not read the properties of any server built against the 2019
/// or 2022 edition, and since C.1.4 is what a client sends before connecting,
/// the connection never got started. And the capability byte carried five
/// invented bits: B.7 defines only bit 0 (multiple volumes) and bit 1
/// (removable volumes), so a conformant client read "supports directories" as
/// "supports multiple volumes" and went looking for volumes that did not
/// exist. Per-command support is signalled with error code 12, not here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileServerProperties {
    pub version_number: u8,
    pub max_simultaneous_files: u8,
    pub supports_multiple_volumes: bool,
    pub supports_removable_volumes: bool,
}

/// B.7 bit 0.
pub const FS_CAPABILITY_MULTIPLE_VOLUMES: u8 = 1 << 0;
/// B.7 bit 1.
pub const FS_CAPABILITY_REMOVABLE_VOLUMES: u8 = 1 << 1;

impl Default for FileServerProperties {
    fn default() -> Self {
        Self {
            version_number: FS_VERSION_NUMBER,
            max_simultaneous_files: 16,
            supports_multiple_volumes: false,
            supports_removable_volumes: true,
        }
    }
}

impl FileServerProperties {
    #[must_use]
    pub const fn normalized_for_wire(mut self) -> Self {
        if self.max_simultaneous_files > FS_SUPPORTED_COUNT_MAX {
            self.max_simultaneous_files = FS_SUPPORTED_COUNT_MAX;
        }
        self
    }

    #[must_use]
    pub fn encode_response(&self, tan: TAN) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = FSFunction::GetFileServerProperties.as_u8();
        data[1] = tan;
        data[2] = self.version_number;
        data[3] = self.max_simultaneous_files.min(FS_SUPPORTED_COUNT_MAX);
        let mut caps = 0u8;
        if self.supports_multiple_volumes {
            caps |= FS_CAPABILITY_MULTIPLE_VOLUMES;
        }
        if self.supports_removable_volumes {
            caps |= FS_CAPABILITY_REMOVABLE_VOLUMES;
        }
        data[4] = caps;
        data
    }

    /// Decode a C.1.5 response into `(tan, properties)`.
    #[must_use]
    pub fn decode_response(data: &[u8]) -> Option<(TAN, Self)> {
        if data.len() < 5 || data.len() > 8 || data[5..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        if data[0] != FSFunction::GetFileServerProperties.as_u8() {
            return None;
        }
        if !fs_count_is_supported(data[3]) {
            return None;
        }
        let caps = data[4];
        if caps & !(FS_CAPABILITY_MULTIPLE_VOLUMES | FS_CAPABILITY_REMOVABLE_VOLUMES) != 0 {
            return None;
        }
        Some((
            data[1],
            Self {
                // B.5: any version is accepted and reported as-is.
                version_number: data[2],
                max_simultaneous_files: data[3],
                supports_multiple_volumes: caps & FS_CAPABILITY_MULTIPLE_VOLUMES != 0,
                supports_removable_volumes: caps & FS_CAPABILITY_REMOVABLE_VOLUMES != 0,
            },
        ))
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

    /// B.15 bit 3. A client walking `\\` for the volume list reads this;
    /// under the old DOS-shaped table it was never set, so the list came back
    /// empty while fixed-media directories were classified as volumes.
    #[must_use]
    pub const fn is_volume(&self) -> bool {
        has_attribute(self.attributes, FileAttributes::IsVolume)
    }

    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        has_attribute(self.attributes, FileAttributes::Hidden)
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

/// File Server Status broadcast (ISO 11783-13:2022 C.1.2).
///
/// Byte 1 is the command byte, byte 2 the B.3 status bitfield, byte 3 the
/// number of open files, bytes 4-8 reserved 0xFF. The frame used to start
/// straight at the status byte, with no command byte at all.
///
/// B.3 gives byte 2 two independent flags. The crate modelled one, and its
/// decoder rejected anything outside bit 0 — so a server flushing a write
/// (bit 1) had its status dropped by the client. That is precisely the frame
/// 4.3.3 relies on to extend the request timeout from 600 ms, so the client
/// gave up on a legitimately slow write and reported a spurious fault.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FileServerStatus {
    pub busy_reading: bool,
    pub busy_writing: bool,
    pub number_of_open_files: u8,
}

/// B.3 bit 0.
pub const FS_STATUS_BUSY_READING: u8 = 1 << 0;
/// B.3 bit 1.
pub const FS_STATUS_BUSY_WRITING: u8 = 1 << 1;

impl FileServerStatus {
    /// Whether the server is busy at all — the condition 4.3.3 ties the
    /// extended request timeout to.
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy_reading || self.busy_writing
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = FSFunction::FileServerStatus.as_u8();
        let mut status = 0u8;
        if self.busy_reading {
            status |= FS_STATUS_BUSY_READING;
        }
        if self.busy_writing {
            status |= FS_STATUS_BUSY_WRITING;
        }
        data[1] = status;
        data[2] = self.number_of_open_files;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 3 || data.len() > 8 || data[3..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        if data[0] != FSFunction::FileServerStatus.as_u8() {
            return None;
        }
        let status = data[1];
        if status & !(FS_STATUS_BUSY_READING | FS_STATUS_BUSY_WRITING) != 0
            || !fs_count_is_supported(data[2])
        {
            return None;
        }
        Some(Self {
            busy_reading: status & FS_STATUS_BUSY_READING != 0,
            busy_writing: status & FS_STATUS_BUSY_WRITING != 0,
            number_of_open_files: data[2],
        })
    }
}

/// Client Connection Maintenance message (ISO 11783-13:2022 C.1.3).
///
/// Byte 1 is the command byte, byte 2 the client's version number, bytes 3-8
/// reserved 0xFF. There is no TAN: 4.10 has the CCM *establish* the connection
/// before the client sends anything that carries one.
///
/// The crate used to hold two mutually inconsistent encodings, neither of them
/// C.1.3. One put 0xFF — a value the standard defines as no command at all —
/// in byte 1, so a conformant server NACKed it and the client never connected;
/// the other put the version number in the command slot, which reads as Get
/// File Server Properties with a rolling TAN in the version field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CCMMessage {
    pub version: u8,
}

impl CCMMessage {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = CCM_FUNCTION_CODE;
        data[1] = self.version;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 2 || data.len() > 8 || data[2..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        if data[0] != CCM_FUNCTION_CODE {
            return None;
        }
        Some(Self { version: data[1] })
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

/// Whether `path` is a well-formed, already-normalized ISO 11783-13 path.
///
/// This is a *post*-normalization check: `.` and `..` are rejected because the
/// file server resolves them away first (A.2.3.1 makes that mandatory, not
/// optional), so any that survive to here mean the normalizer was bypassed.
#[must_use]
pub fn is_valid_fs_path(path: &str, allow_root: bool, allow_wildcards: bool) -> bool {
    if path.is_empty() {
        return false;
    }
    let normalized = path.replace('/', "\\");
    if normalized == "\\" || normalized == "\\\\" {
        return allow_root;
    }
    if normalized.chars().any(|c| {
        c.is_control()
            || matches!(c, ':' | '"' | '<' | '>' | '|')
            || (!allow_wildcards && matches!(c, '*' | '?'))
    }) {
        return false;
    }

    let mut body = normalized.as_str();
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
            && component.len() <= 255
            && component.chars().all(|c| {
                !c.is_control()
                    && !matches!(c, '/' | '\\' | ':' | '"' | '<' | '>' | '|')
                    && (allow_wildcards || !matches!(c, '*' | '?'))
            })
    })
}

/// Strict DOS 8.3 path validation helper (stem <= 8 chars, extension <= 3 chars).
pub fn is_dos_8_3_path(path: &str) -> bool {
    let normalized = path.replace('/', "\\");
    if !is_valid_fs_path(&normalized, true, false) {
        return false;
    }
    let mut body = normalized.as_str();
    while let Some(rest) = body.strip_prefix('\\') {
        body = rest;
    }
    while let Some(rest) = body.strip_suffix('\\') {
        body = rest;
    }
    if body.is_empty() {
        return true;
    }
    body.split('\\').all(|component| {
        let parts: Vec<&str> = component.split('.').collect();
        if parts.len() > 2 {
            return false;
        }
        if parts[0].is_empty() || parts[0].len() > 8 {
            return false;
        }
        if parts.len() == 2 && parts[1].len() > 3 {
            return false;
        }
        true
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

    /// The Annex C command bytes verbatim. Every one of these used to be a
    /// different value, so nothing interoperated in either direction.
    #[test]
    fn fs_function_codes_match_annex_c() {
        for (code, function) in [
            (0x00, FSFunction::FileServerStatus),
            (0x01, FSFunction::GetFileServerProperties),
            (0x02, FSFunction::VolumeStatus),
            (0x10, FSFunction::GetCurrentDirectory),
            (0x11, FSFunction::ChangeDirectory),
            (0x20, FSFunction::OpenFile),
            (0x21, FSFunction::SeekFile),
            (0x22, FSFunction::ReadFile),
            (0x23, FSFunction::WriteFile),
            (0x24, FSFunction::CloseFile),
            (0x30, FSFunction::MoveFile),
            (0x31, FSFunction::DeleteFile),
            (0x32, FSFunction::GetFileAttributes),
            (0x33, FSFunction::SetFileAttributes),
            (0x34, FSFunction::GetFileDateTime),
            (0x40, FSFunction::InitializeVolume),
        ] {
            assert_eq!(FSFunction::try_from_u8(code), Some(function));
            assert_eq!(function.as_u8(), code);
        }

        // The five invented commands, and the 0xFF that used to stand in for
        // the CCM, are not commands.
        for undefined in [0x03, 0x12, 0x15, 0x18, 0x19, 0x25, 0x35, 0x41, 0xFF] {
            assert_eq!(FSFunction::from_u8(undefined), None);
        }
        assert_eq!(CCM_FUNCTION_CODE, 0x00);
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

    /// C.1.5, plus the B.5 rule that a peer's version is never a reason to
    /// reject it. Decoding used to demand version 1, so no server built to
    /// the 2019 or 2022 edition could be talked to at all.
    #[test]
    fn file_server_properties_match_c_1_5_and_accept_any_version() {
        let p = FileServerProperties {
            version_number: FS_VERSION_NUMBER,
            max_simultaneous_files: 32,
            supports_multiple_volumes: true,
            supports_removable_volumes: false,
        };
        assert_eq!(
            p.encode_response(0x07),
            [0x01, 0x07, 0x04, 32, 0x01, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            FileServerProperties::decode_response(&p.encode_response(0x07)),
            Some((0x07, p))
        );

        for version in [0, 1, 2, 3, 4, 200, 0xFF] {
            let other = FileServerProperties {
                version_number: version,
                ..p
            };
            assert_eq!(
                FileServerProperties::decode_response(&other.encode_response(0x07)),
                Some((0x07, other)),
                "B.5 forbids rejecting a peer over its reported version"
            );
        }

        // B.7 bits 2-7 are reserved and sent as zero.
        let mut reserved = p.encode_response(0x07);
        reserved[4] |= 0x04;
        assert_eq!(FileServerProperties::decode_response(&reserved), None);
    }

    #[test]
    fn ccm_matches_c_1_3() {
        let m = CCMMessage { version: 4 };
        assert_eq!(m.encode(), [0x00, 0x04, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(CCMMessage::decode(&m.encode()), Some(m));

        // The old encoding put the version in the command slot and a TAN in
        // the version slot; a conformant server read it as a properties query.
        assert_eq!(
            CCMMessage::decode(&[0x04, 0x07, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]),
            None
        );
    }

    #[test]
    fn classic_fixed_size_decoders_reject_short_overlong_and_bad_padding() {
        let props = FileServerProperties::default().encode_response(0x07);
        assert!(FileServerProperties::decode_response(&props[..4]).is_none());
        assert!(
            FileServerProperties::decode_response(&[props.as_slice(), &[0xFF]].concat()).is_none()
        );
        let mut bad_props = props;
        bad_props[5] = 0x00;
        assert!(FileServerProperties::decode_response(&bad_props).is_none());

        let status = FileServerStatus::default().encode();
        assert!(FileServerStatus::decode(&status[..1]).is_none());
        assert!(FileServerStatus::decode(&[status.as_slice(), &[0xFF]].concat()).is_none());
        let mut bad_status = status;
        bad_status[3] = 0x00;
        assert!(FileServerStatus::decode(&bad_status).is_none());

        let ccm = CCMMessage { version: 4 }.encode();
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
            if let Some((tan, props)) = FileServerProperties::decode_response(&data) {
                let _ = props.encode_response(tan);
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
