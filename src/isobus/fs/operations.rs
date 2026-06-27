//! File Server operation registry (ISO 11783-13).
//!
//! A typed, queryable "operation matrix" over [`FSFunction`]: for each
//! file-server function it records the operation category, whether the
//! operation mutates stored data/metadata (the no-mutation rule a guarded
//! server enforces), and whether it acts on an already-open file handle.
//!
//! This is the repo-owned classification GAP.md asks for ("Create a
//! complete FS operation matrix") expressed as code rather than prose, so
//! server/guard logic can consult it directly. It contains no standard
//! prose — only the function→behaviour classification.

use crate::isobus::fs::types::FSFunction;

/// Broad category of a file-server operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FSCategory {
    /// Working-directory operations.
    Directory,
    /// File create/open/read/write/close and per-file metadata.
    File,
    /// Volume-level operations.
    Volume,
    /// File-server status / properties.
    Server,
}

/// Classification of one file-server operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FSOperationInfo {
    pub function: FSFunction,
    pub category: FSCategory,
    /// `true` if the operation can change stored data or metadata. A
    /// read-only / no-mutation guard must reject these.
    pub mutates_storage: bool,
    /// `true` if the operation acts on a previously opened file handle.
    pub requires_open_file: bool,
}

/// The classification for a single function.
#[must_use]
pub const fn operation_info(function: FSFunction) -> FSOperationInfo {
    use FSCategory as C;
    use FSFunction as F;
    let (category, mutates_storage, requires_open_file) = match function {
        F::GetCurrentDirectory => (C::Directory, false, false),
        F::ChangeDirectory => (C::Directory, false, false),
        F::OpenFile => (C::File, false, false),
        F::SeekFile => (C::File, false, true),
        F::ReadFile => (C::File, false, true),
        F::WriteFile => (C::File, true, true),
        F::CloseFile => (C::File, false, true),
        F::MoveFile => (C::File, true, false),
        F::DeleteFile => (C::File, true, false),
        F::GetFileAttributes => (C::File, false, false),
        F::SetFileAttributes => (C::File, true, false),
        F::GetFileDateTime => (C::File, false, false),
        F::MakeDirectory => (C::Directory, true, false),
        F::RemoveDirectory => (C::Directory, true, false),
        F::CopyFile => (C::File, true, false),
        F::GetFileSize => (C::File, false, false),
        F::GetFreeSpace => (C::Volume, false, false),
        F::InitializeVolume => (C::Volume, true, false),
        F::FileServerStatus => (C::Server, false, false),
        F::GetFileServerProperties => (C::Server, false, false),
        F::VolumeStatus => (C::Volume, false, false),
    };
    FSOperationInfo {
        function,
        category,
        mutates_storage,
        requires_open_file,
    }
}

/// Every file-server operation, in function-code order — the full matrix.
pub const ALL_OPERATIONS: [FSFunction; 21] = [
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
];

/// Convenience: `true` if the operation can change stored data/metadata.
#[must_use]
pub const fn mutates_storage(function: FSFunction) -> bool {
    operation_info(function).mutates_storage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_function_is_classified_and_round_trips() {
        for f in ALL_OPERATIONS {
            let info = operation_info(f);
            assert_eq!(info.function, f);
            // The byte code round-trips through the enum.
            assert_eq!(FSFunction::try_from_u8(f.as_u8()), Some(f));
        }
        assert_eq!(ALL_OPERATIONS.len(), 21);
    }

    #[test]
    fn mutating_operations_are_exactly_the_write_family() {
        let mutating: Vec<FSFunction> = ALL_OPERATIONS
            .into_iter()
            .filter(|&f| mutates_storage(f))
            .collect();
        assert_eq!(
            mutating,
            vec![
                FSFunction::WriteFile,
                FSFunction::MoveFile,
                FSFunction::DeleteFile,
                FSFunction::SetFileAttributes,
                FSFunction::MakeDirectory,
                FSFunction::RemoveDirectory,
                FSFunction::CopyFile,
                FSFunction::InitializeVolume,
            ]
        );
    }

    #[test]
    fn open_handle_operations_are_seek_read_write_close() {
        let needs_handle: Vec<FSFunction> = ALL_OPERATIONS
            .into_iter()
            .filter(|&f| operation_info(f).requires_open_file)
            .collect();
        assert_eq!(
            needs_handle,
            vec![
                FSFunction::SeekFile,
                FSFunction::ReadFile,
                FSFunction::WriteFile,
                FSFunction::CloseFile,
            ]
        );
    }

    #[test]
    fn read_only_operations_do_not_mutate() {
        assert!(!mutates_storage(FSFunction::ReadFile));
        assert!(!mutates_storage(FSFunction::GetFileAttributes));
        assert!(!mutates_storage(FSFunction::GetCurrentDirectory));
        assert!(!mutates_storage(FSFunction::FileServerStatus));
    }
}
