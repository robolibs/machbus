//! ISO 11783-13 File Server — types, error codes, properties,
//! connection manager, server, client.
//!
//! Mirrors the C++ `machbus::isobus::fs::*` namespace. Pump-style
//! port: server/client return outbound payloads instead of holding
//! `IsoNet&`.

pub mod client;
pub mod connection;
pub mod error_codes;
pub mod operations;
pub mod properties;
pub mod server;
pub mod types;

pub use client::{
    ClientState, FSClientOutbound, FileClient, FileClientConfig, FileDateTime,
    FileDateTimeResponse, OpenFileInfo, PendingRequest,
};
pub use connection::{
    ClientConnection, ConnectionManager, FS_CLIENT_TIMEOUT_MS, FS_MAX_STATUS_BURST_PER_SEC,
    FS_STATUS_BUSY_INTERVAL_MS, FS_STATUS_IDLE_INTERVAL_MS,
};
pub use error_codes::{
    FSError, FileAttributes, OpenFlags, fs_error_description, fs_error_to_string, get_access_mode,
    has_attribute, has_flag, is_fatal_error, is_retryable_error,
};
pub use operations::{
    ALL_OPERATIONS, FSCategory, FSOperationInfo, mutates_storage, operation_info,
};
pub use properties::{
    FS_CMD_GET_PROPERTIES, FS_CMD_GET_VOLUME_STATUS, FS_CMD_MAINTAIN_VOLUME,
    FS_CMD_PREPARE_VOLUME_REMOVAL, FS_NACK_INVALID_ACCESS, FS_NACK_NOT_SUPPORTED,
    FS_NACK_VOLUME_BUSY, FS_V2_PROPERTIES_VERSION, FSNack, FileServerPropertiesV2, VolumeStateV2,
    VolumeStatus,
};
pub use server::{
    FSOutbound, FileServer, FileServerConfig, OpenFile, ServerClientConnection, encode_ccm,
};
pub use types::{
    CCMMessage, FS_CLASSIC_PROPERTIES_VERSION, FS_SUPPORTED_COUNT_MAX, FSFunction, FileEntry,
    FileHandle, FileServerProperties, FileServerStatus, INVALID_FILE_HANDLE, INVALID_TAN,
    MAX_VOLUME_NAME_BYTES, RESERVED_FILE_HANDLE_0, TAN, TANResponse, VolumeState, has_wildcards,
    is_absolute_path, is_valid_fs_path, is_valid_path_component, is_valid_volume_name,
    pack_dos_date, pack_dos_time, unpack_dos_date, unpack_dos_time,
};
