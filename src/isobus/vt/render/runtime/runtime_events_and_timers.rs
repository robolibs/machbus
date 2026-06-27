impl VtRenderRuntime {
    /// Apply one accepted ECU→VT command to the render runtime.
    ///
    /// Commands that affect layout, values, styles, active masks, or overlays
    /// rebuild the scene. Graphics context commands update the backend command
    /// stream without changing the retained scene graph. Protocol-only commands
    /// are reported as [`RenderUpdate::NotRenderAffecting`] instead of
    /// pretending to redraw.
    pub fn apply_ecu_command(&mut self, command: &VtRuntimeCommand) -> Result<RenderUpdate> {
        match command {
            VtRuntimeCommand::HideShow { id, visible } => {
                self.mutate_pool(|pool| apply_hide_show_to_pool(pool, *id, *visible))
            }
            VtRuntimeCommand::EnableDisable { id, enabled } => self.set_enabled(*id, *enabled),
            VtRuntimeCommand::ChangeActiveMask { mask } => self.set_active_mask(*mask),
            VtRuntimeCommand::ChangeNumericValue { id, value } => {
                let changed = self.apply_numeric_value_state(*id, *value)?;
                self.rebuild_if(changed)
            }
            VtRuntimeCommand::ChangeStringValue { id, text } => {
                self.mutate_pool(|pool| apply_string_value_to_pool(pool, *id, text))
            }
            VtRuntimeCommand::ChangeChildLocation {
                parent,
                child,
                x,
                y,
            } => self.mutate_pool(|pool| {
                apply_child_position_to_pool(pool, *parent, *child, i16::from(*x), i16::from(*y))
            }),
            VtRuntimeCommand::ChangeChildPosition {
                parent,
                child,
                x,
                y,
            } => self.mutate_pool(|pool| {
                apply_child_position_to_pool(pool, *parent, *child, *x as i16, *y as i16)
            }),
            VtRuntimeCommand::ChangeSize { id, width, height } => {
                self.mutate_pool_for_object(*id, |pool| {
                    apply_size_to_pool(pool, *id, *width, *height)
                })
            }
            VtRuntimeCommand::ChangeEndPoint {
                id,
                width,
                height,
                line_direction,
            } => self.mutate_pool(|pool| {
                apply_end_point_to_pool(pool, *id, *width, *height, *line_direction)
            }),
            VtRuntimeCommand::ChangeBackgroundColour { id, colour } => {
                self.change_background_colour(*id, *colour)
            }
            VtRuntimeCommand::ChangeFontAttributes { id, attributes } => self.mutate_pool(|pool| {
                apply_attribute_ref_to_pool(pool, *id, AttributeRefKind::Font, *attributes)
            }),
            VtRuntimeCommand::ChangeFontAttributeValues {
                id,
                colour,
                size,
                font_type,
                style,
            } => self.mutate_pool(|pool| {
                apply_font_attribute_values_to_pool(pool, *id, *colour, *size, *font_type, *style)
            }),
            VtRuntimeCommand::ChangeLineAttributes { id, attributes } => self.mutate_pool(|pool| {
                apply_attribute_ref_to_pool(pool, *id, AttributeRefKind::Line, *attributes)
            }),
            VtRuntimeCommand::ChangeLineAttributeValues {
                id,
                colour,
                width,
                line_art,
            } => self.mutate_pool(|pool| {
                apply_line_attribute_values_to_pool(pool, *id, *colour, *width, *line_art)
            }),
            VtRuntimeCommand::ChangeFillAttributes { id, attributes } => self.mutate_pool(|pool| {
                apply_attribute_ref_to_pool(pool, *id, AttributeRefKind::Fill, *attributes)
            }),
            VtRuntimeCommand::ChangeFillAttributeValues {
                id,
                fill_type,
                colour,
                pattern,
            } => self.mutate_pool(|pool| {
                apply_fill_attribute_values_to_pool(pool, *id, *fill_type, *colour, *pattern)
            }),
            VtRuntimeCommand::ChangeSoftKeyMask {
                data_mask,
                soft_key_mask,
            } => self
                .mutate_pool(|pool| apply_soft_key_mask_to_pool(pool, *data_mask, *soft_key_mask)),
            VtRuntimeCommand::ChangeListItem { list, index, item } => self
                .mutate_pool(|pool| apply_list_item_to_pool(pool, *list, *index as usize, *item)),
            VtRuntimeCommand::ChangePolygonPoint { id, index, x, y } => self.mutate_pool(|pool| {
                apply_polygon_point_to_pool(pool, *id, *index as usize, *x, *y)
            }),
            VtRuntimeCommand::ChangePolygonScale { id, width, height } => {
                self.mutate_pool(|pool| apply_polygon_scale_to_pool(pool, *id, *width, *height))
            }
            VtRuntimeCommand::ExecuteMacro { id } => self.execute_macro(*id),
            VtRuntimeCommand::SelectColourMap { id } => self.select_colour_map(*id),
            VtRuntimeCommand::SelectInputObject { id, open_for_input } => {
                let changed = self
                    .input
                    .select_input_object(&self.scene, *id, *open_for_input);
                Ok(if changed {
                    RenderUpdate::NotRenderAffecting {
                        reason: "input selection changed runtime focus state",
                    }
                } else {
                    RenderUpdate::Unchanged
                })
            }
            VtRuntimeCommand::Esc => {
                self.soft_key_pointer_down = None;
                self.pointing_parent = None;
                self.activation_hold = None;
                Ok(if self.input.abort_open_input().is_some() {
                    RenderUpdate::NotRenderAffecting {
                        reason: "ESC closed the open input transaction",
                    }
                } else {
                    RenderUpdate::Unchanged
                })
            }
            VtRuntimeCommand::ChangeGenericAttribute {
                id,
                attribute_id,
                value,
            } => self.change_generic_attribute(*id, *attribute_id, *value),
            VtRuntimeCommand::ChangePriority { id, priority } => {
                let changed = apply_priority_to_pool(&mut self.pool, *id, *priority)?;
                Ok(if changed {
                    RenderUpdate::NotRenderAffecting {
                        reason: "alarm priority changed runtime ordering metadata",
                    }
                } else {
                    RenderUpdate::Unchanged
                })
            }
            VtRuntimeCommand::LockUnlockMask {
                id,
                locked,
                timeout_ms,
            } => Ok(self.apply_mask_lock(*id, *locked, *timeout_ms)),
            VtRuntimeCommand::ChangeObjectLabel { id, label } => {
                Ok(self.apply_object_label(*id, *label))
            }
            VtRuntimeCommand::GraphicsContext {
                id,
                subcommand,
                payload,
            } => self.record_graphics_context_command(*id, *subcommand, payload.clone()),
            VtRuntimeCommand::AudioSignal | VtRuntimeCommand::SetAudioVolume { .. } => {
                Ok(RenderUpdate::NotRenderAffecting {
                    reason: "audio commands are terminal side effects, not draw commands",
                })
            }
        }
    }

    fn record_graphics_context_command(
        &mut self,
        id: ObjectID,
        subcommand: u8,
        payload: Vec<u8>,
    ) -> Result<RenderUpdate> {
        match self.pool.find(id).map(|obj| obj.r#type) {
            Some(ObjectType::GraphicContext | ObjectType::GraphicsContext) => {
                if !graphics_context_payload_is_canonical(subcommand, &payload) {
                    return Err(Error::invalid_data(
                        "graphics-context subcommand payload is not canonical",
                    ));
                }
                if !graphics_context_reference_targets_are_valid(
                    &self.pool, id, subcommand, &payload,
                ) {
                    return Err(Error::invalid_data(
                        "graphics-context subcommand references an invalid object",
                    ));
                }
                self.graphics_contexts.push(GraphicsContextCommand {
                    object_id: id,
                    subcommand,
                    payload,
                });
                self.dirty = true;
                Ok(RenderUpdate::CommandStreamChanged {
                    reason: "graphics context command replay appended to backend command stream",
                })
            }
            Some(_) => Err(Error::invalid_state(
                "VT render runtime graphics-context command must target a graphics context object",
            )),
            None => Err(Error::invalid_state(
                "VT render runtime graphics-context object id is not in the pool",
            )),
        }
    }

    fn apply_object_label(&mut self, id: ObjectID, label: ObjectLabelState) -> RenderUpdate {
        if !object_label_target_is_valid(&self.pool, id) {
            return RenderUpdate::Unchanged;
        }
        if !object_label_state_is_valid(&self.pool, label) {
            return RenderUpdate::Unchanged;
        }
        if self.object_labels.get(&id).copied() == Some(label) {
            return RenderUpdate::Unchanged;
        }
        self.object_labels.insert(id, label);
        RenderUpdate::NotRenderAffecting {
            reason: "object labels are metadata for proprietary screens and input popups",
        }
    }

    fn apply_mask_lock(&mut self, id: ObjectID, locked: bool, timeout_ms: u16) -> RenderUpdate {
        match self.pool.find(id).map(|obj| obj.r#type) {
            Some(ObjectType::DataMask | ObjectType::WindowMask) => {}
            _ => return RenderUpdate::Unchanged,
        }
        if locked {
            let previous = self
                .mask_locks
                .insert(id, MaskLockState { locked, timeout_ms });
            if previous == Some(MaskLockState { locked, timeout_ms }) {
                RenderUpdate::Unchanged
            } else {
                RenderUpdate::NotRenderAffecting {
                    reason: "mask lock freezes visible data-mask refreshes",
                }
            }
        } else {
            let was_locked = self
                .mask_locks
                .remove(&id)
                .is_some_and(|state| state.locked);
            if id == self.active_mask && self.locked_scene_dirty {
                self.locked_scene_dirty = false;
                let changed_objects = core::mem::take(&mut self.locked_scene_changed_objects);
                self.rebuild_with_changed_objects(&changed_objects);
                RenderUpdate::SceneRebuilt {
                    active_mask: self.active_mask,
                }
            } else if was_locked {
                RenderUpdate::NotRenderAffecting {
                    reason: "mask unlock released refresh hold",
                }
            } else {
                RenderUpdate::Unchanged
            }
        }
    }

    /// Apply one accepted effect recorded by [`VTServer`].
    ///
    /// [`VTServer`]: crate::isobus::vt::server::VTServer
    pub fn apply_server_effect(&mut self, effect: &ServerRenderEffect) -> Result<RenderUpdate> {
        let command = VtRuntimeCommand::from(effect);
        self.apply_ecu_command(&command)
    }

    /// Apply a slice of accepted server effects in order.
    pub fn apply_server_effects(
        &mut self,
        effects: &[ServerRenderEffect],
    ) -> Result<Vec<RenderUpdate>> {
        let mut updates = Vec::with_capacity(effects.len());
        for effect in effects {
            updates.push(self.apply_server_effect(effect)?);
        }
        Ok(updates)
    }

    /// Execute every Macro reference bound to `object` for the raw VT macro
    /// event byte `event_id`, preserving the order carried by the object pool.
    ///
    /// This deliberately accepts the raw event byte instead of guessing a
    /// standard semantic name. Hosts can map pointer/key/input events to their
    /// profile-specific macro event IDs while the render runtime owns the
    /// deterministic decode/apply/rebuild path.
    pub fn execute_macro_event(
        &mut self,
        object: ObjectID,
        event_id: u8,
    ) -> Result<Vec<RenderUpdate>> {
        let index = MacroTriggerIndex::build(&self.pool);
        let macros = index.macros_for(object, event_id).to_vec();
        let mut updates = Vec::with_capacity(macros.len());
        for macro_id in macros {
            updates.push(self.execute_macro(macro_id)?);
        }
        Ok(updates)
    }

    /// Rebuild the scene against the current pool, active mask, and overrides.
    pub fn rebuild(&mut self) {
        self.rebuild_with_changed_object(None);
    }

    fn rebuild_without_activation_release_check(&mut self) {
        self.rebuild_with_changed_objects_inner(&[]);
    }

    fn rebuild_with_changed_object(&mut self, changed_object: Option<ObjectID>) {
        if let Some(changed_object) = changed_object {
            self.rebuild_with_changed_objects(&[changed_object]);
        } else {
            self.rebuild_with_changed_objects(&[]);
        }
    }

    fn rebuild_with_changed_objects(&mut self, changed_objects: &[ObjectID]) {
        let release_events = self.activation_release_events_for_display_change();
        let pressed_soft_key = self.soft_key_pointer_down;
        let pressed_key_group_press = self.input.pointer_down_key_group_press();
        let pressed_key_group_key = pressed_key_group_press.map(|press| press.id());
        let pressed_button = self.input.pointer_down_button();
        let pressed_button_point = self.input.pointer_down_button_point();
        let activation_hold = self.activation_hold;
        self.rebuild_with_changed_objects_inner(changed_objects);
        for event in release_events {
            if self.activation_target_is_still_visible_after_rebuild(&event, pressed_button_point) {
                match event {
                    VtEvent::SoftKeyActivation { id, .. } => {
                        if pressed_soft_key.is_some_and(|pressed| pressed.id == id) {
                            self.soft_key_pointer_down = pressed_soft_key;
                        } else if pressed_key_group_key == Some(id)
                            && let Some(press) = pressed_key_group_press
                        {
                            self.input.restore_pointer_down_key_group_press(press);
                        }
                    }
                    VtEvent::ButtonActivation { id, .. } => {
                        if pressed_button == Some(id)
                            && let Some(point) = pressed_button_point
                        {
                            self.input.restore_pointer_down_button(id, point);
                        }
                    }
                    _ => {}
                }
                if activation_hold.is_some_and(|state| match (state.target, &event) {
                    (
                        ActivationHoldTarget::SoftKey(held),
                        VtEvent::SoftKeyActivation { id, .. },
                    ) => held == *id,
                    (ActivationHoldTarget::Button(held), VtEvent::ButtonActivation { id, .. }) => {
                        held == *id
                    }
                    _ => false,
                }) {
                    self.activation_hold = activation_hold;
                }
            } else {
                self.pending_activation_events.push(event);
            }
        }
    }

    fn rebuild_with_changed_objects_inner(&mut self, changed_objects: &[ObjectID]) {
        self.rebuild_scene_snapshot();
        if self.remove_invalid_user_layout_placements_after_rebuild(changed_objects) {
            self.rebuild_scene_snapshot();
        }
        self.input.bind(&self.scene);
        self.soft_key_pointer_down = None;
        self.pointing_parent = None;
        self.activation_hold = None;
        self.dirty = true;
    }

    fn rebuild_scene_snapshot(&mut self) {
        self.clamp_soft_key_page();
        self.engine = self
            .engine
            .clone()
            .with_colour_map(self.selected_colour_map)
            .with_colour_palette(self.selected_colour_palette)
            .with_soft_key_page(self.soft_key_page)
            .with_animation_elapsed_ms(0)
            .with_animation_elapsed_by_object(
                self.animation_elapsed_by_object
                    .iter()
                    .map(|(id, elapsed)| (*id, *elapsed)),
            )
            .with_placements(self.user_layout_placements.clone())
            .with_overrides(self.overrides.clone());
        self.scene = self.engine.build(&self.pool, self.active_mask);
        self.active_mask = self.scene.active_mask;
    }

    fn remove_invalid_user_layout_placements_after_rebuild(
        &mut self,
        changed_objects: &[ObjectID],
    ) -> bool {
        if self.user_layout_selection.is_empty() {
            return false;
        }

        let mut placement_map = PlacementMap::new();
        let mut selection = HashMap::new();
        let mut changed = false;
        let mut placements: Vec<_> = self.user_layout_selection.values().copied().collect();
        if !changed_objects.is_empty() {
            placements.sort_by_key(|placement| {
                (
                    changed_objects.contains(&placement.object_id()),
                    placement.object_id(),
                )
            });
        }
        for placement in placements {
            let id = placement.object_id();
            let placement_result = self
                .validate_user_layout_placement(placement, false)
                .and_then(|(x, y)| {
                    self.validate_user_layout_placement_does_not_overlap(placement, &selection)?;
                    Ok((x, y))
                });
            match placement_result {
                Ok((x, y)) => {
                    placement_map.set_mut(id, x, y);
                    selection.insert(id, placement);
                }
                Err(_) => changed = true,
            }
        }

        if !changed
            && placement_map == self.user_layout_placements
            && selection == self.user_layout_selection
        {
            return false;
        }

        self.user_layout_placements = placement_map;
        self.user_layout_selection = selection;
        true
    }

    /// Set the local Working Set NAME used to validate External Object Pointer
    /// access against registered referenced Working Set pools.
    pub fn set_working_set_name(&mut self, name0: u32, name1: u32) -> RenderUpdate {
        if self.engine.working_set_name() == Some((name0, name1)) {
            return RenderUpdate::Unchanged;
        }
        self.engine = self.engine.clone().with_working_set_name(name0, name1);
        self.rebuild_or_defer_for_mask_lock()
    }

    /// Register another Working Set pool for External Object Pointer
    /// resolution and rebuild the active scene.
    pub fn register_external_object_pool(
        &mut self,
        name0: u32,
        name1: u32,
        pool: ObjectPool,
    ) -> RenderUpdate {
        if self
            .engine
            .external_object_pool_matches(name0, name1, &pool)
        {
            return RenderUpdate::Unchanged;
        }
        self.engine = self
            .engine
            .clone()
            .with_external_object_pool(name0, name1, pool);
        self.rebuild_or_defer_for_mask_lock()
    }

    /// Remove a previously registered external Working Set pool and rebuild
    /// the active scene. External Object Pointers targeting that NAME fall
    /// back to their local default objects after this call.
    pub fn unregister_external_object_pool(&mut self, name0: u32, name1: u32) -> RenderUpdate {
        if !self.engine.has_external_object_pool(name0, name1) {
            return RenderUpdate::Unchanged;
        }
        self.engine = self
            .engine
            .clone()
            .without_external_object_pool(name0, name1);
        self.rebuild_or_defer_for_mask_lock()
    }

    /// Select a zero-based soft-key page and rebuild if the visible page
    /// changes. Out-of-range pages clamp to the last available page.
    pub fn set_soft_key_page(&mut self, page: u16) -> RenderUpdate {
        let max_page = self.soft_key_page_count().saturating_sub(1);
        let next = page.min(max_page);
        if next == self.soft_key_page {
            return RenderUpdate::Unchanged;
        }
        self.soft_key_page = next;
        self.rebuild();
        RenderUpdate::SceneRebuilt {
            active_mask: self.active_mask,
        }
    }

    /// Advance one application soft-key page, clamping at the last page.
    pub fn next_soft_key_page(&mut self) -> RenderUpdate {
        self.set_soft_key_page(self.soft_key_page.saturating_add(1))
    }

    /// Move one application soft-key page backward, clamping at page zero.
    pub fn previous_soft_key_page(&mut self) -> RenderUpdate {
        self.set_soft_key_page(self.soft_key_page.saturating_sub(1))
    }

    /// Advance runtime-owned Lock/Unlock Mask timers.
    ///
    /// A lock timeout of `0` is treated as host-indefinite and only an
    /// explicit unlock command releases it. Non-zero timeouts count down by the
    /// caller-provided elapsed milliseconds; when the active mask expires, any
    /// deferred scene updates are materialised immediately.
    pub fn advance_mask_lock_time(&mut self, delta_ms: u32) -> RenderUpdate {
        if delta_ms == 0 {
            return RenderUpdate::Unchanged;
        }

        let mut expired = Vec::new();
        for (id, state) in &mut self.mask_locks {
            if !state.locked || state.timeout_ms == 0 {
                continue;
            }
            if delta_ms >= u32::from(state.timeout_ms) {
                expired.push(*id);
            } else {
                state.timeout_ms = state.timeout_ms.saturating_sub(delta_ms as u16);
            }
        }

        if expired.is_empty() {
            return RenderUpdate::Unchanged;
        }

        let mut expired_active_mask = false;
        let mut expired_any = false;
        for id in expired {
            if self.mask_locks.remove(&id).is_some() {
                expired_any = true;
                expired_active_mask |= id == self.active_mask;
            }
        }

        if expired_active_mask && self.locked_scene_dirty {
            self.locked_scene_dirty = false;
            let changed_objects = core::mem::take(&mut self.locked_scene_changed_objects);
            self.rebuild_with_changed_objects(&changed_objects);
            RenderUpdate::SceneRebuilt {
                active_mask: self.active_mask,
            }
        } else if expired_any {
            RenderUpdate::NotRenderAffecting {
                reason: "mask lock timeout expired",
            }
        } else {
            RenderUpdate::Unchanged
        }
    }

    /// Set the animation clock and rebuild the active scene if it changed.
    pub fn set_animation_elapsed_ms(&mut self, elapsed_ms: u32) -> RenderUpdate {
        let visible_ids = self.visible_animating_object_ids();
        if elapsed_ms == self.animation_elapsed_ms
            && visible_ids
                .iter()
                .all(|id| self.animation_elapsed_by_object.get(id).copied() == Some(elapsed_ms))
        {
            return RenderUpdate::Unchanged;
        }
        let previous_frames = self.animation_frame_signature_with(&self.animation_elapsed_by_object);
        let mut next_clocks = self.animation_elapsed_by_object.clone();
        for id in visible_ids {
            next_clocks.insert(id, elapsed_ms);
        }
        let next_frames = self.animation_frame_signature_with(&next_clocks);
        self.animation_elapsed_ms = elapsed_ms;
        self.animation_elapsed_by_object = next_clocks;
        if previous_frames == next_frames {
            return RenderUpdate::Unchanged;
        }
        self.rebuild_or_defer_for_mask_lock()
    }

    /// Advance visible Animation object clocks with saturating arithmetic and rebuild.
    pub fn advance_animation_time(&mut self, delta_ms: u32) -> RenderUpdate {
        if delta_ms == 0 {
            return RenderUpdate::Unchanged;
        }
        let previous_frames = self.animation_frame_signature_with(&self.animation_elapsed_by_object);
        let mut next_clocks = self.animation_elapsed_by_object.clone();
        for id in self.visible_animating_object_ids() {
            let elapsed = next_clocks.get(&id).copied().unwrap_or(0);
            next_clocks.insert(id, elapsed.saturating_add(delta_ms));
        }
        let next_frames = self.animation_frame_signature_with(&next_clocks);
        self.animation_elapsed_ms = self.animation_elapsed_ms.saturating_add(delta_ms);
        self.animation_elapsed_by_object = next_clocks;
        if previous_frames == next_frames {
            return RenderUpdate::Unchanged;
        }
        self.rebuild_or_defer_for_mask_lock()
    }

    /// Advance the animation clock and return the next scheduler hint.
    ///
    /// This is the host-loop friendly form of [`Self::advance_animation_time`]:
    /// callers feed it elapsed time from a terminal/UI/display timer, inspect
    /// `update` to decide whether to redraw, and use
    /// `next_refresh_interval_ms` to arm the next timer without re-scanning the
    /// scene themselves.
    #[must_use]
    pub fn tick_animation(&mut self, delta_ms: u32) -> AnimationTick {
        let update = self.advance_animation_time(delta_ms);
        AnimationTick {
            update,
            next_refresh_interval_ms: self.animation_refresh_interval_ms(),
        }
    }

    fn clamp_soft_key_page(&mut self) {
        let max_page = self.soft_key_page_count().saturating_sub(1);
        self.soft_key_page = self.soft_key_page.min(max_page);
    }

    fn rebuild_if(&mut self, changed: bool) -> Result<RenderUpdate> {
        self.rebuild_if_object_changed(changed, None)
    }

    fn rebuild_if_object_changed(
        &mut self,
        changed: bool,
        changed_object: Option<ObjectID>,
    ) -> Result<RenderUpdate> {
        if changed {
            Ok(self.rebuild_or_defer_for_mask_lock_with_changed_object(
                changed_object,
            ))
        } else {
            Ok(RenderUpdate::Unchanged)
        }
    }

    fn rebuild_or_defer_for_mask_lock(&mut self) -> RenderUpdate {
        self.rebuild_or_defer_for_mask_lock_with_changed_object(None)
    }

    fn rebuild_or_defer_for_mask_lock_with_changed_object(
        &mut self,
        changed_object: Option<ObjectID>,
    ) -> RenderUpdate {
        if self.active_mask_is_locked() {
            self.locked_scene_dirty = true;
            if let Some(changed_object) = changed_object
                && !self.locked_scene_changed_objects.contains(&changed_object)
            {
                self.locked_scene_changed_objects.push(changed_object);
            }
            RenderUpdate::NotRenderAffecting {
                reason: "active mask is locked; visible refresh deferred until unlock",
            }
        } else {
            self.rebuild_with_changed_object(changed_object);
            RenderUpdate::SceneRebuilt {
                active_mask: self.active_mask,
            }
        }
    }

    fn active_mask_is_locked(&self) -> bool {
        self.mask_locks
            .get(&self.active_mask)
            .is_some_and(|state| state.locked)
    }

    fn mutate_pool(
        &mut self,
        f: impl FnOnce(&mut ObjectPool) -> Result<bool>,
    ) -> Result<RenderUpdate> {
        let changed = f(&mut self.pool)?;
        self.rebuild_if(changed)
    }

    fn mutate_pool_for_object(
        &mut self,
        changed_object: ObjectID,
        f: impl FnOnce(&mut ObjectPool) -> Result<bool>,
    ) -> Result<RenderUpdate> {
        let changed = f(&mut self.pool)?;
        self.rebuild_if_object_changed(changed, Some(changed_object))
    }

    fn apply_numeric_value_state(&mut self, id: ObjectID, value: u32) -> Result<bool> {
        if !numeric_value_update_is_valid(&self.pool, id, value) {
            return Ok(false);
        }
        let is_input_list = self
            .pool
            .find(id)
            .is_some_and(|object| object.r#type == ObjectType::InputList);
        let mut changed = apply_numeric_value_to_pool(&mut self.pool, id, value)?;
        if is_input_list {
            changed |= self.overrides.set_numeric_value_mut(id, value);
        }
        Ok(changed)
    }

    fn change_background_colour(&mut self, id: ObjectID, colour: u8) -> Result<RenderUpdate> {
        match self.pool.find(id).map(|object| object.r#type) {
            Some(ObjectType::GraphicContext) => {
                if self.overrides.background(id) == Some(colour) {
                    Ok(RenderUpdate::Unchanged)
                } else {
                    self.overrides.set_background_mut(id, colour);
                    Ok(self.rebuild_or_defer_for_mask_lock())
                }
            }
            Some(_) => self.mutate_pool(|pool| apply_background_to_pool(pool, id, colour)),
            None => Ok(RenderUpdate::Unchanged),
        }
    }

    fn select_colour_map(&mut self, id: ObjectID) -> Result<RenderUpdate> {
        let changed = self.select_colour_map_state(id)?;
        self.rebuild_if_object_changed(changed, Some(id))
    }

    fn select_colour_map_state(&mut self, id: ObjectID) -> Result<bool> {
        if id == ObjectID::NULL {
            let changed = self.selected_colour_map != ObjectID::NULL
                || self.selected_colour_palette != Some(ObjectID::NULL);
            self.selected_colour_map = ObjectID::NULL;
            self.selected_colour_palette = Some(ObjectID::NULL);
            if changed {
                self.update_working_set_special_controls_colour_selection(None)?;
            }
            return Ok(changed);
        }

        let object_type = match self.pool.find(id).map(|obj| obj.r#type) {
            Some(object_type @ (ObjectType::ColourMap | ObjectType::ColourPalette)) => object_type,
            Some(_) => {
                return Err(Error::invalid_state(
                    "VT render runtime colour selection must reference a ColourMap or ColourPalette object",
                ));
            }
            None => {
                return Err(Error::invalid_state(
                    "VT render runtime colour selection is not in the pool",
                ));
            }
        };

        if object_type == ObjectType::ColourMap && id == self.selected_colour_map {
            return Ok(false);
        }
        if object_type == ObjectType::ColourPalette && Some(id) == self.selected_colour_palette {
            return Ok(false);
        }

        match object_type {
            ObjectType::ColourMap => self.selected_colour_map = id,
            ObjectType::ColourPalette => self.selected_colour_palette = Some(id),
            _ => unreachable!("validated colour-selection object type"),
        }
        self.update_working_set_special_controls_colour_selection(Some((object_type, id)))?;
        Ok(true)
    }

    fn change_generic_attribute(
        &mut self,
        id: ObjectID,
        attribute_id: u8,
        value: u32,
    ) -> Result<RenderUpdate> {
        let object_type = self.pool.find(id).map(|obj| obj.r#type);
        if !generic_attribute_update_is_valid(&self.pool, id, attribute_id, value) {
            return Ok(RenderUpdate::Unchanged);
        }
        // Input String enabled (AID 9) has no body field — it is the runtime
        // enabled override, the same state Enable/Disable Object controls.
        if object_type == Some(ObjectType::InputString) && attribute_id == 9 {
            return self.set_enabled(id, value != 0);
        }
        let changed = apply_generic_attribute_to_pool(&mut self.pool, id, attribute_id, value)?;
        if changed && object_type == Some(ObjectType::WorkingSetSpecialControls) {
            match attribute_id {
                2 => self.selected_colour_map = ObjectID(low_u16(value)),
                3 => self.selected_colour_palette = Some(ObjectID(low_u16(value))),
                _ => {}
            }
        }
        self.rebuild_if_object_changed(changed, Some(id))
    }

    fn update_working_set_special_controls_colour_selection(
        &mut self,
        selection: Option<(ObjectType, ObjectID)>,
    ) -> Result<()> {
        let Some(special_controls_id) = self
            .pool
            .objects()
            .iter()
            .find(|obj| obj.r#type == ObjectType::WorkingSetSpecialControls)
            .map(|obj| obj.id)
        else {
            return Ok(());
        };
        let Some(obj) = self.pool.find_mut(special_controls_id) else {
            return Ok(());
        };
        let mut body = obj.get_working_set_special_controls_body()?;
        match selection {
            Some((ObjectType::ColourMap, id)) => body.colour_map = id,
            Some((ObjectType::ColourPalette, id)) => body.colour_palette = id,
            None => {
                body.colour_map = ObjectID::NULL;
                body.colour_palette = ObjectID::NULL;
            }
            Some(_) => unreachable!("validated colour-selection object type"),
        }
        obj.body = body.encode()?;
        Ok(())
    }

    fn execute_macro(&mut self, id: ObjectID) -> Result<RenderUpdate> {
        self.execute_macro_inner(id, &mut Vec::new())
    }

    fn execute_macro_inner(
        &mut self,
        id: ObjectID,
        executing: &mut Vec<ObjectID>,
    ) -> Result<RenderUpdate> {
        if executing.contains(&id) {
            return Ok(RenderUpdate::NotRenderAffecting {
                reason: "recursive macro execution was ignored",
            });
        }
        let Some(obj) = self.pool.find(id) else {
            return Ok(RenderUpdate::Unchanged);
        };
        if obj.r#type != ObjectType::Macro {
            return Ok(RenderUpdate::Unchanged);
        }

        let body = obj.get_macro_body()?;
        let effects = decode_macro_effects(&body);
        executing.push(id);
        let mut changed = false;
        let mut metadata_changed = false;
        let mut command_stream_changed = false;
        let mut mask_lock_rebuilt = false;
        let mut active_mask = self.active_mask;

        for effect in effects {
            match effect {
                MacroEffect::HideShow { object, show } => {
                    changed |= apply_hide_show_to_pool(&mut self.pool, object, show)?;
                }
                MacroEffect::EnableDisable { object, enable } => {
                    if self
                        .pool
                        .find(object)
                        .is_some_and(|object| is_enable_disable_object_type(object.r#type))
                        && self.current_enabled_state(object) != Some(enable)
                    {
                        changed |= self.overrides.set_enabled_mut(object, enable);
                    }
                }
                MacroEffect::SelectInputObject { object, option } => {
                    if option == 0xFF
                        || (option == 0x00
                            && self.pool.find(object).is_some_and(|object| {
                                is_select_input_open_target_type(object.r#type)
                            }))
                    {
                        metadata_changed |=
                            self.input
                                .select_input_object(&self.scene, object, option == 0x00);
                    }
                }
                MacroEffect::ControlAudioSignal { .. } | MacroEffect::SetAudioVolume { .. } => {
                    metadata_changed = true;
                }
                MacroEffect::ChangeNumericValue { object, value } => {
                    changed |= self.apply_numeric_value_state(object, value)?;
                }
                MacroEffect::ChangeStringValue { object, value } => {
                    if let Ok(text) = core::str::from_utf8(&value) {
                        changed |= apply_string_value_to_pool(&mut self.pool, object, text)?;
                    }
                }
                MacroEffect::ChangeChildLocation {
                    parent,
                    child,
                    x,
                    y,
                } => {
                    changed |= apply_child_position_to_pool(
                        &mut self.pool,
                        parent,
                        child,
                        i16::from(x),
                        i16::from(y),
                    )?;
                }
                MacroEffect::ChangeChildPosition {
                    parent,
                    child,
                    x,
                    y,
                } => {
                    changed |= apply_child_position_to_pool(&mut self.pool, parent, child, x, y)?;
                }
                MacroEffect::ChangeSize {
                    object,
                    width,
                    height,
                } => {
                    changed |= apply_size_to_pool(&mut self.pool, object, width, height)?;
                }
                MacroEffect::ChangeBackgroundColour { object, colour } => {
                    if self
                        .pool
                        .find(object)
                        .is_some_and(|object| object.r#type == ObjectType::GraphicContext)
                    {
                        if self.overrides.background(object) != Some(colour) {
                            self.overrides.set_background_mut(object, colour);
                            changed = true;
                        }
                    } else {
                        changed |= apply_background_to_pool(&mut self.pool, object, colour)?;
                    }
                }
                MacroEffect::ChangeFontAttributes {
                    object,
                    colour,
                    size,
                    font_type,
                    style,
                } => {
                    changed |= apply_font_attribute_values_to_pool(
                        &mut self.pool,
                        object,
                        colour,
                        size,
                        font_type,
                        style,
                    )?;
                }
                MacroEffect::ChangeLineAttributes {
                    object,
                    colour,
                    width,
                    line_art,
                } => {
                    changed |= apply_line_attribute_values_to_pool(
                        &mut self.pool,
                        object,
                        colour,
                        width,
                        line_art,
                    )?;
                }
                MacroEffect::ChangeFillAttributes {
                    object,
                    fill_type,
                    colour,
                    pattern,
                } => {
                    changed |= apply_fill_attribute_values_to_pool(
                        &mut self.pool,
                        object,
                        fill_type,
                        colour,
                        pattern,
                    )?;
                }
                MacroEffect::ChangeEndPoint {
                    object,
                    width,
                    height,
                    line_direction,
                } => {
                    changed |= apply_end_point_to_pool(
                        &mut self.pool,
                        object,
                        width,
                        height,
                        line_direction,
                    )?;
                }
                MacroEffect::ChangeSoftKeyMask {
                    mask_type,
                    data_mask,
                    soft_key_mask,
                } => {
                    if self.pool.find(data_mask).is_some_and(|object| {
                        change_soft_key_mask_type_matches(mask_type, object.r#type)
                    }) {
                        changed |=
                            apply_soft_key_mask_to_pool(&mut self.pool, data_mask, soft_key_mask)?;
                    }
                }
                MacroEffect::ChangeListItem { list, index, item } => {
                    changed |=
                        apply_list_item_to_pool(&mut self.pool, list, usize::from(index), item)?;
                }
                MacroEffect::DeleteObjectPool => {
                    self.clear_object_pool_state();
                    active_mask = ObjectID::NULL;
                    changed = true;
                }
                MacroEffect::ChangePriority { object, priority } => {
                    metadata_changed |= apply_priority_to_pool(&mut self.pool, object, priority)?;
                }
                MacroEffect::ChangeObjectLabel { object, label } => {
                    match self.apply_object_label(object, label) {
                        RenderUpdate::NotRenderAffecting { .. } => metadata_changed = true,
                        RenderUpdate::Unchanged
                        | RenderUpdate::SceneRebuilt { .. }
                        | RenderUpdate::CommandStreamChanged { .. } => {}
                    }
                }
                MacroEffect::LockUnlockMask {
                    object,
                    locked,
                    timeout_ms,
                } => match self.apply_mask_lock(object, locked, timeout_ms) {
                    RenderUpdate::SceneRebuilt { .. } => mask_lock_rebuilt = true,
                    RenderUpdate::NotRenderAffecting { .. } => metadata_changed = true,
                    RenderUpdate::Unchanged | RenderUpdate::CommandStreamChanged { .. } => {}
                },
                MacroEffect::ExecuteMacro { object } => {
                    match self.execute_macro_inner(object, executing)? {
                        RenderUpdate::SceneRebuilt { .. } => changed = true,
                        RenderUpdate::CommandStreamChanged { .. } => command_stream_changed = true,
                        RenderUpdate::NotRenderAffecting { .. } => metadata_changed = true,
                        RenderUpdate::Unchanged => {}
                    }
                }
                MacroEffect::ChangePolygonPoint {
                    object,
                    index,
                    x,
                    y,
                } => {
                    changed |= apply_polygon_point_to_pool(
                        &mut self.pool,
                        object,
                        usize::from(index),
                        x,
                        y,
                    )?;
                }
                MacroEffect::ChangePolygonScale {
                    object,
                    width,
                    height,
                } => {
                    changed |= apply_polygon_scale_to_pool(&mut self.pool, object, width, height)?;
                }
                MacroEffect::ChangeGenericAttribute {
                    object,
                    attribute_id,
                    value,
                } => {
                    if generic_attribute_update_is_valid(&self.pool, object, attribute_id, value) {
                        changed |= apply_generic_attribute_to_pool(
                            &mut self.pool,
                            object,
                            attribute_id,
                            value,
                        )?;
                    }
                }
                MacroEffect::SelectColourMap { object } => {
                    let valid = object == ObjectID::NULL
                        || self.pool.find(object).is_some_and(|obj| {
                            matches!(
                                obj.r#type,
                                ObjectType::ColourMap | ObjectType::ColourPalette
                            )
                        });
                    if valid {
                        changed |= self.select_colour_map_state(object)?;
                    }
                }
                MacroEffect::ChangeActiveMask { working_set, mask } => {
                    if self.macro_working_set_matches(working_set)
                        && self.ensure_renderable_mask(mask).is_ok()
                        && mask != active_mask
                    {
                        active_mask = mask;
                        changed = true;
                    }
                }
                MacroEffect::Unsupported { .. } => {}
            }
        }
        executing.pop();

        self.active_mask = active_mask;
        if changed {
            self.rebuild_if(true)
        } else if command_stream_changed {
            Ok(RenderUpdate::CommandStreamChanged {
                reason: "macro changed backend command stream without rebuilding the scene",
            })
        } else if mask_lock_rebuilt {
            Ok(RenderUpdate::SceneRebuilt {
                active_mask: self.active_mask,
            })
        } else if metadata_changed {
            Ok(RenderUpdate::NotRenderAffecting {
                reason: "macro changed VT runtime metadata without changing the scene",
            })
        } else {
            Ok(RenderUpdate::Unchanged)
        }
    }

    fn clear_object_pool_state(&mut self) {
        self.pool.clear();
        self.active_mask = ObjectID::NULL;
        self.selected_colour_map = ObjectID::NULL;
        self.selected_colour_palette = None;
        self.object_labels.clear();
        self.graphics_contexts.clear();
        self.mask_locks.clear();
        self.locked_scene_dirty = false;
        self.locked_scene_changed_objects.clear();
        self.soft_key_page = 0;
        self.soft_key_pointer_down = None;
        self.pointing_parent = None;
        self.activation_hold = None;
        self.pending_activation_events.clear();
        self.animation_elapsed_ms = 0;
        self.animation_elapsed_by_object.clear();
        self.overrides = RuntimeOverrides::default();
        self.user_layout_placements = PlacementMap::new();
        self.user_layout_selection.clear();
        self.input = InputRuntime::new();
    }

    /// Update one runtime visibility override and rebuild if it changed.
    pub fn set_visible(&mut self, id: ObjectID, visible: bool) -> Result<RenderUpdate> {
        self.ensure_known_object(id)?;
        if self.current_visible_state(id) == Some(visible) {
            return Ok(RenderUpdate::Unchanged);
        }
        let changed = self.overrides.set_visible_mut(id, visible);
        self.rebuild_if(changed)
    }

    /// Update one runtime enabled/disabled override and rebuild if it changed.
    pub fn set_enabled(&mut self, id: ObjectID, enabled: bool) -> Result<RenderUpdate> {
        let Some(object_type) = self.pool.find(id).map(|object| object.r#type) else {
            return Err(Error::invalid_state(
                "VT render runtime object id is not in the pool",
            ));
        };
        if !is_enable_disable_object_type(object_type) {
            return Ok(RenderUpdate::Unchanged);
        }
        if self.current_enabled_state(id) == Some(enabled) {
            return Ok(RenderUpdate::Unchanged);
        }
        let changed = self.overrides.set_enabled_mut(id, enabled);
        self.rebuild_if(changed)
    }

    fn current_visible_state(&self, id: ObjectID) -> Option<bool> {
        self.overrides
            .visible(id)
            .or_else(|| self.scene.find(id).map(|node| node.visible))
            .or_else(|| self.pool.find(id).and_then(object_base_visible_state))
    }

    fn current_enabled_state(&self, id: ObjectID) -> Option<bool> {
        self.overrides
            .enabled(id)
            .or_else(|| self.scene.find(id).map(|node| node.enabled))
            .or_else(|| self.pool.find(id).and_then(object_base_enabled_state))
    }

    fn ensure_known_object(&self, id: ObjectID) -> Result<()> {
        if self.pool.find(id).is_some() {
            Ok(())
        } else {
            Err(Error::invalid_state(
                "VT render runtime object id is not in the pool",
            ))
        }
    }

    fn ensure_renderable_mask(&self, id: ObjectID) -> Result<()> {
        match self.pool.find(id).map(|obj| obj.r#type) {
            Some(ObjectType::DataMask | ObjectType::AlarmMask | ObjectType::WindowMask) => Ok(()),
            Some(_) => Err(Error::invalid_state(
                "VT render runtime active mask must be a data, alarm, or window mask",
            )),
            None => Err(Error::invalid_state(
                "VT render runtime active mask is not in the pool",
            )),
        }
    }

    fn macro_working_set_matches(&self, working_set: ObjectID) -> bool {
        working_set == ObjectID::NULL
            || self
                .pool
                .objects()
                .iter()
                .find(|obj| obj.r#type == ObjectType::WorkingSet)
                .is_some_and(|obj| obj.id == working_set)
    }
}
