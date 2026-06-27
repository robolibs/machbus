use alloc::{
    borrow::ToOwned,
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};

use super::error_codes::{
    FSError, FileAttributes, OpenFlags, fs_error_byte_is_valid, get_access_mode,
    open_flags_have_no_reserved_bits,
};
use super::types::{
    CCMMessage, FSFunction, FileHandle, FileServerProperties, FileServerStatus,
    INVALID_FILE_HANDLE, INVALID_TAN, RESERVED_FILE_HANDLE_0, TAN, VolumeState,
    dos_date_time_is_supported, is_valid_fs_path, is_valid_volume_name,
};
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::error::{Error, Result as NetResult};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_FILE_CLIENT_TO_SERVER, PGN_FILE_SERVER_TO_CLIENT};
use crate::net::types::{Address, Pgn};

/// Special CCM keepalive function code.
const CCM_FUNCTION_CODE: u8 = 0xFF;
const READ_FILE_REQUEST_LEN: usize = 8;
const READ_FILE_RESPONSE_HEADER_LEN: usize = 5;
const WRITE_FILE_RESPONSE_LEN: usize = 8;
const VOLUME_MODE_MAINTAIN: u8 = 0x01;
const VOLUME_MODE_PREPARE_REMOVAL: u8 = 0x02;
const VOLUME_MODE_RESERVED_MASK: u8 = !0x03;
const FILE_ATTRIBUTES_RESPONSE_ALLOWED_MASK: u8 = FileAttributes::ReadOnly as u8
    | FileAttributes::Hidden as u8
    | FileAttributes::System as u8
    | FileAttributes::Directory as u8
    | FileAttributes::Archive as u8;
const FILE_ATTRIBUTES_SET_ALLOWED_MASK: u8 = FileAttributes::ReadOnly as u8
    | FileAttributes::Hidden as u8
    | FileAttributes::System as u8
    | FileAttributes::Archive as u8;
const INITIALIZE_VOLUME_FLAGS_RESERVED_MASK: u8 = !0x03;

/// FS Client connection state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ClientState {
    #[default]
    Disconnected,
    WaitingForStatus,
    Connected,
    Error,
}

/// Open file tracking on the client side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenFileInfo {
    pub handle: FileHandle,
    pub path: String,
    pub flags: u8,
    pub position: u32,
    pub size: u32,
}

impl Default for OpenFileInfo {
    fn default() -> Self {
        Self {
            handle: INVALID_FILE_HANDLE,
            path: String::new(),
            flags: 0,
            position: 0,
            size: 0,
        }
    }
}

pub type FileDateTime = (u16, u16);
pub type FileDateTimeResponse = Result<FileDateTime, FSError>;
pub type FileAttributesResponse = Result<u8, FSError>;
/// `(total_bytes, free_bytes)` or an error, from a `GetFreeSpace` query.
pub type FreeSpaceResponse = Result<(u32, u32), FSError>;
pub type FileOperationResponse = Result<(), FSError>;

/// One in-flight request awaiting response. Tracked by TAN.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRequest {
    pub tan: TAN,
    pub function: FSFunction,
    pub timestamp_ms: u32,
    pub request_data: Vec<u8>,
}

impl PendingRequest {
    #[must_use]
    pub fn is_expired(&self, current_time_ms: u32, timeout_ms: u32) -> bool {
        current_time_ms.saturating_sub(self.timestamp_ms) > timeout_ms
    }
}

/// FS Client config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileClientConfig {
    pub ccm_interval_ms: u32,
    pub request_timeout_ms: u32,
    pub server_status_timeout_ms: u32,
    pub retry_delay_ms: u32,
    pub max_retries: u8,
}

impl Default for FileClientConfig {
    fn default() -> Self {
        Self {
            ccm_interval_ms: 2000,
            request_timeout_ms: 6000,
            server_status_timeout_ms: 6000,
            retry_delay_ms: 500,
            max_retries: 3,
        }
    }
}

impl FileClientConfig {
    #[must_use]
    pub const fn with_ccm_interval(mut self, ms: u32) -> Self {
        self.ccm_interval_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_request_timeout(mut self, ms: u32) -> Self {
        self.request_timeout_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_max_retries(mut self, n: u8) -> Self {
        self.max_retries = n;
        self
    }
}

/// One outbound frame from the FS client.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FSClientOutbound {
    pub pgn: Pgn,
    pub data: Vec<u8>,
    pub dest: Option<Address>,
}

impl FSClientOutbound {
    #[must_use]
    pub fn new(data: Vec<u8>, dest: Address) -> Self {
        Self {
            pgn: PGN_FILE_CLIENT_TO_SERVER,
            data,
            dest: Some(dest),
        }
    }

    #[must_use]
    pub fn broadcast(data: Vec<u8>) -> Self {
        Self {
            pgn: PGN_FILE_CLIENT_TO_SERVER,
            data,
            dest: None,
        }
    }
}

// ─── FileClient ───────────────────────────────────────────────────────

/// Pump-style file client. Operation methods produce the outbound
/// payload; responses arrive via [`Self::handle_server_response`] and
/// fire per-operation events with `Result<T, FSError>`.
pub struct FileClient {
    server_address: Address,
    config: FileClientConfig,

    state: ClientState,
    ccm_timer_ms: u32,
    server_status_timer_ms: u32,
    current_time_ms: u32,

    next_tan: TAN,
    pending_requests: BTreeMap<TAN, PendingRequest>,

    server_properties: Option<FileServerProperties>,
    /// Raw version number from the properties response, captured even when the
    /// detailed (v1) properties block does not decode (e.g. a v2 server).
    server_version: Option<u8>,
    server_status: Option<FileServerStatus>,

    open_files: BTreeMap<FileHandle, OpenFileInfo>,
    current_directory: String,

