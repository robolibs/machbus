use crate::isobus::vt::render::animation::animation_frame;
use crate::isobus::vt::render::scene::{
    ChildPlacement, FillPattern, InputValidation, InputValidationRange, NodeKind, Rect, Scene,
    SceneLanguage, SceneNode, SoftKeyKind, SoftKeyNode, UnsupportedRecord,
};
use crate::isobus::vt::render::style::{Palette, ResolvedStyle, StyleResolver};
use crate::isobus::vt::render::text;
use crate::isobus::vt::{
    ObjectID, ObjectPool, ObjectType, OutputListBody, OutputStringBody, VTObject, WindowMaskBody,
    WorkingSetSpecialControlsBody,
};
use std::collections::HashMap;

/// Canvas geometry supplied to the layout engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayoutConfig {
    /// `(width, height)` of the main data area in device pixels.
    pub canvas: (u16, u16),
    /// Rectangle of the soft-key column/row in device pixels.
    pub soft_key_area: Rect,
    /// Physical soft-key positions available on the VT. `0` preserves
    /// legacy unlimited rendering for tests/backends that do not model
    /// physical soft-key paging.
    pub physical_soft_key_count: u8,
    /// Physical soft-key positions reserved for navigation/paging.
    pub navigation_soft_key_count: u8,
    /// Zero-based page of application soft keys to render.
    pub soft_key_page: u16,
    /// Vertical gap (px) used by the auto-layout fall-back.
    pub auto_layout_gap: u16,
}

impl LayoutConfig {
    /// Effective navigation-cell reservation for bounded soft-key profiles.
    ///
    /// Misconfigured host profiles must not let navigation consume every
    /// physical soft-key position: the terminal needs at least one application
    /// cell on a rendered page. `0` physical keys keeps the legacy unbounded
    /// path and therefore reserves no navigation cells.
    #[must_use]
    pub const fn effective_navigation_soft_key_count(self) -> u8 {
        if self.physical_soft_key_count == 0 {
            return 0;
        }
        let requested = if self.navigation_soft_key_count > self.physical_soft_key_count {
            self.physical_soft_key_count
        } else {
            self.navigation_soft_key_count
        };
        let max_navigation = self.physical_soft_key_count.saturating_sub(1);
        if requested > max_navigation {
            max_navigation
        } else {
            requested
        }
    }

    /// Number of application soft-key cells available on one page.
    #[must_use]
    pub const fn application_soft_key_slots(self) -> usize {
        if self.physical_soft_key_count == 0 {
            return usize::MAX;
        }
        let reserved = self.effective_navigation_soft_key_count();
        let slots = self.physical_soft_key_count - reserved;
        if slots == 0 { 1 } else { slots as usize }
    }

    /// `true` when the config models bounded physical soft keys.
    #[must_use]
    pub const fn soft_key_paging_enabled(self) -> bool {
        self.physical_soft_key_count != 0
    }

    /// `true` when the active soft-key mask needs VT-provided page
    /// navigation. A bounded VT still displays every key directly when the
    /// mask supplies no more keys than the physical-key count.
    #[must_use]
    pub const fn soft_key_navigation_required(self, key_count: usize) -> bool {
        self.physical_soft_key_count != 0
            && self.effective_navigation_soft_key_count() != 0
            && key_count > self.physical_soft_key_count as usize
    }
}

fn page_count_for_len(total_keys: usize, app_slots: usize) -> usize {
    if total_keys == 0 || app_slots == 0 || app_slots == usize::MAX {
        1
    } else {
        total_keys.div_ceil(app_slots).max(1)
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            canvas: (480, 240),
            // Soft keys occupy a 64px column on the right edge of a
            // 480-wide terminal by convention; downstream code may
            // override this for landscape soft-key rows.
            soft_key_area: Rect::new(480, 0, 64, 240),
            physical_soft_key_count: 0,
            navigation_soft_key_count: 0,
            soft_key_page: 0,
            auto_layout_gap: 4,
        }
    }
}

pub(crate) fn soft_key_cells_are_horizontal(config: LayoutConfig) -> bool {
    config.soft_key_area.w > config.soft_key_area.h
}

pub(crate) fn soft_key_cell_span_for_config(config: LayoutConfig) -> u16 {
    let main_extent = if soft_key_cells_are_horizontal(config) {
        config.soft_key_area.w
    } else {
        config.soft_key_area.h
    };
    if config.physical_soft_key_count == 0 {
        return main_extent.clamp(1, 40);
    }
    (main_extent / u16::from(config.physical_soft_key_count)).max(1)
}

