impl VTServer {
    #[must_use]
    pub fn new(config: VTServerConfig) -> Self {
        Self {
            state: StateMachine::new(VTServerState::Disconnected),
            clients: Vec::new(),
            status_timer_ms: 0,
            busy_codes: 0,
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
        #[cfg(any(feature = "default", feature = "cli"))]
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

    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn set_storage_path(&mut self, path: impl AsRef<std::path::Path>) {
        let p = path.as_ref().to_path_buf();
        for c in &mut self.clients {
            c.set_storage_path(&p);
        }
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn load_all_versions(&mut self) -> u32 {
        self.clients
            .iter_mut()
            .map(ServerWorkingSet::load_all_versions_from_disk)
            .sum()
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn save_all_versions(&self) -> u32 {
        self.clients
            .iter()
            .map(ServerWorkingSet::save_all_versions_to_disk)
            .sum()
    }

    #[cfg(any(feature = "default", feature = "cli"))]
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
        // §4.6.9: three seconds without a Working Set Maintenance message is an
        // unexpected shutdown of that Working Set Master.
        let mut gone: Vec<Address> = Vec::new();
        for client in &mut self.clients {
            client.since_maintenance_ms = client.since_maintenance_ms.saturating_add(elapsed_ms);
            if client.since_maintenance_ms >= WORKING_SET_MAINTENANCE_TIMEOUT_MS {
                gone.push(client.client_address);
            }
        }
        for addr in gone {
            self.drop_working_set(addr);
        }

        self.status_timer_ms = self.status_timer_ms.saturating_add(elapsed_ms);
        if self.status_timer_ms >= VT_STATUS_INTERVAL_MS {
            self.status_timer_ms -= VT_STATUS_INTERVAL_MS;
            return Some(self.encode_vt_status());
        }
        None
    }

    /// Annex G.2 VT Status: "Bytes 3, 4 — Object ID of the visible Data/Alarm
    /// Mask of the active Working Set or FFFF16 if no Working Set owns the VT.
    /// Bytes 5, 6 — Object ID of the visible Soft Key Mask ... or FFFF16 ... if
    /// there is no Soft Key Mask defined for the active Data/Alarm Mask."
    ///
    /// These four bytes used to carry the working set's *source address*, so a
    /// VT with the active set at SA 0x26 broadcast visible-mask Object ID
    /// 0x2600 and Soft Key Mask 0x0000 — neither of which is an object in any
    /// pool. Anything following §4.7.14 to track the visible mask read garbage.
    fn encode_vt_status(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_STATUS;
        data[1] = self.active_working_set;
        let (data_mask, soft_key_mask) = self
            .find_client(self.active_working_set)
            .filter(|c| c.pool_activated)
            .map_or((ObjectID::NULL, ObjectID::NULL), |c| {
                let mask = c.object_state.active_data_mask;
                let soft_key = c
                    .object_state
                    .soft_key_masks
                    .get(&mask)
                    .copied()
                    .unwrap_or(c.object_state.active_soft_key_mask);
                (mask, soft_key)
            });
        data[2..4].copy_from_slice(&data_mask.0.to_le_bytes());
        data[4..6].copy_from_slice(&soft_key_mask.0.to_le_bytes());
        // Annex H.1: byte 7 is the VT busy-codes bitfield and byte 8 the VT
        // function code of the command being executed (FF16 when idle). Byte 7
        // is not a version — the VT reports that in the Get Memory response.
        data[6] = self.busy_codes;
        data[7] = 0xFF;
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
                let outcome = self.handle_numeric_value_change(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_STRING_VALUE => {
                let outcome = self.handle_string_value_change(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::SELECT_ACTIVE_WORKING_SET => {
                self.handle_select_active_working_set(msg);
                Vec::new()
            }
            cmd::ESC_INPUT => self.handle_esc_input(msg),
            cmd::HIDE_SHOW => {
                let outcome = self.handle_hide_show(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::ENABLE_DISABLE => {
                let outcome = self.handle_enable_disable(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::SELECT_INPUT_OBJECT_COMMAND => self.handle_select_input_object_command(msg),
            cmd::CONTROL_AUDIO_SIGNAL => {
                let outcome = self.handle_control_audio_signal(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::SET_AUDIO_VOLUME => {
                let outcome = self.handle_set_audio_volume(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_CHILD_LOCATION => {
                let outcome = self.handle_change_child_location(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_SIZE => {
                let outcome = self.handle_change_size(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_BACKGROUND_COLOUR => {
                let outcome = self.handle_change_background_colour(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_END_POINT => {
                let outcome = self.handle_change_end_point(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_FONT_ATTRIBUTES => {
                let outcome = self.handle_change_font_attributes(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_LINE_ATTRIBUTES => {
                let outcome = self.handle_change_line_attributes(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_FILL_ATTRIBUTES => {
                let outcome = self.handle_change_fill_attributes(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_ACTIVE_MASK => {
                let outcome = self.handle_change_active_mask(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_SOFT_KEY_MASK => {
                let outcome = self.handle_change_soft_key_mask(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_ATTRIBUTE => {
                let outcome = self.handle_change_attribute(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::GET_ATTRIBUTE_VALUE => self.handle_get_attribute_value(msg),
            cmd::CHANGE_PRIORITY => {
                let outcome = self.handle_change_priority(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_LIST_ITEM => {
                let outcome = self.handle_change_list_item(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::DELETE_OBJECT_POOL => {
                let outcome = self.handle_delete_object_pool(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_CHILD_POSITION => {
                let outcome = self.handle_change_child_position(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_OBJECT_LABEL => {
                let outcome = self.handle_change_object_label(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_POLYGON_POINT => {
                let outcome = self.handle_change_polygon_point(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::CHANGE_POLYGON_SCALE => {
                let outcome = self.handle_change_polygon_scale(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::GRAPHICS_CONTEXT => self.handle_graphics_context(msg),
            cmd::SELECT_COLOUR_MAP => {
                let outcome = self.handle_select_colour_map(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::LOCK_UNLOCK_MASK => {
                let outcome = self.handle_lock_unlock_mask(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::EXECUTE_MACRO => {
                let outcome = self.handle_execute_macro(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::EXECUTE_EXTENDED_MACRO => {
                let outcome = self.handle_execute_extended_macro(msg);
                self.annex_f_response(msg, outcome)
            }
            cmd::WORKING_SET_MAINTENANCE => self.handle_working_set_maintenance(msg),
            cmd::UNSUPPORTED_VT_FUNCTION => Vec::new(),
            _ => vec![OutboundFrame::to(
                Self::build_unsupported_function(function).to_vec(),
                msg.source,
            )],
        }
    }

    // ─── Per-command handlers ─────────────────────────────────────────

    /// Working Set Maintenance (Annex G.3, function 0xFF).
    ///
    /// §4.6.9: "Each Working Set Master sends the Working Set Maintenance
    /// message once per second. The VT uses this message to ensure that each
    /// Working Set is still present. If the VT does not receive this message
    /// for a period of 3 s or it receives it a second time with the Initiating
    /// bit set it is determined to be an unexpected shutdown of the Working Set
    /// Master."
    ///
    /// This was unhandled, so it fell through to the catch-all and the VT
    /// answered every one with Unsupported VT Function — once per second,
    /// forever — while never noticing a working set going away.
    /// Build the Annex F response for a command, if that command defines one.
    ///
    /// F.1: "The VT shall respond to these commands even if no object pool of
    /// the originating Working Set is loaded. The originator shall wait for a
    /// response before sending another command. Unless stated otherwise,
    /// another command can be sent if a response is not received within 1,5 s."
    ///
    /// Commands with no Annex F response — Object Pool Transfer, answered by
    /// End of Object Pool — return nothing.
    fn annex_f_response(&self, msg: &Message, outcome: CommandOutcome) -> Vec<OutboundFrame> {
        let Some(shape) = VtResponseShape::for_command(msg.data[0]) else {
            return Vec::new();
        };
        vec![OutboundFrame::to(
            shape
                .response(&msg.data, shape.error_bits(outcome))
                .to_vec(),
            msg.source,
        )]
    }

    fn handle_working_set_maintenance(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        self.ensure_client(msg.source);
        // G.3 byte 2 bit 0 on VT version 3 and later; version 2 and prior send
        // 0xFF here, which has bit 0 set, so only treat it as Initiating when
        // the reserved bits are clear as the clause requires.
        let initiating = msg.data[1] & 0x01 != 0 && msg.data[1] & 0xFE == 0;
        let version = msg.data[2];

        let mut shutdown = false;
        if let Some(client) = self.find_client_mut(msg.source) {
            client.since_maintenance_ms = 0;
            client.declared_version = Some(version);
            if initiating {
                // A second Initiating means the master restarted underneath us.
                shutdown = client.seen_initiating;
                client.seen_initiating = true;
            }
        }
        if shutdown {
            self.drop_working_set(msg.source);
        }
        // G.3 defines no response: the VT consumes it silently.
        Vec::new()
    }

    /// Forget a working set whose master has gone away (§4.6.9).
    fn drop_working_set(&mut self, addr: Address) {
        self.clients.retain(|c| c.client_address != addr);
        if self.active_working_set == addr {
            self.active_working_set = NULL_ADDRESS;
        }
        self.on_client_disconnected.emit(&addr);
    }

    fn handle_get_memory(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        // Annex D.2: byte 2 reserved, bytes 3-6 Memory Required, bytes 7-8
        // reserved. The tail check used to start at index 5, which is the top
        // byte of a conformant request's size field — so any working set
        // asking for less than 0xFF000000 bytes was dropped with no reply at
        // all, which Annex F.1 forbids ("The VT shall respond to these commands
        // even if no object pool of the originating Working Set is loaded").
        // G3 applies to the reserved bytes themselves: length only.
        if !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        let required = u32::from_le_bytes([msg.data[2], msg.data[3], msg.data[4], msg.data[5]]);
        self.ensure_client(msg.source);
        // Annex D.3: byte 2 is this VT's version of ISO 11783-6, byte 3 the
        // status. Reporting 0 claimed the 2001 Agritechnica limited feature
        // set regardless of what the server is configured to support.
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_MEMORY_RESPONSE;
        data[1] = self.config.vt_version as u8;
        // D.3 byte 3 status: 0 = enough memory, 1 = not enough, do not transmit.
        // A request the VT cannot satisfy still gets an answer (F.1); silence
        // is what left a conformant working set retrying for 1.5 s at a time.
        let enough = self.config.max_pool_bytes == 0 || required <= self.config.max_pool_bytes;
        data[2] = u8::from(!enough);
        if enough {
            if let Some(client) = self.find_client_mut(msg.source) {
                client.pool_upload_allowed = true;
                client.pool_activation_pending = false;
            }
            if matches!(self.state(), VTServerState::WaitForClientStatus) {
                self.transition(VTServerState::WaitForPoolUpload);
            }
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

    /// E5 — C.2.2 b)1): a pool may arrive as any number of sessions, and only
    /// End of Object Pool closes it. Accumulate here; deserialize once there.
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
        client.pool_staging.extend_from_slice(&msg.data[1..]);
    }

    fn handle_store_version(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) {
            return Vec::new();
        }
        let label = parse_label(&msg.data);
        let mut response = [0xFFu8; 8];
        response[0] = cmd::STORE_VERSION;
        // Annex E.5/E.7/E.9: "Bytes 2―5 Reserved, set to FF16; Byte 6 Error
        // Codes". The code went in byte 2, which the standard reserves — and
        // the client's own canonicality check then discarded every conformant
        // VT's response, so a stored pool was never restored.
        response[5] = match self.find_client_mut(msg.source) {
            Some(c) if c.pool_uploaded && !c.pool.is_empty() => {
                if c.store_version(&label, 5) {
                    0x00
                } else {
                    // E.5 bit 2: insufficient memory available.
                    0x04
                }
            }
            // No pool to store: "any other error" (bit 3). Bit 0 is Reserved.
            _ => 0x08,
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
        // Annex E.5/E.7/E.9: "Bytes 2―5 Reserved, set to FF16; Byte 6 Error
        // Codes". The code went in byte 2, which the standard reserves — and
        // the client's own canonicality check then discarded every conformant
        // VT's response, so a stored pool was never restored.
        // E.7 bit 1: version label is not correct / unknown.
        response[5] = if success { 0x00 } else { 0x02 };

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
        // Annex E.5/E.7/E.9: "Bytes 2―5 Reserved, set to FF16; Byte 6 Error
        // Codes". The code went in byte 2, which the standard reserves — and
        // the client's own canonicality check then discarded every conformant
        // VT's response, so a stored pool was never restored.
        // E.9 bit 1: version label is not correct / unknown.
        response[5] = if success { 0x00 } else { 0x02 };
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
        let outcome = match self.find_client_mut(msg.source) {
            None => EndOfPoolOutcome::NoPool,
            Some(c) => {
                let staged = core::mem::take(&mut c.pool_staging);
                c.pool_upload_allowed = false;
                c.pool_activation_pending = false;
                if staged.is_empty() {
                    // C.2.2 b)3): End of Object Pool with nothing transferred.
                    EndOfPoolOutcome::NoPool
                } else {
                    match ObjectPool::deserialize(&staged) {
                        Err(_) => EndOfPoolOutcome::Malformed,
                        Ok(incoming) if incoming.is_empty() => EndOfPoolOutcome::NoPool,
                        Ok(incoming) => {
                            // C.2.6: a runtime update replaces the objects it
                            // carries and leaves the rest of the pool alone.
                            let mut merged = c.pool.clone();
                            for object in incoming.objects() {
                                merged = merged.with_object(object.clone());
                            }
                            if merged.validate().is_err() {
                                EndOfPoolOutcome::Malformed
                            } else {
                                c.pool = merged;
                                c.pool_uploaded = true;
                                c.pool_activated = true;
                                initialise_working_set_special_controls(
                                    &c.pool,
                                    &mut c.object_state,
                                );
                                EndOfPoolOutcome::Accepted
                            }
                        }
                    }
                }
            }
        };
        if matches!(outcome, EndOfPoolOutcome::Accepted) {
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
            // E8 — C.2.5: "When the VT replies with an error of any type, the
            // VT should delete the object pool from volatile memory storage".
            // Byte 7 bit 3 then truthfully reports that deletion. The old code
            // always claimed bit 1 (a missing object reference) regardless of
            // what actually went wrong.
            if let Some(c) = self.find_client_mut(msg.source) {
                c.pool = ObjectPool::default();
                c.pool_uploaded = false;
                c.pool_activated = false;
                c.object_state = ServerObjectState::default();
            }
            data[1] = 0x01;
            data[6] = match outcome {
                // Bit 1: the pool references an object it does not contain.
                EndOfPoolOutcome::Malformed => 0x02 | 0x08,
                // Bit 2: any other error — here, nothing was transferred.
                _ => 0x04 | 0x08,
            };
        }
        vec![OutboundFrame::to(data.to_vec(), msg.source)]
    }

    fn handle_numeric_value_change(&mut self, msg: &Message) -> CommandOutcome {
        if msg.data.len() != 8 || msg.data[3] != 0xFF {
            return CommandOutcome::Other;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let Some(client) = self
            .find_client(msg.source)
            .filter(|client| client.pool_activated)
        else {
            return CommandOutcome::Other;
        };
        let Some(object) = client.pool.find(id) else {
            return CommandOutcome::Other;
        };
        let object_type = object.r#type;
        let Some(value_width) = numeric_value_width_for_type(object_type) else {
            return CommandOutcome::Other;
        };
        if !numeric_value_payload_width_is_canonical(&msg.data, value_width) {
            return CommandOutcome::Other;
        }
        let raw_val = u32_le(&msg.data[4..]);
        let value = match value_width {
            1 => raw_val & 0xFF,
            2 => raw_val & 0xFFFF,
            _ => raw_val,
        };
        if !numeric_value_is_valid(&client.pool, object, value) {
            return CommandOutcome::Other;
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
        CommandOutcome::Done
    }

    fn handle_string_value_change(&mut self, msg: &Message) -> CommandOutcome {
        if msg.data.len() < 5 {
            return CommandOutcome::Other;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let len = u16_le(&msg.data[3..]) as usize;
        let end = 5 + len;
        if !vt_string_payload_is_canonical(&msg.data, end) {
            return CommandOutcome::Other;
        }
        let Some(s) = decode_vt_string_value(&msg.data[5..end]) else {
            return CommandOutcome::Other;
        };
        let Some(s) = self.normalized_string_value_change(msg.source, id, s) else {
            return CommandOutcome::Other;
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
        CommandOutcome::Done
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
            ObjectType::InputString => {
                let body = obj.get_input_string_body().ok()?;
                if body.variable_reference != ObjectID::NULL {
                    return None;
                }
                body.max_length as usize
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
