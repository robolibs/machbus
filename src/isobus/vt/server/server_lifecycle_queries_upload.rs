impl VTServer {
    #[must_use]
    pub fn new(config: VTServerConfig) -> Self {
        Self {
            state: StateMachine::new(VTServerState::Disconnected),
            clients: Vec::new(),
            status_timer_ms: 0,
            vt_version: config.vt_version,
            screen_width: config.screen_width,
            screen_height: config.screen_height,
            config,
            active_working_set: NULL_ADDRESS,
            aux_channels: Vec::new(),
            on_button_activation: Event::new(),
            on_numeric_value_change: Event::new(),
            on_string_value_change: Event::new(),
            on_input_object_selected: Event::new(),
            on_soft_key_activation: Event::new(),
            on_state_change: Event::new(),
            on_client_connected: Event::new(),
            on_client_disconnected: Event::new(),
            on_active_ws_changed: Event::new(),
        }
    }

    pub fn start(&mut self) -> Result<()> {
        validate_server_advertisement(self.screen_width, self.screen_height, self.vt_version)?;
        self.transition(VTServerState::WaitForClientStatus);
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.transition(VTServerState::Disconnected);
        #[cfg(feature = "default")]
        let _ = self.save_all_versions();
        self.clients.clear();
        Ok(())
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> VTServerState {
        self.state.state()
    }

    #[inline]
    #[must_use]
    pub const fn screen_width(&self) -> u16 {
        self.screen_width
    }

    #[inline]
    #[must_use]
    pub const fn screen_height(&self) -> u16 {
        self.screen_height
    }

    #[inline]
    #[must_use]
    pub fn clients(&self) -> &[ServerWorkingSet] {
        &self.clients
    }

    #[inline]
    #[must_use]
    pub const fn active_working_set(&self) -> Address {
        self.active_working_set
    }

    #[must_use]
    pub fn aux_capabilities(&self) -> &[AuxChannelCapability] {
        &self.aux_channels
    }

    pub fn set_aux_capabilities(&mut self, channels: Vec<AuxChannelCapability>) -> Result<()> {
        if channels.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "VT auxiliary capability response supports at most 255 channels",
            ));
        }
        if channels.iter().any(|channel| !channel.is_valid()) {
            return Err(Error::invalid_data(
                "VT auxiliary capability response contains reserved channel fields",
            ));
        }
        self.aux_channels = channels;
        Ok(())
    }

    pub fn clear_aux_capabilities(&mut self) {
        self.aux_channels.clear();
    }

    /// Bind an uploaded AUX input object to an uploaded AUX function object.
    ///
    /// This is server-side protocol state only. It validates the active object
    /// pool before mutating the assignment cache and deliberately rejects
    /// classic AUX-O/AUX-N cross-assignment.
    pub fn assign_aux_input(
        &mut self,
        client_addr: Address,
        input_object: ObjectID,
        function_object: ObjectID,
    ) -> Result<()> {
        self.validate_aux_assignment(client_addr, input_object, function_object)?;
        let state = self.client_object_state_mut(client_addr).ok_or_else(|| {
            Error::invalid_state("VT AUX assignment requires an active object pool")
        })?;
        state.aux_assignments.insert(input_object, function_object);
        state.aux_input_states.remove(&input_object);
        Ok(())
    }

    pub fn clear_aux_assignment(
        &mut self,
        client_addr: Address,
        input_object: ObjectID,
    ) -> Result<()> {
        if !self.client_pool_has_any_object_type(
            client_addr,
            input_object,
            &[ObjectType::AuxInput, ObjectType::AuxInput2],
        ) {
            return Err(Error::invalid_state(
                "VT AUX assignment clear requires an uploaded AUX input object",
            ));
        }
        let state = self.client_object_state_mut(client_addr).ok_or_else(|| {
            Error::invalid_state("VT AUX assignment requires an active object pool")
        })?;
        state.aux_assignments.remove(&input_object);
        state.aux_input_states.remove(&input_object);
        Ok(())
    }

    /// Apply one AUX input status frame to a previously assigned AUX object.
    ///
    /// Returns `Ok(true)` when the status updated cached assignment state,
    /// `Ok(false)` for well-formed but unassigned input numbers, and `Err` for
    /// malformed/wrong-envelope/wrong-style input that must not mutate state.
    pub fn handle_aux_input_status(&mut self, client_addr: Address, msg: &Message) -> Result<bool> {
        if !valid_vt_peer_address(client_addr) || !valid_vt_peer_address(msg.source) {
            return Err(Error::invalid_data("VT AUX status uses an invalid address"));
        }

        let decoded = match msg.pgn {
            PGN_AUX_INPUT_STATUS => {
                let aux = AuxOFunction::decode(msg)
                    .ok_or_else(|| Error::invalid_data("malformed AUX-O input status"))?;
                if aux.setpoint > 10_000 || !aux_state_matches_type(aux.r#type, aux.state) {
                    return Err(Error::invalid_data("AUX-O status contains invalid state"));
                }
                DecodedAuxInputStatus {
                    style: AuxRuntimeStyle::AuxO,
                    function_number: aux.function_number,
                    r#type: aux.r#type,
                    state: aux.state,
                    setpoint: aux.setpoint,
                }
            }
            PGN_AUX_INPUT_TYPE2 => {
                let aux = AuxNFunction::decode(msg)
                    .ok_or_else(|| Error::invalid_data("malformed AUX-N input status"))?;
                if !aux_state_matches_type(aux.r#type, aux.state) {
                    return Err(Error::invalid_data("AUX-N status contains invalid state"));
                }
                DecodedAuxInputStatus {
                    style: AuxRuntimeStyle::AuxN,
                    function_number: aux.function_number,
                    r#type: aux.r#type,
                    state: aux.state,
                    setpoint: aux.setpoint,
                }
            }
            _ => return Err(Error::invalid_data("wrong PGN for VT AUX input status")),
        };

        let (input_object, function_object) = match self.find_aux_input_object(
            client_addr,
            decoded.style,
            decoded.function_number,
            decoded.r#type,
        )? {
            Some(ids) => ids,
            None => return Ok(false),
        };

        self.validate_aux_assignment(client_addr, input_object, function_object)?;
        let state = self
            .client_object_state_mut(client_addr)
            .ok_or_else(|| Error::invalid_state("VT AUX status requires an active object pool"))?;
        state.aux_input_states.insert(
            input_object,
            AuxInputRuntimeState {
                style: decoded.style,
                input_object,
                function_object,
                function_number: decoded.function_number,
                r#type: decoded.r#type,
                state: decoded.state,
                setpoint: decoded.setpoint,
                source: msg.source,
            },
        );
        Ok(true)
    }

    pub fn set_active_working_set(&mut self, addr: Address) {
        let old = self.active_working_set;
        if old == addr {
            return;
        }
        self.active_working_set = addr;
        self.on_active_ws_changed.emit(&(old, addr));
    }

    // ─── Storage management ───────────────────────────────────────────

    #[cfg(feature = "default")]
    pub fn set_storage_path(&mut self, path: impl AsRef<std::path::Path>) {
        let p = path.as_ref().to_path_buf();
        for c in &mut self.clients {
            c.set_storage_path(&p);
        }
    }

    #[cfg(feature = "default")]
    pub fn load_all_versions(&mut self) -> u32 {
        self.clients
            .iter_mut()
            .map(ServerWorkingSet::load_all_versions_from_disk)
            .sum()
    }

    #[cfg(feature = "default")]
    pub fn save_all_versions(&self) -> u32 {
        self.clients
            .iter()
            .map(ServerWorkingSet::save_all_versions_to_disk)
            .sum()
    }

    #[cfg(feature = "default")]
    pub fn cleanup_expired_versions(&mut self, max_age_days: u32) -> u32 {
        self.clients
            .iter_mut()
            .map(|c| c.cleanup_expired_versions(max_age_days))
            .sum()
    }

    // ─── Outbound message builders (server → client) ──────────────────

    #[must_use]
    pub fn build_button_activation(
        code: KeyActivationCode,
        object_id: impl Into<ObjectID>,
        parent_id: impl Into<ObjectID>,
        key_number: u8,
    ) -> [u8; 8] {
        let object_id = object_id.into();
        let parent_id = parent_id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::BUTTON_ACTIVATION;
        data[1] = code.as_u8();
        data[2..4].copy_from_slice(&object_id.to_le_bytes());
        data[4..6].copy_from_slice(&parent_id.to_le_bytes());
        data[6] = key_number;
        data
    }

    #[must_use]
    pub fn build_soft_key_activation(
        code: KeyActivationCode,
        object_id: impl Into<ObjectID>,
        parent_id: impl Into<ObjectID>,
        key_number: u8,
    ) -> [u8; 8] {
        let object_id = object_id.into();
        let parent_id = parent_id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SOFT_KEY_ACTIVATION;
        data[1] = code.as_u8();
        data[2..4].copy_from_slice(&object_id.to_le_bytes());
        data[4..6].copy_from_slice(&parent_id.to_le_bytes());
        data[6] = key_number;
        data
    }

    #[must_use]
    pub fn build_change_numeric_value(object_id: impl Into<ObjectID>, value: u32) -> [u8; 8] {
        let object_id = object_id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::NUMERIC_VALUE_CHANGE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = 0xFF;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        data
    }

    pub fn build_change_string_value(
        object_id: impl Into<ObjectID>,
        value: &str,
    ) -> Result<Vec<u8>> {
        if value.len() > VT_STRING_VALUE_MAX_LEN {
            return Err(Error::invalid_data(
                "VT string-value notification exceeds u16 length field",
            ));
        }
        let object_id = object_id.into();
        let mut data = Vec::with_capacity(5 + value.len());
        data.push(cmd::STRING_VALUE_CHANGE);
        data.extend_from_slice(&object_id.to_le_bytes());
        data.extend_from_slice(&(value.len() as u16).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
        Ok(data)
    }

    #[must_use]
    pub fn build_select_input_object(
        object_id: impl Into<ObjectID>,
        selected: bool,
        open_for_input: bool,
    ) -> [u8; 8] {
        let object_id = object_id.into();
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_INPUT_OBJECT;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = u8::from(selected);
        data[4] = u8::from(open_for_input);
        data
    }

    #[must_use]
    pub fn build_unsupported_function(function: u8) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::UNSUPPORTED_VT_FUNCTION;
        data[1] = function;
        data
    }

    // ─── Update loop ──────────────────────────────────────────────────

    /// Advance the periodic VT_STATUS broadcast. Returns the broadcast
    /// payload when the cadence elapses, otherwise `None`.
    pub fn update(&mut self, elapsed_ms: u32) -> Option<[u8; 8]> {
        if matches!(self.state(), VTServerState::Disconnected) {
            return None;
        }
        self.status_timer_ms = self.status_timer_ms.saturating_add(elapsed_ms);
        if self.status_timer_ms >= VT_STATUS_INTERVAL_MS {
            self.status_timer_ms -= VT_STATUS_INTERVAL_MS;
            return Some(self.encode_vt_status());
        }
        None
    }

    fn encode_vt_status(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_STATUS;
        data[1] = self.active_working_set;
        data[2] = 0x00;
        data[3] = self.active_working_set;
        data[4] = 0x00;
        data[5] = 0x00;
        data[6] = (self.vt_version & 0xFF) as u8;
        data[7] = 0x00;
        data
    }

    // ─── Inbound dispatch ─────────────────────────────────────────────

    /// Feed an inbound `PGN_ECU_TO_VT` message; returns the outbound
    /// frame(s) the server wants to emit in reply (zero or one in
    /// practice). Side effects: state transitions + events.
    pub fn handle_ecu_message(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !msg.has_usable_envelope_for_pgn(PGN_ECU_TO_VT) || msg.data.is_empty() {
            return Vec::new();
        }
        let function = msg.data[0];
        match function {
            cmd::GET_MEMORY => self.handle_get_memory(msg),
            cmd::OBJECT_POOL_TRANSFER => {
                self.handle_object_pool_transfer(msg);
                Vec::new()
            }
            cmd::STORE_VERSION => self.handle_store_version(msg),
            cmd::LOAD_VERSION => self.handle_load_version(msg),
            cmd::DELETE_VERSION => self.handle_delete_version(msg),
            cmd::GET_VERSIONS => self.handle_get_versions(msg),
            cmd::GET_SUPPORTED_OBJECTS => self.handle_get_supported_objects(msg),
            cmd::GET_HARDWARE => self.handle_get_hardware(msg),
            cmd::GET_SUPPORTED_WIDECHARS => self.handle_get_supported_widechars(msg),
            cmd::GET_NUMBER_SOFTKEYS => self.handle_get_number_of_soft_keys(msg),
            cmd::GET_TEXT_FONT_DATA => self.handle_get_text_font_data(msg),
            cmd::GET_WINDOW_MASK_DATA => self.handle_get_window_mask_data(msg),
            cmd::END_OF_POOL => self.handle_end_of_pool(msg),
            cmd::CHANGE_NUMERIC_VALUE => {
                self.handle_numeric_value_change(msg);
                Vec::new()
            }
            cmd::CHANGE_STRING_VALUE => {
                self.handle_string_value_change(msg);
                Vec::new()
            }
            cmd::SELECT_ACTIVE_WORKING_SET => {
                self.handle_select_active_working_set(msg);
                Vec::new()
            }
            cmd::ESC_INPUT => self.handle_esc_input(msg),
            cmd::HIDE_SHOW => {
                self.handle_hide_show(msg);
                Vec::new()
            }
            cmd::ENABLE_DISABLE => {
                self.handle_enable_disable(msg);
                Vec::new()
            }
            cmd::SELECT_INPUT_OBJECT_COMMAND => self.handle_select_input_object_command(msg),
            cmd::CONTROL_AUDIO_SIGNAL => {
                self.handle_control_audio_signal(msg);
                Vec::new()
            }
            cmd::SET_AUDIO_VOLUME => {
                self.handle_set_audio_volume(msg);
                Vec::new()
            }
            cmd::CHANGE_CHILD_LOCATION => {
                self.handle_change_child_location(msg);
                Vec::new()
            }
            cmd::CHANGE_SIZE => {
                self.handle_change_size(msg);
                Vec::new()
            }
            cmd::CHANGE_BACKGROUND_COLOUR => {
                self.handle_change_background_colour(msg);
                Vec::new()
            }
            cmd::CHANGE_END_POINT => {
                self.handle_change_end_point(msg);
                Vec::new()
            }
            cmd::CHANGE_FONT_ATTRIBUTES => {
                self.handle_change_font_attributes(msg);
                Vec::new()
            }
            cmd::CHANGE_LINE_ATTRIBUTES => {
                self.handle_change_line_attributes(msg);
                Vec::new()
            }
            cmd::CHANGE_FILL_ATTRIBUTES => {
                self.handle_change_fill_attributes(msg);
                Vec::new()
            }
            cmd::CHANGE_ACTIVE_MASK => {
                self.handle_change_active_mask(msg);
                Vec::new()
            }
            cmd::CHANGE_SOFT_KEY_MASK => {
                self.handle_change_soft_key_mask(msg);
                Vec::new()
            }
            cmd::CHANGE_ATTRIBUTE => {
                self.handle_change_attribute(msg);
                Vec::new()
            }
            cmd::GET_ATTRIBUTE_VALUE => self.handle_get_attribute_value(msg),
            cmd::CHANGE_PRIORITY => {
                self.handle_change_priority(msg);
                Vec::new()
            }
            cmd::CHANGE_LIST_ITEM => {
                self.handle_change_list_item(msg);
                Vec::new()
            }
            cmd::DELETE_OBJECT_POOL => {
                self.handle_delete_object_pool(msg);
                Vec::new()
            }
            cmd::CHANGE_CHILD_POSITION => {
                self.handle_change_child_position(msg);
                Vec::new()
            }
            cmd::CHANGE_OBJECT_LABEL => {
                self.handle_change_object_label(msg);
                Vec::new()
            }
            cmd::CHANGE_POLYGON_POINT => {
                self.handle_change_polygon_point(msg);
                Vec::new()
            }
            cmd::CHANGE_POLYGON_SCALE => {
                self.handle_change_polygon_scale(msg);
                Vec::new()
            }
            cmd::GRAPHICS_CONTEXT => self.handle_graphics_context(msg),
            cmd::SELECT_COLOUR_MAP => {
                self.handle_select_colour_map(msg);
                Vec::new()
            }
            cmd::LOCK_UNLOCK_MASK => {
                self.handle_lock_unlock_mask(msg);
                Vec::new()
            }
            cmd::EXECUTE_MACRO => {
                self.handle_execute_macro(msg);
                Vec::new()
            }
            cmd::EXECUTE_EXTENDED_MACRO => {
                self.handle_execute_extended_macro(msg);
                Vec::new()
            }
            cmd::UNSUPPORTED_VT_FUNCTION => Vec::new(),
            _ => vec![OutboundFrame::to(
                Self::build_unsupported_function(function).to_vec(),
                msg.source,
            )],
        }
    }

    // ─── Per-command handlers ─────────────────────────────────────────

    fn handle_get_memory(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 5) {
            return Vec::new();
        }
        self.ensure_client(msg.source);
        if let Some(client) = self.find_client_mut(msg.source) {
            client.pool_upload_allowed = true;
            client.pool_activation_pending = false;
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_MEMORY_RESPONSE;
        data[1] = 0x00;
        data[2] = 0x00;
        if matches!(self.state(), VTServerState::WaitForClientStatus) {
            self.transition(VTServerState::WaitForPoolUpload);
        }
        vec![OutboundFrame::to(data.to_vec(), msg.source)]
    }

    fn handle_get_supported_objects(&self, msg: &Message) -> Vec<OutboundFrame> {
        if msg.source == NULL_ADDRESS || !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        if msg.data[1..].iter().all(|&b| b == 0xFF) {
            return self.handle_get_supported_standard_objects(msg);
        }
        if msg.data[1] == 0x01
            && msg.data[2] == ObjectType::AuxFunction2.as_u8()
            && msg.data[3] == ObjectType::AuxInput2.as_u8()
            && msg.data[4..].iter().all(|&b| b == 0xFF)
        {
            return self.handle_get_supported_aux_objects(msg);
        }
        Vec::new()
    }

    fn handle_get_supported_standard_objects(&self, msg: &Message) -> Vec<OutboundFrame> {
        let mut data = Vec::with_capacity(2 + SUPPORTED_STANDARD_OBJECT_TYPES.len());
        data.push(cmd::GET_SUPPORTED_OBJECTS);
        data.push(u8::try_from(SUPPORTED_STANDARD_OBJECT_TYPES.len()).unwrap_or(u8::MAX));
        data.extend_from_slice(SUPPORTED_STANDARD_OBJECT_TYPES);
        vec![OutboundFrame::to(data, msg.source)]
    }

    fn handle_get_supported_aux_objects(&self, msg: &Message) -> Vec<OutboundFrame> {
        let mut data = Vec::with_capacity(3 + self.aux_channels.len() * 5);
        data.push(cmd::GET_SUPPORTED_OBJECTS);
        data.push(0x01);
        data.push(self.aux_channels.len() as u8);
        for channel in &self.aux_channels {
            data.extend(channel.encode());
        }
        vec![OutboundFrame::to(data, msg.source)]
    }

    fn handle_get_supported_widechars(&self, msg: &Message) -> Vec<OutboundFrame> {
        if msg.source == NULL_ADDRESS || !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        if !msg.data[6..8].iter().all(|&b| b == 0xFF) {
            return Vec::new();
        }

        let plane = msg.data[1];
        let first = u16_le(&msg.data[2..]);
        let last = u16_le(&msg.data[4..]);
        let mut data = Vec::new();
        data.push(cmd::GET_SUPPORTED_WIDECHARS);
        data.push(plane);
        data.extend_from_slice(&first.to_le_bytes());
        data.extend_from_slice(&last.to_le_bytes());

        let error = if plane > 16 {
            0x02
        } else if first > last {
            0x10
        } else {
            0x00
        };
        data.push(error);
        if error != 0 {
            data.push(0);
            return vec![OutboundFrame::to(data, msg.source)];
        }

        let mut ranges = Vec::new();
        if plane == 0 {
            for &(range_first, range_last) in WIDECHAR_MINIMUM_CODE_PLANE_0 {
                let clipped_first = range_first.max(first);
                let clipped_last = range_last.min(last);
                if clipped_first <= clipped_last {
                    ranges.push((clipped_first, clipped_last));
                }
            }
        }
        if ranges.len() > u8::MAX as usize {
            data[6] = 0x01;
            data.push(0);
            return vec![OutboundFrame::to(data, msg.source)];
        }
        data.push(ranges.len() as u8);
        for (range_first, range_last) in ranges {
            data.extend_from_slice(&range_first.to_le_bytes());
            data.extend_from_slice(&range_last.to_le_bytes());
        }
        vec![OutboundFrame::to(data, msg.source)]
    }

    /// Get Hardware response (0xC7): `[fn][boot][graphic type][hw features]
    /// [X pixels u16][Y pixels u16]`. X/Y are the configured screen dimensions.
    fn handle_get_hardware(&self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_parameterless_vt_request(&msg.data) {
            return Vec::new();
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_HARDWARE;
        data[1] = 0xFF; // boot time not available
        data[2] = self.config.graphic_type;
        data[3] = self.config.hardware_features;
        data[4..6].copy_from_slice(&self.screen_width.to_le_bytes());
        data[6..8].copy_from_slice(&self.screen_height.to_le_bytes());
        vec![OutboundFrame::to(data.to_vec(), msg.source)]
    }

    /// Get Number Of Soft Keys response (0xC2): `[fn][rsvd×3][X dots][Y dots]
    /// [virtual count][physical count]`.
    fn handle_get_number_of_soft_keys(&self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_parameterless_vt_request(&msg.data) {
            return Vec::new();
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_NUMBER_SOFTKEYS;
        data[4] = self.config.soft_key_x_pixels;
        data[5] = self.config.soft_key_y_pixels;
        data[6] = self.config.virtual_soft_keys;
        data[7] = self.config.physical_soft_keys;
        vec![OutboundFrame::to(data.to_vec(), msg.source)]
    }

    /// Get Text Font Data response (0xC3): `[fn][rsvd×4][small sizes][large
    /// sizes][styles]` (bitfields).
    fn handle_get_text_font_data(&self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_parameterless_vt_request(&msg.data) {
            return Vec::new();
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_TEXT_FONT_DATA;
        data[5] = self.config.small_font_sizes;
        data[6] = self.config.large_font_sizes;
        data[7] = self.config.font_styles;
        vec![OutboundFrame::to(data.to_vec(), msg.source)]
    }

    /// Get Window Mask Data response (0xC4): `[fn][user-layout data-mask
    /// background][user-layout soft-key-cell background][rsvd×5]`.
    fn handle_get_window_mask_data(&self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_parameterless_vt_request(&msg.data) {
            return Vec::new();
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_WINDOW_MASK_DATA;
        data[1] = self.config.user_layout_data_mask_background_colour;
        data[2] = self.config.user_layout_soft_key_background_colour;
        vec![OutboundFrame::to(data.to_vec(), msg.source)]
    }

    fn handle_object_pool_transfer(&mut self, msg: &Message) {
        if msg.data.len() < 2 {
            return;
        }
        let Some(client) = self.find_client_mut(msg.source) else {
            return;
        };
        if !client.pool_upload_allowed {
            return;
        }
        let Ok(pool) = ObjectPool::deserialize(&msg.data[1..]) else {
            return;
        };
        if pool.is_empty() || pool.validate().is_err() {
            return;
        }
        client.pool = pool;
        client.pool_uploaded = true;
        client.pool_upload_allowed = false;
        client.pool_activation_pending = true;
        client.pool_activated = false;
        client.object_state = ServerObjectState::default();
    }

    fn handle_store_version(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        let label = parse_label(&msg.data);
        let mut response = [0xFFu8; 8];
        response[0] = cmd::STORE_VERSION;
        response[1] = match self.find_client_mut(msg.source) {
            Some(c) if c.pool_uploaded && !c.pool.is_empty() => {
                if c.store_version(&label, 5) {
                    0x00
                } else {
                    0x02
                }
            }
            _ => 0x01,
        };
        vec![OutboundFrame::to(response.to_vec(), msg.source)]
    }

    fn handle_load_version(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        let label = parse_label(&msg.data);
        let success = self
            .find_client_mut(msg.source)
            .map(|c| c.load_version(&label))
            .unwrap_or(false);

        let mut response = [0xFFu8; 8];
        response[0] = cmd::LOAD_VERSION;
        response[1] = if success { 0x00 } else { 0x01 };

        if success {
            if !matches!(self.state(), VTServerState::Connected) {
                self.transition(VTServerState::Connected);
            }
            self.on_client_connected.emit(&msg.source);
        }
        vec![OutboundFrame::to(response.to_vec(), msg.source)]
    }

    fn handle_delete_version(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        let label = parse_label(&msg.data);
        let success = self
            .find_client_mut(msg.source)
            .map(|c| c.delete_version(&label))
            .unwrap_or(false);
        let mut response = [0xFFu8; 8];
        response[0] = cmd::DELETE_VERSION;
        response[1] = if success { 0x00 } else { 0x01 };
        vec![OutboundFrame::to(response.to_vec(), msg.source)]
    }

    fn handle_get_versions(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_parameterless_vt_request(&msg.data) {
            return Vec::new();
        }
        let mut response = vec![cmd::GET_VERSIONS_RESPONSE];
        if let Some(client) = self.find_client(msg.source) {
            if !client.stored_versions.is_empty() {
                let count = client.stored_versions.len().min(MAX_STORED_VERSIONS);
                response.push(count as u8);
                for ver in client.stored_versions.iter().take(count) {
                    let bytes = ver.label.as_bytes();
                    for i in 0..7 {
                        response.push(if i < bytes.len() { bytes[i] } else { 0x20 });
                    }
                }
            } else {
                response.push(0);
            }
        } else {
            response.push(0);
        }
        while response.len() < 8 {
            response.push(0xFF);
        }
        vec![OutboundFrame::to(response, msg.source)]
    }

    fn handle_end_of_pool(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_parameterless_vt_request(&msg.data) {
            return Vec::new();
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::END_OF_POOL;
        let accepted = match self.find_client_mut(msg.source) {
            Some(c)
                if c.pool_activation_pending
                    && c.pool_uploaded
                    && !c.pool.is_empty()
                    && c.pool.validate().is_ok() =>
            {
                c.pool_activated = true;
                c.pool_upload_allowed = false;
                c.pool_activation_pending = false;
                initialise_working_set_special_controls(&c.pool, &mut c.object_state);
                true
            }
            Some(c) => {
                c.pool_upload_allowed = false;
                c.pool_activation_pending = false;
                false
            }
            None => false,
        };
        if accepted {
            data[1] = 0x00;
            data[6] = 0x00;
            if !matches!(self.state(), VTServerState::Connected) {
                self.transition(VTServerState::Connected);
            }
            if self.active_working_set == NULL_ADDRESS {
                self.set_active_working_set(msg.source);
            }
            self.on_client_connected.emit(&msg.source);
        } else {
            data[1] = 0x01;
            data[6] = 0x02;
        }
        vec![OutboundFrame::to(data.to_vec(), msg.source)]
    }

    fn handle_numeric_value_change(&mut self, msg: &Message) {
        if msg.data.len() != 8 || msg.data[3] != 0xFF {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let Some(client) = self
            .find_client(msg.source)
            .filter(|client| client.pool_activated)
        else {
            return;
        };
        let Some(object) = client.pool.find(id) else {
            return;
        };
        let object_type = object.r#type;
        let Some(value_width) = numeric_value_width_for_type(object_type) else {
            return;
        };
        if !numeric_value_payload_width_is_canonical(&msg.data, value_width) {
            return;
        }
        let value = u32_le(&msg.data[4..]);
        if !numeric_value_is_valid(&client.pool, object, value) {
            return;
        }
        if let Some(state) = self.activated_client_object_state_mut(msg.source) {
            state.numeric_values.insert(id, value);
            // Change Numeric Value is the authoritative value source; drop any
            // prior Change Attribute overlay for this object's value AID so
            // Get Attribute Value stays coherent.
            if let Some(value_aid) = value_attribute_id_for_type(object_type) {
                state.attributes.remove(&(id, value_aid));
            }
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeNumericValue { id, value });
            self.on_numeric_value_change.emit(&(id, value));
        }
    }

    fn handle_string_value_change(&mut self, msg: &Message) {
        if msg.data.len() < 5 {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let len = u16_le(&msg.data[3..]) as usize;
        let end = 5 + len;
        if !vt_string_payload_is_canonical(&msg.data, end) {
            return;
        }
        let Some(s) = decode_vt_string_value(&msg.data[5..end]) else {
            return;
        };
        let Some(s) = self.normalized_string_value_change(msg.source, id, s) else {
            return;
        };
        if let Some(state) = self.activated_client_object_state_mut(msg.source) {
            state.string_values.insert(id, s.clone());
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeStringValue {
                    id,
                    text: s.clone(),
                });
            self.on_string_value_change.emit(&(id, s));
        }
    }

    fn normalized_string_value_change(
        &self,
        addr: Address,
        id: ObjectID,
        text: &str,
    ) -> Option<String> {
        if !valid_vt_peer_address(addr) {
            return None;
        }
        let client = self
            .find_client(addr)
            .filter(|client| client.pool_activated)?;
        let obj = client.pool.find(id)?;
        let max_len = match obj.r#type {
            ObjectType::StringVariable => obj.get_string_variable_body().ok()?.length as usize,
            ObjectType::OutputString => {
                let body = obj.get_output_string_body().ok()?;
                if body.variable_reference != ObjectID::NULL {
                    return None;
                }
                body.value.len()
            }
            ObjectType::InputAttributes => obj
                .get_input_attributes_body()
                .ok()?
                .validation_string
                .len(),
            _ => return None,
        };
        if text.len() > max_len {
            return None;
        }
        let mut bytes = text.as_bytes().to_vec();
        bytes.resize(max_len, b' ');
        String::from_utf8(bytes).ok()
    }

    fn handle_select_active_working_set(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 2) {
            return;
        }
        if self.activated_client_object_state_mut(msg.source).is_some() {
            self.set_active_working_set(msg.source);
        }
    }

    fn handle_esc_input(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 1) {
            return Vec::new();
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            let selected_input_object = state.selected_input_object;
            state.open_input_object = ObjectID::NULL;
            state.input_escape_count = state.input_escape_count.saturating_add(1);
            state.accepted_effects.push(ServerRenderEffect::Esc);
            let mut data = [0xFFu8; 8];
            data[0] = cmd::VT_ESC;
            data[1..3].copy_from_slice(&selected_input_object.to_le_bytes());
            return vec![OutboundFrame::to(data.to_vec(), msg.source)];
        }
        Vec::new()
    }

}