pub(crate) fn soft_key_cell_rect_for_config(
    config: LayoutConfig,
    cell_index: usize,
    cell_span_count: usize,
    cell_span: u16,
) -> Rect {
    let area = config.soft_key_area;
    let span = cell_span.saturating_mul(cell_span_count.max(1) as u16);
    if soft_key_cells_are_horizontal(config) {
        Rect::new(
            area.x + cell_index as i32 * i32::from(cell_span),
            area.y,
            span,
            area.h,
        )
    } else {
        Rect::new(
            area.x,
            area.y + cell_index as i32 * i32::from(cell_span),
            area.w,
            span,
        )
    }
}

/// Per-child explicit placement overrides. When an id is absent, the
/// layout engine uses its deterministic auto-stack.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlacementMap {
    map: HashMap<u16, (i16, i16)>,
}

impl PlacementMap {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the position of a child within its parent mask/container.
    /// Coordinates are signed to match the ISO 11783-6 child-location
    /// encoding (children may sit at negative offsets).
    #[must_use]
    pub fn set(mut self, id: impl Into<ObjectID>, x: i16, y: i16) -> Self {
        self.map.insert(id.into().0, (x, y));
        self
    }

    #[must_use]
    pub fn get(&self, id: ObjectID) -> Option<(i16, i16)> {
        self.map.get(&id.0).copied()
    }

    /// Update a placement in place.
    pub fn set_mut(&mut self, id: impl Into<ObjectID>, x: i16, y: i16) {
        self.map.insert(id.into().0, (x, y));
    }

    /// Remove a placement override in place.
    pub fn remove_mut(&mut self, id: impl Into<ObjectID>) {
        self.map.remove(&id.into().0);
    }
}

