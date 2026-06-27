//! ISO 11783-13 legacy file-transfer types (PGN 0xAA00 / 0xAB00).
//!
//! Mirrors the data types from C++ `machbus::isobus::file_transfer.hpp`.
//! The C++ `FileServer` and `FileClient` classes are
//! **superseded** by the `isobus::fs::*` modules (Phase 15) and are
//! not ported. The wire enums and configuration / property structs
//! are kept here for users still on the legacy PGNs.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

/// File-operation control byte (`data\[0\]` in client→server PGN).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FileOperation {
    Read = 0x01,
    Write = 0x02,
    Delete = 0x03,
    List = 0x04,
    GetAttributes = 0x05,
    SetAttributes = 0x06,
    OpenFile = 0x10,
    CloseFile = 0x11,
    ReadData = 0x12,
    WriteData = 0x13,
    SeekFile = 0x14,
    GetCurrentDir = 0x20,
    ChangeCurrentDir = 0x21,
    MakeDir = 0x22,
    RemoveDir = 0x23,
    MoveFile = 0x30,
    CopyFile = 0x31,
    GetFileSize = 0x40,
    GetFreeSpace = 0x41,
    GetVolumeInfo = 0x50,
    GetServerStatus = 0x60,
}

impl FileOperation {
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        Some(match v {
            0x01 => Self::Read,
            0x02 => Self::Write,
            0x03 => Self::Delete,
            0x04 => Self::List,
            0x05 => Self::GetAttributes,
            0x06 => Self::SetAttributes,
            0x10 => Self::OpenFile,
            0x11 => Self::CloseFile,
            0x12 => Self::ReadData,
            0x13 => Self::WriteData,
            0x14 => Self::SeekFile,
            0x20 => Self::GetCurrentDir,
            0x21 => Self::ChangeCurrentDir,
            0x22 => Self::MakeDir,
            0x23 => Self::RemoveDir,
            0x30 => Self::MoveFile,
            0x31 => Self::CopyFile,
            0x40 => Self::GetFileSize,
            0x41 => Self::GetFreeSpace,
            0x50 => Self::GetVolumeInfo,
            0x60 => Self::GetServerStatus,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Error code returned in server responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FileTransferError {
    #[default]
    NoError = 0x00,
    FileNotFound = 0x01,
    AccessDenied = 0x02,
    DiskFull = 0x03,
    InvalidFilename = 0x04,
    ServerBusy = 0x05,
    InvalidHandle = 0x06,
    EndOfFile = 0x07,
    VolumeNotMounted = 0x08,
    IoError = 0x09,
    InvalidSeekPosition = 0x0A,
    InvalidParameter = 0x0B,
    FileAlreadyOpen = 0x0C,
    DirectoryNotEmpty = 0x0D,
    Unknown = 0xFF,
}

impl FileTransferError {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match Self::try_from_u8(v) {
            Some(error) => error,
            None => Self::Unknown,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::NoError),
            0x01 => Some(Self::FileNotFound),
            0x02 => Some(Self::AccessDenied),
            0x03 => Some(Self::DiskFull),
            0x04 => Some(Self::InvalidFilename),
            0x05 => Some(Self::ServerBusy),
            0x06 => Some(Self::InvalidHandle),
            0x07 => Some(Self::EndOfFile),
            0x08 => Some(Self::VolumeNotMounted),
            0x09 => Some(Self::IoError),
            0x0A => Some(Self::InvalidSeekPosition),
            0x0B => Some(Self::InvalidParameter),
            0x0C => Some(Self::FileAlreadyOpen),
            0x0D => Some(Self::DirectoryNotEmpty),
            0xFF => Some(Self::Unknown),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FileServerState {
    #[default]
    Idle,
    Active,
    Busy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FileClientState {
    #[default]
    Idle,
    Transferring,
    Complete,
    Error,
}

/// File attribute bitflags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FileAttribute {
    ReadOnly = 0x01,
    Hidden = 0x02,
    System = 0x04,
    Directory = 0x10,
    Archive = 0x20,
    Volume = 0x40,
}

impl FileAttribute {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Per-file properties.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileProperties {
    pub name: String,
    pub size_bytes: u32,
    /// Bitwise OR of [`FileAttribute`] values.
    pub attributes: u8,
    /// Packed DOS-format date.
    pub date: u32,
    /// Packed DOS-format time.
    pub time: u32,
}

impl FileProperties {
    #[inline]
    #[must_use]
    pub const fn is_directory(&self) -> bool {
        (self.attributes & FileAttribute::Directory.as_u8()) != 0
    }

    #[inline]
    #[must_use]
    pub const fn is_read_only(&self) -> bool {
        (self.attributes & FileAttribute::ReadOnly.as_u8()) != 0
    }

    #[inline]
    #[must_use]
    pub const fn is_hidden(&self) -> bool {
        (self.attributes & FileAttribute::Hidden.as_u8()) != 0
    }
}

/// Volume metadata.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VolumeInfo {
    pub name: String,
    pub total_bytes: u32,
    pub free_bytes: u32,
    pub attributes: u8,
    pub removable: bool,
}

/// Periodic interval for `Active` server status broadcasts.
pub const FILE_SERVER_STATUS_INTERVAL_MS: u32 = 2_000;
/// Faster interval used while the server is `Busy`.
pub const FILE_SERVER_BUSY_STATUS_INTERVAL_MS: u32 = 200;
/// Client-side request timeout.
pub const FS_REQUEST_TIMEOUT_MS: u32 = 6_000;

/// Server config.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileServerConfig {
    pub base_path: String,
    pub status_interval_ms: u32,
    pub max_open_files: u8,
    pub volume_name: String,
    pub volume_total_bytes: u32,
    pub volume_free_bytes: u32,
}

impl Default for FileServerConfig {
    fn default() -> Self {
        Self {
            base_path: String::new(),
            status_interval_ms: FILE_SERVER_STATUS_INTERVAL_MS,
            max_open_files: 16,
            volume_name: "ISOBUS".to_string(),
            volume_total_bytes: 1024 * 1024,
            volume_free_bytes: 512 * 1024,
        }
    }
}

impl FileServerConfig {
    #[must_use]
    pub fn path(mut self, p: impl Into<String>) -> Self {
        self.base_path = p.into();
        self
    }

    #[must_use]
    pub fn status_interval(mut self, ms: u32) -> Self {
        self.status_interval_ms = ms;
        self
    }

    #[must_use]
    pub fn max_files(mut self, n: u8) -> Self {
        self.max_open_files = n;
        self
    }

    #[must_use]
    pub fn volume(mut self, name: impl Into<String>, total: u32, free: u32) -> Self {
        self.volume_name = name.into();
        self.volume_total_bytes = total;
        self.volume_free_bytes = free;
        self
    }
}

/// Per-handle open-file state used by a server implementation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OpenFileState {
    pub filename: String,
    pub data: Vec<u8>,
    pub position: u32,
    pub writable: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_operation_byte_values_match_spec() {
        assert_eq!(FileOperation::Read.as_u8(), 0x01);
        assert_eq!(FileOperation::OpenFile.as_u8(), 0x10);
        assert_eq!(FileOperation::GetCurrentDir.as_u8(), 0x20);
        assert_eq!(FileOperation::MoveFile.as_u8(), 0x30);
        assert_eq!(FileOperation::GetVolumeInfo.as_u8(), 0x50);
    }

    #[test]
    fn file_operation_from_u8_rejects_reserved_codes() {
        for op in [
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
        ] {
            assert_eq!(FileOperation::from_u8(op.as_u8()), Some(op));
        }
        for reserved in [0x00, 0x07, 0x0F, 0x15, 0x24, 0x32, 0x42, 0x51, 0xFF] {
            assert_eq!(FileOperation::from_u8(reserved), None);
        }
    }

    #[test]
    fn file_transfer_error_round_trips() {
        for e in [
            FileTransferError::NoError,
            FileTransferError::FileNotFound,
            FileTransferError::DiskFull,
            FileTransferError::EndOfFile,
            FileTransferError::FileAlreadyOpen,
            FileTransferError::Unknown,
        ] {
            assert_eq!(FileTransferError::from_u8(e.as_u8()), e);
        }
        // Unknown value collapses to Unknown.
        assert_eq!(FileTransferError::from_u8(0xAA), FileTransferError::Unknown);
    }

    #[test]
    fn file_properties_attribute_queries() {
        let dir = FileProperties {
            name: "logs".into(),
            attributes: FileAttribute::Directory.as_u8(),
            ..Default::default()
        };
        assert!(dir.is_directory());
        assert!(!dir.is_read_only());

        let ro_file = FileProperties {
            name: "config.txt".into(),
            attributes: FileAttribute::ReadOnly.as_u8() | FileAttribute::Hidden.as_u8(),
            size_bytes: 1024,
            ..Default::default()
        };
        assert!(!ro_file.is_directory());
        assert!(ro_file.is_read_only());
        assert!(ro_file.is_hidden());
    }

    #[test]
    fn file_server_config_fluent() {
        let cfg = FileServerConfig::default()
            .path("/data/iso")
            .max_files(8)
            .volume("HOMEFARM", 4 * 1024 * 1024, 1024 * 1024);
        assert_eq!(cfg.base_path, "/data/iso");
        assert_eq!(cfg.max_open_files, 8);
        assert_eq!(cfg.volume_name, "HOMEFARM");
    }

    #[test]
    fn intervals_match_spec() {
        assert_eq!(FILE_SERVER_STATUS_INTERVAL_MS, 2_000);
        assert_eq!(FILE_SERVER_BUSY_STATUS_INTERVAL_MS, 200);
        assert_eq!(FS_REQUEST_TIMEOUT_MS, 6_000);
    }
}
