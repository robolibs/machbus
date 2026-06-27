impl VtRenderRuntime {
    /// Build a runtime from an already deserialised object pool.
    ///
    /// `ObjectID::NULL` is used for the first build so the layout engine
    /// resolves the standard initial mask from the Working Set child list.
    pub fn from_pool(pool: ObjectPool, config: LayoutConfig) -> Result<Self> {
        Self::from_pool_with_state(pool, config, RenderInitialState::default())
    }

    /// Build a runtime from the server-side working-set cache.
    ///
    /// The uploaded pool is cloned, and the server's current active mask plus
    /// render-affecting state are folded into the first scene.
    pub fn from_server_working_set(ws: &ServerWorkingSet, config: LayoutConfig) -> Result<Self> {
        let mut overrides = RuntimeOverrides::new();
        for (id, visible) in &ws.object_state.visibility {
            if ws
                .pool
                .find(*id)
                .is_some_and(|object| object.r#type == ObjectType::Container)
            {
                overrides.set_visible_mut(*id, *visible);
            }
        }
        for (id, enabled) in &ws.object_state.enable_state {
            overrides.set_enabled_mut(*id, *enabled);
        }
        for (id, colour) in &ws.object_state.background_colours {
            if ws
                .pool
                .find(*id)
                .is_some_and(|object| object.r#type == ObjectType::GraphicContext)
            {
                overrides.set_background_mut(*id, *colour);
            }
        }
        for (id, value) in &ws.object_state.numeric_values {
            if ws
                .pool
                .find(*id)
                .is_some_and(|object| object.r#type == ObjectType::InputList)
            {
                overrides.set_numeric_value_mut(*id, *value);
            }
        }
        let active = ws.object_state.active_data_mask;
        let selected_colour_map = ws.object_state.selected_colour_map;
        let selected_colour_palette = ws.object_state.selected_colour_palette;
        let object_labels = ws.object_state.object_labels.clone();
        let graphics_contexts = ws.object_state.graphics_contexts.clone();
        let mask_locks = ws.object_state.mask_locks.clone();
        let selected_input_object = ws.object_state.selected_input_object;
        let open_input_object = ws.object_state.open_input_object;
        let mut pool = ws.pool.clone();
        apply_server_object_state_to_pool(&mut pool, &ws.object_state)?;
        Self::from_pool_with_state(
            pool,
            config,
            RenderInitialState {
                requested_mask: active,
                selected_colour_map,
                selected_colour_palette,
                object_labels,
                graphics_contexts,
                mask_locks,
                selected_input_object,
                open_input_object,
                overrides,
                user_layout_placements: PlacementMap::new(),
                user_layout_selection: HashMap::new(),
            },
        )
    }

    fn from_pool_with_state(
        pool: ObjectPool,
        config: LayoutConfig,
        initial: RenderInitialState,
    ) -> Result<Self> {
        pool.validate()?;
        let initial = initial.with_working_set_special_controls(&pool);
        let mut object_labels = object_labels_from_pool(&pool)?;
        object_labels.extend(initial.object_labels);
        let engine = LayoutEngine::new(config)
            .with_colour_map(initial.selected_colour_map)
            .with_colour_palette(initial.selected_colour_palette)
            .with_placements(initial.user_layout_placements.clone())
            .with_overrides(initial.overrides.clone());
        let scene = engine.build(&pool, initial.requested_mask);
        let active_mask = scene.active_mask;
        let mut input = InputRuntime::new();
        input.bind(&scene);
        if initial.selected_input_object != ObjectID::NULL {
            input.select_input_object(
                &scene,
                initial.selected_input_object,
                initial.open_input_object == initial.selected_input_object,
            );
        }
        Ok(Self {
            pool,
            active_mask,
            selected_colour_map: initial.selected_colour_map,
            selected_colour_palette: initial.selected_colour_palette,
            object_labels,
            graphics_contexts: initial.graphics_contexts,
            mask_locks: initial.mask_locks,
            locked_scene_dirty: false,
            locked_scene_changed_objects: Vec::new(),
            soft_key_page: 0,
            soft_key_pointer_down: None,
            pointing_parent: None,
            activation_hold: None,
            pending_activation_events: Vec::new(),
            animation_elapsed_ms: 0,
            animation_elapsed_by_object: HashMap::new(),
            overrides: initial.overrides,
            user_layout_placements: initial.user_layout_placements,
            user_layout_selection: initial.user_layout_selection,
            engine,
            scene,
            input,
            dirty: false,
        })
    }

    #[inline]
    #[must_use]
    pub fn pool(&self) -> &ObjectPool {
        &self.pool
    }

    #[inline]
    #[must_use]
    pub const fn active_mask(&self) -> ObjectID {
        self.active_mask
    }

    /// Activation transitions that were generated by runtime display changes
    /// rather than by a fresh operator event.
    ///
    /// ISO 11783-6 requires a pressed Soft Key/Button to be released to the
    /// Working Set when the displayed mask erases it. `set_active_mask` queues
    /// those release events here so hosts can lower them to VT-to-ECU messages
    /// after applying the accepted ECU command.
    #[inline]
    #[must_use]
    pub fn pending_activation_events(&self) -> &[VtEvent] {
        &self.pending_activation_events
    }

    /// Drain activation transitions generated by runtime display changes.
    #[must_use]
    pub fn take_pending_activation_events(&mut self) -> Vec<VtEvent> {
        let mut events = Vec::new();
        core::mem::swap(&mut events, &mut self.pending_activation_events);
        events
    }

    #[inline]
    #[must_use]
    pub const fn scene(&self) -> &Scene {
        &self.scene
    }

    #[inline]
    #[must_use]
    pub const fn input(&self) -> &InputRuntime {
        &self.input
    }

    #[must_use]
    pub fn object_label(&self, id: ObjectID) -> Option<ObjectLabelState> {
        self.object_labels.get(&id).copied()
    }

    #[must_use]
    pub fn object_label_text(&self, id: ObjectID) -> Option<String> {
        let label = self.object_label(id)?;
        if label.string_variable == ObjectID::NULL {
            return None;
        }
        let obj = self.pool.find(label.string_variable)?;
        if obj.r#type != ObjectType::StringVariable {
            return None;
        }
        let body = obj.get_string_variable_body().ok()?;
        Some(String::from_utf8_lossy(&body.value).into_owned())
    }

    #[must_use]
    pub fn graphics_context_commands(&self) -> &[GraphicsContextCommand] {
        &self.graphics_contexts
    }

    pub fn graphics_context_commands_for(
        &self,
        id: ObjectID,
    ) -> impl Iterator<Item = &GraphicsContextCommand> {
        self.graphics_contexts
            .iter()
            .filter(move |command| command.object_id == id)
    }

    /// Current zero-based application soft-key page.
    #[inline]
    #[must_use]
    pub const fn soft_key_page(&self) -> u16 {
        self.soft_key_page
    }

    /// Current animation clock used for materialising Animation frames.
    #[inline]
    #[must_use]
    pub const fn animation_elapsed_ms(&self) -> u32 {
        self.animation_elapsed_ms
    }

    /// Smallest visible Animation refresh interval on the active scene.
    ///
    /// Hosts can use this as a scheduler hint: when it returns `Some(ms)`, call
    /// [`advance_animation_time`] or [`set_animation_elapsed_ms`] after that
    /// interval to trigger deterministic invalidation.
    ///
    /// [`advance_animation_time`]: Self::advance_animation_time
    /// [`set_animation_elapsed_ms`]: Self::set_animation_elapsed_ms
    #[must_use]
    pub fn animation_refresh_interval_ms(&self) -> Option<u16> {
        self.scene
            .visible_nodes()
            .filter(|node| node.object_type == ObjectType::Animation)
            .filter_map(|node| {
                let object = self.pool.find(node.id)?;
                let body = object.get_animation_body().ok()?;
                let effective_enabled = self.overrides.enabled(node.id).unwrap_or(body.enabled != 0);
                (effective_enabled && body.refresh_interval_ms != 0).then_some(body.refresh_interval_ms)
            })
            .min()
    }

    fn visible_animating_object_ids(&self) -> Vec<ObjectID> {
        let mut ids = Vec::new();
        for node in self
            .scene
            .visible_nodes()
            .filter(|node| node.object_type == ObjectType::Animation)
        {
            let Some(object) = self.pool.find(node.id) else {
                continue;
            };
            let Ok(body) = object.get_animation_body() else {
                continue;
            };
            let effective_enabled = self.overrides.enabled(node.id).unwrap_or(body.enabled != 0);
            if effective_enabled
                && body.refresh_interval_ms != 0
                && !ids.contains(&node.id)
            {
                ids.push(node.id);
            }
        }
        ids
    }

    fn animation_frame_signature_with(
        &self,
        clocks: &HashMap<ObjectID, u32>,
    ) -> Vec<(ObjectID, Option<usize>, Option<ObjectID>)> {
        self.scene
            .visible_nodes()
            .filter(|node| node.object_type == ObjectType::Animation)
            .map(|node| {
                let frame = self.pool.find(node.id).and_then(|object| {
                    let body = object.get_animation_body().ok()?;
                    let effective_enabled =
                        self.overrides.enabled(node.id).unwrap_or(body.enabled != 0);
                    crate::isobus::vt::render::animation_frame(
                        &body,
                        &object.children_pos,
                        effective_enabled,
                        clocks.get(&node.id).copied().unwrap_or(0),
                    )
                });
                (
                    node.id,
                    frame.map(|frame| frame.index),
                    frame.map(|frame| frame.object),
                )
            })
            .collect()
    }

    /// Number of soft-key pages for the current active mask and layout
    /// configuration. Returns `1` for masks without a soft-key mask, for
    /// unbounded legacy rendering, or for an empty mask.
    #[must_use]
    pub fn soft_key_page_count(&self) -> u16 {
        soft_key_page_count_for(&self.pool, self.active_mask, self.engine.config())
    }

    #[inline]
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Currently selected input object, if any.
    #[inline]
    #[must_use]
    pub const fn selected_input(&self) -> Option<ObjectID> {
        self.input.selected_input()
    }

    /// Currently open input object, if any.
    #[inline]
    #[must_use]
    pub const fn open_input(&self) -> Option<ObjectID> {
        self.input.open_input()
    }

    /// Mark the current scene as consumed by a host backend.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Render the current scene through the existing command-list backend.
    #[must_use]
    pub fn render_commands(&self, renderer: &GtuiRenderer) -> Vec<RenderCommand> {
        let scene_renderer;
        let renderer = if let Some(palette) = &self.scene.effective_palette {
            scene_renderer = GtuiRenderer::new(palette.clone());
            &scene_renderer
        } else {
            renderer
        };
        let mut commands = renderer.render(&self.scene);
        let mut graphics_states = HashMap::new();
        for command in &self.graphics_contexts {
            commands.push(RenderCommand::GraphicsContextReplay {
                object_id: command.object_id,
                subcommand: command.subcommand,
                payload: command.payload.clone(),
            });
            commands.extend(graphics_context_render_commands(
                &self.pool,
                &self.scene,
                &self.engine,
                renderer,
                command,
                self.overrides.background(command.object_id),
                &mut graphics_states,
            ));
        }
        commands
    }

    /// Alias for callers following the plan's API sketch.
    #[must_use]
    pub fn render(&self, renderer: &GtuiRenderer) -> Vec<RenderCommand> {
        self.render_commands(renderer)
    }

    /// Select a new active data/alarm/window mask and rebuild the scene.
    pub fn set_active_mask(&mut self, mask: ObjectID) -> Result<RenderUpdate> {
        if mask == self.active_mask {
            return Ok(RenderUpdate::Unchanged);
        }
        let previous_mask = self.active_mask;
        self.ensure_renderable_mask(mask)?;
        let release_events = self.activation_release_events_for_display_change();
        self.active_mask = mask;
        self.mask_locks.remove(&previous_mask);
        self.locked_scene_dirty = false;
        self.clamp_soft_key_page();
        self.rebuild_without_activation_release_check();
        self.pending_activation_events.extend(release_events);
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: self.active_mask,
        })
    }

    /// Apply one operator event against the current scene.
    #[must_use]
    pub fn handle_operator_event(&mut self, event: OperatorEvent) -> Vec<VtEvent> {
        if Self::operator_event_starts_activation_or_navigation(&event)
            && self.activation_press_is_pending()
        {
            return vec![VtEvent::Ignored {
                reason: "simultaneous soft-key/button activation is not supported",
            }];
        }
        let events = if let OperatorEvent::SoftKeyNavigation(kind) = event {
            vec![self.handle_soft_key_navigation(kind)]
        } else if let OperatorEvent::PhysicalSoftKey(cell_index) = event {
            vec![self.handle_physical_soft_key(cell_index)]
        } else if let OperatorEvent::PhysicalSoftKeyDown(cell_index) = event {
            vec![self.handle_physical_soft_key_down(cell_index)]
        } else if let OperatorEvent::PhysicalSoftKeyUp(cell_index) = event {
            vec![self.handle_physical_soft_key_up(cell_index)]
        } else if let OperatorEvent::Commit = event
            && let Some(event) = self.handle_selected_activation_commit()
        {
            vec![event]
        } else if let Some(events) = self.handle_active_pointing_event(&event) {
            events
        } else if let Some(events) = self.handle_soft_key_pointer_event(&event) {
            events
        } else if let Some(events) = self.handle_pointing_start_event(&event) {
            events
        } else {
            self.input.handle(&self.scene, &event)
        };
        self.update_activation_hold_state(&events);
        events
    }

    fn activation_press_is_pending(&self) -> bool {
        self.activation_hold.is_some()
            || self.soft_key_pointer_down.is_some()
            || self.input.pointer_down_key_group_key().is_some()
    }

    fn operator_event_starts_activation_or_navigation(event: &OperatorEvent) -> bool {
        matches!(
            event,
            OperatorEvent::Tap(_, _)
                | OperatorEvent::PointerDown(_, _)
                | OperatorEvent::PhysicalSoftKey(_)
                | OperatorEvent::PhysicalSoftKeyDown(_)
                | OperatorEvent::SoftKeyActivate(_)
                | OperatorEvent::SoftKeyNavigation(_)
        )
    }

    /// Advance held-activation timing and emit any due `Held` events.
    ///
    /// This is intentionally host-driven: terminal/UI backends decide how often
    /// to call it from their event loop. The runtime only tracks the currently
    /// pressed soft key or button and produces deterministic repeat events.
    #[must_use]
    pub fn advance_activation_hold_time(
        &mut self,
        delta_ms: u32,
        timing: ActivationHoldTiming,
    ) -> Vec<VtEvent> {
        if delta_ms == 0 || timing.repeat_interval_ms == 0 {
            return Vec::new();
        }
        let Some(state) = self.activation_hold.as_mut() else {
            return Vec::new();
        };
        if state.elapsed_ms == 0 {
            state.next_due_ms = timing.initial_delay_ms;
        }
        state.elapsed_ms = state.elapsed_ms.saturating_add(delta_ms);
        let mut out = Vec::new();
        while state.elapsed_ms >= state.next_due_ms {
            out.push(match state.target {
                ActivationHoldTarget::SoftKey(id) => VtEvent::SoftKeyActivation {
                    id,
                    code: KeyActivationCode::Held,
                },
                ActivationHoldTarget::Button(id) => VtEvent::ButtonActivation {
                    id,
                    code: KeyActivationCode::Held,
                },
            });
            let next = state
                .next_due_ms
                .saturating_add(timing.repeat_interval_ms.max(1));
            if next == state.next_due_ms {
                break;
            }
            state.next_due_ms = next;
        }
        out
    }

    /// Advance held-activation timing and lower due `Held` events to bus
    /// payloads.
    ///
    /// If semantic-event lowering fails, the runtime is restored to its
    /// pre-advance state so a failed bus bridge cannot consume hold timing.
    pub fn advance_activation_hold_time_with_bus_messages(
        &mut self,
        delta_ms: u32,
        timing: ActivationHoldTiming,
    ) -> Result<(Vec<VtEvent>, Vec<VtBusMessage>)> {
        let before = self.clone();
        let events = self.advance_activation_hold_time(delta_ms, timing);
        let messages = match self.bus_messages_for_events(&events) {
            Ok(messages) => messages,
            Err(err) => {
                *self = before;
                return Err(err);
            }
        };
        Ok((events, messages))
    }

    /// Advance held-activation timing and lower due `Held` events into full
    /// PGN/addressed VT-to-ECU messages.
    ///
    /// The VT/ECU endpoint pair is validated before runtime hold timing is
    /// advanced, so an unusable message envelope cannot consume repeat timing.
    pub fn advance_activation_hold_time_with_messages(
        &mut self,
        delta_ms: u32,
        timing: ActivationHoldTiming,
        vt_source: Address,
        ecu_destination: Address,
    ) -> Result<(Vec<VtEvent>, Vec<Message>)> {
        validate_vt_to_ecu_envelope(vt_source, ecu_destination)?;
        let (events, bus_messages) =
            self.advance_activation_hold_time_with_bus_messages(delta_ms, timing)?;
        let messages = bus_messages
            .into_iter()
            .map(|message| message.try_into_message(vt_source, ecu_destination))
            .collect::<Result<Vec<_>>>()?;
        Ok((events, messages))
    }

    /// Build VT On User-Layout Hide/Show semantic events for the currently
    /// placed Window Mask and Key Group objects.
    ///
    /// ISO uses this VT-to-ECU notification to tell Working Sets which
    /// user-layout Window Mask / Key Group objects have been displayed or
    /// removed. The runtime reports the current scene snapshot in deterministic
    /// scene order and packs two object/status records per event.
    #[must_use]
    pub fn user_layout_hide_show_events(
        &self,
        transfer_sequence_number: Option<u8>,
    ) -> Vec<VtEvent> {
        pack_hide_show_records(
            self.user_layout_visibility_records(),
            transfer_sequence_number,
        )
    }

    /// Build payload-ready VT On User-Layout Hide/Show messages for the
    /// current scene snapshot.
    pub fn user_layout_hide_show_messages(
        &self,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Vec<VtBusMessage>> {
        self.bus_messages_for_events(&self.user_layout_hide_show_events(transfer_sequence_number))
    }

    /// Build full PGN/addressed VT On User-Layout Hide/Show messages for the
    /// current scene snapshot.
    pub fn user_layout_hide_show_full_messages(
        &self,
        vt_source: Address,
        ecu_destination: Address,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Vec<Message>> {
        self.messages_for_events(
            &self.user_layout_hide_show_events(transfer_sequence_number),
            vt_source,
            ecu_destination,
        )
    }

    /// Build VT On User-Layout Hide/Show semantic events for the active Data
    /// Mask and active Soft Key Mask.
    ///
    /// ISO uses the same H.20 message to tell an inactive-but-still-visible
    /// Working Set that its active Data Mask / Soft Key Mask is shown, or to
    /// hide those masks just before making that Working Set active again.
    #[must_use]
    pub fn active_mask_hide_show_events(
        &self,
        shown: bool,
        transfer_sequence_number: Option<u8>,
    ) -> Vec<VtEvent> {
        pack_hide_show_records(
            self.active_mask_visibility_records(shown),
            transfer_sequence_number,
        )
    }

    /// Build payload-ready VT On User-Layout Hide/Show messages for the active
    /// Data Mask / Soft Key Mask visibility state.
    pub fn active_mask_hide_show_messages(
        &self,
        shown: bool,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Vec<VtBusMessage>> {
        self.bus_messages_for_events(&self.active_mask_hide_show_events(
            shown,
            transfer_sequence_number,
        ))
    }

    /// Build full PGN/addressed VT On User-Layout Hide/Show messages for the
    /// active Data Mask / Soft Key Mask visibility state.
    pub fn active_mask_hide_show_full_messages(
        &self,
        shown: bool,
        vt_source: Address,
        ecu_destination: Address,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Vec<Message>> {
        self.messages_for_events(
            &self.active_mask_hide_show_events(shown, transfer_sequence_number),
            vt_source,
            ecu_destination,
        )
    }

    /// Return the current operator-selected user-layout placements.
    ///
    /// Hosts can persist this small logical-cell snapshot in their own
    /// non-volatile storage and later pass it to
    /// [`Self::restore_user_layout_placements`]. The snapshot uses logical
    /// grid/key-cell coordinates, not pixels, so it can be recalled after a
    /// display-profile size change. The returned records are sorted by object
    /// id so host persistence is deterministic across processes.
    #[must_use]
    pub fn user_layout_placements(&self) -> Vec<UserLayoutPlacement> {
        let mut placements: Vec<_> = self.user_layout_selection.values().copied().collect();
        placements.sort_by_key(|placement| placement.object_id());
        placements
    }

    /// Restore a host-persisted user-layout placement snapshot.
    ///
    /// The full snapshot is validated against the currently loaded pool before
    /// any runtime state changes. Restore accepts currently unavailable
    /// Window Masks / Key Groups so their assigned cells can be blanked as
    /// required by the standard; interactive operator placement still rejects
    /// unavailable targets.
    pub fn restore_user_layout_placements(
        &mut self,
        placements: &[UserLayoutPlacement],
    ) -> Result<RenderUpdate> {
        let mut placement_map = PlacementMap::new();
        let mut selection = HashMap::new();
        for placement in placements {
            let id = placement.object_id();
            if selection.contains_key(&id) {
                return Err(Error::invalid_state(
                    "user-layout placement snapshot contains duplicate object ids",
                ));
            }
            let (x, y) = self.validate_user_layout_placement(*placement, false)?;
            self.validate_user_layout_placement_does_not_overlap(*placement, &selection)?;
            placement_map.set_mut(id, x, y);
            selection.insert(id, *placement);
        }
        if placement_map == self.user_layout_placements && selection == self.user_layout_selection {
            return Ok(RenderUpdate::Unchanged);
        }
        self.user_layout_placements = placement_map;
        self.user_layout_selection = selection;
        Ok(self.rebuild_or_defer_for_mask_lock())
    }

    /// Place an available Window Mask into the VT user-layout data-mask grid.
    ///
    /// The ISO user-layout data-mask grid is 2 columns × 6 rows. This method
    /// models the VT/operator-selected placement state: it rejects unavailable
    /// windows, selections whose standard cell span would overflow the grid,
    /// and selections that would overlap another placed Window Mask. Accepted
    /// placements are stored in the runtime and rebuild the scene.
    pub fn place_window_mask_in_user_layout(
        &mut self,
        id: ObjectID,
        column: u8,
        row: u8,
    ) -> Result<RenderUpdate> {
        let placement = UserLayoutPlacement::WindowMask { id, column, row };
        let (x, y) = self.validate_user_layout_placement(placement, true)?;
        self.validate_user_layout_placement_does_not_overlap(
            placement,
            &self.user_layout_selection,
        )?;
        Ok(self.set_user_layout_placement(placement, x, y))
    }

    /// Place an available Key Group into the VT user-layout soft-key cells.
    ///
    /// A Key Group occupies as many consecutive soft-key cells as it has child
    /// Key objects (one to four). The runtime rejects overlapping Key Group
    /// soft-key ranges; accepted placements are kept in runtime state and
    /// applied on later scene rebuilds.
    pub fn place_key_group_in_user_layout(
        &mut self,
        id: ObjectID,
        first_cell: u8,
    ) -> Result<RenderUpdate> {
        let placement = UserLayoutPlacement::KeyGroup { id, first_cell };
        let (x, y) = self.validate_user_layout_placement(placement, true)?;
        self.validate_user_layout_placement_does_not_overlap(
            placement,
            &self.user_layout_selection,
        )?;
        Ok(self.set_user_layout_placement(placement, x, y))
    }

    /// Clear an operator-selected user-layout placement override.
    pub fn clear_user_layout_placement(&mut self, id: ObjectID) -> RenderUpdate {
        if self.user_layout_placements.get(id).is_none()
            && !self.user_layout_selection.contains_key(&id)
        {
            return RenderUpdate::Unchanged;
        }
        self.user_layout_placements.remove_mut(id);
        self.user_layout_selection.remove(&id);
        self.rebuild_or_defer_for_mask_lock()
    }

    /// Apply one operator event and immediately lower bus-affecting semantic
    /// events into payload-ready VT-to-ECU messages.
    ///
    /// This stateful path also emits Select Input Object messages when an
    /// operator event selects, opens, commits, cancels, or deselects an input.
    /// Preview, page-navigation, and ignored events otherwise produce no bus
    /// message. Pointer press/release/drag-off paths emit explicit activation
    /// codes; completed tap-style soft-key and button activations are still
    /// represented as a press+release pair because that high-level event means
    /// "the complete click/tap was accepted".
    ///
    /// If semantic-event lowering fails, the runtime is restored to its
    /// pre-event state so input focus/edit transactions stay atomic with
    /// payload-ready bus emission.
    pub fn handle_operator_event_with_bus_messages(
        &mut self,
        event: OperatorEvent,
    ) -> Result<(Vec<VtEvent>, Vec<VtBusMessage>)> {
        let before = self.clone();
        let before_selected = self.input.selected_input();
        let before_open = self.input.open_input();
        let events = self.handle_operator_event(event);
        let after_selected = self.input.selected_input();
        let after_open = self.input.open_input();

        let mut messages = Vec::new();
        self.append_input_state_opening_messages(
            before_selected,
            before_open,
            after_selected,
            after_open,
            &mut messages,
        );
        for event in &events {
            if let Err(err) = self.append_bus_messages_for_event(event, &mut messages) {
                *self = before;
                return Err(err);
            }
        }
        self.append_input_state_closing_messages(
            before_open,
            after_selected,
            after_open,
            &mut messages,
        );
        Ok((events, messages))
    }

    /// Apply one operator event and immediately lower bus-affecting semantic
    /// events into full PGN/addressed VT-to-ECU messages.
    ///
    /// Ordering is identical to [`Self::handle_operator_event_with_bus_messages`]:
    /// selected/open-input transitions that must precede the event are emitted
    /// first, event payloads follow, and close transitions are emitted last.
    pub fn handle_operator_event_with_messages(
        &mut self,
        event: OperatorEvent,
        vt_source: Address,
        ecu_destination: Address,
    ) -> Result<(Vec<VtEvent>, Vec<Message>)> {
        validate_vt_to_ecu_envelope(vt_source, ecu_destination)?;
        let (events, bus_messages) = self.handle_operator_event_with_bus_messages(event)?;
        let messages = bus_messages
            .into_iter()
            .map(|message| message.try_into_message(vt_source, ecu_destination))
            .collect::<Result<Vec<_>>>()?;
        Ok((events, messages))
    }

    /// Lower semantic VT events into payload-ready VT-to-ECU messages.
    ///
    /// The returned payloads still need to be routed by the host as
    /// VT-to-ECU/J1939 frames. This method deliberately does not emit messages
    /// for edit previews, local soft-key page navigation, or ignored events.
    pub fn bus_messages_for_events(&self, events: &[VtEvent]) -> Result<Vec<VtBusMessage>> {
        let mut messages = Vec::new();
        for event in events {
            self.append_bus_messages_for_event(event, &mut messages)?;
        }
        Ok(messages)
    }

    /// Lower semantic VT events into full PGN/addressed VT-to-ECU messages.
    pub fn messages_for_events(
        &self,
        events: &[VtEvent],
        vt_source: Address,
        ecu_destination: Address,
    ) -> Result<Vec<Message>> {
        validate_vt_to_ecu_envelope(vt_source, ecu_destination)?;
        let messages = self
            .bus_messages_for_events(events)?
            .into_iter()
            .map(|message| message.try_into_message(vt_source, ecu_destination))
            .collect::<Result<Vec<_>>>()?;
        Ok(messages)
    }

    fn append_bus_messages_for_event(
        &self,
        event: &VtEvent,
        out: &mut Vec<VtBusMessage>,
    ) -> Result<()> {
        match event {
            VtEvent::BooleanValueChanged { id, value } => {
                validate_bus_event_object_id(*id, "boolean value change")?;
                out.push(VtBusMessage::numeric_value_change(*id, u32::from(*value)));
            }
            VtEvent::NumberValueChanged { id, raw } => {
                validate_bus_event_object_id(*id, "number value change")?;
                out.push(VtBusMessage::numeric_value_change(*id, *raw));
            }
            VtEvent::ListSelectionChanged { id, index } => {
                validate_bus_event_object_id(*id, "list selection change")?;
                let raw = u32::try_from(*index)
                    .map_err(|_| Error::invalid_data("Input List selection exceeds u32"))?;
                out.push(VtBusMessage::numeric_value_change(*id, raw));
            }
            VtEvent::StringValueChanged { id, text } => {
                validate_bus_event_object_id(*id, "string value change")?;
                out.push(VtBusMessage::string_value_change(*id, text)?);
            }
            VtEvent::InputEsc {
                id,
                error_code,
                transfer_sequence_number,
            } => {
                validate_bus_event_object_id(*id, "VT ESC")?;
                if let Some(tan) = transfer_sequence_number {
                    out.push(VtBusMessage::vt_esc_with_transfer_sequence_number(
                        *id,
                        *error_code,
                        *tan,
                    )?);
                } else {
                    out.push(VtBusMessage::vt_esc(*id, *error_code));
                }
            }
            VtEvent::FocusChanged { id } => {
                validate_bus_event_object_id(*id, "select input object")?;
                if is_input_object(self.scene.find(*id)) {
                    push_bus_message_if_missing(
                        out,
                        VtBusMessage::select_input_object(
                            *id,
                            true,
                            self.input.open_input() == Some(*id),
                        ),
                    );
                }
            }
            VtEvent::SoftKeyActivated { id } => {
                validate_bus_event_object_id(*id, "soft-key activation")?;
                let (parent_id, key_number) = self.soft_key_activation_context(*id)?;
                out.push(VtBusMessage::soft_key_activation(
                    KeyActivationCode::Pressed,
                    *id,
                    parent_id,
                    key_number,
                ));
                out.push(VtBusMessage::soft_key_activation(
                    KeyActivationCode::Released,
                    *id,
                    parent_id,
                    key_number,
                ));
            }
            VtEvent::SoftKeyActivation { id, code } => {
                validate_bus_event_object_id(*id, "soft-key activation")?;
                let (parent_id, key_number) = self.soft_key_activation_context(*id)?;
                out.push(VtBusMessage::soft_key_activation(
                    *code, *id, parent_id, key_number,
                ));
            }
            VtEvent::ButtonActivated { id } => {
                validate_bus_event_object_id(*id, "button activation")?;
                let (parent_id, key_number) = self.button_activation_context(*id)?;
                out.push(VtBusMessage::button_activation(
                    KeyActivationCode::Pressed,
                    *id,
                    parent_id,
                    key_number,
                ));
                out.push(VtBusMessage::button_activation(
                    KeyActivationCode::Released,
                    *id,
                    parent_id,
                    key_number,
                ));
            }
            VtEvent::ButtonActivation { id, code } => {
                validate_bus_event_object_id(*id, "button activation")?;
                let (parent_id, key_number) = self.button_activation_context(*id)?;
                out.push(VtBusMessage::button_activation(
                    *code, *id, parent_id, key_number,
                ));
            }
            VtEvent::UserLayoutHideShow {
                first,
                second,
                transfer_sequence_number,
            } => {
                validate_bus_event_object_id(first.0, "user-layout hide/show")?;
                if let Some((id, _)) = second {
                    validate_bus_event_object_id(*id, "user-layout hide/show")?;
                }
                out.push(VtBusMessage::user_layout_hide_show(
                    *first,
                    *second,
                    *transfer_sequence_number,
                )?);
            }
            VtEvent::PointingEvent {
                x,
                y,
                touch_state,
                parent_mask,
                transfer_sequence_number,
            } => {
                validate_bus_event_object_id(*parent_mask, "pointing event")?;
                out.push(VtBusMessage::pointing_event(
                    *x,
                    *y,
                    *touch_state,
                    *parent_mask,
                    *transfer_sequence_number,
                )?);
            }
            VtEvent::StringEditPreview { .. }
            | VtEvent::NumberEditPreview { .. }
            | VtEvent::ListSelectionPreview { .. }
            | VtEvent::SoftKeyPageChanged { .. }
            | VtEvent::Ignored { .. } => {}
        }
        Ok(())
    }

    fn append_input_state_opening_messages(
        &self,
        before_selected: Option<ObjectID>,
        before_open: Option<ObjectID>,
        after_selected: Option<ObjectID>,
        after_open: Option<ObjectID>,
        out: &mut Vec<VtBusMessage>,
    ) {
        if before_selected != after_selected {
            if let Some(id) = before_selected
                && is_input_object(self.scene.find(id))
            {
                push_bus_message_if_missing(
                    out,
                    VtBusMessage::select_input_object(id, false, false),
                );
            }
            if let Some(id) = after_selected
                && is_input_object(self.scene.find(id))
            {
                push_bus_message_if_missing(
                    out,
                    VtBusMessage::select_input_object(id, true, after_open == Some(id)),
                );
            }
            return;
        }

        if before_open != after_open
            && let Some(id) = after_open
            && is_input_object(self.scene.find(id))
        {
            push_bus_message_if_missing(out, VtBusMessage::select_input_object(id, true, true));
        }
    }

    fn append_input_state_closing_messages(
        &self,
        before_open: Option<ObjectID>,
        after_selected: Option<ObjectID>,
        after_open: Option<ObjectID>,
        out: &mut Vec<VtBusMessage>,
    ) {
        if before_open == after_open || after_open.is_some() {
            return;
        }
        let Some(id) = before_open.or(after_selected) else {
            return;
        };
        if is_input_object(self.scene.find(id)) {
            push_bus_message_if_missing(out, VtBusMessage::select_input_object(id, true, false));
        }
    }

    fn set_user_layout_placement(
        &mut self,
        placement: UserLayoutPlacement,
        x: i16,
        y: i16,
    ) -> RenderUpdate {
        let id = placement.object_id();
        let previous = self.user_layout_placements.get(id);
        if previous == Some((x, y)) && self.user_layout_selection.get(&id) == Some(&placement) {
            return RenderUpdate::Unchanged;
        }
        self.user_layout_placements.set_mut(id, x, y);
        self.user_layout_selection.insert(id, placement);
        self.rebuild_or_defer_for_mask_lock()
    }

    fn validate_user_layout_placement(
        &self,
        placement: UserLayoutPlacement,
        require_available: bool,
    ) -> Result<(i16, i16)> {
        match placement {
            UserLayoutPlacement::WindowMask { id, column, row } => {
                self.validate_window_mask_user_layout_placement(id, column, row, require_available)
            }
            UserLayoutPlacement::KeyGroup { id, first_cell } => {
                self.validate_key_group_user_layout_placement(id, first_cell, require_available)
            }
        }
    }

    fn validate_user_layout_placement_does_not_overlap(
        &self,
        placement: UserLayoutPlacement,
        existing: &HashMap<ObjectID, UserLayoutPlacement>,
    ) -> Result<()> {
        for (&other_id, &other) in existing {
            if other_id == placement.object_id() {
                continue;
            }
            if self.user_layout_placements_overlap(placement, other)? {
                return Err(Error::invalid_state(
                    "user-layout placement overlaps an existing placement",
                ));
            }
        }
        Ok(())
    }

    fn user_layout_placements_overlap(
        &self,
        a: UserLayoutPlacement,
        b: UserLayoutPlacement,
    ) -> Result<bool> {
        Ok(rects_overlap(
            self.user_layout_placement_rect(a)?,
            self.user_layout_placement_rect(b)?,
        ))
    }

    fn user_layout_placement_rect(&self, placement: UserLayoutPlacement) -> Result<Rect> {
        match placement {
            UserLayoutPlacement::WindowMask { id, column, row } => {
                let body = self
                    .pool
                    .find(id)
                    .ok_or_else(|| {
                        Error::invalid_state(
                            "user-layout Window Mask overlap target is not in the pool",
                        )
                    })?
                    .get_window_mask_body()?;
                let (cols, rows) = window_mask_cell_span(&body);
                let config = self.engine.config();
                let cell_w = (config.canvas.0 / 2).max(1);
                let cell_h = (config.canvas.1 / 6).max(1);
                Ok(Rect::new(
                    i32::from(column) * i32::from(cell_w),
                    i32::from(row) * i32::from(cell_h),
                    cell_w.saturating_mul(u16::from(cols)),
                    cell_h.saturating_mul(u16::from(rows)),
                ))
            }
            UserLayoutPlacement::KeyGroup { id, first_cell } => {
                let obj = self.pool.find(id).ok_or_else(|| {
                    Error::invalid_state("user-layout Key Group overlap target is not in the pool")
                })?;
                let key_count = self.engine.key_group_slot_count(obj).clamp(1, 4);
                let config = self.engine.config();
                let cell_h = soft_key_cell_height_for_config(config);
                Ok(soft_key_cell_rect_for_config(
                    config,
                    usize::from(first_cell),
                    key_count,
                    cell_h,
                ))
            }
        }
    }

    fn validate_window_mask_user_layout_placement(
        &self,
        id: ObjectID,
        column: u8,
        row: u8,
        require_available: bool,
    ) -> Result<(i16, i16)> {
        let Some(obj) = self.pool.find(id) else {
            return Err(Error::invalid_state(
                "user-layout Window Mask placement target is not in the pool",
            ));
        };
        if obj.r#type != ObjectType::WindowMask {
            return Err(Error::invalid_state(
                "user-layout Window Mask placement must target a Window Mask object",
            ));
        }
        let body = obj.get_window_mask_body()?;
        if require_available && body.options & 0x01 == 0 {
            return Err(Error::invalid_state(
                "user-layout Window Mask placement target is not available",
            ));
        }
        let (cols, rows) = window_mask_cell_span(&body);
        if column >= 2
            || row >= 6
            || column.saturating_add(cols) > 2
            || row.saturating_add(rows) > 6
        {
            return Err(Error::invalid_state(
                "user-layout Window Mask placement does not fit the 2x6 cell grid",
            ));
        }

        let config = self.engine.config();
        let cell_w = (config.canvas.0 / 2).max(1);
        let cell_h = (config.canvas.1 / 6).max(1);
        let x = i16::try_from(u16::from(column).saturating_mul(cell_w))
            .map_err(|_| Error::invalid_state("user-layout Window Mask x position overflows"))?;
        let y = i16::try_from(u16::from(row).saturating_mul(cell_h))
            .map_err(|_| Error::invalid_state("user-layout Window Mask y position overflows"))?;
        Ok((x, y))
    }

    fn validate_key_group_user_layout_placement(
        &self,
        id: ObjectID,
        first_cell: u8,
        require_available: bool,
    ) -> Result<(i16, i16)> {
        let Some(obj) = self.pool.find(id) else {
            return Err(Error::invalid_state(
                "user-layout Key Group placement target is not in the pool",
            ));
        };
        if obj.r#type != ObjectType::KeyGroup {
            return Err(Error::invalid_state(
                "user-layout Key Group placement must target a Key Group object",
            ));
        }
        let body = obj.get_key_group_body()?;
        if require_available && body.options & 0x01 == 0 {
            return Err(Error::invalid_state(
                "user-layout Key Group placement target is not available",
            ));
        }
        let key_count = self.engine.key_group_slot_count(obj);
        if !(1..=4).contains(&key_count) {
            return Err(Error::invalid_state(
                "user-layout Key Group placement requires 1..=4 Key Group child slots",
            ));
        }

        let config = self.engine.config();
        let cell_h = soft_key_cell_height_for_config(config);
        let cell_count = soft_key_cell_count_for_config(config, cell_h);
        if usize::from(first_cell).saturating_add(key_count) > cell_count {
            return Err(Error::invalid_state(
                "user-layout Key Group placement does not fit the soft-key cells",
            ));
        }
        let first_cell = usize::from(first_cell);
        let last_cell = first_cell.saturating_add(key_count);
        if self.scene.soft_keys.iter().any(|key| {
            key.visible
                && key.kind == SoftKeyKind::Application
                && (first_cell..last_cell).contains(&usize::from(key.cell_index))
        }) {
            return Err(Error::invalid_state(
                "user-layout Key Group placement must not claim active Soft Key Mask application cells",
            ));
        }
        let reserved_navigation_cells = self.active_soft_key_navigation_cells();
        if reserved_navigation_cells != 0 {
            let first_navigation_cell = cell_count.saturating_sub(reserved_navigation_cells);
            if first_cell >= first_navigation_cell || last_cell > first_navigation_cell
            {
                return Err(Error::invalid_state(
                    "user-layout Key Group placement must not claim soft-key navigation cells",
                ));
            }
        }
        let rect = soft_key_cell_rect_for_config(config, first_cell, key_count, cell_h);
        let x = i16::try_from(rect.x)
            .map_err(|_| Error::invalid_state("user-layout Key Group x position overflows"))?;
        let y = i16::try_from(rect.y)
            .map_err(|_| Error::invalid_state("user-layout Key Group y position overflows"))?;
        Ok((x, y))
    }

    fn active_soft_key_navigation_cells(&self) -> usize {
        self.scene
            .soft_keys
            .iter()
            .filter(|key| {
                key.visible
                    && matches!(
                        key.kind,
                        SoftKeyKind::NavigationPrevious | SoftKeyKind::NavigationNext
                    )
            })
            .count()
    }

    fn user_layout_visibility_records(&self) -> Vec<(ObjectID, bool)> {
        let mut records: Vec<_> = self
            .scene
            .nodes
            .iter()
            .filter(|node| {
                matches!(
                    node.object_type,
                    ObjectType::WindowMask | ObjectType::KeyGroup
                )
            })
            .map(|node| (node.id, node.visible && node.enabled))
            .collect();
        records.sort_by_key(|(id, _)| *id);
        records
    }

    fn active_mask_visibility_records(&self, shown: bool) -> Vec<(ObjectID, bool)> {
        let mut records = Vec::new();
        if self.active_mask != ObjectID::NULL
            && self
                .pool
                .find(self.active_mask)
                .is_some_and(|obj| obj.r#type == ObjectType::DataMask)
        {
            records.push((self.active_mask, shown));
        }
        if let Some(soft_key_mask) = active_soft_key_mask(&self.pool, self.active_mask) {
            records.push((soft_key_mask, shown));
        }
        records
    }

    fn update_activation_hold_state(&mut self, events: &[VtEvent]) {
        for event in events {
            match *event {
                VtEvent::SoftKeyActivation {
                    id,
                    code: KeyActivationCode::Pressed,
                } => {
                    self.activation_hold =
                        Some(ActivationHoldState::new(ActivationHoldTarget::SoftKey(id)));
                }
                VtEvent::ButtonActivation {
                    id,
                    code: KeyActivationCode::Pressed,
                } => {
                    self.activation_hold =
                        Some(ActivationHoldState::new(ActivationHoldTarget::Button(id)));
                }
                VtEvent::SoftKeyActivation {
                    code: KeyActivationCode::Released | KeyActivationCode::Aborted,
                    ..
                }
                | VtEvent::ButtonActivation {
                    code: KeyActivationCode::Released | KeyActivationCode::Aborted,
                    ..
                } => {
                    self.activation_hold = None;
                }
                VtEvent::PointingEvent { .. } => {
                    self.activation_hold = None;
                }
                _ => {}
            }
        }
    }

    fn activation_release_events_for_display_change(&self) -> Vec<VtEvent> {
        let mut events = Vec::new();
        if let Some(state) = self.soft_key_pointer_down
            && state.kind == SoftKeyKind::Application
        {
            events.push(VtEvent::SoftKeyActivation {
                id: state.id,
                code: KeyActivationCode::Released,
            });
        }
        if let Some(id) = self.input.pointer_down_key_group_key()
            && !events.iter().any(|event| {
                matches!(
                    event,
                    VtEvent::SoftKeyActivation {
                        id: existing,
                        ..
                    } if *existing == id
                )
            })
        {
            events.push(VtEvent::SoftKeyActivation {
                id,
                code: KeyActivationCode::Released,
            });
        }
        if let Some(id) = self.input.pointer_down_button() {
            events.push(VtEvent::ButtonActivation {
                id,
                code: KeyActivationCode::Released,
            });
        }
        if events.is_empty()
            && let Some(state) = self.activation_hold
        {
            events.push(match state.target {
                ActivationHoldTarget::SoftKey(id) => VtEvent::SoftKeyActivation {
                    id,
                    code: KeyActivationCode::Released,
                },
                ActivationHoldTarget::Button(id) => VtEvent::ButtonActivation {
                    id,
                    code: KeyActivationCode::Released,
                },
            });
        }
        events
    }

    fn activation_target_is_still_visible_after_rebuild(
        &self,
        event: &VtEvent,
        pressed_button_point: Option<(i32, i32)>,
    ) -> bool {
        match *event {
            VtEvent::SoftKeyActivation { id, .. } => {
                self.scene
                    .soft_keys
                    .iter()
                    .any(|key| {
                        key.id == id
                            && key.visible
                            && key.enabled
                            && key.kind == SoftKeyKind::Application
                    })
                    || self.scene.nodes.iter().any(|node| {
                        node.visible
                            && node.enabled
                            && matches!(
                                &node.kind,
                                NodeKind::KeyGroup {
                                    available: true,
                                    key_ids,
                                    ..
                                } if key_ids.contains(&id)
                            )
                    })
            }
            VtEvent::ButtonActivation { id, .. } => {
                self.scene.find(id).is_some_and(|node| {
                    node.visible
                        && node.enabled
                        && matches!(node.kind, NodeKind::Button { enabled: true, .. })
                        && pressed_button_point.is_none_or(|(px, py)| node.rect.contains(px, py))
                })
            }
            _ => true,
        }
    }

    fn soft_key_activation_context(&self, id: ObjectID) -> Result<(ObjectID, u8)> {
        if let Some(key) = self.scene.soft_keys.iter().find(|key| {
            key.id == id && key.kind == SoftKeyKind::Application && key.visible && key.enabled
        }) {
            let parent =
                active_soft_key_mask(&self.pool, self.active_mask).unwrap_or(ObjectID::NULL);
            return Ok((parent, key.key_number));
        }

        for node in &self.scene.nodes {
            let crate::isobus::vt::render::scene::NodeKind::KeyGroup {
                available: true,
                key_ids,
                key_numbers,
                ..
            } = &node.kind
            else {
                continue;
            };
            if !node.visible || !node.enabled {
                continue;
            }
            if let Some((_, key_number)) = key_ids
                .iter()
                .zip(key_numbers.iter())
                .find(|(key_id, _)| **key_id == id)
            {
                return Ok((node.id, *key_number));
            }
        }

        Err(Error::invalid_data(
            "soft-key activation does not resolve to a visible application key",
        ))
    }

    fn button_activation_context(&self, id: ObjectID) -> Result<(ObjectID, u8)> {
        let Some(node) = self.scene.find(id) else {
            return Err(Error::invalid_data(
                "button activation object is not present in the scene",
            ));
        };
        let crate::isobus::vt::render::scene::NodeKind::Button {
            key_number,
            enabled,
            ..
        } = &node.kind
        else {
            return Err(Error::invalid_data(
                "button activation object is not a Button scene node",
            ));
        };
        if !node.visible || !node.enabled || !enabled {
            return Err(Error::invalid_data(
                "button activation object is not visible and enabled",
            ));
        }
        Ok((node.parent, *key_number))
    }

    fn handle_active_pointing_event(&mut self, event: &OperatorEvent) -> Option<Vec<VtEvent>> {
        let parent_mask = self.pointing_parent?;
        match *event {
            OperatorEvent::PointerMove(px, py) => {
                let rect = self.pointing_parent_rect(parent_mask)?;
                Some(vec![self.pointing_event_for(
                    parent_mask,
                    rect,
                    px,
                    py,
                    KeyActivationCode::Held,
                )])
            }
            OperatorEvent::PointerUp(px, py) => {
                self.pointing_parent = None;
                let rect = self.pointing_parent_rect(parent_mask)?;
                Some(vec![self.pointing_event_for(
                    parent_mask,
                    rect,
                    px,
                    py,
                    KeyActivationCode::Released,
                )])
            }
            _ => None,
        }
    }

    fn handle_pointing_start_event(&mut self, event: &OperatorEvent) -> Option<Vec<VtEvent>> {
        let (px, py, tap) = match *event {
            OperatorEvent::PointerDown(px, py) => (px, py, false),
            OperatorEvent::Tap(px, py) => (px, py, true),
            _ => return None,
        };
        if let Some(node) = self.scene.disabled_interactive_hit_test(px, py) {
            let reason = if matches!(node.kind, NodeKind::Button { .. }) {
                "button is disabled"
            } else {
                "input field is disabled"
            };
            return Some(vec![VtEvent::Ignored { reason }]);
        }
        if self.scene.soft_key_hit_test(px, py).is_some() || self.scene.hit_test(px, py).is_some() {
            return None;
        }
        let (parent_mask, rect) = self.pointing_parent_at(px, py)?;
        let pressed =
            self.pointing_event_for(parent_mask, rect, px, py, KeyActivationCode::Pressed);
        self.activation_hold = None;
        self.soft_key_pointer_down = None;
        if tap {
            return Some(vec![
                pressed,
                self.pointing_event_for(parent_mask, rect, px, py, KeyActivationCode::Released),
            ]);
        }
        self.pointing_parent = Some(parent_mask);
        Some(vec![pressed])
    }

    fn pointing_parent_at(&self, px: i32, py: i32) -> Option<(ObjectID, Rect)> {
        if !self.scene.mask_rect.contains(px, py) {
            return None;
        }
        if let Some(node) = self.scene.nodes.iter().rev().find(|node| {
            node.visible
                && node.enabled
                && node.object_type == ObjectType::WindowMask
                && node.rect.contains(px, py)
                && node.clip.is_none_or(|clip| clip.contains(px, py))
                && self.window_mask_is_free_form(node.id)
        }) {
            return Some((node.id, node.rect));
        }
        match self.pool.find(self.active_mask).map(|obj| obj.r#type) {
            Some(ObjectType::DataMask) => Some((self.active_mask, self.scene.mask_rect)),
            Some(ObjectType::WindowMask) if self.window_mask_is_free_form(self.active_mask) => {
                Some((self.active_mask, self.scene.mask_rect))
            }
            _ => None,
        }
    }

    fn pointing_parent_rect(&self, parent_mask: ObjectID) -> Option<Rect> {
        if parent_mask == self.active_mask {
            return Some(self.scene.mask_rect);
        }
        self.scene
            .nodes
            .iter()
            .rev()
            .find(|node| {
                node.id == parent_mask
                    && node.visible
                    && node.enabled
                    && node.object_type == ObjectType::WindowMask
                    && self.window_mask_is_free_form(node.id)
            })
            .map(|node| node.rect)
    }

    fn window_mask_is_free_form(&self, id: ObjectID) -> bool {
        self.pool
            .find(id)
            .and_then(|obj| obj.get_window_mask_body().ok())
            .is_some_and(|body| body.window_type == 0)
    }

    fn pointing_event_for(
        &self,
        parent_mask: ObjectID,
        rect: Rect,
        px: i32,
        py: i32,
        touch_state: KeyActivationCode,
    ) -> VtEvent {
        let (x, y) = pointing_coordinates(rect, px, py);
        VtEvent::PointingEvent {
            x,
            y,
            touch_state,
            parent_mask,
            transfer_sequence_number: None,
        }
    }

    fn handle_soft_key_pointer_event(&mut self, event: &OperatorEvent) -> Option<Vec<VtEvent>> {
        let (px, py, pointer_event) = match event {
            OperatorEvent::Tap(px, py) => (*px, *py, SoftKeyPointerEvent::Tap),
            OperatorEvent::PointerDown(px, py) => (*px, *py, SoftKeyPointerEvent::Down),
            OperatorEvent::PointerMove(px, py) => (*px, *py, SoftKeyPointerEvent::Move),
            OperatorEvent::PointerUp(px, py) => (*px, *py, SoftKeyPointerEvent::Up),
            _ => return None,
        };
        let Some(key) = self.scene.soft_key_hit_test(px, py) else {
            if matches!(
                pointer_event,
                SoftKeyPointerEvent::Move | SoftKeyPointerEvent::Up
            )
                && let Some(state) = self.soft_key_pointer_down
            {
                if state.source == SoftKeyPressSource::Physical {
                    return Some(Vec::new());
                }
                let state = self
                    .soft_key_pointer_down
                    .take()
                    .expect("checked soft-key pointer state");
                if state.kind == SoftKeyKind::Application {
                    return Some(vec![VtEvent::SoftKeyActivation {
                        id: state.id,
                        code: KeyActivationCode::Aborted,
                    }]);
                }
                return Some(vec![VtEvent::Ignored {
                    reason: "soft-key release did not match pressed cell",
                }]);
            }
            return None;
        };
        let id = key.id;
        let kind = key.kind;
        match pointer_event {
            SoftKeyPointerEvent::Tap => Some(vec![self.activate_soft_key_cell(id, kind)]),
            SoftKeyPointerEvent::Down => {
                self.soft_key_pointer_down = Some(SoftKeyPressState::pointer(id, kind));
                if kind == SoftKeyKind::Application {
                    return Some(vec![VtEvent::SoftKeyActivation {
                        id,
                        code: KeyActivationCode::Pressed,
                    }]);
                }
                Some(vec![VtEvent::Ignored {
                    reason: "soft-key activation waits for release",
                }])
            }
            SoftKeyPointerEvent::Move => {
                let pressed = self.soft_key_pointer_down;
                if pressed
                    .is_some_and(|state| state.id == id && state.kind == kind)
                {
                    return Some(Vec::new());
                }
                if pressed.is_some_and(|state| state.source == SoftKeyPressSource::Physical) {
                    return Some(Vec::new());
                }
                if let Some(state) = self.soft_key_pointer_down.take()
                    && state.kind == SoftKeyKind::Application
                {
                    return Some(vec![VtEvent::SoftKeyActivation {
                        id: state.id,
                        code: KeyActivationCode::Aborted,
                    }]);
                }
                Some(Vec::new())
            }
            SoftKeyPointerEvent::Up => {
                let pressed = self.soft_key_pointer_down;
                if pressed.is_some_and(|state| state.source == SoftKeyPressSource::Physical) {
                    return Some(Vec::new());
                }
                let pressed = self.soft_key_pointer_down.take();
                if !pressed.is_some_and(|state| state.id == id && state.kind == kind) {
                    if let Some(state) = pressed
                        && state.kind == SoftKeyKind::Application
                    {
                        return Some(vec![VtEvent::SoftKeyActivation {
                            id: state.id,
                            code: KeyActivationCode::Aborted,
                        }]);
                    }
                    return Some(vec![VtEvent::Ignored {
                        reason: "soft-key release did not match pressed cell",
                    }]);
                }
                if kind == SoftKeyKind::Application {
                    return Some(vec![VtEvent::SoftKeyActivation {
                        id,
                        code: KeyActivationCode::Released,
                    }]);
                }
                Some(vec![self.activate_soft_key_cell(id, kind)])
            }
        }
    }

    fn activate_soft_key_cell(&mut self, id: ObjectID, kind: SoftKeyKind) -> VtEvent {
        match kind {
            SoftKeyKind::Application => VtEvent::SoftKeyActivated { id },
            SoftKeyKind::NavigationNext | SoftKeyKind::NavigationPrevious => {
                self.handle_soft_key_navigation(kind)
            }
        }
    }

    fn handle_selected_activation_commit(&mut self) -> Option<VtEvent> {
        if self.input.open_input().is_some() {
            return None;
        }
        let id = self.input.selected_input()?;
        if let Some((key_id, key_kind)) = self
            .scene
            .soft_keys
            .iter()
            .find(|key| key.id == id && key.visible && key.enabled)
            .map(|key| (key.id, key.kind))
        {
            return Some(self.activate_soft_key_cell(key_id, key_kind));
        }
        if self.visible_key_group_contains_key(id) {
            return Some(VtEvent::SoftKeyActivated { id });
        }
        if let Some(node) = self.scene.find(id)
            && node.visible
            && node.enabled
            && matches!(node.kind, NodeKind::Button { enabled: true, .. })
        {
            return Some(VtEvent::ButtonActivated { id });
        }
        None
    }

    fn handle_physical_soft_key(&mut self, cell_index: u8) -> VtEvent {
        if let Some(key) = self.scene.soft_key_cell(cell_index) {
            let id = key.id;
            let kind = key.kind;
            return self.activate_soft_key_cell(id, kind);
        }

        if self
            .scene
            .soft_keys
            .iter()
            .any(|key| key.visible && key.cell_index == cell_index)
        {
            return VtEvent::Ignored {
                reason: "physical soft-key cell is not available",
            };
        }

        if let Some(id) = self.key_group_key_for_physical_soft_key(cell_index) {
            return VtEvent::SoftKeyActivated { id };
        }

        VtEvent::Ignored {
            reason: "physical soft-key cell is not available",
        }
    }

    fn handle_physical_soft_key_down(&mut self, cell_index: u8) -> VtEvent {
        if self.soft_key_pointer_down.is_some() || self.input.pointer_down_key_group_key().is_some()
        {
            return VtEvent::Ignored {
                reason: "simultaneous soft-key/button activation is not supported",
            };
        }

        if let Some(key) = self.scene.soft_key_cell(cell_index) {
            let id = key.id;
            let kind = key.kind;
            self.soft_key_pointer_down = Some(SoftKeyPressState::physical(id, kind));
            if kind == SoftKeyKind::Application {
                return VtEvent::SoftKeyActivation {
                    id,
                    code: KeyActivationCode::Pressed,
                };
            }
            return VtEvent::Ignored {
                reason: "soft-key activation waits for release",
            };
        }

        if self
            .scene
            .soft_keys
            .iter()
            .any(|key| key.visible && key.cell_index == cell_index)
        {
            return VtEvent::Ignored {
                reason: "physical soft-key cell is not available",
            };
        }

        if let Some(id) = self.key_group_key_for_physical_soft_key(cell_index) {
            self.input.restore_physical_key_group_key_press(id);
            return VtEvent::SoftKeyActivation {
                id,
                code: KeyActivationCode::Pressed,
            };
        }

        VtEvent::Ignored {
            reason: "physical soft-key cell is not available",
        }
    }

    fn handle_physical_soft_key_up(&mut self, cell_index: u8) -> VtEvent {
        if self
            .soft_key_pointer_down
            .is_some_and(|state| state.source == SoftKeyPressSource::Pointer)
        {
            return VtEvent::Ignored {
                reason: "physical soft-key release did not match a pressed cell",
            };
        }

        if let Some(state) = self.soft_key_pointer_down.take() {
            if let Some(key) = self.scene.soft_key_cell(cell_index)
                && key.id == state.id
                && key.kind == state.kind
            {
                if state.kind == SoftKeyKind::Application {
                    return VtEvent::SoftKeyActivation {
                        id: state.id,
                        code: KeyActivationCode::Released,
                    };
                }
                return self.activate_soft_key_cell(state.id, state.kind);
            }
            if state.kind == SoftKeyKind::Application {
                return VtEvent::SoftKeyActivation {
                    id: state.id,
                    code: KeyActivationCode::Aborted,
                };
            }
            return VtEvent::Ignored {
                reason: "soft-key release did not match pressed cell",
            };
        }

        if let Some(pressed_id) = self.input.take_physical_key_group_key_press() {
            if self.key_group_key_for_physical_soft_key(cell_index) == Some(pressed_id) {
                return VtEvent::SoftKeyActivation {
                    id: pressed_id,
                    code: KeyActivationCode::Released,
                };
            }
            return VtEvent::SoftKeyActivation {
                id: pressed_id,
                code: KeyActivationCode::Aborted,
            };
        }

        VtEvent::Ignored {
            reason: "physical soft-key release did not match a pressed cell",
        }
    }

    fn visible_key_group_contains_key(&self, id: ObjectID) -> bool {
        id != ObjectID::NULL
            && self.scene.nodes.iter().any(|node| {
                node.visible
                    && node.enabled
                    && matches!(
                        &node.kind,
                        NodeKind::KeyGroup {
                            available: true,
                            key_ids,
                            ..
                        } if key_ids.contains(&id)
                    )
            })
    }

    fn key_group_key_for_physical_soft_key(&self, cell_index: u8) -> Option<ObjectID> {
        let config = self.engine.config();
        let cell_h = soft_key_cell_height_for_config(config);
        if usize::from(cell_index) >= soft_key_cell_count_for_config(config, cell_h) {
            return None;
        }
        let area = config.soft_key_area;
        if area.w == 0 || area.h == 0 {
            return None;
        }

        let rect = soft_key_cell_rect_for_config(config, usize::from(cell_index), 1, cell_h);
        let px = rect.x + i32::from(rect.w / 2);
        let py = rect.y + i32::from(rect.h / 2);

        self.scene.nodes.iter().rev().find_map(|node| {
            if !node.visible || !node.enabled || !node.rect.contains(px, py) {
                return None;
            }
            let NodeKind::KeyGroup {
                available: true,
                key_ids,
                ..
            } = &node.kind
            else {
                return None;
            };
            key_group_key_at_scene_point(node, key_ids, px, py)
        })
    }

    fn handle_soft_key_navigation(&mut self, kind: SoftKeyKind) -> VtEvent {
        let update = match kind {
            SoftKeyKind::NavigationNext => self.next_soft_key_page(),
            SoftKeyKind::NavigationPrevious => self.previous_soft_key_page(),
            SoftKeyKind::Application => {
                return VtEvent::Ignored {
                    reason: "application soft key requires an object id",
                };
            }
        };
        match update {
            RenderUpdate::SceneRebuilt { .. } => VtEvent::SoftKeyPageChanged {
                page: self.soft_key_page,
                page_count: self.soft_key_page_count(),
            },
            RenderUpdate::Unchanged => VtEvent::Ignored {
                reason: "soft-key page unchanged",
            },
            RenderUpdate::NotRenderAffecting { reason }
            | RenderUpdate::CommandStreamChanged { reason } => VtEvent::Ignored { reason },
        }
    }

}

fn pack_hide_show_records(
    records: Vec<(ObjectID, bool)>,
    transfer_sequence_number: Option<u8>,
) -> Vec<VtEvent> {
    records
        .chunks(2)
        .map(|chunk| VtEvent::UserLayoutHideShow {
            first: chunk[0],
            second: chunk.get(1).copied(),
            transfer_sequence_number,
        })
        .collect()
}