fn working_set_special_controls(pool: &ObjectPool) -> Option<WorkingSetSpecialControlsBody> {
    pool.objects()
        .iter()
        .find(|obj| obj.r#type == ObjectType::WorkingSetSpecialControls)
        .and_then(|obj| obj.get_working_set_special_controls_body().ok())
}

fn working_set_languages(pool: &ObjectPool) -> Vec<SceneLanguage> {
    if let Some(controls) = working_set_special_controls(pool)
        && !controls.languages.is_empty()
    {
        return controls
            .languages
            .into_iter()
            .map(|pair| SceneLanguage {
                language: pair.language,
                country: pair.country,
            })
            .collect();
    }
    pool.objects()
        .iter()
        .find(|obj| obj.r#type == ObjectType::WorkingSet)
        .and_then(|obj| obj.get_working_set_body().ok())
        .map(|body| {
            body.languages
                .into_iter()
                .map(|language| SceneLanguage {
                    language,
                    country: [b' '; 2],
                })
                .collect()
        })
        .unwrap_or_default()
}

fn external_pool_allows_reference(
    pool: &ObjectPool,
    referencing_name: (u32, u32),
    target: ObjectID,
) -> bool {
    pool.objects().iter().any(|object| {
        object.r#type == ObjectType::ExternalObjectDefinition
            && object
                .get_external_object_definition_body()
                .ok()
                .is_some_and(|body| {
                    body.options & 0x01 != 0
                        && body.name0 == referencing_name.0
                        && body.name1 == referencing_name.1
                        && body.object_ids.contains(&target)
                })
    })
}

#[derive(Debug, Clone)]
struct ScaledGraphicSource {
    width: u16,
    height: u16,
    format: u8,
    options: u8,
    standard_png: bool,
    transparent: bool,
    transparency: u8,
    data: Vec<u8>,
}

fn scale_dimension_preserving_aspect(source: u16, other_source: u16, other_target: u16) -> u16 {
    if other_source == 0 {
        return 0;
    }
    let scaled = u32::from(source).saturating_mul(u32::from(other_target));
    let rounded = scaled.saturating_add(u32::from(other_source) / 2) / u32::from(other_source);
    u16::try_from(rounded).unwrap_or(u16::MAX)
}

fn scaled_graphic_destination(
    base_x: i32,
    base_y: i32,
    field_width: u16,
    field_height: u16,
    source_width: u16,
    source_height: u16,
    scale_type: u8,
) -> Rect {
    let scaling_value = scale_type & 0x07;
    let horizontal_justification = (scale_type >> 3) & 0x03;
    let vertical_justification = (scale_type >> 5) & 0x03;

    let (mut width, mut height) = match scaling_value {
        // 0 = Not scaled, use the raw graphic's width/height.
        0 => (source_width, source_height),
        // 1 = Scale to Width, maintaining aspect ratio.
        1 => (
            field_width,
            scale_dimension_preserving_aspect(source_height, source_width, field_width),
        ),
        // 2 = Scale to Height, maintaining aspect ratio.
        2 => (
            scale_dimension_preserving_aspect(source_width, source_height, field_height),
            field_height,
        ),
        // 3 = Scale to Width and Height, distortion allowed.
        3 => (field_width, field_height),
        // 4 = Fit inside Width and Height, maintaining aspect ratio.
        4 => {
            if source_width == 0 || source_height == 0 || field_width == 0 || field_height == 0 {
                (0, 0)
            } else {
                let by_width =
                    scale_dimension_preserving_aspect(source_height, source_width, field_width);
                if by_width <= field_height {
                    (field_width, by_width)
                } else {
                    (
                        scale_dimension_preserving_aspect(
                            source_width,
                            source_height,
                            field_height,
                        ),
                        field_height,
                    )
                }
            }
        }
        _ => (0, 0),
    };
    if width == 0 && field_width != 0 && source_width == 0 {
        width = field_width;
    }
    if height == 0 && field_height != 0 && source_height == 0 {
        height = field_height;
    }

    let spare_width = field_width.saturating_sub(width);
    let spare_height = field_height.saturating_sub(height);
    let x_offset = match horizontal_justification {
        1 => spare_width / 2,
        2 => spare_width,
        _ => 0,
    };
    let y_offset = match vertical_justification {
        1 => spare_height / 2,
        2 => spare_height,
        _ => 0,
    };

    Rect::new(
        base_x + i32::from(x_offset),
        base_y + i32::from(y_offset),
        width,
        height,
    )
}

fn resolve_scaled_graphic_source(
    pool: &ObjectPool,
    id: ObjectID,
    visited_pointers: &mut Vec<ObjectID>,
) -> Option<Result<Option<ScaledGraphicSource>, &'static str>> {
    if id == ObjectID::NULL {
        return Some(Ok(None));
    }
    let object = pool.find(id)?;
    match object.r#type {
        ObjectType::GraphicData => Some(object.get_graphic_data_body().map_or(
            Err("ScaledGraphic references undecodable GraphicData body"),
            |body| {
                Ok(Some(ScaledGraphicSource {
                    width: 0,
                    height: 0,
                    format: body.format,
                    options: 0,
                    standard_png: true,
                    transparent: false,
                    transparency: u8::MAX,
                    data: body.data,
                }))
            },
        )),
        ObjectType::PictureGraphic => Some(object.get_picture_graphic_body().map_or(
            Err("ScaledGraphic references undecodable PictureGraphic body"),
            |body| {
                Ok(Some(ScaledGraphicSource {
                    width: body.actual_width,
                    height: body.actual_height,
                    format: body.format,
                    options: if body.options & 0x04 != 0 { 0x01 } else { 0 },
                    standard_png: false,
                    transparent: body.options & 0x01 != 0,
                    transparency: body.transparency,
                    data: body.data,
                }))
            },
        )),
        ObjectType::ObjectPointer => {
            if visited_pointers.contains(&id) {
                return Some(Err("ScaledGraphic ObjectPointer reference cycle detected"));
            }
            let body = match object.get_object_pointer_body() {
                Ok(body) => body,
                Err(_) => {
                    return Some(Err(
                        "ScaledGraphic references undecodable ObjectPointer body",
                    ));
                }
            };
            visited_pointers.push(id);
            let resolved = resolve_scaled_graphic_source(pool, body.value, visited_pointers);
            visited_pointers.pop();
            resolved.or(Some(Err("ScaledGraphic ObjectPointer target is missing")))
        }
        _ => Some(Err(
            "ScaledGraphic value reference is not GraphicData, PictureGraphic, or ObjectPointer",
        )),
    }
}

pub(crate) fn window_mask_cell_span(body: &WindowMaskBody) -> (u8, u8) {
    match body.window_type {
        0 => (body.width_cells.clamp(1, 2), body.height_cells.clamp(1, 6)),
        1..=9 => (1, 1),
        10..=18 => (2, 1),
        _ => (0, 0),
    }
}

fn translated_child(origin_x: i32, origin_y: i32, cp: &ChildPlacement) -> ChildPlacement {
    ChildPlacement::new(
        cp.id,
        saturating_i32_to_i16(origin_x.saturating_add(i32::from(cp.x))),
        saturating_i32_to_i16(origin_y.saturating_add(i32::from(cp.y))),
    )
}

fn saturating_i32_to_i16(value: i32) -> i16 {
    if value < i32::from(i16::MIN) {
        i16::MIN
    } else if value > i32::from(i16::MAX) {
        i16::MAX
    } else {
        value as i16
    }
}