    pub on_connected: Event<()>,
    pub on_disconnected: Event<()>,
    pub on_error: Event<FSError>,
    pub on_file_opened: Event<(FileHandle, String)>,
    pub on_file_closed: Event<FileHandle>,
    /// `(tan, Ok(handle))` or `(tan, Err(error))` — fired when an
    /// `OpenFile` response arrives.
    pub on_open_response: Event<(TAN, Result<FileHandle, FSError>)>,
    /// `(tan, Ok(properties))` — fired when a
    /// `GetFileServerProperties` response arrives.
    pub on_properties_response: Event<(TAN, Result<FileServerProperties, FSError>)>,
    /// `(tan, Ok(status))` — fired when a `FileServerStatus` response arrives.
    pub on_status_response: Event<(TAN, Result<FileServerStatus, FSError>)>,
    pub on_close_response: Event<(TAN, Result<FileHandle, FSError>)>,
    /// `(tan, Ok(bytes_read_into))` — payload owned by the event,
    /// callers should clone if they need it past the dispatch frame.
    pub on_read_response: Event<(TAN, Result<Vec<u8>, FSError>)>,
    /// `(tan, Ok(bytes_written))` count.
    pub on_write_response: Event<(TAN, Result<u16, FSError>)>,
    pub on_seek_response: Event<(TAN, Result<(), FSError>)>,
    pub on_current_directory_response: Event<(TAN, Result<String, FSError>)>,
    pub on_change_directory_response: Event<(TAN, Result<String, FSError>)>,
    pub on_move_response: Event<(TAN, FileOperationResponse)>,
    pub on_delete_response: Event<(TAN, FileOperationResponse)>,
    pub on_make_directory_response: Event<(TAN, FileOperationResponse)>,
    pub on_remove_directory_response: Event<(TAN, FileOperationResponse)>,
    pub on_copy_response: Event<(TAN, FileOperationResponse)>,
    pub on_get_file_size_response: Event<(TAN, Result<u32, FSError>)>,
    /// `(TAN, Result<(total_bytes, free_bytes), FSError>)`.
    pub on_get_free_space_response: Event<(TAN, FreeSpaceResponse)>,
    pub on_file_attributes_response: Event<(TAN, FileAttributesResponse)>,
    pub on_set_file_attributes_response: Event<(TAN, FileOperationResponse)>,
    pub on_file_date_time_response: Event<(TAN, FileDateTimeResponse)>,
    pub on_initialize_volume_response: Event<(TAN, FileOperationResponse)>,
    pub on_volume_status: Event<VolumeState>,
}

impl FileClient {
    #[must_use]
    pub fn new(config: FileClientConfig) -> Self {
        Self {
            server_address: NULL_ADDRESS,
            config,
            state: ClientState::Disconnected,
            ccm_timer_ms: 0,
            server_status_timer_ms: 0,
            current_time_ms: 0,
            next_tan: 0,
            pending_requests: BTreeMap::new(),
            server_properties: None,
            server_version: None,
            server_status: None,
            open_files: BTreeMap::new(),
            current_directory: "\\".to_string(),
            on_connected: Event::new(),
            on_disconnected: Event::new(),
            on_error: Event::new(),
            on_file_opened: Event::new(),
            on_file_closed: Event::new(),
            on_open_response: Event::new(),
            on_properties_response: Event::new(),
            on_status_response: Event::new(),
            on_close_response: Event::new(),
            on_read_response: Event::new(),
            on_write_response: Event::new(),
            on_seek_response: Event::new(),
            on_current_directory_response: Event::new(),
            on_change_directory_response: Event::new(),
            on_move_response: Event::new(),
            on_make_directory_response: Event::new(),
            on_remove_directory_response: Event::new(),
            on_copy_response: Event::new(),
            on_get_file_size_response: Event::new(),
            on_get_free_space_response: Event::new(),
            on_delete_response: Event::new(),
            on_file_attributes_response: Event::new(),
            on_set_file_attributes_response: Event::new(),
            on_file_date_time_response: Event::new(),
            on_initialize_volume_response: Event::new(),
            on_volume_status: Event::new(),
        }
    }

    // ─── Connection management ────────────────────────────────────────

    /// Begin connecting to a server. Returns the initial
    /// `GetFileServerProperties` request payload that the caller must
    /// ship.
    pub fn connect_to_server(&mut self, server: Address) -> Option<FSClientOutbound> {
        self.try_connect_to_server(server).ok()
    }

    /// Begin connecting to a server, returning an explicit local error when
    /// the client FSM cannot emit the protocol request.
    pub fn try_connect_to_server(&mut self, server: Address) -> NetResult<FSClientOutbound> {
        if server == NULL_ADDRESS || server == BROADCAST_ADDRESS {
            return Err(Error::invalid_data(
                "FS client connect requested with an unusable server address",
            ));
        }
        if self.state != ClientState::Disconnected {
            return Err(Error::invalid_state(
                "FS client connect requested while not disconnected",
            ));
        }
        self.server_address = server;
        self.state = ClientState::WaitingForStatus;
        self.ccm_timer_ms = 0;
        self.server_status_timer_ms = 0;
        self.try_request_server_properties()
    }

    /// Disconnect. Returns any close-file requests the caller may
    /// optionally ship; the client clears local handles regardless.
    pub fn disconnect(&mut self) -> Vec<FSClientOutbound> {
        let handles: Vec<FileHandle> = self.open_files.keys().copied().collect();
        let mut out = Vec::with_capacity(handles.len());
        for handle in &handles {
            // We can't go through close_file() here because that
            // re-tracks handles; just emit the bytes.
            let mut data = vec![0xFFu8; 8];
            data[0] = FSFunction::CloseFile.as_u8();
            data[1] = self.allocate_tan();
            data[2] = *handle;
            out.push(FSClientOutbound::new(data, self.server_address));
        }
        self.state = ClientState::Disconnected;
        self.server_address = NULL_ADDRESS;
        self.pending_requests.clear();
        self.open_files.clear();
        self.current_directory = "\\".to_string();
        self.on_disconnected.emit(&());
        out
    }

    #[inline]
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        matches!(self.state, ClientState::Connected)
    }

    #[inline]
    #[must_use]
    pub const fn state(&self) -> ClientState {
        self.state
    }

    #[inline]
    #[must_use]
    pub const fn server_address(&self) -> Address {
        self.server_address
    }

    #[inline]
    #[must_use]
    pub fn current_directory(&self) -> &str {
        &self.current_directory
    }

    #[must_use]
    pub fn server_properties(&self) -> Option<FileServerProperties> {
        self.server_properties
    }

    /// Version number the file server reported in its properties response
    /// (`None` until a properties response has been received). Captured from
    /// the raw response even when the detailed (v1) block does not decode, so
    /// it is honest for a v2 server. Use it to gate version-dependent features
    /// (ISO 11783-13 version negotiation).
    #[must_use]
    pub fn server_version(&self) -> Option<u8> {
        self.server_version
    }

