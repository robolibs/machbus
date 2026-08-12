//! ISO 11783-13 File Server error codes + open / attribute flags.
//!
//! Mirrors the C++ `machbus::isobus::fs::error_codes.hpp`.

/// ISO 11783-13:2022 B.9 Error Code table.
///
/// Everything from 5 upward used to be shifted by one or two, and an
/// unimplemented function answered 20, which B.9 marks reserved. A conformant
/// server's 10 (media is not present) was read as "volume out of free space",
/// so a client retried a write forever instead of prompting the operator, and
/// its 12 (function not supported) was read as "media not present", which
/// [`FSError::is_fatal`] treats as unrecoverable.
///
/// The former `WrongType` and `MaxHandles` values have no code of their own:
/// B.9 gives a file-where-a-directory-was-expected as the reason for 2, and
/// the handle ceiling as the reason for 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FSError {
    #[default]
    Success = 0,
    AccessDenied = 1,
    InvalidAccess = 2,
    TooManyOpen = 3,
    NotFound = 4,
    InvalidHandle = 5,
    InvalidSourceName = 6,
    InvalidDestName = 7,
    NoSpace = 8,
    WriteFail = 9,
    MediaNotPresent = 10,
    ReadFail = 11,
    NotSupported = 12,
    NotInitialized = 13,
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
            5 => Some(Self::InvalidHandle),
            6 => Some(Self::InvalidSourceName),
            7 => Some(Self::InvalidDestName),
            8 => Some(Self::NoSpace),
            9 => Some(Self::WriteFail),
            10 => Some(Self::MediaNotPresent),
            11 => Some(Self::ReadFail),
            12 => Some(Self::NotSupported),
            13 => Some(Self::NotInitialized),
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
            Self::InvalidHandle => "Invalid Handle",
            Self::InvalidSourceName => "Invalid Source Name",
            Self::InvalidDestName => "Invalid Dest Name",
            Self::NoSpace => "No Space",
            Self::WriteFail => "Write Fail",
            Self::MediaNotPresent => "Media Not Present",
            Self::ReadFail => "Read Fail",
            Self::NotSupported => "Not Supported",
            Self::NotInitialized => "Not Initialized",
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
            Self::InvalidAccess => {
                "Invalid access - the path names a file where a directory was expected, or vice versa"
            }
            Self::TooManyOpen => {
                "Too many files open - the file server's handle ceiling is reached"
            }
            Self::NotFound => "File, path or volume not found",
            Self::InvalidHandle => "The request names a handle the file server does not know",
            Self::InvalidSourceName => "Invalid given source name",
            Self::InvalidDestName => "Invalid given destination name",
            Self::NoSpace => "Volume out of free space",
            Self::WriteFail => "Failure during a write operation",
            Self::MediaNotPresent => "Media is not present",
            Self::ReadFail => "Failure during a read operation",
            Self::NotSupported => "Function not supported",
            Self::NotInitialized => "Volume is possibly not initialized",
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
        matches!(self, Self::TooManyOpen | Self::WriteFail | Self::ReadFail)
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

/// ISO 11783-13:2022 B.15 Attributes.
///
/// Bits 0 and 1 describe the entry; bits 2 and 5-7 describe the volume it
/// lives on; bits 3 and 4 say what kind of entry it is.
///
/// These used to carry DOS/FAT meanings, and only read-only, hidden and
/// directory happened to line up. `Volume` sat at bit 6, which B.15 defines as
/// "volume is not removable", so every entry on fixed media was classified as
/// a volume while a real volume entry (bit 3) was invisible — a client walking
/// the volume list saw none. `System` at bit 2 is really "volume supports the
/// hidden attribute" and `Archive` at bit 5 is really "volume supports long
/// filenames", both of which Set File Attributes let a client claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FileAttributes {
    #[default]
    None = 0x00,
    /// Bit 0 — the entry's read-only attribute is set.
    ReadOnly = 0x01,
    /// Bit 1 — the entry's hidden attribute is set.
    Hidden = 0x02,
    /// Bit 2 — the volume supports the hidden attribute.
    VolumeSupportsHidden = 0x04,
    /// Bit 3 — the entry specifies a volume.
    IsVolume = 0x08,
    /// Bit 4 — the entry specifies a directory.
    Directory = 0x10,
    /// Bit 5 — the volume supports long filenames.
    VolumeSupportsLongFilenames = 0x20,
    /// Bit 6 — the volume is *not* removable.
    VolumeNotRemovable = 0x40,
    /// Bit 7 — the volume is case-sensitive. A client that cannot read this
    /// has no way to know whether `Task.xml` and `TASK.XML` are one file,
    /// which A.2.2.1 warns about.
    VolumeCaseSensitive = 0x80,
}

