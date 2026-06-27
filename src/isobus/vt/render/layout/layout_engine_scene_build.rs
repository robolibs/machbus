impl LayoutEngine {
    #[must_use]
    pub fn new(config: LayoutConfig) -> Self {
        Self {
            config,
            placements: PlacementMap::new(),
            palette: Palette::default_isobus(),
            colour_map: None,
            colour_palette: None,
            working_set_name: None,
            external_pools: Vec::new(),
            overrides: RuntimeOverrides::new(),
            animation_elapsed_ms: 0,
            animation_elapsed_by_object: HashMap::new(),
        }
    }

    #[must_use]
    pub fn with_placements(mut self, placements: PlacementMap) -> Self {
        self.placements = placements;
        self
    }

    /// Layer runtime visibility/enabled overrides (e.g. from macro
    /// Hide/Show / Enable/Disable effects) onto built scenes.
    #[must_use]
    pub fn with_overrides(mut self, overrides: RuntimeOverrides) -> Self {
        self.overrides = overrides;
        self
    }

    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Select a Colour Map object for this build. `NULL` restores the
    /// default one-to-one colour index mapping. If no explicit selection is
    /// configured, an initial Working Set Special Controls colour map is used
    /// when present.
    #[must_use]
    pub fn with_colour_map(mut self, colour_map: ObjectID) -> Self {
        self.colour_map = Some(colour_map);
        self
    }

    /// Select a Colour Palette object for this build.
    ///
    /// `None` preserves the legacy/default behaviour of using the first
    /// Colour Palette object in the pool. `Some(NULL)` forces the terminal
    /// default palette. `Some(id)` uses that specific Colour Palette object.
    #[must_use]
    pub fn with_colour_palette(mut self, colour_palette: Option<ObjectID>) -> Self {
        self.colour_palette = colour_palette;
        self
    }

    /// Provide the NAME of the local Working Set master.
    ///
    /// External Object Pointer resolution is standard-valid only when the
    /// referenced pool contains an enabled External Object Definition whose
    /// NAME matches this local Working Set. Without a local NAME, the layout
    /// engine deliberately falls back to the pointer's default object.
    #[must_use]
    pub fn with_working_set_name(mut self, name0: u32, name1: u32) -> Self {
        self.working_set_name = Some((name0, name1));
        self
    }

    /// Current local Working Set NAME used for External Object Pointer
    /// reference validation.
    #[must_use]
    pub const fn working_set_name(&self) -> Option<(u32, u32)> {
        self.working_set_name
    }

    /// Register another Working Set's object pool for External Object Pointer
    /// resolution. The `(name0, name1)` pair is the referenced Working Set's
    /// NAME split into the two standard four-byte attributes used by External
    /// Reference NAME objects.
    #[must_use]
    pub fn with_external_object_pool(mut self, name0: u32, name1: u32, pool: ObjectPool) -> Self {
        if let Some(existing) = self
            .external_pools
            .iter_mut()
            .find(|external| external.name0 == name0 && external.name1 == name1)
        {
            existing.pool = pool;
        } else {
            self.external_pools
                .push(ExternalObjectPool { name0, name1, pool });
        }
        self
    }

    /// `true` when a referenced Working Set pool with this NAME is already
    /// registered.
    #[must_use]
    pub fn has_external_object_pool(&self, name0: u32, name1: u32) -> bool {
        self.external_pools
            .iter()
            .any(|external| external.name0 == name0 && external.name1 == name1)
    }

    /// `true` when the currently registered referenced Working Set pool for
    /// this NAME is byte/model-identical to `pool`.
    #[must_use]
    pub fn external_object_pool_matches(&self, name0: u32, name1: u32, pool: &ObjectPool) -> bool {
        self.external_pools.iter().any(|external| {
            external.name0 == name0 && external.name1 == name1 && external.pool == *pool
        })
    }

    /// Remove a registered external Working Set pool by NAME.
    ///
    /// Hosts should call this when a referenced Working Set is unloaded or
    /// disconnects, so External Object Pointers deterministically fall back to
    /// their local default objects instead of retaining stale referenced UI.
    #[must_use]
    pub fn without_external_object_pool(mut self, name0: u32, name1: u32) -> Self {
        self.external_pools
            .retain(|external| external.name0 != name0 || external.name1 != name1);
        self
    }

    /// Select the animation clock used when materialising Animation
    /// objects. The default is zero, so static renders show the first
    /// frame deterministically.
    #[must_use]
    pub fn with_animation_elapsed_ms(mut self, elapsed_ms: u32) -> Self {
        self.animation_elapsed_ms = elapsed_ms;
        self
    }

    /// Select per-Animation clocks used when materialising Animation objects.
    ///
    /// Runtime-driven renders use this to model the standard rule that an
    /// Animation timer is per object and is suspended while the object is not
    /// visible. Objects absent from the map fall back to the layout-wide
    /// `animation_elapsed_ms`, preserving deterministic static rendering for
    /// direct `LayoutEngine` callers.
    #[must_use]
    pub fn with_animation_elapsed_by_object(
        mut self,
        clocks: impl IntoIterator<Item = (ObjectID, u32)>,
    ) -> Self {
        self.animation_elapsed_by_object = clocks
            .into_iter()
            .map(|(id, elapsed)| (id.0, elapsed))
            .collect();
        self
    }

    #[inline]
    #[must_use]
    fn animation_elapsed_for(&self, id: ObjectID) -> u32 {
        self.animation_elapsed_by_object
            .get(&id.0)
            .copied()
            .unwrap_or(self.animation_elapsed_ms)
    }

    /// Configure physical soft-key pagination. `physical_count == 0`
    /// disables paging and renders every key in the mask.
    #[must_use]
    pub fn with_soft_key_counts(mut self, physical_count: u8, navigation_count: u8) -> Self {
        self.config.physical_soft_key_count = physical_count;
        self.config.navigation_soft_key_count =
            navigation_count.min(physical_count.saturating_sub(1));
        self
    }

    /// Select the zero-based soft-key page for this scene build.
    #[must_use]
    pub fn with_soft_key_page(mut self, page: u16) -> Self {
        self.config.soft_key_page = page;
        self
    }

    fn soft_key_cell_height(&self) -> u16 {
        soft_key_cell_span_for_config(self.config)
    }

    fn window_mask_rect(&self, x: i32, y: i32, body: &WindowMaskBody) -> Rect {
        let (cols, rows) = window_mask_cell_span(body);
        let cell_w = (self.config.canvas.0 / 2).max(1);
        let cell_h = (self.config.canvas.1 / 6).max(1);
        Rect::new(
            x,
            y,
            cell_w.saturating_mul(u16::from(cols)),
            cell_h.saturating_mul(u16::from(rows)),
        )
    }

    #[inline]
    #[must_use]
    pub fn config(&self) -> LayoutConfig {
        self.config
    }

    /// Build a scene for the given active mask. The mask must already
    /// exist in the pool; passing [`ObjectID::NULL`] selects the first
    /// child of the Working Set (the standard "initial mask").
    #[must_use]
    pub fn build(&self, pool: &ObjectPool, active_mask: ObjectID) -> Scene {
        let effective_palette = self.effective_palette(pool);
        let resolver = StyleResolver::new(pool, effective_palette.clone());
        let mask_id = self.resolve_initial_mask(pool, active_mask);
        let (canvas_w, canvas_h) = self.config.canvas;
        let mut scene = Scene::new(mask_id, (canvas_w, canvas_h));
        scene.effective_palette = Some(effective_palette);
        scene.mask_rect = Rect::new(0, 0, canvas_w, canvas_h);
        scene.supported_languages = working_set_languages(pool);

        let Some(mask) = pool.find(mask_id) else {
            scene.unsupported.push(UnsupportedRecord {
                id: mask_id,
                object_type: ObjectType::WorkingSet,
                reason: "active mask not present in object pool",
            });
            return scene;
        };

        let (background, soft_key_mask, children) = match self.decode_mask(mask) {
            Some(m) => m,
            None => {
                scene.unsupported.push(UnsupportedRecord {
                    id: mask_id,
                    object_type: mask.r#type,
                    reason: "mask object has an undecodable body",
                });
                return scene;
            }
        };
        scene.background = background;
        let child_placements = self.mask_child_placements(mask, &children, &mut scene);

        let mut path = Vec::new();
        let initial_clip = if mask.r#type == ObjectType::WindowMask {
            Some(scene.mask_rect)
        } else {
            None
        };
        let mut state = BuildState {
            scene: &mut scene,
            path: &mut path,
            clip: initial_clip,
            visible: true,
        };
        for placement in child_placements {
            self.build_node(pool, &resolver, mask_id, &placement, &mut state);
        }

        if soft_key_mask != ObjectID::NULL {
            self.build_soft_keys(pool, soft_key_mask, &mut scene);
        }
        scene
    }

    fn mask_child_placements(
        &self,
        mask: &VTObject,
        children: &[crate::isobus::vt::ChildRef],
        scene: &mut Scene,
    ) -> Vec<ChildPlacement> {
        if mask.r#type != ObjectType::WindowMask {
            return self.layout_children(children);
        }
        let Ok(body) = mask.get_window_mask_body() else {
            scene.unsupported.push(UnsupportedRecord {
                id: mask.id,
                object_type: ObjectType::WindowMask,
                reason: "active Window Mask has an undecodable body",
            });
            return Vec::new();
        };
        scene.mask_rect = self.window_mask_rect(0, 0, &body);
        if body.options & 0x01 == 0 {
            return Vec::new();
        }
        match body.window_type {
            0 => self.layout_children(children),
            1..=18 => window_required_object_placements(&body, scene.mask_rect),
            _ => {
                scene.unsupported.push(UnsupportedRecord {
                    id: mask.id,
                    object_type: ObjectType::WindowMask,
                    reason: "active Window Mask has an unsupported window type",
                });
                Vec::new()
            }
        }
    }

    /// Build a detached scene rooted at one drawable VT object placed at an
    /// explicit top-left coordinate.
    ///
    /// Graphics Context Draw VT Object uses this path to draw an ordinary VT
    /// object into a canvas instead of through the active mask's child list.
    /// The returned scene intentionally has no soft keys.
    #[must_use]
    pub fn build_object_at(&self, pool: &ObjectPool, id: ObjectID, x: i32, y: i32) -> Scene {
        let effective_palette = self.effective_palette(pool);
        let resolver = StyleResolver::new(pool, effective_palette.clone());
        let (canvas_w, canvas_h) = self.config.canvas;
        let mut scene = Scene::new(id, (canvas_w, canvas_h));
        scene.effective_palette = Some(effective_palette);
        scene.mask_rect = Rect::new(x, y, canvas_w, canvas_h);

        let placement = ChildPlacement::new(id, i32_to_i16_saturating(x), i32_to_i16_saturating(y));
        let mut path = Vec::new();
        let mut state = BuildState {
            scene: &mut scene,
            path: &mut path,
            clip: None,
            visible: true,
        };
        self.build_node(pool, &resolver, ObjectID::NULL, &placement, &mut state);
        scene
    }

    /// The palette used for this build: the engine's configured palette
    /// with any VT6 Colour Palette object in the pool overlaid on top,
    /// then the selected Colour Map applied as an index indirection.
    fn effective_palette(&self, pool: &ObjectPool) -> Palette {
        let mut palette = self.palette.clone();
        let special_controls = working_set_special_controls(pool);
        let initial_colour_palette = special_controls.as_ref().map(|body| body.colour_palette);
        let selected_palette = self.colour_palette.or(initial_colour_palette);
        let palette_object = match selected_palette {
            Some(ObjectID::NULL) => None,
            Some(id) => pool
                .find(id)
                .filter(|obj| obj.r#type == ObjectType::ColourPalette),
            None => pool
                .objects()
                .iter()
                .find(|o| o.r#type == ObjectType::ColourPalette),
        };
        if let Some(obj) = palette_object
            && let Ok(body) = obj.get_colour_palette_body()
        {
            palette.apply_colour_palette(&body.entries_argb);
        }
        let colour_map = self.colour_map.unwrap_or_else(|| {
            special_controls
                .as_ref()
                .map(|body| body.colour_map)
                .unwrap_or(ObjectID::NULL)
        });
        if colour_map != ObjectID::NULL
            && let Some(obj) = pool.find(colour_map)
            && obj.r#type == ObjectType::ColourMap
            && let Ok(body) = obj.get_colour_map_body()
        {
            let base = palette.clone();
            for (index, mapped_index) in body.entries.iter().take(256).enumerate() {
                palette.set_entry(index as u8, base.resolve(*mapped_index));
            }
        }
        palette
    }

    /// Resolve which mask to render. Prefer the requested id (if it is a mask),
    /// then the Working Set's `active_mask` field (the standard initial mask),
    /// then the first Working Set child that is itself a mask, then any mask in
    /// the pool. A Working Set's first child is its *designator*, not a mask, so
    /// it is not used as a fall-back.
    fn resolve_initial_mask(&self, pool: &ObjectPool, requested: ObjectID) -> ObjectID {
        let is_mask = |id: ObjectID| {
            pool.find(id).is_some_and(|o| {
                matches!(
                    o.r#type,
                    ObjectType::DataMask | ObjectType::AlarmMask | ObjectType::WindowMask
                )
            })
        };
        if requested != ObjectID::NULL && is_mask(requested) {
            return requested;
        }
        if let Some(ws) = pool
            .objects()
            .iter()
            .find(|o| o.r#type == ObjectType::WorkingSet)
        {
            if let Ok(body) = ws.get_working_set_body()
                && is_mask(body.active_mask)
            {
                return body.active_mask;
            }
            if let Some(child) = ws.children.iter().copied().find(|&c| is_mask(c)) {
                return child;
            }
        }
        pool.objects()
            .iter()
            .find(|o| {
                matches!(
                    o.r#type,
                    ObjectType::DataMask | ObjectType::AlarmMask | ObjectType::WindowMask
                )
            })
            .map(|o| o.id)
            .unwrap_or(requested)
    }
    fn decode_mask(
        &self,
        mask: &VTObject,
    ) -> Option<(u8, ObjectID, Vec<crate::isobus::vt::ChildRef>)> {
        match mask.r#type {
            ObjectType::DataMask => {
                let b = mask.get_data_mask_body().ok()?;
                Some((
                    b.background_color,
                    b.soft_key_mask,
                    positional_children(mask),
                ))
            }
            ObjectType::AlarmMask => {
                let b = mask.get_alarm_mask_body().ok()?;
                Some((
                    b.background_color,
                    b.soft_key_mask,
                    positional_children(mask),
                ))
            }
            ObjectType::WindowMask => {
                let b = mask.get_window_mask_body().ok()?;
                Some((
                    b.background_color,
                    ObjectID::NULL,
                    positional_children(mask),
                ))
            }
            _ => None,
        }
    }

    /// Assign `(x, y)` to each child. Priority: caller-supplied
    /// [`PlacementMap`] wins; else the real ISO 11783-6 position carried
    /// on the child ref; else a deterministic auto-stack along the left
    /// edge with the configured gap.
    fn layout_children(&self, children: &[crate::isobus::vt::ChildRef]) -> Vec<ChildPlacement> {
        let mut out = Vec::with_capacity(children.len());
        let mut auto_y: i16 = 0;
        let gap = self.config.auto_layout_gap;
        for cref in children {
            // Explicit placement override wins.
            let (x, y) = match self.placements.get(cref.id) {
                Some(p) => p,
                // Use the real position if it's non-origin (i.e. the codec
                // actually parsed a location). A zero position is treated
                // as "unset" so the auto-stack still applies to legacy
                // pools that carry no positional data.
                None if cref.x != 0 || cref.y != 0 => (cref.x, cref.y),
                None => (0, auto_y),
            };
            out.push(ChildPlacement::new(cref.id, x, y));
            // Advance the cursor past this row.
            auto_y = y.saturating_add(gap as i16);
        }
        out
    }

    /// Build one node (recursing into containers). Records the node
    /// into `scene.nodes` / `scene.unsupported` as appropriate.
    fn build_node(
        &self,
        pool: &ObjectPool,
        resolver: &StyleResolver<'_>,
        parent: ObjectID,
        placement: &ChildPlacement,
        state: &mut BuildState<'_>,
    ) {
        let id = placement.id;

        // Cycle guard: stop if this id is already on the active path.
        if state.path.contains(&id) {
            state.scene.unsupported.push(UnsupportedRecord {
                id,
                object_type: ObjectType::Container,
                reason: "reference cycle detected while laying out children",
            });
            return;
        }

        let Some(obj) = pool.find(id) else {
            state.scene.unsupported.push(UnsupportedRecord {
                id,
                object_type: ObjectType::ObjectPointer,
                reason: "child id not present in object pool",
            });
            return;
        };

        if obj.r#type == ObjectType::ObjectPointer {
            let Ok(body) = obj.get_object_pointer_body() else {
                state.scene.unsupported.push(UnsupportedRecord {
                    id,
                    object_type: ObjectType::ObjectPointer,
                    reason: "object pointer has an undecodable body",
                });
                return;
            };
            if body.value == ObjectID::NULL {
                return;
            }
            if pool.find(body.value).is_none() {
                state.scene.unsupported.push(UnsupportedRecord {
                    id,
                    object_type: ObjectType::ObjectPointer,
                    reason: "object pointer target not present in object pool",
                });
                return;
            }
            state.path.push(id);
            self.build_node(
                pool,
                resolver,
                parent,
                &ChildPlacement::new(body.value, placement.x, placement.y),
                state,
            );
            state.path.pop();
            return;
        }

        if obj.r#type == ObjectType::ExternalObjectPointer {
            let Ok(body) = obj.get_external_object_pointer_body() else {
                state.scene.unsupported.push(UnsupportedRecord {
                    id,
                    object_type: ObjectType::ExternalObjectPointer,
                    reason: "external object pointer has an undecodable body",
                });
                return;
            };
            if let Some((external_pool, target)) = self.resolve_external_object(pool, &body)
                && !Self::external_target_pointer_chain_reaches_null(external_pool, target)
            {
                let external_palette = self.effective_palette(external_pool);
                let external_resolver = StyleResolver::new(external_pool, external_palette);
                let mut external_path = Vec::new();
                let mut external_state = BuildState {
                    scene: state.scene,
                    path: &mut external_path,
                    clip: state.clip,
                    visible: state.visible,
                };
                self.build_node(
                    external_pool,
                    &external_resolver,
                    id,
                    &ChildPlacement::new(target, placement.x, placement.y),
                    &mut external_state,
                );
                state.clip = external_state.clip;
                state.visible = external_state.visible;
                return;
            }
            if body.default_object_id == ObjectID::NULL {
                return;
            }
            if pool.find(body.default_object_id).is_none() {
                state.scene.unsupported.push(UnsupportedRecord {
                    id,
                    object_type: ObjectType::ExternalObjectPointer,
                    reason: "external object pointer default object not present in object pool",
                });
                return;
            }
            state.path.push(id);
            self.build_node(
                pool,
                resolver,
                parent,
                &ChildPlacement::new(body.default_object_id, placement.x, placement.y),
                state,
            );
            state.path.pop();
            return;
        }

        let base = BaseNode {
            id,
            object_type: obj.r#type,
            parent,
            x: i32::from(placement.x),
            y: i32::from(placement.y),
        };

        state.path.push(id);
        let built = self.materialise(pool, resolver, obj, base, state);
        state.path.pop();

        if let Some(mut node) = built {
            // Layer runtime overrides (macro Hide/Show, Enable/Disable) on
            // top of the object's own option-bit state.
            let local_visible = self.overrides.visible(id).unwrap_or(node.visible);
            node.visible = state.visible && local_visible;
            if let Some(enabled) = self.overrides.enabled(id) {
                node.enabled = enabled;
            }
            node.clip = state.clip;
            state.scene.nodes.push(node);
        }
    }

    fn resolve_external_object<'a>(
        &'a self,
        local_pool: &ObjectPool,
        body: &crate::isobus::vt::ExternalObjectPointerBody,
    ) -> Option<(&'a ObjectPool, ObjectID)> {
        let target = body.external_object_id;
        if target == ObjectID::NULL {
            return None;
        }
        let local_name = self.working_set_name?;
        let reference = local_pool
            .find(body.external_reference_name)?
            .get_external_reference_name_body()
            .ok()?;
        if reference.options & 0x01 == 0 {
            return None;
        }
        let external = self
            .external_pools
            .iter()
            .find(|pool| pool.name0 == reference.name0 && pool.name1 == reference.name1)?;
        if !external_pool_allows_reference(&external.pool, local_name, target) {
            return None;
        }
        external.pool.find(target)?;
        Some((&external.pool, target))
    }

    /// ISO 11783-6 B.24 draws the local default object when an
    /// ExternalObjectPointer targets an ObjectPointer chain whose current value
    /// is NULL. Ordinary ObjectPointer NULLs still disappear in their own local
    /// pool context; this helper is only used by the external-pointer fallback
    /// rule before we enter the referenced pool's normal layout path.
    fn external_target_pointer_chain_reaches_null(pool: &ObjectPool, target: ObjectID) -> bool {
        let mut current = target;
        let mut seen = Vec::new();
        loop {
            if current == ObjectID::NULL {
                return true;
            }
            if seen.contains(&current) {
                return false;
            }
            seen.push(current);
            let Some(object) = pool.find(current) else {
                return false;
            };
            if object.r#type != ObjectType::ObjectPointer {
                return false;
            }
            let Ok(body) = object.get_object_pointer_body() else {
                return false;
            };
            current = body.value;
        }
    }

    /// Turn a `VTObject` into a concrete [`SceneNode`]. Objects that are
    /// not directly drawable are recorded as unsupported.
    fn materialise(
        &self,
        pool: &ObjectPool,
        resolver: &StyleResolver<'_>,
        obj: &VTObject,
        base: BaseNode,
        state: &mut BuildState<'_>,
    ) -> Option<SceneNode> {
        let mk =
            |rect: Rect, style: ResolvedStyle, visible: bool, enabled: bool, kind: NodeKind| {
                SceneNode {
                    id: base.id,
                    object_type: base.object_type,
                    parent: base.parent,
                    rect,
                    clip: None,
                    style,
                    visible,
                    enabled,
                    kind,
                }
            };

        match obj.r#type {
            ObjectType::Container => {
                let body = obj.get_container_body().ok()?;
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let children = self.layout_children(&positional_children(obj));
                let node = mk(
                    rect,
                    ResolvedStyle::default(),
                    !body.hidden,
                    true,
                    NodeKind::Group {
                        background: 0,
                        transparent_bg: false,
                        children,
                    },
                );
                // Recurse into container children with translated origin.
                let inherited_visible = state.visible;
                let container_visible = self.overrides.visible(base.id).unwrap_or(!body.hidden);
                state.visible = inherited_visible && container_visible;
                for cp in self.layout_children(&positional_children(obj)) {
                    let translated = translated_child(base.x, base.y, &cp);
                    self.build_node(pool, resolver, base.id, &translated, state);
                }
                state.visible = inherited_visible;
                Some(node)
            }
            ObjectType::DataMask | ObjectType::AlarmMask | ObjectType::WindowMask => {
                // A mask appearing as a *child* (e.g. a Window Mask nested
                // inside a Data Mask) is rendered as a grouping node with
                // its own background, then its children are laid out
                // relative to it.
                let (bg, _, transparent_bg) = decode_mask_background(obj);
                let window_body = if obj.r#type == ObjectType::WindowMask {
                    obj.get_window_mask_body().ok()
                } else {
                    None
                };
                let rect = window_body.as_ref().map_or_else(
                    || Rect::new(base.x, base.y, self.config.canvas.0, self.config.canvas.1),
                    |body| self.window_mask_rect(base.x, base.y, body),
                );
                let (enabled, children) = match window_body.as_ref() {
                    None => (true, self.layout_children(&positional_children(obj))),
                    Some(body) if body.options & 0x01 == 0 => (false, Vec::new()),
                    Some(body) if body.window_type == 0 => {
                        (true, self.layout_children(&positional_children(obj)))
                    }
                    Some(body) if (1..=18).contains(&body.window_type) => {
                        (true, window_required_object_placements(body, rect))
                    }
                    Some(_) => (false, Vec::new()),
                };
                let node = mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    enabled,
                    NodeKind::Group {
                        background: bg,
                        transparent_bg: enabled && transparent_bg,
                        children: children.clone(),
                    },
                );
                if enabled {
                    let previous_clip = state.clip;
                    if obj.r#type == ObjectType::WindowMask {
                        state.clip = combine_clip(previous_clip, rect);
                    }
                    for cp in children {
                        let translated = translated_child(base.x, base.y, &cp);
                        self.build_node(pool, resolver, base.id, &translated, state);
                    }
                    state.clip = previous_clip;
                }
                Some(node)
            }
            ObjectType::OutputString => {
                let body = obj.get_output_string_body().ok()?;
                let mut style = resolver.resolve_font(body.font_attributes);
                style.background = resolver.colour(body.background_color);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                Some(mk(
                    rect,
                    style,
                    true,
                    true,
                    NodeKind::OutputString {
                        text: self.resolve_string_value(pool, &body),
                        transparent_bg: body.options & 0x01 != 0,
                        justification: body.justification,
                    },
                ))
            }
            ObjectType::OutputNumber => {
                let body = obj.get_output_number_body().ok()?;
                let mut style = resolver.resolve_font(body.font_attributes);
                style.background = resolver.colour(body.background_color);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let raw = self.resolve_number_value_or(pool, body.variable_reference, body.value);
                Some(mk(
                    rect,
                    style,
                    true,
                    true,
                    NodeKind::OutputNumber {
                        text: format_number(
                            raw,
                            body.offset,
                            body.scale,
                            body.number_of_decimals,
                            body.format,
                            body.options,
                            Some(usize::from(rect.w / style.font.cell_w.max(1))),
                        ),
                        transparent_bg: body.options & 0x01 != 0,
                        justification: body.justification,
                    },
                ))
            }
            ObjectType::OutputList => {
                let body = obj.get_output_list_body().ok()?;
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let selected = self.resolve_number_value_or(
                    pool,
                    body.variable_reference,
                    u32::from(body.value),
                ) as usize;
                let selected_text = body
                    .items
                    .get(selected)
                    .and_then(|item_id| self.resolve_output_list_item_text(pool, *item_id));
                let selected_item = self.selected_output_list_item(pool, &body, selected);
                let selected_item_materialized = selected_item
                    .filter(|item_id| !state.path.contains(item_id))
                    .is_some_and(|item_id| {
                        let previous_clip = state.clip;
                        state.clip = combine_clip(previous_clip, rect);
                        self.build_node(
                            pool,
                            resolver,
                            base.id,
                            &ChildPlacement::new(
                                item_id,
                                saturating_i32_to_i16(base.x),
                                saturating_i32_to_i16(base.y),
                            ),
                            state,
                        );
                        state.clip = previous_clip;
                        true
                    });
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::OutputList {
                        selected,
                        item_count: body.items.len(),
                        selected_text,
                        selected_item_materialized,
                    },
                ))
            }
            ObjectType::Line => {
                let body = obj.get_output_line_body().ok()?;
                let style = resolver.overlay_line(ResolvedStyle::default(), body.line_attributes);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                Some(mk(
                    rect,
                    style,
                    true,
                    true,
                    NodeKind::OutputLine {
                        direction: body.line_direction,
                    },
                ))
            }
            ObjectType::Rectangle => {
                let body = obj.get_output_rectangle_body().ok()?;
                let style = resolver.overlay_fill(
                    resolver.overlay_line(ResolvedStyle::default(), body.line_attributes),
                    body.fill_attributes,
                );
                let fill_pattern = self.fill_pattern(pool, body.fill_attributes);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                Some(mk(
                    rect,
                    style,
                    true,
                    true,
                    NodeKind::OutputRectangle {
                        line_suppression: body.line_suppression,
                        fill_pattern,
                    },
                ))
            }
            ObjectType::Ellipse => {
                let body = obj.get_output_ellipse_body().ok()?;
                let style = resolver.overlay_fill(
                    resolver.overlay_line(ResolvedStyle::default(), body.line_attributes),
                    body.fill_attributes,
                );
                let fill_pattern = self.fill_pattern(pool, body.fill_attributes);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                Some(mk(
                    rect,
                    style,
                    true,
                    true,
                    NodeKind::OutputEllipse {
                        filled: style.fill_type.is_solid(),
                        fill_pattern,
                        closed: body.ellipse_type == 0 || body.start_angle == body.end_angle,
                        ellipse_type: body.ellipse_type,
                        start_angle: body.start_angle,
                        end_angle: body.end_angle,
                    },
                ))
            }
            ObjectType::Polygon => {
                let body = obj.get_output_polygon_body().ok()?;
                let style = resolver.overlay_fill(
                    resolver.overlay_line(ResolvedStyle::default(), body.line_attributes),
                    body.fill_attributes,
                );
                let fill_pattern = self.fill_pattern(pool, body.fill_attributes);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                Some(mk(
                    rect,
                    style,
                    true,
                    true,
                    NodeKind::OutputPolygon {
                        points: body
                            .points
                            .iter()
                            .map(|p| (p.x as i16, p.y as i16))
                            .collect(),
                        fill_pattern,
                    },
                ))
            }
            ObjectType::Meter => {
                let body = obj.get_meter_body().ok()?;
                let rect = Rect::new(base.x, base.y, body.width, body.width);
                let value = self.resolve_number_value_or(pool, body.variable_reference, body.value);
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::Meter {
                        value,
                        min_value: body.min_value as i32,
                        max_value: body.max_value as i32,
                        needle_colour: body.needle_color,
                        border_colour: body.border_color,
                        arc_colour: body.arc_and_tick_color,
                        show_value: body.options & 0x01 != 0,
                        number_of_ticks: body.number_of_ticks,
                        start_angle: body.start_angle,
                        end_angle: body.end_angle,
                    },
                ))
            }
            ObjectType::LinearBarGraph => {
                let body = obj.get_linear_bar_graph_body().ok()?;
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let value = self.resolve_number_value_or(pool, body.variable_reference, body.value);
                let target_value = self.resolve_number_value_or(
                    pool,
                    body.target_value_variable_reference,
                    body.target_value,
                );
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::LinearBarGraph {
                        value,
                        target_value,
                        min_value: body.min_value as i32,
                        max_value: body.max_value as i32,
                        colour: body.color,
                        target_line_colour: body.target_line_color,
                        show_border: body.options & 0x01 != 0,
                        show_target_line: body.options & 0x02 != 0,
                        show_ticks: body.options & 0x04 != 0,
                        number_of_ticks: body.number_of_ticks,
                        line_only: body.options & 0x08 != 0,
                        horizontal: body.options & 0x10 != 0,
                        direction_positive: body.options & 0x20 != 0,
                    },
                ))
            }
            ObjectType::ArchedBarGraph => {
                let body = obj.get_arched_bar_graph_body().ok()?;
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let value = self.resolve_number_value_or(pool, body.variable_reference, body.value);
                let target_value = self.resolve_number_value_or(
                    pool,
                    body.target_value_variable_reference,
                    body.target_value,
                );
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::ArchedBarGraph {
                        value,
                        target_value,
                        min_value: body.min_value as i32,
                        max_value: body.max_value as i32,
                        colour: body.color,
                        target_line_colour: body.target_line_color,
                        show_border: body.options & 0x01 != 0,
                        show_target_line: body.options & 0x02 != 0,
                        line_only: body.options & 0x08 != 0,
                        clockwise: body.options & 0x10 != 0,
                        start_angle: body.start_angle,
                        end_angle: body.end_angle,
                        bar_width: body.bar_width,
                    },
                ))
            }
            ObjectType::PictureGraphic => {
                let body = obj.get_picture_graphic_body().ok()?;
                let display_width = if body.width == 0 {
                    body.actual_width
                } else {
                    body.width
                };
                let rect = Rect::new(base.x, base.y, display_width, body.actual_height);
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::PictureGraphic {
                        raw_width: body.actual_width,
                        raw_height: body.actual_height,
                        format: body.format,
                        options: body.options,
                        transparency: body.transparency,
                        data: body.data,
                    },
                ))
            }
            ObjectType::ScaledGraphic => {
                let body = obj.get_scaled_graphic_body().ok()?;
                let Some(source) = resolve_scaled_graphic_source(pool, body.value, &mut Vec::new())
                else {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::ScaledGraphic,
                        reason: "ScaledGraphic value reference is missing",
                    });
                    return None;
                };
                let source = match source {
                    Ok(Some(source)) => source,
                    Ok(None) => return None,
                    Err(reason) => {
                        state.scene.unsupported.push(UnsupportedRecord {
                            id: base.id,
                            object_type: ObjectType::ScaledGraphic,
                            reason,
                        });
                        return None;
                    }
                };
                let source_width = if source.width == 0 {
                    body.width
                } else {
                    source.width
                };
                let source_height = if source.height == 0 {
                    body.height
                } else {
                    source.height
                };
                let rect = scaled_graphic_destination(
                    base.x,
                    base.y,
                    body.width,
                    body.height,
                    source_width,
                    source_height,
                    body.scale_type,
                );
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::ScaledGraphic {
                        width: source_width,
                        height: source_height,
                        format: source.format,
                        options: source.options,
                        standard_png: source.standard_png,
                        transparent: source.transparent,
                        transparency: source.transparency,
                        data: source.data,
                    },
                ))
            }
            ObjectType::ScaledBitmap => {
                let body = obj.get_scaled_bitmap_body().ok()?;
                let Some(bitmap_data) = pool.find(body.bitmap_data) else {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::ScaledBitmap,
                        reason: "ScaledBitmap references missing GraphicData object",
                    });
                    return None;
                };
                if bitmap_data.r#type != ObjectType::GraphicData {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::ScaledBitmap,
                        reason: "ScaledBitmap bitmap_data reference is not GraphicData",
                    });
                    return None;
                }
                let Ok(graphic_data) = bitmap_data.get_graphic_data_body() else {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::ScaledBitmap,
                        reason: "ScaledBitmap references undecodable GraphicData body",
                    });
                    return None;
                };
                if body.width == 0 || body.height == 0 {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::ScaledBitmap,
                        reason: "ScaledBitmap source bitmap dimensions are zero",
                    });
                    return None;
                }
                // Display size = source bitmap dimensions scaled by the f32
                // scale factors (truncated, clamped to a non-zero u16); the
                // GTUI ScaledBitmap path expands the indexed payload into this
                // destination rect at the body offset.
                let scaled = |dim: u16, scale: f32| -> u16 {
                    if !scale.is_finite() || scale <= 0.0 {
                        return dim;
                    }
                    let value = f32::from(dim) * scale;
                    if value < 1.0 {
                        1
                    } else if value >= f32::from(u16::MAX) {
                        u16::MAX
                    } else {
                        value as u16
                    }
                };
                Some(mk(
                    Rect::new(
                        base.x + i32::from(body.offset_x),
                        base.y + i32::from(body.offset_y),
                        scaled(body.width, body.scale_x),
                        scaled(body.height, body.scale_y),
                    ),
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::ScaledBitmap {
                        width: body.width,
                        height: body.height,
                        format: body.format,
                        options: body.options,
                        data: graphic_data.data,
                    },
                ))
            }
            ObjectType::GraphicContext => {
                let body = obj.get_graphic_context_body().ok()?;
                let background = self
                    .overrides
                    .background(base.id)
                    .unwrap_or(body.background_colour);
                let width = if body.viewport_width == 0 {
                    body.canvas_width
                } else {
                    body.viewport_width
                };
                let height = if body.viewport_height == 0 {
                    body.canvas_height
                } else {
                    body.viewport_height
                };
                if width == 0 || height == 0 {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::GraphicContext,
                        reason: "GraphicContext viewport/canvas dimensions are zero",
                    });
                    return None;
                }
                Some(mk(
                    Rect::new(
                        base.x + i32::from(body.viewport_x),
                        base.y + i32::from(body.viewport_y),
                        width,
                        height,
                    ),
                    ResolvedStyle {
                        background: resolver.colour(background),
                        ..ResolvedStyle::default()
                    },
                    true,
                    true,
                    NodeKind::GraphicContext {
                        canvas_width: body.canvas_width,
                        canvas_height: body.canvas_height,
                        background,
                        transparency_colour: body.transparency_colour,
                        transparent: body.options & 0x01 != 0,
                    },
                ))
            }
            ObjectType::GraphicsContext => {
                // machbus compatibility extension (type 50): a geometry-less
                // graphics-context state object. Rendered best-effort as a
                // fixed-extent fill+border swatch using its drawing state; a
                // fully transparent context (transparency 0) draws nothing.
                let body = obj.get_graphics_context_body().ok()?;
                let ctx = body.context;
                if ctx.transparency == 0 {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::GraphicsContext,
                        reason: "GraphicsContext is fully transparent",
                    });
                    return None;
                }
                const GRAPHICS_CONTEXT_DEFAULT_EXTENT: u16 = 16;
                Some(mk(
                    Rect::new(
                        base.x,
                        base.y,
                        GRAPHICS_CONTEXT_DEFAULT_EXTENT,
                        GRAPHICS_CONTEXT_DEFAULT_EXTENT,
                    ),
                    ResolvedStyle::default(),
                    true,
                    true,
                    NodeKind::GraphicsContext {
                        fill_rgb: ctx.fill_color_rgb,
                        line_rgb: ctx.line_color_rgb,
                        line_width: ctx.line_width,
                        line_style: ctx.line_style,
                    },
                ))
            }
            ObjectType::Animation => {
                let body = obj.get_animation_body().ok()?;
                if obj.children_pos.is_empty() {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::Animation,
                        reason: "Animation contains no child objects",
                    });
                    return None;
                }
                let effective_enabled =
                    self.overrides.enabled(base.id).unwrap_or(body.enabled != 0);
                let frame = animation_frame(
                    &body,
                    &obj.children_pos,
                    effective_enabled,
                    self.animation_elapsed_for(base.id),
                )?;
                if state.path.contains(&frame.object) {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::Animation,
                        reason: "Animation frame reference cycle detected",
                    });
                    return None;
                }
                let Some(frame_obj) = pool.find(frame.object) else {
                    state.scene.unsupported.push(UnsupportedRecord {
                        id: base.id,
                        object_type: ObjectType::Animation,
                        reason: "Animation frame object is missing",
                    });
                    return None;
                };
                state.path.push(frame.object);
                let previous_clip = state.clip;
                state.clip =
                    combine_clip(previous_clip, Rect::new(base.x, base.y, body.width, body.height));
                let mut node = self.materialise(
                    pool,
                    resolver,
                    frame_obj,
                    BaseNode {
                        id: base.id,
                        object_type: ObjectType::Animation,
                        parent: base.parent,
                        x: base.x + i32::from(frame.x),
                        y: base.y + i32::from(frame.y),
                    },
                    state,
                );
                state.clip = previous_clip;
                state.path.pop();
                if let Some(node) = &mut node {
                    node.object_type = ObjectType::Animation;
                }
                node
            }
            ObjectType::InputBoolean => {
                let body = obj.get_input_boolean_body().ok()?;
                // ISO 11783-6 Input Boolean has a single width dimension
                // (the input is square) and a dedicated `enabled` field.
                let rect = Rect::new(base.x, base.y, body.width, body.width);
                let enabled = body.enabled != 0;
                let value = if body.variable_reference == ObjectID::NULL {
                    body.value != 0
                } else {
                    self.resolve_number_value(pool, body.variable_reference) != 0
                };
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    enabled,
                    NodeKind::InputBoolean { enabled, value },
                ))
            }
            ObjectType::InputString => {
                let body = obj.get_input_string_body().ok()?;
                let mut style = resolver.resolve_font(body.font_attributes);
                style.background = resolver.colour(body.background_color);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                Some(mk(
                    rect,
                    style,
                    true,
                    true,
                    NodeKind::InputString {
                        enabled: true,
                        text: self.resolve_string_variable(pool, body.variable_reference),
                        transparent_bg: body.options & 0x01 != 0,
                        auto_wrap: body.options & 0x02 != 0,
                        justification: body.justification,
                        max_length: body.max_length,
                        validation: resolve_input_validation(
                            pool,
                            body.input_attributes,
                            body.variable_reference,
                        ),
                    },
                ))
            }
            ObjectType::InputNumber => {
                let body = obj.get_input_number_body().ok()?;
                let mut style = resolver.resolve_font(body.font_attributes);
                style.background = resolver.colour(body.background_color);
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let enabled = body.options2 & 0x01 != 0;
                let real_time_editing = body.options2 & 0x02 != 0;
                let raw = self.resolve_number_value_or(pool, body.variable_reference, body.value);
                Some(mk(
                    rect,
                    style,
                    true,
                    enabled,
                    NodeKind::InputNumber {
                        enabled,
                        real_time_editing,
                        text: format_number(
                            raw,
                            body.offset,
                            body.scale,
                            body.number_of_decimals,
                            body.format,
                            body.options,
                            Some(usize::from(rect.w / style.font.cell_w.max(1))),
                        ),
                        transparent_bg: body.options & 0x01 != 0,
                        justification: body.justification,
                        min_value: body.min_value,
                        max_value: body.max_value,
                    },
                ))
            }
            ObjectType::InputList => {
                let body = obj.get_input_list_body().ok()?;
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let enabled = body.options & 0x01 != 0;
                let real_time_editing = body.options & 0x02 != 0;
                let selected = self.overrides.numeric_value(base.id).unwrap_or_else(|| {
                    self.resolve_number_value_or(
                        pool,
                        body.variable_reference,
                        u32::from(body.value),
                    )
                }) as usize;
                let selected_text = body
                    .items
                    .get(selected)
                    .and_then(|item_id| self.resolve_output_list_item_text(pool, *item_id));
                let selected_item_materialized = body
                    .items
                    .get(selected)
                    .copied()
                    .filter(|item_id| !state.path.contains(item_id))
                    .filter(|item_id| {
                        self.input_list_item_can_materialise_display_value(
                            pool,
                            *item_id,
                            &mut Vec::new(),
                        )
                    })
                    .is_some_and(|item_id| {
                        let previous_clip = state.clip;
                        state.clip = combine_clip(previous_clip, rect);
                        self.build_node(
                            pool,
                            resolver,
                            base.id,
                            &ChildPlacement::new(
                                item_id,
                                saturating_i32_to_i16(base.x),
                                saturating_i32_to_i16(base.y),
                            ),
                            state,
                        );
                        state.clip = previous_clip;
                        true
                    });
                let selectable_indices = body
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(index, item_id)| {
                        self.input_list_item_is_operator_selectable(
                            pool,
                            *item_id,
                            &mut Vec::new(),
                        )
                        .then_some(index)
                    })
                    .collect();
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    enabled,
                    NodeKind::InputList {
                        enabled,
                        real_time_editing,
                        selected,
                        item_count: body.items.len(),
                        selectable_indices,
                        selected_text,
                        selected_item_materialized,
                    },
                ))
            }
            ObjectType::Button => {
                // Buttons normally live under a Key inside a soft-key
                // mask, but if one appears directly on a data mask we
                // still render it inline.
                let body = obj.get_button_body().ok()?;
                let rect = Rect::new(base.x, base.y, body.width, body.height);
                let enabled = body.options & 0x10 == 0;
                let transparent_bg = body.options & 0x08 != 0;
                let draw_border = body.options & 0x24 == 0;
                let style = ResolvedStyle {
                    background: resolver.colour(body.background_color),
                    foreground: resolver.colour(body.border_color),
                    ..ResolvedStyle::default()
                };
                let label = obj
                    .children
                    .iter()
                    .find_map(|&child_id| {
                        self.resolve_key_child_text(pool, child_id, &mut Vec::new())
                    })
                    .unwrap_or_default();
                Some(mk(
                    rect,
                    style,
                    true,
                    enabled,
                    NodeKind::Button {
                        label,
                        enabled,
                        transparent_bg,
                        draw_border,
                        key_number: body.key_code,
                    },
                ))
            }
            ObjectType::Key => {
                let body = obj.get_key_body().ok()?;
                let cell = soft_key_cell_rect_for_config(
                    self.config,
                    0,
                    1,
                    self.soft_key_cell_height(),
                );
                let style = ResolvedStyle {
                    background: resolver.colour(body.background_color),
                    ..ResolvedStyle::default()
                };
                let label = self.resolve_key_label(pool, base.id);
                Some(mk(
                    Rect::new(base.x, base.y, cell.w, cell.h),
                    style,
                    true,
                    true,
                    NodeKind::KeyDesignator {
                        label,
                        key_number: body.key_code,
                    },
                ))
            }
            ObjectType::KeyGroup => {
                let body = obj.get_key_group_body().ok()?;
                let available = body.options & 0x01 != 0;
                let transparent = body.options & 0x02 != 0;
                let cell_h = self.soft_key_cell_height();
                let key_entries = obj
                    .children
                    .iter()
                    .take(4)
                    .map(|&child| self.resolve_key_group_entry(pool, child))
                    .collect::<Vec<_>>();
                let key_ids = key_entries
                    .iter()
                    .map(|entry| {
                        entry
                            .as_ref()
                            .map(|entry| entry.key_id)
                            .unwrap_or(ObjectID::NULL)
                    })
                    .collect::<Vec<_>>();
                let key_count = key_ids.len().clamp(1, 4);
                let base_rect =
                    soft_key_cell_rect_for_config(self.config, 0, key_count, cell_h)
                        .translate(base.x - self.config.soft_key_area.x, base.y - self.config.soft_key_area.y);
                let rect = Rect::new(base.x, base.y, base_rect.w, base_rect.h);
                let labels = key_entries
                    .iter()
                    .map(|entry| {
                        entry
                            .as_ref()
                            .map(|entry| self.resolve_key_label(entry.pool, entry.key_id))
                            .unwrap_or_default()
                    })
                    .collect::<Vec<_>>();
                let key_numbers = key_entries
                    .iter()
                    .map(|entry| {
                        entry
                            .as_ref()
                            .and_then(|entry| entry.pool.find(entry.key_id))
                            .and_then(|key| key.get_key_body().ok())
                            .map_or(0, |body| body.key_code)
                    })
                    .collect::<Vec<_>>();
                Some(mk(
                    rect,
                    ResolvedStyle::default(),
                    true,
                    available,
                    NodeKind::KeyGroup {
                        available,
                        transparent,
                        key_ids,
                        key_numbers,
                        labels,
                    },
                ))
            }
            // Reference / auxiliary / non-drawable object families. They
            // are recorded so the coverage ledger stays honest, but are
            // not emitted as visible scene nodes.
            ObjectType::NumberVariable
            | ObjectType::StringVariable
            | ObjectType::FontAttributes
            | ObjectType::LineAttributes
            | ObjectType::FillAttributes
            | ObjectType::InputAttributes
            | ObjectType::ExtendedInputAttributes
            | ObjectType::ObjectPointer
            | ObjectType::Macro
            | ObjectType::WorkingSet
            | ObjectType::WorkingSetSpecialControls
            | ObjectType::SoftKeyMask
            | ObjectType::AuxFunction
            | ObjectType::AuxInput
            | ObjectType::AuxFunction2
            | ObjectType::AuxInput2
            | ObjectType::AuxControlDesig
            | ObjectType::GraphicData
            | ObjectType::ColourMap
            | ObjectType::ExternalObjectDefinition
            | ObjectType::ExternalReferenceName
            | ObjectType::ExternalObjectPointer
            | ObjectType::ColourPalette
            | ObjectType::ObjectLabelRef => {
                state.scene.unsupported.push(UnsupportedRecord {
                    id: base.id,
                    object_type: obj.r#type,
                    reason: "object type is reference/auxiliary, not directly drawable",
                });
                None
            }
        }
    }

    fn build_soft_keys(&self, pool: &ObjectPool, skm_id: ObjectID, scene: &mut Scene) {
        let Some(skm) = pool.find(skm_id) else {
            return;
        };
        if skm.r#type != ObjectType::SoftKeyMask {
            return;
        }
        let Ok(body) = skm.get_soft_key_mask_body() else {
            return;
        };
        let soft_key_entries = self.resolved_soft_key_entries(pool, skm);
        let paging = self
            .config
            .soft_key_navigation_required(soft_key_entries.len());
        let app_slots = self.config.application_soft_key_slots();
        let (start, end) = if paging {
            let start = usize::from(self.config.soft_key_page).saturating_mul(app_slots);
            let end = start.saturating_add(app_slots).min(soft_key_entries.len());
            (start, end)
        } else {
            (0, soft_key_entries.len())
        };
        let page_count = if paging {
            page_count_for_len(soft_key_entries.len(), app_slots)
        } else {
            1
        };
        let displayed = end.saturating_sub(start);
        let physical_cells = if self.config.soft_key_paging_enabled() {
            usize::from(self.config.physical_soft_key_count.max(1))
        } else {
            displayed.max(1)
        };
        let cell_h = if self.config.soft_key_paging_enabled() {
            soft_key_cell_span_for_config(self.config)
        } else if soft_key_cells_are_horizontal(self.config) {
            (self.config.soft_key_area.w / physical_cells as u16).max(1)
        } else {
            (self.config.soft_key_area.h / physical_cells as u16).max(1)
        };
        for (i, entry) in soft_key_entries[start..end].iter().enumerate() {
            let Some(entry) = entry else {
                continue;
            };
            let key_id = entry.key_id;
            let Some(key) = entry.pool.find(key_id) else {
                continue;
            };
            if key.r#type != ObjectType::Key {
                continue;
            }
            let key_background = key
                .get_key_body()
                .ok()
                .map(|body| body.background_color)
                .unwrap_or(body.background_color);
            let key_number = key.get_key_body().ok().map_or(0, |body| body.key_code);
            let palette = self.effective_palette(entry.pool);
            let style = ResolvedStyle {
                background: palette.resolve(key_background),
                ..ResolvedStyle::default()
            };
            let label = self.resolve_key_label(entry.pool, key_id);
            scene.soft_keys.push(SoftKeyNode {
                id: key_id,
                kind: SoftKeyKind::Application,
                cell_index: i.try_into().unwrap_or(u8::MAX),
                rect: soft_key_cell_rect_for_config(self.config, i, 1, cell_h),
                style,
                visible: true,
                enabled: true,
                key_number,
                label,
            });
        }
        if paging && page_count > 1 {
            self.build_navigation_soft_keys(scene, page_count, cell_h);
        }
    }

    fn resolved_soft_key_entries<'a>(
        &'a self,
        pool: &'a ObjectPool,
        skm: &VTObject,
    ) -> Vec<Option<ResolvedSoftKeyEntry<'a>>> {
        let mut entries: Vec<Option<ResolvedSoftKeyEntry<'a>>> = skm
            .children
            .iter()
            .copied()
            .map(|child| self.resolve_soft_key_entry(pool, child, &mut Vec::new()))
            .collect();
        while entries.last().is_some_and(Option::is_none) {
            entries.pop();
        }
        entries
    }

    pub(crate) fn key_group_slot_count(&self, obj: &VTObject) -> usize {
        obj.children.len()
    }

    fn resolve_soft_key_entry<'a>(
        &'a self,
        pool: &'a ObjectPool,
        id: ObjectID,
        path: &mut Vec<ObjectID>,
    ) -> Option<ResolvedSoftKeyEntry<'a>> {
        if path.contains(&id) {
            return None;
        }
        let obj = pool.find(id)?;
        match obj.r#type {
            ObjectType::Key => Some(ResolvedSoftKeyEntry { key_id: id, pool }),
            ObjectType::ObjectPointer => {
                let body = obj.get_object_pointer_body().ok()?;
                if body.value == ObjectID::NULL {
                    None
                } else {
                    path.push(id);
                    let resolved = self.resolve_soft_key_entry(pool, body.value, path);
                    path.pop();
                    resolved
                }
            }
            ObjectType::ExternalObjectPointer => {
                let body = obj.get_external_object_pointer_body().ok()?;
                if let Some((external_pool, target)) = self.resolve_external_object(pool, &body)
                    && external_pool
                        .find(target)
                        .is_some_and(|object| object.r#type == ObjectType::Key)
                {
                    return Some(ResolvedSoftKeyEntry {
                        key_id: target,
                        pool: external_pool,
                    });
                }
                if body.default_object_id == ObjectID::NULL {
                    None
                } else {
                    path.push(id);
                    let resolved = self.resolve_soft_key_entry(pool, body.default_object_id, path);
                    path.pop();
                    resolved
                }
            }
            _ => None,
        }
    }

    fn resolve_key_label(&self, pool: &ObjectPool, key_id: ObjectID) -> String {
        let Some(key) = pool
            .find(key_id)
            .filter(|key| key.r#type == ObjectType::Key)
        else {
            return String::new();
        };
        for &child_id in &key.children {
            if let Some(text) = self.resolve_key_child_text(pool, child_id, &mut Vec::new())
                && !text.is_empty()
            {
                return text;
            }
        }
        key.get_key_body()
            .ok()
            .map(|body| body.key_code.to_string())
            .unwrap_or_default()
    }

    fn resolve_key_group_entry<'a>(
        &'a self,
        pool: &'a ObjectPool,
        child_id: ObjectID,
    ) -> Option<ResolvedSoftKeyEntry<'a>> {
        let obj = pool.find(child_id)?;
        match obj.r#type {
            ObjectType::Key => Some(ResolvedSoftKeyEntry {
                key_id: child_id,
                pool,
            }),
            ObjectType::ObjectPointer => {
                let body = obj.get_object_pointer_body().ok()?;
                let target = pool.find(body.value)?;
                (target.r#type == ObjectType::Key).then_some(ResolvedSoftKeyEntry {
                    key_id: body.value,
                    pool,
                })
            }
            ObjectType::ExternalObjectPointer => {
                let body = obj.get_external_object_pointer_body().ok()?;
                if let Some((external_pool, target)) = self.resolve_external_object(pool, &body)
                    && external_pool
                        .find(target)
                        .is_some_and(|object| object.r#type == ObjectType::Key)
                {
                    return Some(ResolvedSoftKeyEntry {
                        key_id: target,
                        pool: external_pool,
                    });
                }
                if body.default_object_id == ObjectID::NULL {
                    return None;
                }
                let target = pool.find(body.default_object_id)?;
                (target.r#type == ObjectType::Key).then_some(ResolvedSoftKeyEntry {
                    key_id: body.default_object_id,
                    pool,
                })
            }
            _ => None,
        }
    }

    fn resolve_key_child_text(
        &self,
        pool: &ObjectPool,
        child_id: ObjectID,
        path: &mut Vec<ObjectID>,
    ) -> Option<String> {
        if path.contains(&child_id) {
            return None;
        }
        let obj = pool.find(child_id)?;
        if obj.r#type == ObjectType::ObjectPointer {
            let body = obj.get_object_pointer_body().ok()?;
            if body.value == ObjectID::NULL {
                return None;
            }
            path.push(child_id);
            let label = self.resolve_key_child_text(pool, body.value, path);
            path.pop();
            return label;
        }
        self.resolve_output_list_item_text(pool, child_id)
    }

    fn build_navigation_soft_keys(&self, scene: &mut Scene, page_count: usize, cell_h: u16) {
        let nav_count = usize::from(self.config.effective_navigation_soft_key_count());
        if nav_count == 0 {
            return;
        }
        let physical_cells = usize::from(self.config.physical_soft_key_count.max(1));
        let current_page = usize::from(self.config.soft_key_page).min(page_count.saturating_sub(1));
        let first_nav_cell = physical_cells.saturating_sub(nav_count);
        let mut push_nav = |cell: usize, kind: SoftKeyKind, label: &'static str, enabled: bool| {
            scene.soft_keys.push(SoftKeyNode {
                id: ObjectID::NULL,
                kind,
                cell_index: cell.try_into().unwrap_or(u8::MAX),
                rect: soft_key_cell_rect_for_config(self.config, cell, 1, cell_h),
                style: ResolvedStyle::default(),
                visible: true,
                enabled,
                key_number: 0,
                label: label.to_string(),
            });
        };
        if nav_count >= 2 {
            push_nav(
                first_nav_cell,
                SoftKeyKind::NavigationPrevious,
                "<",
                current_page > 0,
            );
            push_nav(
                first_nav_cell + 1,
                SoftKeyKind::NavigationNext,
                ">",
                current_page + 1 < page_count,
            );
        } else if current_page + 1 < page_count {
            push_nav(first_nav_cell, SoftKeyKind::NavigationNext, ">", true);
        } else if current_page > 0 {
            push_nav(first_nav_cell, SoftKeyKind::NavigationPrevious, "<", true);
        }
    }

    fn resolve_string_value(&self, pool: &ObjectPool, body: &OutputStringBody) -> String {
        if body.variable_reference != ObjectID::NULL {
            let v = self.resolve_string_variable(pool, body.variable_reference);
            if !v.is_empty() {
                return v;
            }
        }
        text::decode_lossy(&body.value)
    }

    fn resolve_string_variable(&self, pool: &ObjectPool, var_ref: ObjectID) -> String {
        if var_ref == ObjectID::NULL {
            return String::new();
        }
        let Some(obj) = pool.find(var_ref) else {
            return String::new();
        };
        if obj.r#type != ObjectType::StringVariable {
            return String::new();
        }
        obj.get_string_variable_body()
            .map(|b| text::decode_lossy(&b.value))
            .unwrap_or_default()
    }

    fn resolve_number_value(&self, pool: &ObjectPool, var_ref: ObjectID) -> u32 {
        if var_ref == ObjectID::NULL {
            return 0;
        }
        let Some(obj) = pool.find(var_ref) else {
            return 0;
        };
        if obj.r#type != ObjectType::NumberVariable {
            return 0;
        }
        obj.get_number_variable_body().map(|b| b.value).unwrap_or(0)
    }

    /// Resolve a numeric value, falling back to the object's own raw
    /// `value` field when the variable reference is NULL (ISO 11783-6
    /// semantics for numeric VT objects with an inline raw value).
    fn resolve_number_value_or<T: Into<u32>>(
        &self,
        pool: &ObjectPool,
        var_ref: ObjectID,
        raw_value: T,
    ) -> u32 {
        if var_ref == ObjectID::NULL {
            return raw_value.into();
        }
        self.resolve_number_value(pool, var_ref)
    }

    fn fill_pattern(&self, pool: &ObjectPool, fill_attributes: ObjectID) -> Option<FillPattern> {
        let fill_obj = pool.find(fill_attributes)?;
        if fill_obj.r#type != ObjectType::FillAttributes {
            return None;
        }
        let fill = fill_obj.get_fill_attributes_body().ok()?;
        if fill.fill_type != 3 || fill.fill_pattern == ObjectID::NULL {
            return None;
        }
        let pattern_obj = pool.find(fill.fill_pattern)?;
        if pattern_obj.r#type != ObjectType::PictureGraphic {
            return None;
        }
        let pattern = pattern_obj.get_picture_graphic_body().ok()?;
        Some(FillPattern {
            object_id: fill.fill_pattern,
            width: pattern.actual_width,
            height: pattern.actual_height,
            format: pattern.format,
            compressed: pattern.options & 0x04 != 0,
            data: pattern.data,
        })
    }

}

fn combine_clip(current: Option<Rect>, next: Rect) -> Option<Rect> {
    let Some(current) = current else {
        return Some(next);
    };

    let left = current.x.max(next.x);
    let top = current.y.max(next.y);
    let right = current.right().min(next.right());
    let bottom = current.bottom().min(next.bottom());
    if right <= left || bottom <= top {
        return Some(Rect::new(left, top, 0, 0));
    }
    Some(Rect::new(
        left,
        top,
        u16::try_from(right.saturating_sub(left)).unwrap_or(u16::MAX),
        u16::try_from(bottom.saturating_sub(top)).unwrap_or(u16::MAX),
    ))
}
