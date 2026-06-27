impl FileServer {
    #[must_use]
    pub fn new(config: FileServerConfig) -> Self {
        let config = FileServerConfig {
            max_open_files_per_client: config.max_open_files_per_client.min(FS_SUPPORTED_COUNT_MAX),
            max_open_files_total: config.max_open_files_total.min(FS_SUPPORTED_COUNT_MAX),
            ..config
        };
        let props = FileServerProperties {
            max_simultaneous_files: config.max_open_files_total,
            ..FileServerProperties::default()
        }
        .normalized_for_wire();
        Self {
            config,
            files: BTreeMap::new(),
            file_attrs: BTreeMap::new(),
            file_date_times: BTreeMap::new(),
            directories: vec!["\\".to_string()],
            open_files: Vec::new(),
            next_handle: 1,
            clients: BTreeMap::new(),
            busy: false,
            status_timer_ms: 0,
            current_time_ms: 0,
            volume_state: StateMachine::new(VolumeState::Present),
            volume_name: "ISOBUS".to_string(),
            volume_removal_timer_ms: 0,
            volume_max_removal_time_ms: 10_000,
            volume_maintain_requests: Vec::new(),
            volume_capacity_bytes: 64 * 1024 * 1024,
            properties: props,
            on_client_connected: Event::new(),
            on_client_disconnected: Event::new(),
            on_file_opened: Event::new(),
            on_file_closed: Event::new(),
            on_volume_preparing_for_removal: Event::new(),
            on_volume_removed: Event::new(),
            on_volume_present: Event::new(),
        }
    }

    // ─── File / dir management ────────────────────────────────────────

    pub fn add_file(&mut self, path: impl Into<String>, data: Vec<u8>, attrs: u8) -> Result<()> {
        let path = normalize_preloaded_file_path(&path.into())?;
        self.files.insert(path.clone(), data);
        self.file_attrs.insert(path.clone(), attrs);
        self.file_date_times.insert(path, default_file_date_time());
        Ok(())
    }

    /// Set the total volume capacity reported by the free-space query.
    pub const fn set_volume_capacity_bytes(&mut self, bytes: u64) {
        self.volume_capacity_bytes = bytes;
    }

    /// Bytes currently used (sum of stored file sizes).
    #[must_use]
    pub fn used_bytes(&self) -> u64 {
        self.files.values().map(|d| d.len() as u64).sum()
    }

    /// Bytes free on the volume (capacity minus used, saturating).
    #[must_use]
    pub fn free_bytes(&self) -> u64 {
        self.volume_capacity_bytes.saturating_sub(self.used_bytes())
    }

    pub fn remove_file(&mut self, path: &str) -> bool {
        let Ok(path) = normalize_preloaded_file_path(path) else {
            return false;
        };
        let removed = self.files.remove(&path).is_some();
        self.file_attrs.remove(&path);
        self.file_date_times.remove(&path);
        removed
    }

    pub fn add_directory(&mut self, path: impl Into<String>) -> Result<()> {
        let p = normalize_directory_path(&path.into(), "\\")?;
        if !self.directories.contains(&p) {
            self.directories.push(p.clone());
        }
        self.file_date_times
            .entry(p)
            .or_insert_with(default_file_date_time);
        Ok(())
    }

    pub fn set_file_date_time(
        &mut self,
        path: impl Into<String>,
        date: u16,
        time: u16,
    ) -> Result<()> {
        if !dos_date_time_is_supported(date, time) {
            return Err(Error::invalid_data(
                "FileServer: DOS file date/time contains out-of-range fields",
            ));
        }
        let path = path.into();
        if let Ok(file_path) = normalize_preloaded_file_path(&path)
            && self.files.contains_key(&file_path)
        {
            self.file_date_times.insert(file_path, (date, time));
            return Ok(());
        }
        if let Ok(directory_path) = normalize_directory_path(&path, "\\")
            && self.directories.iter().any(|dir| dir == &directory_path)
        {
            self.file_date_times.insert(directory_path, (date, time));
            return Ok(());
        }
        Err(Error::invalid_data(
            "FileServer: date/time path must name an existing file or directory",
        ))
    }

    #[must_use]
    pub fn directory_exists(&self, path: &str) -> bool {
        let Ok(path) = normalize_directory_path(path, "\\") else {
            return false;
        };
        self.directories.iter().any(|d| d == &path)
    }

    #[must_use]
    pub fn list_directory(&self, path: &str, pattern: &str) -> Vec<FileEntry> {
        let Ok(path) = normalize_directory_path(path, "\\") else {
            return Vec::new();
        };
        let mut entries = Vec::new();
        for (file_path, data) in &self.files {
            if let Some(filename) = file_path.strip_prefix(&path) {
                if filename.is_empty() || filename.contains('\\') {
                    continue;
                }
                if pattern != "*" && !wildcard_match(filename, pattern) {
                    continue;
                }
                entries.push(FileEntry {
                    name: filename.to_string(),
                    size: fs_file_size_to_wire(data.len()),
                    attributes: *self.file_attrs.get(file_path).unwrap_or(&0),
                    date: self.file_date_time(file_path).0,
                    time: self.file_date_time(file_path).1,
                });
            }
        }
        for dir in &self.directories {
            if dir == &path {
                continue;
            }
            if let Some(subdir) = dir.strip_prefix(&path)
                && !subdir.is_empty()
            {
                if let Some(idx) = subdir.find('\\')
                    && idx < subdir.len() - 1
                {
                    continue;
                }
                if pattern != "*" && !wildcard_match(subdir, pattern) {
                    continue;
                }
                entries.push(FileEntry {
                    name: subdir.to_string(),
                    size: 0,
                    attributes: FileAttributes::Directory.bit(),
                    date: self.file_date_time(dir).0,
                    time: self.file_date_time(dir).1,
                });
            }
        }
        entries
    }

    // ─── Properties / volume / busy ───────────────────────────────────

    #[must_use]
    pub const fn get_properties(&self) -> FileServerProperties {
        self.properties
    }

    pub fn set_properties(&mut self, p: FileServerProperties) {
        self.properties = p.normalized_for_wire();
    }

    #[must_use]
    pub fn get_volume_state(&self) -> VolumeState {
        self.volume_state.state()
    }

    /// Initiate volume removal. Returns the broadcast frames to ship.
    pub fn prepare_volume_for_removal(&mut self) -> Vec<FSOutbound> {
        match self.volume_state.state() {
            VolumeState::Present | VolumeState::InUse => {
                self.volume_state
                    .transition(VolumeState::PreparingForRemoval);
                self.volume_removal_timer_ms = 0;
                self.volume_maintain_requests.clear();
                self.on_volume_preparing_for_removal.emit(&());
                vec![self.broadcast_volume_status()]
            }
            _ => Vec::new(),
        }
    }

    pub fn receive_volume_maintain_request(&mut self, client: Address) {
        if self.volume_state.state() != VolumeState::PreparingForRemoval {
            return;
        }
        if !self.volume_maintain_requests.contains(&client) {
            self.volume_maintain_requests.push(client);
        }
    }

    pub fn clear_volume_maintain_request(&mut self, client: Address) {
        self.volume_maintain_requests.retain(|&a| a != client);
    }

    pub fn set_volume_removed(&mut self) -> Vec<FSOutbound> {
        self.volume_state.transition(VolumeState::Removed);
        self.clear_media_dependent_client_state();
        self.on_volume_removed.emit(&());
        vec![self.broadcast_volume_status()]
    }

    pub fn reinsert_volume(&mut self) -> Option<FSOutbound> {
        if self.volume_state.state() != VolumeState::Removed {
            return None;
        }
        self.volume_state.transition(VolumeState::Present);
        self.on_volume_present.emit(&());
        Some(self.broadcast_volume_status())
    }

    pub fn set_busy(&mut self, busy: bool) {
        self.busy = busy;
    }

    #[must_use]
    pub const fn is_busy(&self) -> bool {
        self.busy
    }

    #[must_use]
    pub fn volume_name(&self) -> &str {
        &self.volume_name
    }

    pub fn set_volume_name(&mut self, name: impl Into<String>) -> Result<()> {
        let name = name.into();
        if !is_valid_volume_name(&name) {
            return Err(Error::invalid_data(
                "FileServer: volume_name must be 1..=255 bytes and contain no path separators, wildcards, quotes, control characters, or reserved DOS punctuation",
            ));
        }
        self.volume_name = name;
        Ok(())
    }

    // ─── Update loop ──────────────────────────────────────────────────

    /// Advance timers, run volume FSM, prune disconnected clients,
    /// emit periodic status broadcasts.
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<FSOutbound> {
        let mut out = Vec::new();
        self.current_time_ms = self.current_time_ms.saturating_add(elapsed_ms);

        if let Some(frame) = self.update_volume_state_machine(elapsed_ms) {
            out.push(frame);
        }

        self.cleanup_expired_tan_cache();
        self.cleanup_disconnected_clients();

        self.status_timer_ms = self.status_timer_ms.saturating_add(elapsed_ms);
        let interval = if self.busy {
            self.config.busy_status_interval_ms
        } else {
            self.config.status_broadcast_interval_ms
        };
        if self.status_timer_ms >= interval {
            self.status_timer_ms = 0;
            out.push(self.broadcast_status());
        }
        out
    }

    fn broadcast_status(&self) -> FSOutbound {
        let status = FileServerStatus {
            busy: self.busy,
            number_of_open_files: self.open_files.len() as u8,
        };
        FSOutbound::broadcast(status.encode().to_vec())
    }

    fn broadcast_volume_status(&self) -> FSOutbound {
        FSOutbound::broadcast(
            self.encode_volume_status_response(INVALID_TAN, self.effective_volume_state()),
        )
    }

    fn update_volume_state_machine(&mut self, elapsed_ms: u32) -> Option<FSOutbound> {
        let current = self.volume_state.state();
        match current {
            VolumeState::Present => {
                if !self.open_files.is_empty() {
                    self.volume_state.transition(VolumeState::InUse);
                }
                None
            }
            VolumeState::InUse => {
                if self.open_files.is_empty() {
                    self.volume_state.transition(VolumeState::Present);
                }
                None
            }
            VolumeState::PreparingForRemoval => {
                self.volume_removal_timer_ms =
                    self.volume_removal_timer_ms.saturating_add(elapsed_ms);
                let all_closed = self.open_files.is_empty();
                let no_maintain = self.volume_maintain_requests.is_empty();
                let timeout = self.volume_removal_timer_ms >= self.volume_max_removal_time_ms;
                if (all_closed && no_maintain) || timeout {
                    self.volume_state.transition(VolumeState::Removed);
                    self.clear_media_dependent_client_state();
                    self.on_volume_removed.emit(&());
                    return Some(self.broadcast_volume_status());
                }
                None
            }
            VolumeState::Removed => None,
        }
    }

    // ─── Inbound dispatch ─────────────────────────────────────────────

    /// Feed an inbound `PGN_FILE_CLIENT_TO_SERVER` message; returns
    /// the response frame(s) to ship.
    pub fn handle_client_message(&mut self, msg: &Message) -> Vec<FSOutbound> {
        if msg.pgn != PGN_FILE_CLIENT_TO_SERVER
            || !msg.has_usable_source()
            || !msg.has_valid_destination_for_pgn()
            || msg.data.len() < 2
        {
            return Vec::new();
        }
        let function = msg.data[0];
        let tan = msg.data[1];
        if tan == INVALID_TAN {
            if function == CCM_FUNCTION_CODE {
                return Vec::new();
            }
            return vec![FSOutbound::to(
                encode_error_response(function, tan, FSError::TANError),
                msg.source,
            )];
        }

        if function == CCM_FUNCTION_CODE {
            if !fs_payload_len_is_canonical(&msg.data, 2) {
                return Vec::new();
            }
            self.clients
                .entry(msg.source)
                .or_insert_with(|| ServerClientConnection::new(msg.source));
            self.handle_ccm(msg.source, tan);
            return Vec::new();
        }

        self.clients
            .entry(msg.source)
            .or_insert_with(|| ServerClientConnection::new(msg.source));

        // TAN cache hit → resend cached response. Per ISO 11783-13 a duplicate
        // TAN is a retransmission and is replayed idempotently regardless of the
        // request body (enforced by the standard test
        // `file_server_replays_cached_tan_even_when_reused_for_different_operation`).
        if let Some(cached) = self
            .clients
            .get(&msg.source)
            .and_then(|c| c.tan_cache.get(&tan))
            .cloned()
        {
            return vec![FSOutbound::to(cached.response_data, msg.source)];
        }

        if function == FSFunction::VolumeStatus.as_u8() {
            let responses = self.handle_volume_status(msg.source, tan, &msg.data);
            if let Some(first) = responses.first()
                && let Some(client) = self.clients.get_mut(&msg.source)
            {
                client.tan_cache.insert(
                    tan,
                    TANResponse {
                        tan,
                        response_data: first.data.clone(),
                        timestamp_ms: self.current_time_ms,
                    },
                );
            }
            return responses;
        }

        let response = self.execute_function(msg.source, function, tan, &msg.data);

        if let Some(client) = self.clients.get_mut(&msg.source) {
            client.tan_cache.insert(
                tan,
                TANResponse {
                    tan,
                    response_data: response.clone(),
                    timestamp_ms: self.current_time_ms,
                },
            );
        }

        vec![FSOutbound::to(response, msg.source)]
    }

    fn handle_ccm(&mut self, client: Address, _tan: TAN) {
        let timeout = self.config.ccm_timeout_ms;
        let now = self.current_time_ms;
        let was_connected = if let Some(c) = self.clients.get(&client) {
            c.has_active_ccm_connection(now, timeout)
        } else {
            false
        };
        if let Some(c) = self.clients.get_mut(&client) {
            c.update_ccm(now);
        }
        if !was_connected {
            self.on_client_connected.emit(&client);
        }
    }

    // ─── Per-function execution ───────────────────────────────────────

    fn execute_function(
        &mut self,
        client: Address,
        function_code: u8,
        tan: TAN,
        request: &[u8],
    ) -> Vec<u8> {
        let Some(function) = FSFunction::from_u8(function_code) else {
            return encode_error_response(function_code, tan, FSError::NotSupported);
        };
        match function {
            FSFunction::OpenFile => self.handle_open_file(client, tan, request),
            FSFunction::CloseFile => self.handle_close_file(client, tan, request),
            FSFunction::ReadFile => self.handle_read_file(client, tan, request),
            FSFunction::WriteFile => self.handle_write_file(client, tan, request),
            FSFunction::SeekFile => self.handle_seek_file(client, tan, request),
            FSFunction::GetFileServerProperties => self.handle_get_properties(tan, request),
            FSFunction::FileServerStatus => self.handle_get_status(tan, request),
            FSFunction::GetCurrentDirectory => {
                self.handle_get_current_directory(client, tan, request)
            }
            FSFunction::ChangeDirectory => self.handle_change_directory(client, tan, request),
            FSFunction::MakeDirectory => self.handle_make_directory(client, tan, request),
            FSFunction::RemoveDirectory => self.handle_remove_directory(client, tan, request),
            FSFunction::CopyFile => self.handle_copy_file(client, tan, request),
            FSFunction::GetFileSize => self.handle_get_file_size(client, tan, request),
            FSFunction::GetFreeSpace => self.handle_get_free_space(tan),
            FSFunction::MoveFile => self.handle_move_file(client, tan, request),
            FSFunction::DeleteFile => self.handle_delete_file(client, tan, request),
            FSFunction::GetFileAttributes => self.handle_get_file_attributes(client, tan, request),
            FSFunction::SetFileAttributes => self.handle_set_file_attributes(client, tan, request),
            FSFunction::GetFileDateTime => self.handle_get_file_date_time(client, tan, request),
            FSFunction::InitializeVolume => self.handle_initialize_volume(tan, request),
            _ => encode_error_response(function_code, tan, FSError::NotSupported),
        }
    }

    fn clear_all_open_handles(&mut self) {
        self.open_files.clear();
        for conn in self.clients.values_mut() {
            conn.open_handles.clear();
        }
    }

    fn clear_media_dependent_client_state(&mut self) {
        self.open_files.clear();
        for conn in self.clients.values_mut() {
            conn.open_handles.clear();
            conn.current_directory = "\\".to_string();
        }
    }

    fn reject_if_volume_removed(&self, function: FSFunction, tan: TAN) -> Option<Vec<u8>> {
        if self.volume_state.state() == VolumeState::Removed {
            Some(encode_error_response(
                function.as_u8(),
                tan,
                FSError::MediaNotPresent,
            ))
        } else {
            None
        }
    }

    fn handle_open_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::OpenFile, tan) {
            return response;
        }
        if request.len() < 4 {
            return encode_error_response(
                FSFunction::OpenFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let path_len = request[2] as usize;
        let flags = request[3];
        let used = 4 + path_len;
        if !fs_payload_len_is_canonical(request, used) {
            return encode_error_response(
                FSFunction::OpenFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let Some(requested_path) = decode_wire_path(&request[4..4 + path_len]) else {
            return encode_error_response(
                FSFunction::OpenFile.as_u8(),
                tan,
                FSError::InvalidSourceName,
            );
        };

        if !open_flags_have_no_reserved_bits(flags) {
            return encode_error_response(
                FSFunction::OpenFile.as_u8(),
                tan,
                FSError::InvalidAccess,
            );
        }
        let access_mode = get_access_mode(flags);
        let is_dir_listing = access_mode == OpenFlags::OpenDir.bit();
        if is_dir_listing && !self.properties.supports_directories {
            return encode_error_response(FSFunction::OpenFile.as_u8(), tan, FSError::NotSupported);
        }
        if has_flag(flags, OpenFlags::Append)
            && (access_mode == OpenFlags::Read.bit() || is_dir_listing)
        {
            return encode_error_response(
                FSFunction::OpenFile.as_u8(),
                tan,
                FSError::InvalidAccess,
            );
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let (path, directory_pattern) = if is_dir_listing {
            let Ok((path, pattern)) =
                normalize_directory_listing_request(&requested_path, current_directory)
            else {
                return encode_error_response(
                    FSFunction::OpenFile.as_u8(),
                    tan,
                    FSError::InvalidSourceName,
                );
            };
            (path, pattern)
        } else {
            let Ok(path) = normalize_client_path(&requested_path, current_directory, false) else {
                return encode_error_response(
                    FSFunction::OpenFile.as_u8(),
                    tan,
                    FSError::InvalidSourceName,
                );
            };
            (path, "*".to_string())
        };

        // Per-client cap.
        if let Some(conn) = self.clients.get(&client)
            && conn.open_handles.len() >= self.config.max_open_files_per_client as usize
        {
            return encode_error_response(FSFunction::OpenFile.as_u8(), tan, FSError::TooManyOpen);
        }
        // Server cap: the actual configured limit and the advertised
        // FileServerProperties limit must agree from the client's point of
        // view, so enforce the stricter of the two before any file creation.
        let max_open_files_total = usize::from(
            self.config
                .max_open_files_total
                .min(self.properties.max_simultaneous_files),
        );
        if self.open_files.len() >= max_open_files_total {
            return encode_error_response(FSFunction::OpenFile.as_u8(), tan, FSError::MaxHandles);
        }

        if is_dir_listing {
            let mut dir_path = path.clone();
            if !dir_path.is_empty() && !dir_path.ends_with('\\') {
                dir_path.push('\\');
            }
            if !self.directory_exists(&dir_path) {
                if self.file_exists_at_directory_path(&dir_path) {
                    return encode_error_response(
                        FSFunction::OpenFile.as_u8(),
                        tan,
                        FSError::WrongType,
                    );
                }
                return encode_error_response(FSFunction::OpenFile.as_u8(), tan, FSError::NotFound);
            }
        } else {
            if self.is_file_path_directory(&path) {
                return encode_error_response(
                    FSFunction::OpenFile.as_u8(),
                    tan,
                    FSError::WrongType,
                );
            }
            let exists = self.files.contains_key(&path);
            if exists && has_flag(flags, OpenFlags::Create) && has_flag(flags, OpenFlags::Exclusive)
            {
                return encode_error_response(
                    FSFunction::OpenFile.as_u8(),
                    tan,
                    FSError::AccessDenied,
                );
            }
            if exists && self.open_file_access_conflicts(&path, flags) {
                return encode_error_response(
                    FSFunction::OpenFile.as_u8(),
                    tan,
                    FSError::AccessDenied,
                );
            }
            if !exists {
                if !has_flag(flags, OpenFlags::Create) {
                    return encode_error_response(
                        FSFunction::OpenFile.as_u8(),
                        tan,
                        FSError::NotFound,
                    );
                }
                let parent = file_parent_directory_path(&path);
                if !self.directory_exists(&parent) {
                    if self.file_exists_at_directory_path(&parent) {
                        return encode_error_response(
                            FSFunction::OpenFile.as_u8(),
                            tan,
                            FSError::WrongType,
                        );
                    }
                    return encode_error_response(
                        FSFunction::OpenFile.as_u8(),
                        tan,
                        FSError::NotFound,
                    );
                }
                self.files.insert(path.clone(), Vec::new());
                self.file_attrs.insert(path.clone(), 0);
                self.file_date_times
                    .insert(path.clone(), default_file_date_time());
            }
        }

        let initial_position = if !is_dir_listing && has_flag(flags, OpenFlags::Append) {
            match self.files.get(&path) {
                Some(data) => fs_file_size_to_wire(data.len()),
                None => {
                    return encode_error_response(
                        FSFunction::OpenFile.as_u8(),
                        tan,
                        FSError::NotFound,
                    );
                }
            }
        } else {
            0
        };

        let handle = self.allocate_handle();
        if handle == INVALID_FILE_HANDLE {
            return encode_error_response(FSFunction::OpenFile.as_u8(), tan, FSError::MaxHandles);
        }
        self.open_files.push(OpenFile {
            handle,
            owner: client,
            path: path.clone(),
            position: initial_position,
            flags,
            is_directory: is_dir_listing,
            directory_pattern,
        });
        if let Some(conn) = self.clients.get_mut(&client) {
            conn.open_handles.push(handle);
        }
        self.on_file_opened.emit(&(client, path));

        let mut response = vec![0xFFu8; 8];
        response[0] = FSFunction::OpenFile.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response[3] = handle;
        response
    }

    fn handle_close_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if request.len() < 3 {
            return encode_error_response(
                FSFunction::CloseFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        if !fs_payload_len_is_canonical(request, 3) {
            return encode_error_response(
                FSFunction::CloseFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let handle = request[2];
        let pos = self
            .open_files
            .iter()
            .position(|f| f.handle == handle && f.owner == client);
        if let Some(idx) = pos {
            self.open_files.remove(idx);
            if let Some(conn) = self.clients.get_mut(&client) {
                conn.open_handles.retain(|&h| h != handle);
            }
            self.on_file_closed.emit(&(client, handle));
            let mut response = vec![0xFFu8; 8];
            response[0] = FSFunction::CloseFile.as_u8();
            response[1] = tan;
            response[2] = FSError::Success.as_u8();
            return response;
        }
        encode_error_response(FSFunction::CloseFile.as_u8(), tan, FSError::InvalidHandle)
    }

    fn handle_read_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::ReadFile, tan) {
            return response;
        }
        if request.len() < READ_FILE_REQUEST_LEN {
            return encode_error_response(
                FSFunction::ReadFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        if !fs_payload_len_is_canonical(request, READ_FILE_REQUEST_LEN) {
            return encode_error_response(
                FSFunction::ReadFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        if request[6] != 0xFF || request[7] != 0xFF {
            return encode_error_response(
                FSFunction::ReadFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let handle = request[2];
        let count = u16::from_le_bytes([request[3], request[4]]) as u32;
        let report_hidden = match request[5] {
            0 | 0xFF => false,
            1 => true,
            _ => {
                return encode_error_response(
                    FSFunction::ReadFile.as_u8(),
                    tan,
                    FSError::InvalidAccess,
                );
            }
        };
        let Some(idx) = self
            .open_files
            .iter()
            .position(|f| f.handle == handle && f.owner == client)
        else {
            return encode_error_response(
                FSFunction::ReadFile.as_u8(),
                tan,
                FSError::InvalidHandle,
            );
        };
        if self.open_files[idx].is_directory {
            return self.handle_read_directory(idx, tan, count, report_hidden);
        }
        let mode = get_access_mode(self.open_files[idx].flags);
        if mode != OpenFlags::Read.bit() && mode != OpenFlags::ReadWrite.bit() {
            return encode_error_response(
                FSFunction::ReadFile.as_u8(),
                tan,
                FSError::InvalidAccess,
            );
        }
        let path = self.open_files[idx].path.clone();
        let position = self.open_files[idx].position;
        let Some(data) = self.files.get(&path) else {
            return encode_error_response(FSFunction::ReadFile.as_u8(), tan, FSError::NotFound);
        };
        let file_len = fs_file_size_to_wire(data.len());
        if position >= file_len {
            return encode_error_response(FSFunction::ReadFile.as_u8(), tan, FSError::EndOfFile);
        }
        let available = file_len - position;
        let to_read = count.min(available);
        let mut response = vec![0u8; READ_FILE_RESPONSE_HEADER_LEN + to_read as usize];
        response[0] = FSFunction::ReadFile.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response[3..5].copy_from_slice(&(to_read as u16).to_le_bytes());
        response[5..].copy_from_slice(&data[position as usize..(position + to_read) as usize]);
        self.open_files[idx].position += to_read;
        response
    }

    fn handle_read_directory(
        &mut self,
        open_index: usize,
        tan: TAN,
        count: u32,
        report_hidden: bool,
    ) -> Vec<u8> {
        let path = self.open_files[open_index].path.clone();
        let pattern = self.open_files[open_index].directory_pattern.clone();
        let position = self.open_files[open_index].position as usize;
        let mut entries = self.list_directory(&path, &pattern);
        if !report_hidden {
            entries.retain(|entry| !has_attribute(entry.attributes, FileAttributes::Hidden));
        }
        if position >= entries.len() {
            return encode_error_response(FSFunction::ReadFile.as_u8(), tan, FSError::EndOfFile);
        }
        let to_read = usize::try_from(count)
            .unwrap_or(usize::MAX)
            .min(entries.len() - position);
        let mut payload = Vec::new();
        for entry in &entries[position..position + to_read] {
            let Some(encoded) = encode_directory_entry(entry) else {
                return encode_error_response(
                    FSFunction::ReadFile.as_u8(),
                    tan,
                    FSError::InvalidLength,
                );
            };
            payload.extend_from_slice(&encoded);
        }
        let mut response = Vec::with_capacity(READ_FILE_RESPONSE_HEADER_LEN + payload.len());
        response.push(FSFunction::ReadFile.as_u8());
        response.push(tan);
        response.push(FSError::Success.as_u8());
        response.extend_from_slice(&(to_read as u16).to_le_bytes());
        response.extend_from_slice(&payload);
        self.open_files[open_index].position = self.open_files[open_index]
            .position
            .saturating_add(to_read as u32);
        response
    }

    fn handle_write_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::WriteFile, tan) {
            return response;
        }
        if request.len() < 5 {
            return encode_error_response(
                FSFunction::WriteFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let handle = request[2];
        let count = u16::from_le_bytes([request[3], request[4]]) as usize;
        let used = 5 + count;
        if !fs_payload_len_is_canonical(request, used) {
            return encode_error_response(
                FSFunction::WriteFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let Some(idx) = self
            .open_files
            .iter()
            .position(|f| f.handle == handle && f.owner == client)
        else {
            return encode_error_response(
                FSFunction::WriteFile.as_u8(),
                tan,
                FSError::InvalidHandle,
            );
        };
        if self.open_files[idx].is_directory {
            return encode_error_response(
                FSFunction::WriteFile.as_u8(),
                tan,
                FSError::InvalidHandle,
            );
        }
        let mode = get_access_mode(self.open_files[idx].flags);
        if mode != OpenFlags::Write.bit() && mode != OpenFlags::ReadWrite.bit() {
            return encode_error_response(
                FSFunction::WriteFile.as_u8(),
                tan,
                FSError::InvalidAccess,
            );
        }
        let path = self.open_files[idx].path.clone();
        if self
            .file_attrs
            .get(&path)
            .is_some_and(|attrs| has_attribute(*attrs, FileAttributes::ReadOnly))
        {
            return encode_error_response(
                FSFunction::WriteFile.as_u8(),
                tan,
                FSError::AccessDenied,
            );
        }
        let position = self.open_files[idx].position as usize;
        let Some(data) = self.files.get_mut(&path) else {
            return encode_error_response(FSFunction::WriteFile.as_u8(), tan, FSError::NotFound);
        };
        let Some(new_position) = self.open_files[idx].position.checked_add(count as u32) else {
            return encode_error_response(FSFunction::WriteFile.as_u8(), tan, FSError::NoSpace);
        };
        let Some(end) = position.checked_add(count) else {
            return encode_error_response(FSFunction::WriteFile.as_u8(), tan, FSError::NoSpace);
        };
        if end > data.len() {
            data.resize(end, 0);
        }
        data[position..end].copy_from_slice(&request[5..5 + count]);
        self.open_files[idx].position = new_position;
        self.touch_file_date_time(&path);

        let mut response = vec![0xFFu8; WRITE_FILE_RESPONSE_LEN];
        response[0] = FSFunction::WriteFile.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response[3..5].copy_from_slice(&(count as u16).to_le_bytes());
        response
    }

    fn handle_seek_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::SeekFile, tan) {
            return response;
        }
        if request.len() < 7 {
            return encode_error_response(
                FSFunction::SeekFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        if !fs_payload_len_is_canonical(request, 7) {
            return encode_error_response(
                FSFunction::SeekFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let handle = request[2];
        let position = u32::from_le_bytes(request[3..7].try_into().unwrap());
        if let Some(f) = self
            .open_files
            .iter_mut()
            .find(|f| f.handle == handle && f.owner == client)
        {
            if f.is_directory {
                return encode_error_response(
                    FSFunction::SeekFile.as_u8(),
                    tan,
                    FSError::InvalidHandle,
                );
            }
            f.position = position;
            let mut response = vec![0xFFu8; 8];
            response[0] = FSFunction::SeekFile.as_u8();
            response[1] = tan;
            response[2] = FSError::Success.as_u8();
            return response;
        }
        encode_error_response(FSFunction::SeekFile.as_u8(), tan, FSError::InvalidHandle)
    }

    fn handle_get_properties(&self, tan: TAN, request: &[u8]) -> Vec<u8> {
        if !fs_payload_len_is_canonical(request, 2) {
            return encode_error_response(
                FSFunction::GetFileServerProperties.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let props_data = self.properties.encode();
        let mut response = vec![0xFFu8; 8];
        response[0] = FSFunction::GetFileServerProperties.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        for (i, &b) in props_data.iter().enumerate() {
            if i + 3 < response.len() {
                response[3 + i] = b;
            }
        }
        response
    }

    fn handle_get_status(&self, tan: TAN, request: &[u8]) -> Vec<u8> {
        if !fs_payload_len_is_canonical(request, 2) {
            return encode_error_response(
                FSFunction::FileServerStatus.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let status = FileServerStatus {
            busy: self.busy,
            number_of_open_files: self.open_files.len() as u8,
        };
        let status_data = status.encode();
        let mut response = vec![0xFFu8; 8];
        response[0] = FSFunction::FileServerStatus.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        for (i, &b) in status_data.iter().enumerate() {
            if i + 3 < response.len() {
                response[3 + i] = b;
            }
        }
        response
    }

    fn handle_get_current_directory(&self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::GetCurrentDirectory, tan)
        {
            return response;
        }
        if !fs_payload_len_is_canonical(request, 2) {
            return encode_error_response(
                FSFunction::GetCurrentDirectory.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        if !self.properties.supports_directories {
            return encode_error_response(
                FSFunction::GetCurrentDirectory.as_u8(),
                tan,
                FSError::NotSupported,
            );
        }
        let cwd = self
            .clients
            .get(&client)
            .map_or("\\", |c| c.current_directory.as_str());
        let cwd_bytes = cwd.as_bytes();
        if cwd_bytes.len() > FS_WIRE_STRING_MAX_LEN {
            return encode_error_response(
                FSFunction::GetCurrentDirectory.as_u8(),
                tan,
                FSError::InvalidLength,
            );
        }
        let mut response = vec![0u8; 4 + cwd_bytes.len()];
        response[0] = FSFunction::GetCurrentDirectory.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response[3] = cwd_bytes.len() as u8;
        response[4..].copy_from_slice(cwd_bytes);
        response
    }

    fn handle_change_directory(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::ChangeDirectory, tan) {
            return response;
        }
        if request.len() < 3 {
            return encode_error_response(
                FSFunction::ChangeDirectory.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let path_len = request[2] as usize;
        let used = 3 + path_len;
        if !fs_payload_len_is_canonical(request, used) {
            return encode_error_response(
                FSFunction::ChangeDirectory.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        if !self.properties.supports_directories {
            return encode_error_response(
                FSFunction::ChangeDirectory.as_u8(),
                tan,
                FSError::NotSupported,
            );
        }
        let Some(requested_path) = decode_wire_path(&request[3..3 + path_len]) else {
            return encode_error_response(
                FSFunction::ChangeDirectory.as_u8(),
                tan,
                FSError::InvalidSourceName,
            );
        };

        if requested_path == ".." {
            let conn = self
                .clients
                .entry(client)
                .or_insert_with(|| ServerClientConnection::new(client));
            let cwd = &mut conn.current_directory;
            if cwd != "\\" {
                let trimmed = &cwd[..cwd.len() - 1];
                if let Some(idx) = trimmed.rfind('\\') {
                    *cwd = cwd[..idx + 1].to_string();
                } else {
                    *cwd = "\\".to_string();
                }
            }
        } else if requested_path == "." {
            // no-op
        } else if requested_path.is_empty() || requested_path == "\\" {
            let conn = self
                .clients
                .entry(client)
                .or_insert_with(|| ServerClientConnection::new(client));
            conn.current_directory = "\\".to_string();
        } else {
            let current_directory = self
                .clients
                .get(&client)
                .map_or("\\", |conn| conn.current_directory.as_str());
            let Ok(target_path) = normalize_directory_path(&requested_path, current_directory)
            else {
                return encode_error_response(
                    FSFunction::ChangeDirectory.as_u8(),
                    tan,
                    FSError::InvalidSourceName,
                );
            };
            if !self.directories.iter().any(|d| d == &target_path) {
                if self.file_exists_at_directory_path(&target_path) {
                    return encode_error_response(
                        FSFunction::ChangeDirectory.as_u8(),
                        tan,
                        FSError::WrongType,
                    );
                }
                return encode_error_response(
                    FSFunction::ChangeDirectory.as_u8(),
                    tan,
                    FSError::NotFound,
                );
            }
            let conn = self
                .clients
                .entry(client)
                .or_insert_with(|| ServerClientConnection::new(client));
            conn.current_directory = target_path;
        }

        let mut response = vec![0xFFu8; 8];
        response[0] = FSFunction::ChangeDirectory.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response
    }

    fn handle_make_directory(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        let func = FSFunction::MakeDirectory.as_u8();
        if let Some(response) = self.reject_if_volume_removed(FSFunction::MakeDirectory, tan) {
            return response;
        }
        if request.len() < 3 {
            return encode_error_response(func, tan, FSError::MalformedRequest);
        }
        let path_len = request[2] as usize;
        let used = 3 + path_len;
        if !fs_payload_len_is_canonical(request, used) {
            return encode_error_response(func, tan, FSError::MalformedRequest);
        }
        if !self.properties.supports_directories {
            return encode_error_response(func, tan, FSError::NotSupported);
        }
        let Some(requested_path) = decode_wire_path(&request[3..3 + path_len]) else {
            return encode_error_response(func, tan, FSError::InvalidSourceName);
        };
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Ok(target_path) = normalize_directory_path(&requested_path, current_directory) else {
            return encode_error_response(func, tan, FSError::InvalidSourceName);
        };
        // A file already occupies the path ⇒ wrong type; an existing directory
        // is an idempotent success; otherwise create it.
        if self.file_exists_at_directory_path(&target_path) {
            return encode_error_response(func, tan, FSError::WrongType);
        }
        if !self.directories.iter().any(|d| d == &target_path) {
            self.directories.push(target_path);
        }
        let mut response = vec![0xFFu8; 8];
        response[0] = func;
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response
    }

    fn handle_remove_directory(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        let func = FSFunction::RemoveDirectory.as_u8();
        if let Some(response) = self.reject_if_volume_removed(FSFunction::RemoveDirectory, tan) {
            return response;
        }
        if request.len() < 3 {
            return encode_error_response(func, tan, FSError::MalformedRequest);
        }
        let path_len = request[2] as usize;
        let used = 3 + path_len;
        if !fs_payload_len_is_canonical(request, used) {
            return encode_error_response(func, tan, FSError::MalformedRequest);
        }
        if !self.properties.supports_directories {
            return encode_error_response(func, tan, FSError::NotSupported);
        }
        let Some(requested_path) = decode_wire_path(&request[3..3 + path_len]) else {
            return encode_error_response(func, tan, FSError::InvalidSourceName);
        };
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Ok(target_path) = normalize_directory_path(&requested_path, current_directory) else {
            return encode_error_response(func, tan, FSError::InvalidSourceName);
        };
        // Cannot remove the root, a non-existent directory, or a non-empty one.
        if target_path == "\\" {
            return encode_error_response(func, tan, FSError::AccessDenied);
        }
        if !self.directories.iter().any(|d| d == &target_path) {
            return encode_error_response(func, tan, FSError::NotFound);
        }
        // `target_path` already ends with the directory separator, so it is the
        // child prefix. A child file or sub-directory ⇒ not empty.
        let non_empty = self.files.keys().any(|f| f.starts_with(&target_path))
            || self
                .directories
                .iter()
                .any(|d| d != &target_path && d.starts_with(&target_path));
        if non_empty {
            return encode_error_response(func, tan, FSError::AccessDenied);
        }
        self.directories.retain(|d| d != &target_path);
        let mut response = vec![0xFFu8; 8];
        response[0] = func;
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response
    }

    fn handle_move_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::MoveFile, tan) {
            return response;
        }
        if !self.properties.supports_move_file {
            return encode_error_response(FSFunction::MoveFile.as_u8(), tan, FSError::NotSupported);
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Some((source_path, destination_path)) =
            parse_two_counted_file_paths(request, current_directory)
        else {
            return encode_error_response(
                FSFunction::MoveFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        };
        if source_path == destination_path {
            return encode_error_response(
                FSFunction::MoveFile.as_u8(),
                tan,
                FSError::InvalidDestName,
            );
        }
        if self.is_file_path_directory(&source_path) {
            return encode_error_response(FSFunction::MoveFile.as_u8(), tan, FSError::WrongType);
        }
        if self.is_path_open(&source_path) || self.is_path_open(&destination_path) {
            return encode_error_response(FSFunction::MoveFile.as_u8(), tan, FSError::AccessDenied);
        }
        if self.files.contains_key(&destination_path) || self.directory_exists(&destination_path) {
            return encode_error_response(FSFunction::MoveFile.as_u8(), tan, FSError::AccessDenied);
        }
        let destination_parent = file_parent_directory_path(&destination_path);
        if !self.directory_exists(&destination_parent) {
            if self.file_exists_at_directory_path(&destination_parent) {
                return encode_error_response(
                    FSFunction::MoveFile.as_u8(),
                    tan,
                    FSError::WrongType,
                );
            }
            return encode_error_response(
                FSFunction::MoveFile.as_u8(),
                tan,
                FSError::InvalidDestName,
            );
        }
        if self
            .file_attrs
            .get(&source_path)
            .is_some_and(|attrs| has_attribute(*attrs, FileAttributes::ReadOnly))
        {
            return encode_error_response(FSFunction::MoveFile.as_u8(), tan, FSError::AccessDenied);
        }
        let Some(data) = self.files.remove(&source_path) else {
            return encode_error_response(FSFunction::MoveFile.as_u8(), tan, FSError::NotFound);
        };
        let attrs = self.file_attrs.remove(&source_path).unwrap_or(0);
        let date_time = self
            .file_date_times
            .remove(&source_path)
            .unwrap_or_else(default_file_date_time);
        self.files.insert(destination_path.clone(), data);
        self.file_attrs.insert(destination_path.clone(), attrs);
        self.file_date_times.insert(destination_path, date_time);
        success_response(FSFunction::MoveFile, tan)
    }

    fn handle_get_free_space(&self, tan: TAN) -> Vec<u8> {
        let func = FSFunction::GetFreeSpace.as_u8();
        if let Some(response) = self.reject_if_volume_removed(FSFunction::GetFreeSpace, tan) {
            return response;
        }
        let total = u32::try_from(self.volume_capacity_bytes).unwrap_or(u32::MAX);
        let free = u32::try_from(self.free_bytes()).unwrap_or(u32::MAX);
        let mut response = vec![0xFFu8; 11];
        response[0] = func;
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response[3..7].copy_from_slice(&total.to_le_bytes());
        response[7..11].copy_from_slice(&free.to_le_bytes());
        response
    }

    fn handle_get_file_size(&self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        let func = FSFunction::GetFileSize.as_u8();
        if let Some(response) = self.reject_if_volume_removed(FSFunction::GetFileSize, tan) {
            return response;
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Some(path) = parse_counted_file_path(request, 2, current_directory) else {
            return encode_error_response(func, tan, FSError::MalformedRequest);
        };
        let Some(data) = self.files.get(&path) else {
            if self.directory_exists(&path) {
                return encode_error_response(func, tan, FSError::WrongType);
            }
            return encode_error_response(func, tan, FSError::NotFound);
        };
        let size = u32::try_from(data.len()).unwrap_or(u32::MAX);
        let mut response = success_response(FSFunction::GetFileSize, tan);
        response[3..7].copy_from_slice(&size.to_le_bytes());
        response
    }

    fn handle_copy_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        let func = FSFunction::CopyFile.as_u8();
        if let Some(response) = self.reject_if_volume_removed(FSFunction::CopyFile, tan) {
            return response;
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Some((source_path, destination_path)) =
            parse_two_counted_file_paths(request, current_directory)
        else {
            return encode_error_response(func, tan, FSError::MalformedRequest);
        };
        if source_path == destination_path {
            return encode_error_response(func, tan, FSError::InvalidDestName);
        }
        if self.is_file_path_directory(&source_path) {
            return encode_error_response(func, tan, FSError::WrongType);
        }
        let Some(data) = self.files.get(&source_path).cloned() else {
            return encode_error_response(func, tan, FSError::NotFound);
        };
        if self.files.contains_key(&destination_path) || self.directory_exists(&destination_path) {
            return encode_error_response(func, tan, FSError::AccessDenied);
        }
        let destination_parent = file_parent_directory_path(&destination_path);
        if !self.directory_exists(&destination_parent) {
            if self.file_exists_at_directory_path(&destination_parent) {
                return encode_error_response(func, tan, FSError::WrongType);
            }
            return encode_error_response(func, tan, FSError::InvalidDestName);
        }
        let attrs = self.file_attrs.get(&source_path).copied().unwrap_or(0);
        let date_time = self
            .file_date_times
            .get(&source_path)
            .copied()
            .unwrap_or_else(default_file_date_time);
        self.files.insert(destination_path.clone(), data);
        self.file_attrs.insert(destination_path.clone(), attrs);
        self.file_date_times.insert(destination_path, date_time);
        success_response(FSFunction::CopyFile, tan)
    }

    fn handle_delete_file(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::DeleteFile, tan) {
            return response;
        }
        if !self.properties.supports_delete_file {
            return encode_error_response(
                FSFunction::DeleteFile.as_u8(),
                tan,
                FSError::NotSupported,
            );
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Some(path) = parse_counted_file_path(request, 2, current_directory) else {
            return encode_error_response(
                FSFunction::DeleteFile.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        };
        if self.is_file_path_directory(&path) {
            return encode_error_response(FSFunction::DeleteFile.as_u8(), tan, FSError::WrongType);
        }
        if self.is_path_open(&path) {
            return encode_error_response(
                FSFunction::DeleteFile.as_u8(),
                tan,
                FSError::AccessDenied,
            );
        }
        if self
            .file_attrs
            .get(&path)
            .is_some_and(|attrs| has_attribute(*attrs, FileAttributes::ReadOnly))
        {
            return encode_error_response(
                FSFunction::DeleteFile.as_u8(),
                tan,
                FSError::AccessDenied,
            );
        }
        if self.files.remove(&path).is_some() {
            self.file_attrs.remove(&path);
            self.file_date_times.remove(&path);
            return success_response(FSFunction::DeleteFile, tan);
        }
        encode_error_response(FSFunction::DeleteFile.as_u8(), tan, FSError::NotFound)
    }

    fn handle_get_file_attributes(&self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::GetFileAttributes, tan) {
            return response;
        }
        if !self.properties.supports_file_attributes {
            return encode_error_response(
                FSFunction::GetFileAttributes.as_u8(),
                tan,
                FSError::NotSupported,
            );
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Some(path) = parse_counted_file_path(request, 2, current_directory) else {
            return encode_error_response(
                FSFunction::GetFileAttributes.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        };
        let attrs = if let Some(attrs) = self.file_attrs.get(&path).copied() {
            attrs
        } else if self.directory_exists(&path) {
            FileAttributes::Directory.bit()
        } else {
            return encode_error_response(
                FSFunction::GetFileAttributes.as_u8(),
                tan,
                FSError::NotFound,
            );
        };
        let mut response = success_response(FSFunction::GetFileAttributes, tan);
        response[3] = attrs;
        response
    }

    fn handle_set_file_attributes(&mut self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::SetFileAttributes, tan) {
            return response;
        }
        if !self.properties.supports_file_attributes {
            return encode_error_response(
                FSFunction::SetFileAttributes.as_u8(),
                tan,
                FSError::NotSupported,
            );
        }
        if request.len() < 4 {
            return encode_error_response(
                FSFunction::SetFileAttributes.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        }
        let new_attrs = request[3];
        if new_attrs
            & !(FileAttributes::ReadOnly.bit()
                | FileAttributes::Hidden.bit()
                | FileAttributes::System.bit()
                | FileAttributes::Archive.bit())
            != 0
        {
            return encode_error_response(
                FSFunction::SetFileAttributes.as_u8(),
                tan,
                FSError::InvalidAccess,
            );
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Some(path) = parse_counted_file_path_with_count_at(request, 2, 4, current_directory)
        else {
            return encode_error_response(
                FSFunction::SetFileAttributes.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        };
        if self.is_file_path_directory(&path) {
            return encode_error_response(
                FSFunction::SetFileAttributes.as_u8(),
                tan,
                FSError::WrongType,
            );
        }
        if !self.files.contains_key(&path) {
            return encode_error_response(
                FSFunction::SetFileAttributes.as_u8(),
                tan,
                FSError::NotFound,
            );
        }
        if self.is_path_open(&path) {
            return encode_error_response(
                FSFunction::SetFileAttributes.as_u8(),
                tan,
                FSError::AccessDenied,
            );
        }
        self.file_attrs.insert(path, new_attrs);
        success_response(FSFunction::SetFileAttributes, tan)
    }

    fn handle_get_file_date_time(&self, client: Address, tan: TAN, request: &[u8]) -> Vec<u8> {
        if let Some(response) = self.reject_if_volume_removed(FSFunction::GetFileDateTime, tan) {
            return response;
        }
        let current_directory = self
            .clients
            .get(&client)
            .map_or("\\", |conn| conn.current_directory.as_str());
        let Some(path) = parse_counted_file_path_u16(request, current_directory) else {
            return encode_error_response(
                FSFunction::GetFileDateTime.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        };
        let Ok(path) = path else {
            return encode_error_response(
                FSFunction::GetFileDateTime.as_u8(),
                tan,
                FSError::InvalidSourceName,
            );
        };
        if path == "\\" {
            return encode_error_response(
                FSFunction::GetFileDateTime.as_u8(),
                tan,
                FSError::AccessDenied,
            );
        }

        let directory_path = ensure_directory_suffix(path.clone());
        let date_time_path = if self.files.contains_key(&path) {
            path.as_str()
        } else if self.directory_exists(&directory_path) {
            directory_path.as_str()
        } else {
            return encode_error_response(
                FSFunction::GetFileDateTime.as_u8(),
                tan,
                FSError::NotFound,
            );
        };

        let (date, time) = self.file_date_time(date_time_path);
        let mut response = vec![0xFFu8; 8];
        response[0] = FSFunction::GetFileDateTime.as_u8();
        response[1] = tan;
        response[2] = FSError::Success.as_u8();
        response[3..5].copy_from_slice(&date.to_le_bytes());
        response[5..7].copy_from_slice(&time.to_le_bytes());
        response
    }

    fn handle_volume_status(
        &mut self,
        client: Address,
        tan: TAN,
        request: &[u8],
    ) -> Vec<FSOutbound> {
        let error = || {
            vec![FSOutbound::to(
                self.encode_volume_status_error_response(tan),
                client,
            )]
        };

        if request.len() < 5 {
            return error();
        }
        let mode = request[2];
        if mode & VOLUME_MODE_RESERVED_MASK != 0
            || mode == (VOLUME_MODE_MAINTAIN | VOLUME_MODE_PREPARE_REMOVAL)
        {
            return error();
        }
        let path_len = u16::from_le_bytes([request[3], request[4]]) as usize;
        let used = 5 + path_len;
        if !fs_payload_len_is_canonical(request, used) {
            return error();
        }
        let Ok(volume_name) = core::str::from_utf8(&request[5..used]) else {
            return error();
        };
        if !self.volume_status_path_matches(volume_name) {
            return error();
        }

        match mode {
            VOLUME_MODE_PREPARE_REMOVAL => {
                let frames = self.prepare_volume_for_removal();
                if frames.is_empty() {
                    vec![FSOutbound::to(
                        self.encode_volume_status_response(tan, self.effective_volume_state()),
                        client,
                    )]
                } else {
                    frames
                }
            }
            VOLUME_MODE_MAINTAIN => {
                self.receive_volume_maintain_request(client);
                vec![FSOutbound::to(
                    self.encode_volume_status_response(tan, self.effective_volume_state()),
                    client,
                )]
            }
            0 => {
                self.clear_volume_maintain_request(client);
                if self.volume_state.state() == VolumeState::PreparingForRemoval
                    && self.open_files.is_empty()
                    && self.volume_maintain_requests.is_empty()
                {
                    self.volume_state.transition(VolumeState::Removed);
                    self.clear_media_dependent_client_state();
                    self.on_volume_removed.emit(&());
                    vec![self.broadcast_volume_status()]
                } else {
                    vec![FSOutbound::to(
                        self.encode_volume_status_response(tan, self.effective_volume_state()),
                        client,
                    )]
                }
            }
            _ => error(),
        }
    }

    fn handle_initialize_volume(&mut self, tan: TAN, request: &[u8]) -> Vec<u8> {
        if !self.properties.supports_volume_management {
            return encode_error_response(
                FSFunction::InitializeVolume.as_u8(),
                tan,
                FSError::NotSupported,
            );
        }
        if let Some(response) = self.reject_if_volume_removed(FSFunction::InitializeVolume, tan) {
            return response;
        }
        let Ok(new_volume_name) = parse_initialize_volume_request(request) else {
            return encode_error_response(
                FSFunction::InitializeVolume.as_u8(),
                tan,
                FSError::MalformedRequest,
            );
        };
        if !self.open_files.is_empty() {
            return encode_error_response(
                FSFunction::InitializeVolume.as_u8(),
                tan,
                FSError::AccessDenied,
            );
        }
        self.files.clear();
        self.file_attrs.clear();
        self.file_date_times.clear();
        self.directories.clear();
        self.directories.push("\\".to_string());
        self.clear_all_open_handles();
        for client in self.clients.values_mut() {
            client.current_directory = "\\".to_string();
        }
        if let Some(volume_name) = new_volume_name {
            self.volume_name = volume_name;
        }
        if self.volume_state.state() == VolumeState::Removed {
            self.volume_state.transition(VolumeState::Present);
        }
        success_response(FSFunction::InitializeVolume, tan)
    }

    fn allocate_handle(&mut self) -> FileHandle {
        for _ in 0..255 {
            let candidate = self.next_handle;
            self.next_handle = self.next_handle.wrapping_add(1);
            if self.next_handle == 0 || self.next_handle == INVALID_FILE_HANDLE {
                self.next_handle = 1;
            }
            if candidate != INVALID_FILE_HANDLE
                && candidate != RESERVED_FILE_HANDLE_0
                && !self.open_files.iter().any(|f| f.handle == candidate)
            {
                return candidate;
            }
        }
        INVALID_FILE_HANDLE
    }

    fn cleanup_expired_tan_cache(&mut self) {
        let now = self.current_time_ms;
        let timeout = self.config.tan_cache_timeout_ms;
        for client in self.clients.values_mut() {
            client.tan_cache.retain(|_, v| !v.is_expired(now, timeout));
        }
    }

    fn cleanup_disconnected_clients(&mut self) {
        let now = self.current_time_ms;
        let timeout = self.config.ccm_timeout_ms;
        let to_remove: Vec<Address> = self
            .clients
            .iter()
            .filter(|(_, c)| !c.is_connected(now, timeout))
            .map(|(&addr, _)| addr)
            .collect();
        for addr in &to_remove {
            self.open_files.retain(|f| f.owner != *addr);
            let was_connected = self.clients.remove(addr).is_some_and(|c| c.ccm_seen);
            if was_connected {
                self.on_client_disconnected.emit(addr);
            }
        }
    }

    // ─── Test / introspection helpers ─────────────────────────────────

    #[must_use]
    pub fn open_files(&self) -> &[OpenFile] {
        &self.open_files
    }

    #[must_use]
    pub fn clients(&self) -> &BTreeMap<Address, ServerClientConnection> {
        &self.clients
    }

    #[must_use]
    fn is_path_open(&self, path: &str) -> bool {
        self.open_files.iter().any(|open| open.path == path)
    }

    fn effective_volume_state(&self) -> VolumeState {
        match self.volume_state.state() {
            VolumeState::Removed | VolumeState::PreparingForRemoval => self.volume_state.state(),
            _ if !self.open_files.is_empty() || !self.volume_maintain_requests.is_empty() => {
                VolumeState::InUse
            }
            _ => VolumeState::Present,
        }
    }

    fn volume_status_path_matches(&self, path: &str) -> bool {
        path.is_empty() || path == "\\" || path == self.volume_name
    }

    fn encode_volume_status_response(&self, tan: TAN, state: VolumeState) -> Vec<u8> {
        let name = self.volume_name.as_bytes();
        let mut response = Vec::with_capacity(6 + name.len());
        response.push(FSFunction::VolumeStatus.as_u8());
        response.push(tan);
        response.push(state.as_u8());
        response.push(self.volume_max_removal_time_minutes());
        response.extend_from_slice(&(name.len() as u16).to_le_bytes());
        response.extend_from_slice(name);
        response
    }

    fn encode_volume_status_error_response(&self, tan: TAN) -> Vec<u8> {
        let mut response = self.encode_volume_status_response(tan, self.effective_volume_state());
        response[2] = VOLUME_STATUS_ERROR;
        response
    }

    fn volume_max_removal_time_minutes(&self) -> u8 {
        let minutes = self.volume_max_removal_time_ms.div_ceil(60_000);
        minutes.min(250) as u8
    }

    fn open_file_access_conflicts(&self, path: &str, requested_flags: u8) -> bool {
        let requested_writes = open_mode_writes(requested_flags);
        self.open_files.iter().any(|open| {
            !open.is_directory
                && open.path == path
                && (requested_writes || open_mode_writes(open.flags))
        })
    }

    fn is_file_path_directory(&self, path: &str) -> bool {
        if path == "\\" {
            return true;
        }
        let directory_path = ensure_directory_suffix(path.to_string());
        self.directories.iter().any(|d| d == &directory_path)
    }

    fn file_exists_at_directory_path(&self, directory_path: &str) -> bool {
        if directory_path == "\\" {
            return false;
        }
        let file_path = directory_path.trim_end_matches('\\');
        self.files.contains_key(file_path)
    }

    fn file_date_time(&self, path: &str) -> (u16, u16) {
        self.file_date_times
            .get(path)
            .copied()
            .unwrap_or_else(default_file_date_time)
    }

    fn touch_file_date_time(&mut self, path: &str) {
        self.file_date_times
            .insert(path.to_string(), default_file_date_time());
    }
}