    /// `true` if the connected file server reports at least version `minimum`.
    #[must_use]
    pub fn server_supports_version(&self, minimum: u8) -> bool {
        self.server_version().is_some_and(|v| v >= minimum)
    }

    #[must_use]
    pub fn server_status(&self) -> Option<FileServerStatus> {
        self.server_status
    }

    #[must_use]
    pub fn open_files(&self) -> &BTreeMap<FileHandle, OpenFileInfo> {
        &self.open_files
    }

    // ─── File operations ──────────────────────────────────────────────

    pub fn request_server_properties(&mut self) -> FSClientOutbound {
        self.build_fixed_server_request(FSFunction::GetFileServerProperties)
    }

    /// Build a `GetFileServerProperties` request only when a usable server
    /// address is already selected and the client FSM is in a state where that
    /// request is legal. Invalid local state does not allocate a TAN.
    pub fn try_request_server_properties(&mut self) -> NetResult<FSClientOutbound> {
        self.ensure_server_address_for_request()?;
        if !matches!(
            self.state,
            ClientState::WaitingForStatus | ClientState::Connected
        ) {
            return Err(Error::not_connected());
        }
        Ok(self.build_fixed_server_request(FSFunction::GetFileServerProperties))
    }

    pub fn request_server_status(&mut self) -> FSClientOutbound {
        self.build_fixed_server_request(FSFunction::FileServerStatus)
    }