fn window_required_object_placements(body: &WindowMaskBody, rect: Rect) -> Vec<ChildPlacement> {
    let half_w = i16::try_from(i32::from(rect.w / 2)).unwrap_or(i16::MAX);
    match body.window_type {
        1 | 4 | 10 | 13 | 9 | 18 => body
            .required_objects
            .iter()
            .copied()
            .take(2)
            .enumerate()
            .map(|(index, id)| {
                let x = if index == 0 { 0 } else { half_w };
                ChildPlacement::new(id, x, 0)
            })
            .collect(),
        2 | 3 | 5 | 6 | 7 | 8 | 11 | 12 | 14 | 15 | 16 | 17 => body
            .required_objects
            .first()
            .copied()
            .map(|id| vec![ChildPlacement::new(id, 0, 0)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// Runtime visibility / enabled overrides layered on top of an object's
/// own option bits. These hold the *runtime* state produced by Hide/Show
/// and Enable/Disable commands (e.g. from macros) — state that is not part
/// of the object pool definition and must survive a scene rebuild.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RuntimeOverrides {
    visibility: HashMap<u16, bool>,
    enabled: HashMap<u16, bool>,
    backgrounds: HashMap<u16, u8>,
    numeric_values: HashMap<u16, u32>,
}

impl RuntimeOverrides {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn set_visible(mut self, id: impl Into<ObjectID>, visible: bool) -> Self {
        self.set_visible_mut(id, visible);
        self
    }

    #[must_use]
    pub fn set_enabled(mut self, id: impl Into<ObjectID>, enabled: bool) -> Self {
        self.set_enabled_mut(id, enabled);
        self
    }

    /// Update a runtime visibility override in place.
    pub fn set_visible_mut(&mut self, id: impl Into<ObjectID>, visible: bool) -> bool {
        self.visibility.insert(id.into().0, visible) != Some(visible)
    }

    /// Update a runtime enabled/disabled override in place.
    pub fn set_enabled_mut(&mut self, id: impl Into<ObjectID>, enabled: bool) -> bool {
        self.enabled.insert(id.into().0, enabled) != Some(enabled)
    }

    /// Update a runtime-only background-colour override in place.
    pub fn set_background_mut(&mut self, id: impl Into<ObjectID>, colour: u8) {
        self.backgrounds.insert(id.into().0, colour);
    }

    /// Update a runtime-only numeric value override in place.
    ///
    /// This is used for render-visible value state that has no inline value
    /// field in the object-pool body, such as Input List selected index state
    /// retained by an accepted Change Numeric Value command.
    pub fn set_numeric_value_mut(&mut self, id: impl Into<ObjectID>, value: u32) -> bool {
        self.numeric_values.insert(id.into().0, value) != Some(value)
    }

    /// Fold a macro apply report's visibility/enabled changes into the
    /// override set (later changes win).
    pub fn apply_report(&mut self, report: &crate::isobus::vt::render::macros::MacroApplyReport) {
        for (id, show) in &report.visibility_changes {
            self.visibility.insert(id.0, *show);
        }
        for (id, enable) in &report.enable_changes {
            self.enabled.insert(id.0, *enable);
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.visibility.is_empty()
            && self.enabled.is_empty()
            && self.backgrounds.is_empty()
            && self.numeric_values.is_empty()
    }

    #[must_use]
    pub(crate) fn visible(&self, id: ObjectID) -> Option<bool> {
        self.visibility.get(&id.0).copied()
    }

    #[must_use]
    pub(crate) fn enabled(&self, id: ObjectID) -> Option<bool> {
        self.enabled.get(&id.0).copied()
    }

    #[must_use]
    pub(crate) fn background(&self, id: ObjectID) -> Option<u8> {
        self.backgrounds.get(&id.0).copied()
    }

    #[must_use]
    pub(crate) fn numeric_value(&self, id: ObjectID) -> Option<u32> {
        self.numeric_values.get(&id.0).copied()
    }
}

/// The layout engine. Stateless aside from configuration; one instance
/// can build many scenes.
#[derive(Debug, Clone)]
pub struct LayoutEngine {
    config: LayoutConfig,
    placements: PlacementMap,
    palette: Palette,
    colour_map: Option<ObjectID>,
    colour_palette: Option<ObjectID>,
    working_set_name: Option<(u32, u32)>,
    external_pools: Vec<ExternalObjectPool>,
    overrides: RuntimeOverrides,
    animation_elapsed_ms: u32,
    animation_elapsed_by_object: HashMap<u16, u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalObjectPool {
    name0: u32,
    name1: u32,
    pool: ObjectPool,
}

struct ResolvedSoftKeyEntry<'a> {
    key_id: ObjectID,
    pool: &'a ObjectPool,
}
