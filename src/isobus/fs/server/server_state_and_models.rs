use alloc::{
    borrow::ToOwned,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use super::error_codes::{
    FSError, FileAttributes, OpenFlags, get_access_mode, has_attribute, has_flag,
    open_flags_have_no_reserved_bits,
};
use super::types::{
    CCMMessage, FS_SUPPORTED_COUNT_MAX, FSFunction, FileEntry, FileHandle, FileServerProperties,
    FileServerStatus, INVALID_FILE_HANDLE, INVALID_TAN, RESERVED_FILE_HANDLE_0, TAN, TANResponse,
    VolumeState, dos_date_time_is_supported, has_wildcards, is_absolute_path, is_valid_fs_path,
    is_valid_volume_name, pack_dos_date, pack_dos_time,
};
use crate::net::constants::NULL_ADDRESS;
use crate::net::error::{Error, Result};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_FILE_CLIENT_TO_SERVER, PGN_FILE_SERVER_TO_CLIENT};
use crate::net::state_machine::StateMachine;
use crate::net::types::{Address, Pgn};

/// CCM (Client Connection Maintenance) sentinel function code (FF =
/// "not a real function code; treat as keepalive").
const CCM_FUNCTION_CODE: u8 = 0xFF;

/// FS string lengths carried in command/response payloads are one byte.
const FS_WIRE_STRING_MAX_LEN: usize = u8::MAX as usize;
const READ_FILE_REQUEST_LEN: usize = 8;
const READ_FILE_RESPONSE_HEADER_LEN: usize = 5;
const WRITE_FILE_RESPONSE_LEN: usize = 8;
const VOLUME_MODE_MAINTAIN: u8 = 0x01;
const VOLUME_MODE_PREPARE_REMOVAL: u8 = 0x02;
const VOLUME_MODE_RESERVED_MASK: u8 = !0x03;
const VOLUME_STATUS_ERROR: u8 = 0xFF;
const INITIALIZE_VOLUME_FLAGS_RESERVED_MASK: u8 = !0x03;

/// Per-server tracking of one client (different from
/// [`super::connection::ClientConnection`]; see module doc).
#[derive(Debug, Clone, Default)]
pub struct ServerClientConnection {
    pub client_address: Address,
    pub ccm_seen: bool,
    pub last_ccm_timestamp_ms: u32,
    pub current_directory: String,
    pub open_handles: Vec<FileHandle>,
    /// TAN cache for idempotency (ISO 11783-13 §7.2.2).
    pub tan_cache: BTreeMap<TAN, TANResponse>,
}

impl ServerClientConnection {
    fn new(addr: Address) -> Self {
        Self {
            client_address: addr,
            ccm_seen: false,
            last_ccm_timestamp_ms: 0,
            current_directory: "\\".to_string(),
            open_handles: Vec::new(),
            tan_cache: BTreeMap::new(),
        }
    }

    fn is_connected(&self, current_time_ms: u32, timeout_ms: u32) -> bool {
        current_time_ms.saturating_sub(self.last_ccm_timestamp_ms) <= timeout_ms
    }

    fn has_active_ccm_connection(&self, current_time_ms: u32, timeout_ms: u32) -> bool {
        self.ccm_seen && self.is_connected(current_time_ms, timeout_ms)
    }

    fn update_ccm(&mut self, current_time_ms: u32) {
        self.ccm_seen = true;
        self.last_ccm_timestamp_ms = current_time_ms;
    }
}

/// One open file handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFile {
    pub handle: FileHandle,
    pub owner: Address,
    pub path: String,
    pub position: u32,
    pub flags: u8,
    pub is_directory: bool,
    pub directory_pattern: String,
}

impl Default for OpenFile {
    fn default() -> Self {
        Self {
            handle: INVALID_FILE_HANDLE,
            owner: NULL_ADDRESS,
            path: String::new(),
            position: 0,
            flags: 0,
            is_directory: false,
            directory_pattern: "*".to_string(),
        }
    }
}

/// Server config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileServerConfig {
    pub status_broadcast_interval_ms: u32,
    pub busy_status_interval_ms: u32,
    pub ccm_timeout_ms: u32,
    pub tan_cache_timeout_ms: u32,
    pub max_open_files_per_client: u8,
    pub max_open_files_total: u8,
}

impl Default for FileServerConfig {
    fn default() -> Self {
        Self {
            status_broadcast_interval_ms: 2000,
            busy_status_interval_ms: 200,
            ccm_timeout_ms: 6000,
            tan_cache_timeout_ms: 10_000,
            max_open_files_per_client: 8,
            max_open_files_total: 32,
        }
    }
}

impl FileServerConfig {
    #[must_use]
    pub const fn with_status_interval(mut self, ms: u32) -> Self {
        self.status_broadcast_interval_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_busy_interval(mut self, ms: u32) -> Self {
        self.busy_status_interval_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_ccm_timeout(mut self, ms: u32) -> Self {
        self.ccm_timeout_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_max_files_per_client(mut self, n: u8) -> Self {
        self.max_open_files_per_client = if n > FS_SUPPORTED_COUNT_MAX {
            FS_SUPPORTED_COUNT_MAX
        } else {
            n
        };
        self
    }

    #[must_use]
    pub const fn with_max_files_total(mut self, n: u8) -> Self {
        self.max_open_files_total = if n > FS_SUPPORTED_COUNT_MAX {
            FS_SUPPORTED_COUNT_MAX
        } else {
            n
        };
        self
    }
}

/// One outbound frame from the FS server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FSOutbound {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub dest: Option<Address>,
}

impl FSOutbound {
    #[must_use]
    pub fn broadcast(data: Vec<u8>) -> Self {
        Self {
            pgn: PGN_FILE_SERVER_TO_CLIENT,
            data,
            dest: None,
        }
    }

    #[must_use]
    pub fn to(data: Vec<u8>, dest: Address) -> Self {
        Self {
            pgn: PGN_FILE_SERVER_TO_CLIENT,
            data,
            dest: Some(dest),
        }
    }
}

/// ISO 11783-13 enhanced File Server.
pub struct FileServer {
    config: FileServerConfig,

    files: BTreeMap<String, Vec<u8>>,
    file_attrs: BTreeMap<String, u8>,
    file_date_times: BTreeMap<String, (u16, u16)>,
    directories: Vec<String>,
    open_files: Vec<OpenFile>,
    next_handle: FileHandle,

    clients: BTreeMap<Address, ServerClientConnection>,

    busy: bool,
    status_timer_ms: u32,
    current_time_ms: u32,

    volume_state: StateMachine<VolumeState>,
    volume_name: String,
    volume_removal_timer_ms: u32,
    volume_max_removal_time_ms: u32,
    volume_maintain_requests: Vec<Address>,
    /// Total volume capacity in bytes (for the free-space query).
    volume_capacity_bytes: u64,

    properties: FileServerProperties,

    pub on_client_connected: Event<Address>,
    pub on_client_disconnected: Event<Address>,
    pub on_file_opened: Event<(Address, String)>,
    pub on_file_closed: Event<(Address, FileHandle)>,
    pub on_volume_preparing_for_removal: Event<()>,
    pub on_volume_removed: Event<()>,
    pub on_volume_present: Event<()>,
}