    /// Build a `FileServerStatus` request only once the File Client is
    /// connected. Invalid local state does not allocate a TAN.
    pub fn try_request_server_status(&mut self) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        self.ensure_server_address_for_request()?;
        Ok(self.build_fixed_server_request(FSFunction::FileServerStatus))
    }

    fn build_fixed_server_request(&mut self, function: FSFunction) -> FSClientOutbound {
        let mut data = vec![0xFFu8; 8];
        data[0] = function.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        self.track_request(tan, function, data.clone());
        FSClientOutbound::new(data, self.server_address)
    }

    pub fn open_file(&mut self, path: &str, flags: u8) -> Option<FSClientOutbound> {
        self.try_open_file(path, flags).ok()
    }

    /// Build an `OpenFile` request or return the precise local validation
    /// failure. Invalid requests do not allocate a TAN or enter the pending
    /// request table.
    pub fn try_open_file(&mut self, path: &str, flags: u8) -> NetResult<FSClientOutbound> {
        if !self.is_connected() {
            return Err(Error::not_connected());
        }
        let open_dir = get_access_mode(flags) == OpenFlags::OpenDir.bit();
        if open_dir
            && self
                .server_properties
                .is_some_and(|props| !props.supports_directories)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise directory support",
            ));
        }
        if !path.is_ascii()
            || path.len() > u8::MAX as usize
            || !open_flags_have_no_reserved_bits(flags)
            || !is_valid_fs_path(path, open_dir, false)
        {
            return Err(Error::invalid_data(format!(
                "invalid FS path or OpenFile path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 4 + path.len()];
        data[0] = FSFunction::OpenFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3] = flags;
        data[4..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::OpenFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn close_file(&mut self, handle: FileHandle) -> Option<FSClientOutbound> {
        self.try_close_file(handle).ok()
    }

    /// Build a `CloseFile` request for a known open handle.
    pub fn try_close_file(&mut self, handle: FileHandle) -> NetResult<FSClientOutbound> {
        if !self.open_files.contains_key(&handle) {
            return Err(Error::invalid_data(format!(
                "unknown FS file handle {handle}"
            )));
        }
        let mut data = vec![0xFFu8; 8];
        data[0] = FSFunction::CloseFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = handle;
        self.track_request(tan, FSFunction::CloseFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn read_file(&mut self, handle: FileHandle, count: u16) -> Option<FSClientOutbound> {
        self.try_read_file(handle, count).ok()
    }

    /// Build a `ReadFile` request for a known open handle.
    pub fn try_read_file(&mut self, handle: FileHandle, count: u16) -> NetResult<FSClientOutbound> {
        if !self.open_files.contains_key(&handle) {
            return Err(Error::invalid_data(format!(
                "unknown FS file handle {handle}"
            )));
        }
        let mut data = vec![0xFFu8; READ_FILE_REQUEST_LEN];
        data[0] = FSFunction::ReadFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = handle;
        data[3..5].copy_from_slice(&count.to_le_bytes());
        self.track_request(tan, FSFunction::ReadFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn write_file(
        &mut self,
        handle: FileHandle,
        data_payload: &[u8],
    ) -> Option<FSClientOutbound> {
        self.try_write_file(handle, data_payload).ok()
    }

    /// Build a `WriteFile` request for a known open handle and a payload that
    /// fits the ISO 11783-13 two-byte count field.
    pub fn try_write_file(
        &mut self,
        handle: FileHandle,
        data_payload: &[u8],
    ) -> NetResult<FSClientOutbound> {
        if !self.open_files.contains_key(&handle) {
            return Err(Error::invalid_data(format!(
                "unknown FS file handle {handle}"
            )));
        }
        if data_payload.len() > u16::MAX as usize {
            return Err(Error::invalid_data(format!(
                "FS WriteFile payload length {} exceeds 65535 bytes",
                data_payload.len()
            )));
        }
        let used = 5 + data_payload.len();
        let mut data = vec![0xFFu8; WRITE_FILE_RESPONSE_LEN.max(used)];
        data[0] = FSFunction::WriteFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = handle;
        data[3..5].copy_from_slice(&(data_payload.len() as u16).to_le_bytes());
        data[5..5 + data_payload.len()].copy_from_slice(data_payload);
        self.track_request(tan, FSFunction::WriteFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn seek_file(&mut self, handle: FileHandle, position: u32) -> Option<FSClientOutbound> {
        self.try_seek_file(handle, position).ok()
    }

    /// Build a `SeekFile` request for a known open handle.
    pub fn try_seek_file(
        &mut self,
        handle: FileHandle,
        position: u32,
    ) -> NetResult<FSClientOutbound> {
        if !self.open_files.contains_key(&handle) {
            return Err(Error::invalid_data(format!(
                "unknown FS file handle {handle}"
            )));
        }
        let mut data = vec![0xFFu8; 8];
        data[0] = FSFunction::SeekFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = handle;
        data[3..7].copy_from_slice(&position.to_le_bytes());
        self.track_request(tan, FSFunction::SeekFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn get_current_directory(&mut self) -> Option<FSClientOutbound> {
        self.try_get_current_directory().ok()
    }

    /// Build a `GetCurrentDirectory` request once the client is connected.
    pub fn try_get_current_directory(&mut self) -> NetResult<FSClientOutbound> {
        if !self.is_connected() {
            return Err(Error::not_connected());
        }
        if self
            .server_properties
            .is_some_and(|props| !props.supports_directories)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise directory support",
            ));
        }
        let mut data = vec![0xFFu8; 8];
        data[0] = FSFunction::GetCurrentDirectory.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        self.track_request(tan, FSFunction::GetCurrentDirectory, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn change_directory(&mut self, path: &str) -> Option<FSClientOutbound> {
        self.try_change_directory(path).ok()
    }

    /// Build a `ChangeDirectory` request once connected and after local path
    /// validation.
    pub fn try_change_directory(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        if !self.is_connected() {
            return Err(Error::not_connected());
        }
        if self
            .server_properties
            .is_some_and(|props| !props.supports_directories)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise directory support",
            ));
        }
        if !path.is_ascii()
            || path.len() > u8::MAX as usize
            || (path != "." && path != ".." && !is_valid_fs_path(path, true, false))
        {
            return Err(Error::invalid_data(format!(
                "invalid FS directory path or ChangeDirectory path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 3 + path.len()];
        data[0] = FSFunction::ChangeDirectory.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::ChangeDirectory, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    /// Build a Make Directory request for `path`. Response arrives on
    /// [`Self::on_make_directory_response`].
    pub fn try_make_directory(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        if !self.is_connected() {
            return Err(Error::not_connected());
        }
        if self
            .server_properties
            .is_some_and(|props| !props.supports_directories)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise directory support",
            ));
        }
        if !path.is_ascii() || path.len() > u8::MAX as usize || !is_valid_fs_path(path, true, false)
        {
            return Err(Error::invalid_data(format!(
                "invalid FS MakeDirectory path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 3 + path.len()];
        data[0] = FSFunction::MakeDirectory.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::MakeDirectory, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    /// Build a Remove Directory request for `path`. Response arrives on
    /// [`Self::on_remove_directory_response`].
    pub fn try_remove_directory(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        if !self.is_connected() {
            return Err(Error::not_connected());
        }
        if self
            .server_properties
            .is_some_and(|props| !props.supports_directories)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise directory support",
            ));
        }
        if !path.is_ascii() || path.len() > u8::MAX as usize || !is_valid_fs_path(path, true, false)
        {
            return Err(Error::invalid_data(format!(
                "invalid FS RemoveDirectory path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 3 + path.len()];
        data[0] = FSFunction::RemoveDirectory.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::RemoveDirectory, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn move_file(
        &mut self,
        source_path: &str,
        destination_path: &str,
    ) -> Option<FSClientOutbound> {
        self.try_move_file(source_path, destination_path).ok()
    }

    /// Build a `MoveFile` request with one-byte source and destination path
    /// counts. Invalid paths do not allocate a TAN or enter the pending table.
    pub fn try_move_file(
        &mut self,
        source_path: &str,
        destination_path: &str,
    ) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        if self
            .server_properties
            .is_some_and(|props| !props.supports_move_file)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise MoveFile support",
            ));
        }
        if !is_valid_one_byte_file_path(source_path)
            || !is_valid_one_byte_file_path(destination_path)
            || source_path == destination_path
        {
            return Err(Error::invalid_data(format!(
                "invalid FS MoveFile source/destination lengths {} -> {}",
                source_path.len(),
                destination_path.len()
            )));
        }
        let mut data = vec![0u8; 4 + source_path.len() + destination_path.len()];
        data[0] = FSFunction::MoveFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = source_path.len() as u8;
        data[3] = destination_path.len() as u8;
        data[4..4 + source_path.len()].copy_from_slice(source_path.as_bytes());
        data[4 + source_path.len()..].copy_from_slice(destination_path.as_bytes());
        self.track_request(tan, FSFunction::MoveFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    /// Build a `CopyFile` request (one-byte source + destination counts).
    /// Response arrives on [`Self::on_copy_response`].
    pub fn try_copy_file(
        &mut self,
        source_path: &str,
        destination_path: &str,
    ) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        if !is_valid_one_byte_file_path(source_path)
            || !is_valid_one_byte_file_path(destination_path)
            || source_path == destination_path
        {
            return Err(Error::invalid_data(format!(
                "invalid FS CopyFile source/destination lengths {} -> {}",
                source_path.len(),
                destination_path.len()
            )));
        }
        let mut data = vec![0u8; 4 + source_path.len() + destination_path.len()];
        data[0] = FSFunction::CopyFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = source_path.len() as u8;
        data[3] = destination_path.len() as u8;
        data[4..4 + source_path.len()].copy_from_slice(source_path.as_bytes());
        data[4 + source_path.len()..].copy_from_slice(destination_path.as_bytes());
        self.track_request(tan, FSFunction::CopyFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn delete_file(&mut self, path: &str) -> Option<FSClientOutbound> {
        self.try_delete_file(path).ok()
    }

    /// Build a `DeleteFile` request with a one-byte path count.
    pub fn try_delete_file(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        if self
            .server_properties
            .is_some_and(|props| !props.supports_delete_file)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise DeleteFile support",
            ));
        }
        if !is_valid_one_byte_file_path(path) {
            return Err(Error::invalid_data(format!(
                "invalid FS DeleteFile path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 3 + path.len()];
        data[0] = FSFunction::DeleteFile.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::DeleteFile, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn get_file_attributes(&mut self, path: &str) -> Option<FSClientOutbound> {
        self.try_get_file_attributes(path).ok()
    }

    /// Build a `GetFreeSpace` request (volume-wide; no path). Response (total +
    /// free bytes) arrives on [`Self::on_get_free_space_response`].
    pub fn try_get_free_space(&mut self) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        let tan = self.allocate_tan();
        let data = vec![FSFunction::GetFreeSpace.as_u8(), tan];
        self.track_request(tan, FSFunction::GetFreeSpace, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    /// Build a `GetFileSize` request. Response (the file size in bytes) arrives
    /// on [`Self::on_get_file_size_response`].
    pub fn try_get_file_size(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        if !is_valid_one_byte_file_path(path) {
            return Err(Error::invalid_data(format!(
                "invalid FS GetFileSize path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 3 + path.len()];
        data[0] = FSFunction::GetFileSize.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::GetFileSize, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    /// Build a `GetFileAttributes` request with a one-byte path count.
    pub fn try_get_file_attributes(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        if self
            .server_properties
            .is_some_and(|props| !props.supports_file_attributes)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise file-attribute support",
            ));
        }
        if !is_valid_one_byte_file_path(path) {
            return Err(Error::invalid_data(format!(
                "invalid FS GetFileAttributes path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 3 + path.len()];
        data[0] = FSFunction::GetFileAttributes.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::GetFileAttributes, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn set_file_attributes(&mut self, path: &str, attrs: u8) -> Option<FSClientOutbound> {
        self.try_set_file_attributes(path, attrs).ok()
    }

    /// Build a `SetFileAttributes` request. The client rejects reserved
    /// attribute bits locally before allocating a TAN.
    pub fn try_set_file_attributes(
        &mut self,
        path: &str,
        attrs: u8,
    ) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        if self
            .server_properties
            .is_some_and(|props| !props.supports_file_attributes)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise file-attribute support",
            ));
        }
        if !is_valid_one_byte_file_path(path) || attrs & !FILE_ATTRIBUTES_SET_ALLOWED_MASK != 0 {
            return Err(Error::invalid_data(format!(
                "invalid FS SetFileAttributes path length {} or attributes 0x{attrs:02X}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 4 + path.len()];
        data[0] = FSFunction::SetFileAttributes.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = path.len() as u8;
        data[3] = attrs;
        data[4..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::SetFileAttributes, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn get_file_date_time(&mut self, path: &str) -> Option<FSClientOutbound> {
        self.try_get_file_date_time(path).ok()
    }

    /// Build a `GetFileDateTime` request using the current variable-length
    /// path-count field. Invalid requests do not allocate a TAN or enter the
    /// pending request table.
    pub fn try_get_file_date_time(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        if !self.is_connected() {
            return Err(Error::not_connected());
        }
        if !path.is_ascii()
            || path.len() > u16::MAX as usize
            || !is_valid_fs_path(path, true, false)
        {
            return Err(Error::invalid_data(format!(
                "invalid FS path or GetFileDateTime path length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 4 + path.len()];
        data[0] = FSFunction::GetFileDateTime.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2..4].copy_from_slice(&(path.len() as u16).to_le_bytes());
        data[4..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::GetFileDateTime, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn request_volume_status(&mut self, path: &str) -> Option<FSClientOutbound> {
        self.try_request_volume_status(path).ok()
    }

    pub fn try_request_volume_status(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        self.try_volume_status_request(path, 0)
    }

    pub fn prepare_volume_for_removal(&mut self, path: &str) -> Option<FSClientOutbound> {
        self.try_prepare_volume_for_removal(path).ok()
    }

    pub fn try_prepare_volume_for_removal(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        self.try_volume_status_request(path, VOLUME_MODE_PREPARE_REMOVAL)
    }

    pub fn maintain_volume(&mut self, path: &str) -> Option<FSClientOutbound> {
        self.try_maintain_volume(path).ok()
    }

    pub fn try_maintain_volume(&mut self, path: &str) -> NetResult<FSClientOutbound> {
        self.try_volume_status_request(path, VOLUME_MODE_MAINTAIN)
    }

    fn try_volume_status_request(
        &mut self,
        path: &str,
        volume_mode: u8,
    ) -> NetResult<FSClientOutbound> {
        if self.server_address == NULL_ADDRESS || self.server_address == BROADCAST_ADDRESS {
            return Err(Error::not_connected());
        }
        if volume_mode & VOLUME_MODE_RESERVED_MASK != 0
            || volume_mode == (VOLUME_MODE_MAINTAIN | VOLUME_MODE_PREPARE_REMOVAL)
        {
            return Err(Error::invalid_data(
                "invalid FS VolumeStatus request mode bits",
            ));
        }
        if path.len() > u16::MAX as usize || (!path.is_empty() && !is_valid_volume_name(path)) {
            return Err(Error::invalid_data(format!(
                "invalid FS volume name or VolumeStatus name length {}",
                path.len()
            )));
        }
        let mut data = vec![0u8; 5 + path.len()];
        data[0] = FSFunction::VolumeStatus.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2] = volume_mode;
        data[3..5].copy_from_slice(&(path.len() as u16).to_le_bytes());
        data[5..].copy_from_slice(path.as_bytes());
        self.track_request(tan, FSFunction::VolumeStatus, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    pub fn initialize_volume(
        &mut self,
        volume_name: &str,
        available_space: u32,
        flags: u8,
    ) -> Option<FSClientOutbound> {
        self.try_initialize_volume(volume_name, available_space, flags)
            .ok()
    }

    /// Build an `InitializeVolume` request using the counted current layout.
    /// The client rejects reserved flag bits and invalid volume names locally.
    pub fn try_initialize_volume(
        &mut self,
        volume_name: &str,
        available_space: u32,
        flags: u8,
    ) -> NetResult<FSClientOutbound> {
        self.ensure_connected_for_request()?;
        if self
            .server_properties
            .is_some_and(|props| !props.supports_volume_management)
        {
            return Err(Error::invalid_state(
                "FS server properties do not advertise volume-management support",
            ));
        }
        if flags & INITIALIZE_VOLUME_FLAGS_RESERVED_MASK != 0
            || volume_name.len() > u16::MAX as usize
            || (!volume_name.is_empty() && !is_valid_volume_name(volume_name))
        {
            return Err(Error::invalid_data(format!(
                "invalid FS InitializeVolume name length {} or flags 0x{flags:02X}",
                volume_name.len()
            )));
        }
        let mut data = vec![0u8; 9 + volume_name.len()];
        data[0] = FSFunction::InitializeVolume.as_u8();
        let tan = self.allocate_tan();
        data[1] = tan;
        data[2..6].copy_from_slice(&available_space.to_le_bytes());
        data[6] = flags;
        data[7..9].copy_from_slice(&(volume_name.len() as u16).to_le_bytes());
        data[9..].copy_from_slice(volume_name.as_bytes());
        self.track_request(tan, FSFunction::InitializeVolume, data.clone());
        Ok(FSClientOutbound::new(data, self.server_address))
    }

    // ─── Update loop ──────────────────────────────────────────────────

    /// Advance timers; returns CCM keepalive frames + outbound retries
    /// for expired requests. Disconnects on server-status timeout.
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<FSClientOutbound> {
        self.current_time_ms = self.current_time_ms.saturating_add(elapsed_ms);
        let mut out = Vec::new();

        if self.is_connected() {
            self.ccm_timer_ms = self.ccm_timer_ms.saturating_add(elapsed_ms);
            if self.ccm_timer_ms >= self.config.ccm_interval_ms {
                self.ccm_timer_ms = 0;
                out.push(self.send_ccm());
            }
        }

        if matches!(
            self.state,
            ClientState::WaitingForStatus | ClientState::Connected
        ) {
            self.server_status_timer_ms = self.server_status_timer_ms.saturating_add(elapsed_ms);
            if self.server_status_timer_ms >= self.config.server_status_timeout_ms {
                let _ = self.disconnect();
            }
        }

        let timeout = self.config.request_timeout_ms;
        let now = self.current_time_ms;
        self.pending_requests
            .retain(|_, req| !req.is_expired(now, timeout));

        out
    }

    fn send_ccm(&mut self) -> FSClientOutbound {
        let tan = self.allocate_tan();
        let _ = CCMMessage { version: 1, tan };
        let mut data = [0xFFu8; 8];
        data[0] = CCM_FUNCTION_CODE;
        data[1] = tan;
        FSClientOutbound::new(data.to_vec(), self.server_address)
    }

    // ─── Inbound dispatch ─────────────────────────────────────────────

    /// Feed an inbound `PGN_FILE_SERVER_TO_CLIENT` message. Updates
    /// state and fires the relevant event.
    pub fn handle_server_response(&mut self, msg: &Message) {
        if msg.pgn != PGN_FILE_SERVER_TO_CLIENT
            || !msg.has_usable_source()
            || !msg.has_valid_destination_for_pgn()
        {
            return;
        }
        if self.server_address != NULL_ADDRESS && msg.source != self.server_address {
            return;
        }
        if msg.data.len() < 2 {
            return;
        }
        let _function = msg.data[0];
        let tan = msg.data[1];

        if tan == 0xFF {
            if self.handle_status_broadcast(&msg.data) {
                self.server_status_timer_ms = 0;
            }
            return;
        }

        let Some(pending) = self.pending_requests.get(&tan).cloned() else {
            if self.handle_status_broadcast(&msg.data) {
                self.server_status_timer_ms = 0;
            }
            return;
        };
        if msg.data[0] != pending.function.as_u8() {
            return;
        }
        if pending.function == FSFunction::VolumeStatus {
            let requested_name = parse_volume_status_request_name(&pending.request_data);
            if self.handle_volume_status_response(&msg.data, Some(tan), requested_name.as_deref()) {
                self.server_status_timer_ms = 0;
            }
            return;
        }
        let Some(pending) = self.pending_requests.remove(&tan) else {
            return;
        };
        self.server_status_timer_ms = 0;
        match pending.function {
            FSFunction::GetFileServerProperties => self.handle_properties_response(tan, &msg.data),
            FSFunction::FileServerStatus => self.handle_status_response(tan, &msg.data),
            FSFunction::OpenFile => {
                self.handle_open_response(tan, &pending.request_data, &msg.data)
            }
            FSFunction::CloseFile => {
                self.handle_close_response(tan, &pending.request_data, &msg.data)
            }
            FSFunction::ReadFile => {
                self.handle_read_response(tan, &pending.request_data, &msg.data)
            }
            FSFunction::WriteFile => {
                self.handle_write_response(tan, &pending.request_data, &msg.data)
            }
            FSFunction::SeekFile => {
                self.handle_seek_response(tan, &pending.request_data, &msg.data)
            }
            FSFunction::GetCurrentDirectory => self.handle_get_directory_response(tan, &msg.data),
            FSFunction::ChangeDirectory => {
                self.handle_change_directory_response(tan, &pending.request_data, &msg.data)
            }
            FSFunction::MoveFile => {
                let result = self.parse_simple_operation_response(&msg.data, false);
                self.on_move_response.emit(&(tan, result));
            }
            FSFunction::DeleteFile => {
                let result = self.parse_simple_operation_response(&msg.data, false);
                self.on_delete_response.emit(&(tan, result));
            }
            FSFunction::MakeDirectory => {
                let result = self.parse_simple_operation_response(&msg.data, false);
                self.on_make_directory_response.emit(&(tan, result));
            }
            FSFunction::RemoveDirectory => {
                let result = self.parse_simple_operation_response(&msg.data, false);
                self.on_remove_directory_response.emit(&(tan, result));
            }
            FSFunction::CopyFile => {
                let result = self.parse_simple_operation_response(&msg.data, false);
                self.on_copy_response.emit(&(tan, result));
            }
            FSFunction::GetFileSize => self.handle_get_file_size_response(tan, &msg.data),
            FSFunction::GetFreeSpace => self.handle_get_free_space_response(tan, &msg.data),
            FSFunction::GetFileAttributes => self.handle_file_attributes_response(tan, &msg.data),
            FSFunction::SetFileAttributes => {
                let result = self.parse_simple_operation_response(&msg.data, false);
                self.on_set_file_attributes_response.emit(&(tan, result));
            }
            FSFunction::GetFileDateTime => self.handle_file_date_time_response(tan, &msg.data),
            FSFunction::InitializeVolume => {
                let result = self.parse_simple_operation_response(&msg.data, true);
                self.on_initialize_volume_response.emit(&(tan, result));
            }
            FSFunction::VolumeStatus => unreachable!("VolumeStatus is handled before TAN removal"),
        }
    }

    fn handle_status_broadcast(&mut self, data: &[u8]) -> bool {
        if data.len() < 2 {
            return false;
        }
        if let Some(status) = FileServerStatus::decode(data) {
            self.server_status = Some(status);
            return true;
        }
        if data.len() < 3 {
            return false;
        }
        if data[1] == INVALID_TAN {
            return self.handle_volume_status_response(data, None, None);
        }
        false
    }

    fn handle_volume_status_response(
        &mut self,
        data: &[u8],
        tan: Option<TAN>,
        requested_name: Option<&str>,
    ) -> bool {
        let Some((vol_state, response_name)) = parse_volume_status_fields(data) else {
            return false;
        };
        if let Some(requested_name) = requested_name.filter(|name| !name.is_empty())
            && response_name != Some(requested_name)
        {
            return false;
        }
        if let Some(tan) = tan {
            self.pending_requests.remove(&tan);
        } else {
            self.pending_requests
                .retain(|_, pending| pending.function != FSFunction::VolumeStatus);
        }
        if vol_state == VolumeState::Removed {
            self.open_files.clear();
            self.current_directory = "\\".to_string();
        }
        self.on_volume_status.emit(&vol_state);
        true
    }

    fn handle_properties_response(&mut self, tan: TAN, response: &[u8]) {
        if response.len() < 3 {
            self.on_properties_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_properties_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_properties_response.emit(&(tan, Err(error)));
            return;
        }
        // Skip the 3-byte response header.
        if response.len() < 6 {
            self.on_properties_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        // Capture the reported version (first properties byte) regardless of
        // whether the detailed v1 block decodes — enables version negotiation.
        self.server_version = Some(response[3]);
        let Some(properties) = FileServerProperties::decode(&response[3..]) else {
            self.on_properties_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        };
        self.server_properties = Some(properties);
        self.on_properties_response.emit(&(tan, Ok(properties)));
        if self.state == ClientState::WaitingForStatus {
            self.state = ClientState::Connected;
            self.on_connected.emit(&());
        }
    }

    fn handle_status_response(&mut self, tan: TAN, response: &[u8]) {
        if response.len() < 5 {
            self.on_status_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_status_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_status_response.emit(&(tan, Err(error)));
            return;
        }
        if let Some(status) = FileServerStatus::decode(&response[3..]) {
            self.server_status = Some(status);
            self.on_status_response.emit(&(tan, Ok(status)));
        } else {
            self.on_status_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
        }
    }

    fn handle_open_response(&mut self, tan: TAN, request: &[u8], response: &[u8]) {
        if response.len() < 4 {
            self.on_open_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_open_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_open_response.emit(&(tan, Err(error)));
            return;
        }
        if !fs_payload_len_is_canonical(response, 4) {
            self.on_open_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let handle = response[3];
        if handle == RESERVED_FILE_HANDLE_0 || handle == INVALID_FILE_HANDLE {
            self.on_open_response
                .emit(&(tan, Err(FSError::InvalidHandle)));
            return;
        }
        // Recover path + flags from the original request (bytes 2..).
        if request.len() >= 4 {
            let path_len = request[2] as usize;
            let flags = request[3];
            if request.len() >= 4 + path_len {
                let path_bytes = &request[4..4 + path_len];
                if !path_bytes.is_ascii() {
                    self.on_open_response
                        .emit(&(tan, Err(FSError::InvalidSourceName)));
                    return;
                }
                let path = core::str::from_utf8(path_bytes)
                    .expect("ASCII File Server path bytes are valid UTF-8")
                    .to_owned();
                self.open_files.insert(
                    handle,
                    OpenFileInfo {
                        handle,
                        path: path.clone(),
                        flags,
                        position: 0,
                        size: 0,
                    },
                );
                self.on_file_opened.emit(&(handle, path));
            }
        }
        self.on_open_response.emit(&(tan, Ok(handle)));
    }

    fn handle_close_response(&mut self, tan: TAN, request: &[u8], response: &[u8]) {
        let handle = if request.len() >= 3 {
            request[2]
        } else {
            INVALID_FILE_HANDLE
        };
        if response.len() < 3 {
            self.on_close_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_close_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_close_response.emit(&(tan, Err(error)));
            return;
        }
        if !fs_payload_len_is_canonical(response, 3) {
            self.on_close_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        self.open_files.remove(&handle);
        self.on_file_closed.emit(&handle);
        self.on_close_response.emit(&(tan, Ok(handle)));
    }

    fn handle_read_response(&mut self, tan: TAN, request: &[u8], response: &[u8]) {
        let handle = if request.len() >= 3 {
            request[2]
        } else {
            INVALID_FILE_HANDLE
        };
        if response.len() < READ_FILE_RESPONSE_HEADER_LEN {
            self.on_read_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_read_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            // EOF returns Ok(empty).
            if error == FSError::EndOfFile {
                self.on_read_response.emit(&(tan, Ok(Vec::new())));
            } else {
                self.on_error.emit(&error);
                self.on_read_response.emit(&(tan, Err(error)));
            }
            return;
        }
        let count = u16::from_le_bytes([response[3], response[4]]) as usize;
        let requested = request
            .get(3..5)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as usize)
            .unwrap_or(0);
        if count > requested {
            self.on_read_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let end = READ_FILE_RESPONSE_HEADER_LEN + count;
        if !fs_payload_len_is_canonical(response, end) {
            self.on_read_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let data = response[READ_FILE_RESPONSE_HEADER_LEN..end].to_vec();
        if let Some(info) = self.open_files.get_mut(&handle) {
            info.position = info.position.saturating_add(data.len() as u32);
        }
        self.on_read_response.emit(&(tan, Ok(data)));
    }

    fn handle_write_response(&mut self, tan: TAN, request: &[u8], response: &[u8]) {
        let handle = if request.len() >= 3 {
            request[2]
        } else {
            INVALID_FILE_HANDLE
        };
        if response.len() < WRITE_FILE_RESPONSE_LEN {
            self.on_write_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_write_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_write_response.emit(&(tan, Err(error)));
            return;
        }
        if !fs_payload_len_is_canonical(response, WRITE_FILE_RESPONSE_LEN) {
            self.on_write_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let written = u16::from_le_bytes([response[3], response[4]]);
        let requested = request
            .get(3..5)
            .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]))
            .unwrap_or(0);
        if written > requested {
            self.on_write_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        if let Some(info) = self.open_files.get_mut(&handle) {
            info.position = info.position.saturating_add(written as u32);
        }
        self.on_write_response.emit(&(tan, Ok(written)));
    }

    fn handle_seek_response(&mut self, tan: TAN, request: &[u8], response: &[u8]) {
        let handle = if request.len() >= 3 {
            request[2]
        } else {
            INVALID_FILE_HANDLE
        };
        let position = if request.len() >= 7 {
            u32::from_le_bytes(request[3..7].try_into().unwrap())
        } else {
            0
        };
        if response.len() < 3 {
            self.on_seek_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_seek_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_seek_response.emit(&(tan, Err(error)));
            return;
        }
        if !fs_payload_len_is_canonical(response, 3) {
            self.on_seek_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        if let Some(info) = self.open_files.get_mut(&handle) {
            info.position = position;
        }
        self.on_seek_response.emit(&(tan, Ok(())));
    }

    fn handle_get_directory_response(&mut self, tan: TAN, response: &[u8]) {
        if response.len() < 4 {
            self.on_current_directory_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_current_directory_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_current_directory_response.emit(&(tan, Err(error)));
            return;
        }
        let path_len = response[3] as usize;
        let end = 4 + path_len;
        if !fs_payload_len_is_canonical(response, end) {
            self.on_current_directory_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let path_bytes = &response[4..end];
        if !path_bytes.is_ascii() {
            self.on_current_directory_response
                .emit(&(tan, Err(FSError::InvalidSourceName)));
            return;
        }
        let path = core::str::from_utf8(path_bytes)
            .expect("ASCII File Server path bytes are valid UTF-8")
            .to_owned();
        if path.is_empty() || !is_valid_fs_path(&path, true, false) {
            self.on_current_directory_response
                .emit(&(tan, Err(FSError::InvalidSourceName)));
            return;
        }
        self.current_directory = path.clone();
        self.on_current_directory_response.emit(&(tan, Ok(path)));
    }

    fn handle_change_directory_response(&mut self, tan: TAN, request: &[u8], response: &[u8]) {
        if response.len() < 3 {
            self.on_change_directory_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_change_directory_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_change_directory_response.emit(&(tan, Err(error)));
            return;
        }
        if !fs_payload_len_is_canonical(response, 3) {
            self.on_change_directory_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        if request.len() < 3 {
            self.on_change_directory_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let path_len = request[2] as usize;
        let end = 3 + path_len;
        if end > request.len() {
            self.on_change_directory_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let requested_path_bytes = &request[3..end];
        if !requested_path_bytes.is_ascii() {
            self.on_change_directory_response
                .emit(&(tan, Err(FSError::InvalidSourceName)));
            return;
        }
        let requested_path = core::str::from_utf8(requested_path_bytes)
            .expect("ASCII File Server path bytes are valid UTF-8")
            .to_owned();
        let Some(path) =
            resolve_client_directory_response_path(&self.current_directory, &requested_path)
        else {
            self.on_change_directory_response
                .emit(&(tan, Err(FSError::InvalidSourceName)));
            return;
        };
        self.current_directory = path.clone();
        self.on_change_directory_response.emit(&(tan, Ok(path)));
    }

    fn parse_simple_operation_response(
        &mut self,
        response: &[u8],
        clears_media_state: bool,
    ) -> FileOperationResponse {
        if response.len() < 3 {
            return Err(FSError::MalformedRequest);
        }
        let error = decode_response_error(response)?;
        if error != FSError::Success {
            self.on_error.emit(&error);
            return Err(error);
        }
        if !fs_payload_len_is_canonical(response, 3) {
            return Err(FSError::MalformedRequest);
        }
        if clears_media_state {
            self.open_files.clear();
            self.current_directory = "\\".to_string();
        }
        Ok(())
    }

    fn handle_file_attributes_response(&mut self, tan: TAN, response: &[u8]) {
        if response.len() < 4 {
            self.on_file_attributes_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_file_attributes_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_file_attributes_response.emit(&(tan, Err(error)));
            return;
        }
        if !fs_payload_len_is_canonical(response, 4)
            || response[3] & !FILE_ATTRIBUTES_RESPONSE_ALLOWED_MASK != 0
        {
            self.on_file_attributes_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        self.on_file_attributes_response
            .emit(&(tan, Ok(response[3])));
    }

    fn handle_get_file_size_response(&mut self, tan: TAN, response: &[u8]) {
        if response.len() < 7 {
            self.on_get_file_size_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_get_file_size_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_get_file_size_response.emit(&(tan, Err(error)));
            return;
        }
        let size = u32::from_le_bytes([response[3], response[4], response[5], response[6]]);
        self.on_get_file_size_response.emit(&(tan, Ok(size)));
    }

    fn handle_get_free_space_response(&mut self, tan: TAN, response: &[u8]) {
        if response.len() < 11 {
            self.on_get_free_space_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_get_free_space_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_get_free_space_response.emit(&(tan, Err(error)));
            return;
        }
        let total = u32::from_le_bytes([response[3], response[4], response[5], response[6]]);
        let free = u32::from_le_bytes([response[7], response[8], response[9], response[10]]);
        self.on_get_free_space_response
            .emit(&(tan, Ok((total, free))));
    }

    fn handle_file_date_time_response(&mut self, tan: TAN, response: &[u8]) {
        if response.len() < 3 {
            self.on_file_date_time_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let error = match decode_response_error(response) {
            Ok(error) => error,
            Err(error) => {
                self.on_file_date_time_response.emit(&(tan, Err(error)));
                return;
            }
        };
        if error != FSError::Success {
            self.on_error.emit(&error);
            self.on_file_date_time_response.emit(&(tan, Err(error)));
            return;
        }
        if response.len() != 8 || response[7] != 0xFF {
            self.on_file_date_time_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        let date = u16::from_le_bytes([response[3], response[4]]);
        let time = u16::from_le_bytes([response[5], response[6]]);
        if !dos_date_time_is_supported(date, time) {
            self.on_file_date_time_response
                .emit(&(tan, Err(FSError::MalformedRequest)));
            return;
        }
        self.on_file_date_time_response
            .emit(&(tan, Ok((date, time))));
    }

    // ─── Internals ────────────────────────────────────────────────────

    fn ensure_connected_for_request(&self) -> NetResult<()> {
        if self.is_connected() {
            Ok(())
        } else {
            Err(Error::not_connected())
        }
    }

    fn ensure_server_address_for_request(&self) -> NetResult<()> {
        if self.server_address == NULL_ADDRESS || self.server_address == BROADCAST_ADDRESS {
            Err(Error::invalid_state(
                "FS client request has no usable destination server address",
            ))
        } else {
            Ok(())
        }
    }

    fn allocate_tan(&mut self) -> TAN {
        let tan = self.next_tan;
        self.next_tan = self.next_tan.wrapping_add(1);
        if self.next_tan == INVALID_TAN {
            self.next_tan = 0;
        }
        tan
    }

    fn track_request(&mut self, tan: TAN, function: FSFunction, request: Vec<u8>) {
        self.pending_requests.insert(
            tan,
            PendingRequest {
                tan,
                function,
                timestamp_ms: self.current_time_ms,
                request_data: request,
            },
        );
    }
}

fn fs_payload_len_is_canonical(data: &[u8], used: usize) -> bool {
    used <= data.len()
        && (data.len() == used
            || (used <= 8 && data.len() == 8 && data[used..].iter().all(|&b| b == 0xFF)))
}

