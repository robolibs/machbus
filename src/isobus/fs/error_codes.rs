//! ISO 11783-13 File Server error codes + open / attribute flags.
//!
//! Mirrors the C++ `machbus::isobus::fs::error_codes.hpp`.

/// 48-entry standard error set (only the populated codes; gaps are
/// reserved per the spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FSError {
    #[default]
    Success = 0,
    AccessDenied = 1,
    InvalidAccess = 2,
    TooManyOpen = 3,
    NotFound = 4,
    WrongType = 5,
    MaxHandles = 6,
    InvalidHandle = 7,
    InvalidSourceName = 8,
    InvalidDestName = 9,
    NoSpace = 10,
    WriteFail = 11,
    MediaNotPresent = 12,
    NotInitialized = 13,
    NotSupported = 20,
    InvalidLength = 42,
    OutOfMemory = 43,
    OtherError = 44,
    EndOfFile = 45,
    TANError = 46,
    MalformedRequest = 47,
}

impl FSError {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match Self::try_from_u8(v) {
            Some(error) => error,
            None => Self::OtherError,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Success),
            1 => Some(Self::AccessDenied),
            2 => Some(Self::InvalidAccess),
            3 => Some(Self::TooManyOpen),
            4 => Some(Self::NotFound),
            5 => Some(Self::WrongType),
            6 => Some(Self::MaxHandles),
            7 => Some(Self::InvalidHandle),
            8 => Some(Self::InvalidSourceName),
            9 => Some(Self::InvalidDestName),
            10 => Some(Self::NoSpace),
            11 => Some(Self::WriteFail),
            12 => Some(Self::MediaNotPresent),
            13 => Some(Self::NotInitialized),
            20 => Some(Self::NotSupported),
            42 => Some(Self::InvalidLength),
            43 => Some(Self::OutOfMemory),
            44 => Some(Self::OtherError),
            45 => Some(Self::EndOfFile),
            46 => Some(Self::TANError),
            47 => Some(Self::MalformedRequest),
            _ => None,
        }
    }

    /// Short string suitable for logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::AccessDenied => "Access Denied",
            Self::InvalidAccess => "Invalid Access",
            Self::TooManyOpen => "Too Many Open",
            Self::NotFound => "Not Found",
            Self::WrongType => "Wrong Type",
            Self::MaxHandles => "Max Handles",
            Self::InvalidHandle => "Invalid Handle",
            Self::InvalidSourceName => "Invalid Source Name",
            Self::InvalidDestName => "Invalid Dest Name",
            Self::NoSpace => "No Space",
            Self::WriteFail => "Write Fail",
            Self::MediaNotPresent => "Media Not Present",
            Self::NotInitialized => "Not Initialized",
            Self::NotSupported => "Not Supported",
            Self::InvalidLength => "Invalid Length",
            Self::OutOfMemory => "Out Of Memory",
            Self::OtherError => "Other Error",
            Self::EndOfFile => "End Of File",
            Self::TANError => "TAN Error",
            Self::MalformedRequest => "Malformed Request",
        }
    }

    /// Long-form description.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Success => "Operation completed successfully",
            Self::AccessDenied => "File access denied due to insufficient permissions",
            Self::InvalidAccess => "Invalid access mode requested (read/write/append)",
            Self::TooManyOpen => "Too many files already open by this client",
            Self::NotFound => "File or directory not found at specified path",
            Self::WrongType => "Wrong type - expected file but found directory or vice versa",
            Self::MaxHandles => "Maximum number of file handles reached server-wide",
            Self::InvalidHandle => "Invalid file handle specified in request",
            Self::InvalidSourceName => "Invalid source filename (illegal characters or format)",
            Self::InvalidDestName => "Invalid destination filename (illegal characters or format)",
            Self::NoSpace => "Insufficient space on volume for operation",
            Self::WriteFail => "Write operation failed (I/O error or media fault)",
            Self::MediaNotPresent => "Removable media not present in drive",
            Self::NotInitialized => "File system not initialized or mount failed",
            Self::NotSupported => "Operation not supported by this file server",
            Self::InvalidLength => "Invalid data length in request",
            Self::OutOfMemory => "Insufficient memory to complete operation",
            Self::OtherError => "Other unspecified error occurred",
            Self::EndOfFile => "End of file reached during read operation",
            Self::TANError => "Transaction number (TAN) mismatch or error",
            Self::MalformedRequest => "Request message is malformed or invalid",
        }
    }

    /// Indicates a non-recoverable condition.
    #[must_use]
    pub const fn is_fatal(self) -> bool {
        matches!(
            self,
            Self::OutOfMemory | Self::NotInitialized | Self::MediaNotPresent
        )
    }

    /// Indicates a retry might succeed.
    #[must_use]
    pub const fn is_retryable(self) -> bool {
        matches!(self, Self::TooManyOpen | Self::MaxHandles | Self::WriteFail)
    }
}