/// The B.15 bits a client may set through Set File Attributes: the two that
/// describe the entry itself. The rest describe the volume and are the file
/// server's to report.
pub const FILE_ATTRIBUTES_CLIENT_SETTABLE: u8 =
    FileAttributes::ReadOnly as u8 | FileAttributes::Hidden as u8;

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

    /// The B.9 table verbatim. Everything from 5 up used to be shifted, and
    /// "function not supported" was emitted as 20 out of the reserved range.
    #[test]
    fn fs_error_codes_match_the_b9_table() {
        for (code, error) in [
            (0, FSError::Success),
            (1, FSError::AccessDenied),
            (2, FSError::InvalidAccess),
            (3, FSError::TooManyOpen),
            (4, FSError::NotFound),
            (5, FSError::InvalidHandle),
            (6, FSError::InvalidSourceName),
            (7, FSError::InvalidDestName),
            (8, FSError::NoSpace),
            (9, FSError::WriteFail),
            (10, FSError::MediaNotPresent),
            (11, FSError::ReadFail),
            (12, FSError::NotSupported),
            (13, FSError::NotInitialized),
            (42, FSError::InvalidLength),
            (43, FSError::OutOfMemory),
            (44, FSError::OtherError),
            (45, FSError::EndOfFile),
            (46, FSError::TANError),
            (47, FSError::MalformedRequest),
        ] {
            assert_eq!(FSError::try_from_u8(code), Some(error));
            assert_eq!(error.as_u8(), code);
        }

        for reserved in [14, 20, 41, 48, 99, 255] {
            assert_eq!(
                FSError::try_from_u8(reserved),
                None,
                "{reserved} is reserved"
            );
            assert_eq!(FSError::from_u8(reserved), FSError::OtherError);
        }
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

    /// The B.15 table verbatim. The old values were DOS/FAT ones: `Volume`
    /// sat on bit 6 ("volume is not removable"), so no entry ever reported
    /// itself as a volume and every fixed-media entry claimed to be one.
    #[test]
    fn file_attribute_bits_match_the_b15_table() {
        for (bit, attribute) in [
            (0x01, FileAttributes::ReadOnly),
            (0x02, FileAttributes::Hidden),
            (0x04, FileAttributes::VolumeSupportsHidden),
            (0x08, FileAttributes::IsVolume),
            (0x10, FileAttributes::Directory),
            (0x20, FileAttributes::VolumeSupportsLongFilenames),
            (0x40, FileAttributes::VolumeNotRemovable),
            (0x80, FileAttributes::VolumeCaseSensitive),
        ] {
            assert_eq!(attribute.bit(), bit);
            assert!(has_attribute(bit, attribute));
        }

        let bits = FileAttributes::ReadOnly | FileAttributes::Hidden;
        assert_eq!(bits, 0x03);
        assert!(!has_attribute(bits, FileAttributes::Directory));

        // Only the two entry-level bits are a client's to set; the rest
        // describe the volume and used to be writable.
        assert_eq!(FILE_ATTRIBUTES_CLIENT_SETTABLE, 0x03);
    }
}
