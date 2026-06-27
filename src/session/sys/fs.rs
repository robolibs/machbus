//! `stack.fs()` — File Server client handle.
//!
//! Wraps [`crate::isobus::fs::FileClient`] (pump-style). Inbound
//! `PGN_FILE_SERVER_TO_CLIENT` frames are routed through
//! `FileClient::handle_server_response`; FSM-driven CCM keepalives
//! and retries are auto-shipped on [`Stack::tick`]. Async-style
//! request/response semantics: each `open_file` / `read_file` etc.
//! returns the TAN, and the matching response arrives as
//! [`FsEvent::OpenResponse`] / [`FsEvent::ReadResponse`] etc.

use alloc::{string::String, vec::Vec};
use core::result::Result as CoreResult;

use crate::isobus::fs::{
    FSError, FileHandle, FileServerProperties, FileServerStatus, TAN, VolumeState,
};

/// File-server client events on the unified queue.
#[derive(Debug, Clone, PartialEq)]
pub enum FsEvent {
    /// Connected to the server (CCM exchange completed).
    Connected,
    /// Disconnected (timeout or explicit).
    Disconnected,
    /// `open_file` response.
    OpenResponse {
        tan: TAN,
        result: CoreResult<FileHandle, FSError>,
    },
    /// `get_file_server_properties` response.
    PropertiesResponse {
        tan: TAN,
        result: CoreResult<FileServerProperties, FSError>,
    },
    /// `file_server_status` response.
    StatusResponse {
        tan: TAN,
        result: CoreResult<FileServerStatus, FSError>,
    },
    /// `close_file` response.
    CloseResponse {
        tan: TAN,
        result: CoreResult<FileHandle, FSError>,
    },
    /// `read_file` response — `Ok(payload)` carries the bytes read.
    ReadResponse {
        tan: TAN,
        result: CoreResult<Vec<u8>, FSError>,
    },
    /// `write_file` response — `Ok(bytes_written)`.
    WriteResponse {
        tan: TAN,
        result: CoreResult<u16, FSError>,
    },
    /// `seek_file` response.
    SeekResponse {
        tan: TAN,
        result: CoreResult<(), FSError>,
    },
    /// `get_current_directory` response.
    CurrentDirectoryResponse {
        tan: TAN,
        result: CoreResult<String, FSError>,
    },
    /// `change_directory` response.
    ChangeDirectoryResponse {
        tan: TAN,
        result: CoreResult<String, FSError>,
    },
    /// `move_file` response.
    MoveResponse {
        tan: TAN,
        result: CoreResult<(), FSError>,
    },
    /// `delete_file` response.
    DeleteResponse {
        tan: TAN,
        result: CoreResult<(), FSError>,
    },
    /// `get_file_attributes` response.
    FileAttributesResponse {
        tan: TAN,
        result: CoreResult<u8, FSError>,
    },
    /// `set_file_attributes` response.
    SetFileAttributesResponse {
        tan: TAN,
        result: CoreResult<(), FSError>,
    },
    /// `get_file_date_time` response — `Ok((date, time))` carries packed
    /// filesystem date/time fields.
    FileDateTimeResponse {
        tan: TAN,
        result: CoreResult<(u16, u16), FSError>,
    },
    /// `initialize_volume` response.
    InitializeVolumeResponse {
        tan: TAN,
        result: CoreResult<(), FSError>,
    },
    /// Volume status update or response.
    VolumeStatus { state: VolumeState },
    /// Generic server-side error.
    Error(FSError),
}