#[must_use]
pub const fn fs_error_byte_is_valid(v: u8) -> bool {
    FSError::try_from_u8(v).is_some()
}

impl core::fmt::Display for FSError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[must_use]
pub const fn fs_error_to_string(error: FSError) -> &'static str {
    error.as_str()
}

#[must_use]
pub const fn fs_error_description(error: FSError) -> &'static str {
    error.description()
}

#[must_use]
pub const fn is_fatal_error(error: FSError) -> bool {
    error.is_fatal()
}

#[must_use]
pub const fn is_retryable_error(error: FSError) -> bool {
    error.is_retryable()
}

// ─── OpenFlags ─────────────────────────────────────────────────────────

/// Bitmask flags for `OpenFile` operations. Treat as a `u8`-OR'able
/// set. Access mode (read/write/read-write/dir) lives in the low two
/// bits; `Create`/`Append`/`Exclusive` are independent flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum OpenFlags {
    #[default]
    Read = 0x00,
    Write = 0x01,
    ReadWrite = 0x02,
    OpenDir = 0x03,
    Create = 0x04,
    Append = 0x08,
    Exclusive = 0x10,
}

impl OpenFlags {
    #[inline]
    #[must_use]
    pub const fn bit(self) -> u8 {
        self as u8
    }
}

impl core::ops::BitOr for OpenFlags {
    type Output = u8;
    fn bitor(self, rhs: Self) -> u8 {
        self as u8 | rhs as u8
    }
}

impl core::ops::BitOr<u8> for OpenFlags {
    type Output = u8;
    fn bitor(self, rhs: u8) -> u8 {
        self as u8 | rhs
    }
}

#[must_use]
pub const fn has_flag(flags: u8, flag: OpenFlags) -> bool {
    flags & (flag as u8) != 0
}

#[must_use]
pub const fn get_access_mode(flags: u8) -> u8 {
    flags & 0x03
}

#[must_use]
pub const fn open_flags_have_no_reserved_bits(flags: u8) -> bool {
    flags
        & !(OpenFlags::OpenDir as u8
            | OpenFlags::Create as u8
            | OpenFlags::Append as u8
            | OpenFlags::Exclusive as u8)
        == 0
}

// ─── FileAttributes ────────────────────────────────────────────────────

/// File attribute flags (mirrors DOS/FAT semantics).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FileAttributes {
    #[default]
    None = 0x00,
    ReadOnly = 0x01,
    Hidden = 0x02,
    System = 0x04,
    Directory = 0x10,
    Archive = 0x20,
    Volume = 0x40,
}

impl FileAttributes {
    #[inline]
    #[must_use]
    pub const fn bit(self) -> u8 {
        self as u8
    }
}

impl core::ops::BitOr for FileAttributes {
    type Output = u8;
    fn bitor(self, rhs: Self) -> u8 {
        self as u8 | rhs as u8
    }
}

#[must_use]
pub const fn has_attribute(attrs: u8, attr: FileAttributes) -> bool {
    attrs & (attr as u8) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fs_error_round_trip() {
        for e in [
            FSError::Success,
            FSError::AccessDenied,
            FSError::EndOfFile,
            FSError::TANError,
            FSError::MalformedRequest,
        ] {
            assert_eq!(FSError::from_u8(e.as_u8()), e);
        }
        assert_eq!(FSError::from_u8(99), FSError::OtherError);
    }

    #[test]
    fn fatal_and_retryable_classification() {
        assert!(FSError::OutOfMemory.is_fatal());
        assert!(!FSError::Success.is_fatal());
        assert!(FSError::WriteFail.is_retryable());
        assert!(!FSError::AccessDenied.is_retryable());
    }

    #[test]
    fn open_flags_or_yields_bitfield() {
        let bits = OpenFlags::Write | OpenFlags::Create;
        assert_eq!(bits, 0x05);
        assert!(has_flag(bits, OpenFlags::Create));
        assert_eq!(get_access_mode(bits), 0x01);
    }

    #[test]
    fn file_attributes_or_yields_bitfield() {
        let bits = FileAttributes::ReadOnly | FileAttributes::Hidden;
        assert_eq!(bits, 0x03);
        assert!(has_attribute(bits, FileAttributes::ReadOnly));
        assert!(!has_attribute(bits, FileAttributes::Directory));
    }
}
