fn indexed_bitmap_pixel(
    data: &[u8],
    format: u8,
    width: u16,
    x: usize,
    y: usize,
) -> Option<u8> {
    let width = usize::from(width);
    match format {
        0 => {
            let row = width.saturating_add(7) / 8;
            let byte = *data.get(y.checked_mul(row)?.checked_add(x / 8)?)?;
            let shift = 7usize.saturating_sub(x % 8);
            Some((byte >> shift) & 0x01)
        }
        1 => {
            let row = width.saturating_add(1) / 2;
            let byte = *data.get(y.checked_mul(row)?.checked_add(x / 2)?)?;
            if x.is_multiple_of(2) {
                Some((byte >> 4) & 0x0F)
            } else {
                Some(byte & 0x0F)
            }
        }
        2 => data.get(y.checked_mul(width)?.checked_add(x)?).copied(),
        _ => None,
    }
}

fn palette_index(palette: &Palette, colour: Colour) -> u8 {
    palette
        .entries()
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| palette_colour_distance_squared(colour, **candidate))
        .map(|(index, _)| u8::try_from(index).unwrap_or(u8::MAX))
        .unwrap_or(0)
}

fn palette_colour_distance_squared(a: Colour, b: Colour) -> u32 {
    let dr = i32::from(a.r).saturating_sub(i32::from(b.r));
    let dg = i32::from(a.g).saturating_sub(i32::from(b.g));
    let db = i32::from(a.b).saturating_sub(i32::from(b.b));
    u32::try_from(
        dr.saturating_mul(dr)
            .saturating_add(dg.saturating_mul(dg))
            .saturating_add(db.saturating_mul(db)),
    )
    .unwrap_or(u32::MAX)
}

fn graphic_context_viewport(pool: &ObjectPool, id: ObjectID) -> Option<Rect> {
    let object = pool.find(id)?;
    if object.r#type != ObjectType::GraphicContext {
        return None;
    }
    let body = object.get_graphic_context_body().ok()?;
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
        return None;
    }
    Some(Rect::new(
        i32::from(body.viewport_x),
        i32::from(body.viewport_y),
        width,
        height,
    ))
}

fn decode_graphics_context_draw_text(payload: &[u8]) -> Option<(bool, String)> {
    if payload.len() < 2 {
        return None;
    }
    let transparent = payload[0] != 0;
    let len = payload[1] as usize;
    let end = 2usize.checked_add(len)?;
    if end > payload.len() {
        return None;
    }
    Some((
        transparent,
        String::from_utf8_lossy(&payload[2..end]).into_owned(),
    ))
}

fn decode_graphics_context_i16_pair(payload: &[u8]) -> Option<(i32, i32)> {
    if payload.len() < 4 {
        return None;
    }
    Some((
        i32::from(i16::from_le_bytes([payload[0], payload[1]])),
        i32::from(i16::from_le_bytes([payload[2], payload[3]])),
    ))
}

fn decode_graphics_context_u16_pair(payload: &[u8]) -> Option<(u16, u16)> {
    if payload.len() < 4 {
        return None;
    }
    Some((
        u16::from_le_bytes([payload[0], payload[1]]),
        u16::from_le_bytes([payload[2], payload[3]]),
    ))
}

fn decode_graphics_context_u32(payload: &[u8]) -> Option<u32> {
    if payload.len() < 4 {
        return None;
    }
    Some(u32::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ]))
}

fn decode_graphics_context_object_id(payload: &[u8]) -> Option<ObjectID> {
    if payload.len() < 2 {
        return None;
    }
    Some(ObjectID::new(u16::from_le_bytes([payload[0], payload[1]])))
}

fn graphics_context_reference_targets_are_valid(
    pool: &ObjectPool,
    source: ObjectID,
    subcommand: u8,
    payload: &[u8],
) -> bool {
    let Some(reference) = decode_graphics_context_object_id(payload) else {
        return true;
    };
    match subcommand {
        0x04 => pool_reference_has_type(pool, reference, ObjectType::LineAttributes),
        0x05 => pool_reference_has_type(pool, reference, ObjectType::FillAttributes),
        0x06 => pool_reference_has_type(pool, reference, ObjectType::FontAttributes),
        0x12 => {
            reference != ObjectID::NULL
                && reference != source
                && pool.find(reference).is_some_and(|object| {
                    graphics_context_draw_vt_object_type_is_drawable(object.r#type)
                })
                && !object_contains(pool, reference, source, &mut Vec::new())
        }
        0x13 | 0x14 => {
            reference != ObjectID::NULL
                && pool_reference_has_type(pool, reference, ObjectType::PictureGraphic)
        }
        _ => true,
    }
}

const fn graphics_context_draw_vt_object_type_is_drawable(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::DataMask
            | ObjectType::AlarmMask
            | ObjectType::Container
            | ObjectType::Button
            | ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
            | ObjectType::OutputList
            | ObjectType::OutputString
            | ObjectType::OutputNumber
            | ObjectType::Line
            | ObjectType::Rectangle
            | ObjectType::Ellipse
            | ObjectType::Polygon
            | ObjectType::Meter
            | ObjectType::LinearBarGraph
            | ObjectType::ArchedBarGraph
            | ObjectType::PictureGraphic
            | ObjectType::WindowMask
            | ObjectType::Key
            | ObjectType::KeyGroup
            | ObjectType::ScaledGraphic
            | ObjectType::Animation
            | ObjectType::GraphicContext
    )
}

fn pool_reference_has_type(pool: &ObjectPool, reference: ObjectID, expected: ObjectType) -> bool {
    reference == ObjectID::NULL
        || pool
            .find(reference)
            .is_some_and(|object| object.r#type == expected)
}

fn pool_reference_has_any_type(
    pool: &ObjectPool,
    reference: ObjectID,
    expected: &[ObjectType],
) -> bool {
    reference == ObjectID::NULL
        || pool
            .find(reference)
            .is_some_and(|object| expected.contains(&object.r#type))
}

fn window_mask_required_objects_match(
    pool: &ObjectPool,
    body: &crate::isobus::vt::WindowMaskBody,
    window_type: u8,
) -> bool {
    let Some(expected) = window_mask_required_object_types(window_type) else {
        return false;
    };
    body.required_objects.len() == expected.len()
        && body
            .required_objects
            .iter()
            .copied()
            .zip(expected.iter().copied())
            .all(|(reference, expected_type)| {
                reference != ObjectID::NULL
                    && pool_reference_has_type(pool, reference, expected_type)
            })
}

fn decode_graphics_context_polygon_points(payload: &[u8]) -> Option<Vec<(i32, i32)>> {
    let count = usize::from(*payload.first()?);
    let expected = 1usize.checked_add(count.checked_mul(4)?)?;
    if payload.len() < expected {
        return None;
    }
    let mut points = Vec::with_capacity(count);
    for raw in payload[1..expected].chunks_exact(4) {
        points.push((
            i32::from(i16::from_le_bytes([raw[0], raw[1]])),
            i32::from(i16::from_le_bytes([raw[2], raw[3]])),
        ));
    }
    Some(points)
}

fn usize_to_i32(value: usize) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn soft_key_page_count_for(pool: &ObjectPool, active_mask: ObjectID, config: LayoutConfig) -> u16 {
    if !config.soft_key_paging_enabled() {
        return 1;
    }
    let Some(mask_id) = active_soft_key_mask(pool, active_mask) else {
        return 1;
    };
    let Some(mask) = pool.find(mask_id) else {
        return 1;
    };
    if mask.r#type != ObjectType::SoftKeyMask {
        return 1;
    }
    let count = effective_soft_key_entry_count(pool, mask);
    if count == 0 {
        return 1;
    }
    if !config.soft_key_navigation_required(count) {
        return 1;
    }
    let slots = config.application_soft_key_slots();
    if slots == 0 || slots == usize::MAX {
        return 1;
    }
    let pages = count.div_ceil(slots).max(1);
    pages.min(usize::from(u16::MAX)) as u16
}

fn effective_soft_key_entry_count(pool: &ObjectPool, mask: &VTObject) -> usize {
    let mut entries = mask
        .children
        .iter()
        .copied()
        .map(|child| soft_key_entry_resolves_to_key(pool, child, &mut Vec::new()))
        .collect::<Vec<_>>();
    while entries.last().is_some_and(Option::is_none) {
        entries.pop();
    }
    entries.len()
}

fn soft_key_entry_resolves_to_key(
    pool: &ObjectPool,
    id: ObjectID,
    path: &mut Vec<ObjectID>,
) -> Option<ObjectID> {
    if path.contains(&id) {
        return None;
    }
    let obj = pool.find(id)?;
    match obj.r#type {
        ObjectType::Key => Some(id),
        ObjectType::ObjectPointer => {
            let body = obj.get_object_pointer_body().ok()?;
            if body.value == ObjectID::NULL {
                return None;
            }
            path.push(id);
            let resolved = soft_key_entry_resolves_to_key(pool, body.value, path);
            path.pop();
            resolved
        }
        ObjectType::ExternalObjectPointer => {
            let body = obj.get_external_object_pointer_body().ok()?;
            if body.default_object_id == ObjectID::NULL {
                return None;
            }
            path.push(id);
            let resolved = soft_key_entry_resolves_to_key(pool, body.default_object_id, path);
            path.pop();
            resolved
        }
        _ => None,
    }
}

fn active_soft_key_mask(pool: &ObjectPool, active_mask: ObjectID) -> Option<ObjectID> {
    let mask = pool.find(active_mask)?;
    match mask.r#type {
        ObjectType::DataMask => mask
            .get_data_mask_body()
            .ok()
            .map(|body| body.soft_key_mask),
        ObjectType::AlarmMask => mask
            .get_alarm_mask_body()
            .ok()
            .map(|body| body.soft_key_mask),
        _ => None,
    }
    .filter(|id| *id != ObjectID::NULL)
}

fn soft_key_cell_height_for_config(config: LayoutConfig) -> u16 {
    let main_extent = if soft_key_cells_are_horizontal_for_config(config) {
        config.soft_key_area.w
    } else {
        config.soft_key_area.h
    };
    if config.physical_soft_key_count == 0 {
        return main_extent.clamp(1, 40);
    }
    (main_extent / u16::from(config.physical_soft_key_count)).max(1)
}

fn soft_key_cell_count_for_config(config: LayoutConfig, cell_h: u16) -> usize {
    if config.physical_soft_key_count != 0 {
        return usize::from(config.physical_soft_key_count);
    }
    let main_extent = if soft_key_cells_are_horizontal_for_config(config) {
        config.soft_key_area.w
    } else {
        config.soft_key_area.h
    };
    usize::from((main_extent / cell_h.max(1)).max(1))
}

fn soft_key_cells_are_horizontal_for_config(config: LayoutConfig) -> bool {
    config.soft_key_area.w > config.soft_key_area.h
}

fn soft_key_cell_rect_for_config(
    config: LayoutConfig,
    cell_index: usize,
    cell_span_count: usize,
    cell_span: u16,
) -> Rect {
    let area = config.soft_key_area;
    let span = cell_span.saturating_mul(cell_span_count.max(1) as u16);
    if soft_key_cells_are_horizontal_for_config(config) {
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

fn key_group_key_at_scene_point(
    node: &SceneNode,
    key_ids: &[ObjectID],
    pointer_x: i32,
    pointer_y: i32,
) -> Option<ObjectID> {
    if key_ids.is_empty() || node.rect.h == 0 {
        return None;
    }
    let horizontal = node.rect.w > node.rect.h;
    let main_extent = if horizontal { node.rect.w } else { node.rect.h };
    let cell_span = (usize::from(main_extent) / key_ids.len()).max(1);
    let relative = if horizontal {
        pointer_x.saturating_sub(node.rect.x)
    } else {
        pointer_y.saturating_sub(node.rect.y)
    }
    .max(0) as usize;
    let index = (relative / cell_span).min(key_ids.len().saturating_sub(1));
    key_ids
        .get(index)
        .copied()
        .filter(|id| *id != ObjectID::NULL)
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.w != 0
        && a.h != 0
        && b.w != 0
        && b.h != 0
        && a.x < b.right()
        && b.x < a.right()
        && a.y < b.bottom()
        && b.y < a.bottom()
}

fn is_input_object(node: Option<&crate::isobus::vt::render::scene::SceneNode>) -> bool {
    matches!(
        node.map(|node| node.object_type),
        Some(
            ObjectType::InputBoolean
                | ObjectType::InputString
                | ObjectType::InputNumber
                | ObjectType::InputList
        )
    )
}

fn pointing_coordinates(rect: Rect, px: i32, py: i32) -> (u16, u16) {
    let max_x = i32::from(rect.w.saturating_sub(1));
    let max_y = i32::from(rect.h.saturating_sub(1));
    let x = px.saturating_sub(rect.x).clamp(0, max_x);
    let y = py.saturating_sub(rect.y).clamp(0, max_y);
    (
        u16::try_from(x).unwrap_or(u16::MAX),
        u16::try_from(y).unwrap_or(u16::MAX),
    )
}

fn push_bus_message_if_missing(out: &mut Vec<VtBusMessage>, message: VtBusMessage) {
    if !out.iter().any(|existing| existing == &message) {
        out.push(message);
    }
}

fn validate_bus_event_object_id(id: ObjectID, context: &'static str) -> Result<()> {
    if id == ObjectID::NULL {
        return Err(Error::invalid_data(match context {
            "boolean value change" => "boolean value-change event targets NULL object id",
            "number value change" => "number value-change event targets NULL object id",
            "list selection change" => "list selection-change event targets NULL object id",
            "string value change" => "string value-change event targets NULL object id",
            "VT ESC" => "VT ESC event targets NULL object id",
            "select input object" => "Select Input Object event targets NULL object id",
            "soft-key activation" => "soft-key activation event targets NULL object id",
            "button activation" => "button activation event targets NULL object id",
            "pointing event" => "pointing event targets NULL parent mask",
            _ => "VT bus event targets NULL object id",
        }));
    }
    Ok(())
}

impl From<&ServerRenderEffect> for VtRuntimeCommand {
    fn from(effect: &ServerRenderEffect) -> Self {
        match effect {
            ServerRenderEffect::HideShow { id, visible } => Self::HideShow {
                id: *id,
                visible: *visible,
            },
            ServerRenderEffect::EnableDisable { id, enabled } => Self::EnableDisable {
                id: *id,
                enabled: *enabled,
            },
            ServerRenderEffect::SelectInputObject { id, open_for_input } => {
                Self::SelectInputObject {
                    id: *id,
                    open_for_input: *open_for_input,
                }
            }
            ServerRenderEffect::Esc => Self::Esc,
            ServerRenderEffect::ChangeChildLocation {
                parent,
                child,
                x,
                y,
            } => Self::ChangeChildLocation {
                parent: *parent,
                child: *child,
                x: *x,
                y: *y,
            },
            ServerRenderEffect::ChangeChildPosition {
                parent,
                child,
                x,
                y,
            } => Self::ChangeChildPosition {
                parent: *parent,
                child: *child,
                x: *x,
                y: *y,
            },
            ServerRenderEffect::ChangeSize { id, width, height } => Self::ChangeSize {
                id: *id,
                width: *width,
                height: *height,
            },
            ServerRenderEffect::ChangeEndPoint {
                id,
                width,
                height,
                line_direction,
            } => Self::ChangeEndPoint {
                id: *id,
                width: *width,
                height: *height,
                line_direction: *line_direction,
            },
            ServerRenderEffect::ChangeBackgroundColour { id, colour } => {
                Self::ChangeBackgroundColour {
                    id: *id,
                    colour: *colour,
                }
            }
            ServerRenderEffect::ChangeNumericValue { id, value } => Self::ChangeNumericValue {
                id: *id,
                value: *value,
            },
            ServerRenderEffect::ChangeStringValue { id, text } => Self::ChangeStringValue {
                id: *id,
                text: text.clone(),
            },
            ServerRenderEffect::ChangeFontAttributes { id, attributes } => {
                Self::ChangeFontAttributes {
                    id: *id,
                    attributes: *attributes,
                }
            }
            ServerRenderEffect::ChangeFontAttributeValues {
                id,
                colour,
                size,
                font_type,
                style,
            } => Self::ChangeFontAttributeValues {
                id: *id,
                colour: *colour,
                size: *size,
                font_type: *font_type,
                style: *style,
            },
            ServerRenderEffect::ChangeLineAttributes { id, attributes } => {
                Self::ChangeLineAttributes {
                    id: *id,
                    attributes: *attributes,
                }
            }
            ServerRenderEffect::ChangeLineAttributeValues {
                id,
                colour,
                width,
                line_art,
            } => Self::ChangeLineAttributeValues {
                id: *id,
                colour: *colour,
                width: *width,
                line_art: *line_art,
            },
            ServerRenderEffect::ChangeFillAttributes { id, attributes } => {
                Self::ChangeFillAttributes {
                    id: *id,
                    attributes: *attributes,
                }
            }
            ServerRenderEffect::ChangeFillAttributeValues {
                id,
                fill_type,
                colour,
                pattern,
            } => Self::ChangeFillAttributeValues {
                id: *id,
                fill_type: *fill_type,
                colour: *colour,
                pattern: *pattern,
            },
            ServerRenderEffect::ChangeActiveMask { mask } => Self::ChangeActiveMask { mask: *mask },
            ServerRenderEffect::ChangeSoftKeyMask {
                data_mask,
                soft_key_mask,
            } => Self::ChangeSoftKeyMask {
                data_mask: *data_mask,
                soft_key_mask: *soft_key_mask,
            },
            ServerRenderEffect::ChangeGenericAttribute {
                id,
                attribute_id,
                value,
            } => Self::ChangeGenericAttribute {
                id: *id,
                attribute_id: *attribute_id,
                value: *value,
            },
            ServerRenderEffect::ChangePriority { id, priority } => Self::ChangePriority {
                id: *id,
                priority: *priority,
            },
            ServerRenderEffect::ChangeListItem { list, index, item } => Self::ChangeListItem {
                list: *list,
                index: *index,
                item: *item,
            },
            ServerRenderEffect::LockUnlockMask {
                id,
                locked,
                timeout_ms,
            } => Self::LockUnlockMask {
                id: *id,
                locked: *locked,
                timeout_ms: *timeout_ms,
            },
            ServerRenderEffect::ExecuteMacro { id, .. } => Self::ExecuteMacro { id: *id },
            ServerRenderEffect::ChangeObjectLabel { id, label } => Self::ChangeObjectLabel {
                id: *id,
                label: *label,
            },
            ServerRenderEffect::ChangePolygonPoint { id, index, x, y } => {
                Self::ChangePolygonPoint {
                    id: *id,
                    index: *index,
                    x: *x,
                    y: *y,
                }
            }
            ServerRenderEffect::ChangePolygonScale { id, width, height } => {
                Self::ChangePolygonScale {
                    id: *id,
                    width: *width,
                    height: *height,
                }
            }
            ServerRenderEffect::SelectColourMap { id } => Self::SelectColourMap { id: *id },
            ServerRenderEffect::GraphicsContext {
                id,
                subcommand,
                payload,
            } => Self::GraphicsContext {
                id: *id,
                subcommand: *subcommand,
                payload: payload.clone(),
            },
            ServerRenderEffect::AudioSignal => Self::AudioSignal,
            ServerRenderEffect::SetAudioVolume { percent } => {
                Self::SetAudioVolume { percent: *percent }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AttributeRefKind {
    Font,
    Line,
    Fill,
}

fn object_labels_from_pool(pool: &ObjectPool) -> Result<HashMap<ObjectID, ObjectLabelState>> {
    let mut labels = HashMap::new();
    for obj in pool
        .objects()
        .iter()
        .filter(|obj| obj.r#type == ObjectType::ObjectLabelRef)
    {
        let body = obj.get_object_label_ref_body()?;
        for label in body.labels {
            labels.insert(
                label.labelled_object,
                ObjectLabelState {
                    string_variable: label.string_variable,
                    font_type: label.font_type,
                    graphic_designator: label.graphic_designator,
                },
            );
        }
    }
    Ok(labels)
}

fn object_label_state_is_valid(pool: &ObjectPool, label: ObjectLabelState) -> bool {
    if !is_standard_font_type(label.font_type) {
        return false;
    }
    if label.string_variable != ObjectID::NULL
        && !pool_reference_has_type(pool, label.string_variable, ObjectType::StringVariable)
    {
        return false;
    }
    label.graphic_designator == ObjectID::NULL
        || pool.find(label.graphic_designator).is_some_and(|object| {
            crate::isobus::vt::objects::is_object_label_graphic_representation_type(object.r#type)
        })
}

fn object_label_target_is_valid(pool: &ObjectPool, id: ObjectID) -> bool {
    pool.objects()
        .iter()
        .filter(|object| object.r#type == ObjectType::ObjectLabelRef)
        .any(|object| {
            object
                .get_object_label_ref_body()
                .is_ok_and(|body| body.labels.iter().any(|label| label.labelled_object == id))
        })
}

fn apply_server_object_state_to_pool(
    pool: &mut ObjectPool,
    state: &ServerObjectState,
) -> Result<()> {
    for (&id, &value) in &state.numeric_values {
        let _ = apply_numeric_value_to_pool(pool, id, value)?;
    }
    for (id, value) in &state.string_values {
        let _ = apply_string_value_to_pool(pool, *id, value)?;
    }
    for (&id, &visible) in &state.visibility {
        let _ = apply_hide_show_to_pool(pool, id, visible)?;
    }
    for (&data_mask, &soft_key_mask) in &state.soft_key_masks {
        let _ = apply_soft_key_mask_to_pool(pool, data_mask, soft_key_mask)?;
    }
    for (&(parent, child), &(x, y)) in &state.child_locations {
        let _ = apply_child_position_to_pool(pool, parent, child, i16::from(x), i16::from(y))?;
    }
    for (&(parent, child), &(x, y)) in &state.child_positions {
        let _ = apply_child_position_to_pool(pool, parent, child, x as i16, y as i16)?;
    }
    for (&id, &(width, height)) in &state.sizes {
        let _ = apply_size_to_pool(pool, id, width, height)?;
    }
    for (&id, &(width, height, line_direction)) in &state.endpoints {
        let _ = apply_end_point_to_pool(pool, id, width, height, line_direction)?;
    }
    for (&id, &priority) in &state.priorities {
        let _ = apply_priority_to_pool(pool, id, priority)?;
    }
    for (&id, &colour) in &state.background_colours {
        let _ = apply_background_to_pool(pool, id, colour)?;
    }
    for (&id, &attr) in &state.font_attributes {
        let _ = apply_attribute_ref_to_pool(pool, id, AttributeRefKind::Font, attr)?;
    }
    for (&id, &attr) in &state.line_attributes {
        let _ = apply_attribute_ref_to_pool(pool, id, AttributeRefKind::Line, attr)?;
    }
    for (&id, &attr) in &state.fill_attributes {
        let _ = apply_attribute_ref_to_pool(pool, id, AttributeRefKind::Fill, attr)?;
    }
    for (&(list, index), &item) in &state.list_items {
        let _ = apply_list_item_to_pool(pool, list, index as usize, item)?;
    }
    for (&(id, index), &(x, y)) in &state.polygon_points {
        let _ = apply_polygon_point_to_pool(pool, id, index as usize, x, y)?;
    }
    for (&id, &(width, height)) in &state.polygon_scales {
        let _ = apply_polygon_scale_to_pool(pool, id, width, height)?;
    }
    apply_retained_generic_attributes_to_pool(pool, &state.attributes)?;
    Ok(())
}

fn apply_retained_generic_attributes_to_pool(
    pool: &mut ObjectPool,
    attributes: &HashMap<(ObjectID, u8), u32>,
) -> Result<()> {
    let mut pending = attributes
        .iter()
        .map(|(&(id, attribute_id), &value)| (id, attribute_id, value))
        .collect::<Vec<_>>();

    while !pending.is_empty() {
        let mut deferred = Vec::new();
        let mut progressed = false;
        for (id, attribute_id, value) in pending {
            if generic_attribute_update_is_valid(pool, id, attribute_id, value) {
                let _ = apply_generic_attribute_to_pool(pool, id, attribute_id, value)?;
                progressed = true;
            } else {
                deferred.push((id, attribute_id, value));
            }
        }
        if !progressed {
            break;
        }
        pending = deferred;
    }

    Ok(())
}

fn apply_priority_to_pool(pool: &mut ObjectPool, id: ObjectID, priority: u8) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::AlarmMask || priority > 2 {
        return Ok(false);
    }
    let mut body = obj.get_alarm_mask_body()?;
    if body.priority == priority {
        return Ok(false);
    }
    body.priority = priority;
    obj.body = body.encode()?;
    Ok(true)
}

fn object_base_visible_state(obj: &VTObject) -> Option<bool> {
    match obj.r#type {
        ObjectType::Container => obj.get_container_body().ok().map(|body| !body.hidden),
        _ => Some(true),
    }
}

fn object_base_enabled_state(obj: &VTObject) -> Option<bool> {
    match obj.r#type {
        ObjectType::InputBoolean => obj
            .get_input_boolean_body()
            .ok()
            .map(|body| body.enabled != 0),
        ObjectType::InputString => Some(true),
        ObjectType::InputNumber => obj
            .get_input_number_body()
            .ok()
            .map(|body| body.options2 & 0x01 != 0),
        ObjectType::InputList => obj
            .get_input_list_body()
            .ok()
            .map(|body| body.options & 0x01 != 0),
        ObjectType::Button => obj
            .get_button_body()
            .ok()
            .map(|body| body.options & 0x10 == 0),
        ObjectType::Animation => obj.get_animation_body().ok().map(|body| body.enabled != 0),
        _ => Some(true),
    }
}

fn apply_hide_show_to_pool(pool: &mut ObjectPool, id: ObjectID, visible: bool) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::Container {
        return Ok(false);
    }
    let mut body = obj.get_container_body()?;
    let hidden = !visible;
    if body.hidden == hidden {
        return Ok(false);
    }
    body.hidden = hidden;
    Ok(replace_body_if_changed(obj, body.encode()))
}

fn apply_numeric_value_to_pool(pool: &mut ObjectPool, id: ObjectID, value: u32) -> Result<bool> {
    if !numeric_value_update_is_valid(pool, id, value) {
        return Ok(false);
    }
    if pool.set_number_variable_value(id, value) {
        return Ok(true);
    }
    if let Some(body) = pool
        .find(id)
        .filter(|obj| obj.r#type == ObjectType::InputList)
        .and_then(|obj| obj.get_input_list_body().ok())
    {
        return Ok(body.variable_reference != ObjectID::NULL
            && pool.set_number_variable_value(body.variable_reference, value));
    }
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    match obj.r#type {
        ObjectType::OutputNumber => {
            let mut body = obj.get_output_number_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = value;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputNumber => {
            let mut body = obj.get_input_number_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = value;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputBoolean => {
            let mut body = obj.get_input_boolean_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = u8::from(value != 0);
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputList => {
            let mut body = obj.get_input_list_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = low_u8(value);
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::OutputList => {
            let mut body = obj.get_output_list_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = value.min(u32::from(u8::MAX)) as u8;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::ObjectPointer => {
            let mut body = obj.get_object_pointer_body()?;
            body.value = ObjectID(low_u16(value));
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::ExternalObjectPointer => {
            let mut body = obj.get_external_object_pointer_body()?;
            body.external_reference_name = ObjectID(low_u16(value));
            body.external_object_id = ObjectID((value >> 16) as u16);
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::ScaledGraphic => {
            let mut body = obj.get_scaled_graphic_body()?;
            body.value = ObjectID(low_u16(value));
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Animation => {
            let mut body = obj.get_animation_body()?;
            body.value = low_u8(value);
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Meter => {
            let mut body = obj.get_meter_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = value.min(u32::from(u16::MAX)) as u16;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::LinearBarGraph => {
            let mut body = obj.get_linear_bar_graph_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = value.min(u32::from(u16::MAX)) as u16;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::ArchedBarGraph => {
            let mut body = obj.get_arched_bar_graph_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            body.value = value.min(u32::from(u16::MAX)) as u16;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        _ => Ok(false),
    }
}

fn numeric_value_update_is_valid(pool: &ObjectPool, id: ObjectID, value: u32) -> bool {
    let Some(obj) = pool.find(id) else {
        return true;
    };
    if !numeric_value_fits_object_width(obj.r#type, value) {
        return false;
    }
    match obj.r#type {
        ObjectType::InputBoolean => value <= 1,
        ObjectType::Animation => animation_numeric_value_is_valid(obj, value),
        ObjectType::ObjectPointer => {
            object_pointer_numeric_value_is_valid_for_context(pool, id, ObjectID(low_u16(value)))
        }
        ObjectType::ScaledGraphic => {
            scaled_graphic_value_source_is_valid(pool, ObjectID(low_u16(value)))
        }
        ObjectType::ExternalObjectPointer => {
            let reference_name = ObjectID(low_u16(value));
            reference_name == ObjectID::NULL
                || pool_reference_has_type(pool, reference_name, ObjectType::ExternalReferenceName)
        }
        _ => true,
    }
}

fn numeric_value_fits_object_width(object_type: ObjectType, value: u32) -> bool {
    match object_type {
        ObjectType::InputBoolean
        | ObjectType::InputList
        | ObjectType::OutputList
        | ObjectType::Animation => value <= u32::from(u8::MAX),
        ObjectType::Meter
        | ObjectType::LinearBarGraph
        | ObjectType::ArchedBarGraph
        | ObjectType::ObjectPointer
        | ObjectType::ScaledGraphic => value <= u32::from(u16::MAX),
        ObjectType::ExternalObjectPointer
        | ObjectType::InputNumber
        | ObjectType::OutputNumber
        | ObjectType::NumberVariable => true,
        _ => false,
    }
}

fn generic_attribute_update_is_valid(
    pool: &ObjectPool,
    id: ObjectID,
    attribute_id: u8,
    value: u32,
) -> bool {
    let Some(obj) = pool.find(id) else {
        return true;
    };
    if !vt_change_attribute_id_is_supported(obj.r#type, attribute_id) {
        return false;
    }
    if change_attribute_targets_one_byte_field(obj.r#type, attribute_id)
        && value > u32::from(u8::MAX)
    {
        return false;
    }
    if change_attribute_targets_two_byte_field(obj.r#type, attribute_id)
        && value > u32::from(u16::MAX)
    {
        return false;
    }
    if obj.r#type == ObjectType::FillAttributes
        && !fill_attribute_generic_change_is_valid(pool, obj, attribute_id, value)
    {
        return false;
    }
    let reference = ObjectID(low_u16(value));
    match (obj.r#type, attribute_id) {
        (ObjectType::DataMask | ObjectType::AlarmMask, 2) => {
            reference == ObjectID::NULL
                || pool_reference_has_type(pool, reference, ObjectType::SoftKeyMask)
        }
        (ObjectType::AlarmMask, 3) => value <= 2,
        (ObjectType::AlarmMask, 4) => value <= 3,
        (ObjectType::WindowMask, 1) => (1..=2).contains(&value),
        (ObjectType::WindowMask, 2) => (1..=6).contains(&value),
        (ObjectType::WindowMask, 3) => {
            value <= 18
                && obj.get_window_mask_body().is_ok_and(|body| {
                    window_mask_required_objects_match(pool, &body, low_u8(value))
                })
        }
        (ObjectType::WindowMask, 5) | (ObjectType::KeyGroup, 1) => value <= 0x03,
        (ObjectType::WindowMask, 6 | 7) => window_mask_text_reference_is_valid(pool, reference),
        (ObjectType::WindowMask, 8) => window_mask_icon_reference_is_valid(pool, reference),
        (ObjectType::KeyGroup, 2) => key_group_name_reference_is_valid(pool, reference),
        (ObjectType::KeyGroup, 3) => key_group_icon_reference_is_valid(pool, reference),
        (ObjectType::ExternalObjectDefinition | ObjectType::ExternalReferenceName, 1) => {
            value <= 0x01
        }
        (ObjectType::ExternalObjectPointer, 1) => {
            external_object_pointer_default_is_valid_for_context(pool, obj.id, reference)
        }
        (ObjectType::ExternalObjectPointer, 2) => {
            reference == ObjectID::NULL
                || pool_reference_has_type(pool, reference, ObjectType::ExternalReferenceName)
        }
        (ObjectType::OutputString, 4)
        | (ObjectType::OutputNumber, 4)
        | (ObjectType::InputString, 4)
        | (ObjectType::InputNumber, 4) => {
            reference != ObjectID::NULL
                && pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
        }
        (ObjectType::OutputString, 5) => value <= 0x03,
        (ObjectType::OutputString, 6) | (ObjectType::InputString, 7) => {
            pool_reference_has_type(pool, reference, ObjectType::StringVariable)
        }
        (ObjectType::OutputString, 7) | (ObjectType::InputString, 8) => {
            value <= u32::from(u8::MAX) && text_justification_is_valid(low_u8(value))
        }
        (ObjectType::InputString, 5) => pool_reference_has_any_type(
            pool,
            reference,
            &[
                ObjectType::InputAttributes,
                ObjectType::ExtendedInputAttributes,
            ],
        ),
        (ObjectType::InputString, 6) => value <= 0x07,
        (ObjectType::Animation, 5) => value <= 1,
        (ObjectType::InputBoolean, 3) => {
            reference != ObjectID::NULL
                && pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
        }
        (ObjectType::FillAttributes, 3) => {
            pool_reference_has_type(pool, reference, ObjectType::PictureGraphic)
        }
        (ObjectType::InputBoolean, 4)
        | (ObjectType::OutputNumber, 6)
        | (ObjectType::OutputList, 3)
        | (ObjectType::InputNumber, 6)
        | (ObjectType::InputList, 3)
        | (ObjectType::Meter, 11)
        | (ObjectType::LinearBarGraph, 9)
        | (ObjectType::LinearBarGraph, 10)
        | (ObjectType::ArchedBarGraph, 11)
        | (ObjectType::ArchedBarGraph, 12) => {
            pool_reference_has_type(pool, reference, ObjectType::NumberVariable)
        }
        (ObjectType::OutputNumber, 5) | (ObjectType::InputNumber, 5) => value <= 0x0F,
        (ObjectType::OutputNumber, 9) | (ObjectType::InputNumber, 11) => value <= 7,
        (ObjectType::OutputNumber, 10) | (ObjectType::InputNumber, 12) => value <= 1,
        (ObjectType::OutputNumber, 11) | (ObjectType::InputNumber, 13) => {
            value <= u32::from(u8::MAX) && text_justification_is_valid(low_u8(value))
        }
        (ObjectType::OutputNumber, 8) | (ObjectType::InputNumber, 10) => {
            f32::from_bits(value).is_finite()
        }
        (ObjectType::InputNumber, 7) => obj
            .get_input_number_body()
            .is_ok_and(|body| (value as i32) <= body.max_value),
        (ObjectType::InputNumber, 8) => obj
            .get_input_number_body()
            .is_ok_and(|body| body.min_value <= value as i32),
        (ObjectType::FontAttributes, 2) => obj.get_font_attributes_body().is_ok_and(|body| {
            value <= u32::from(u8::MAX)
                && is_standard_font_size_for_style(low_u8(value), body.font_style)
        }),
        (ObjectType::FontAttributes, 3) => {
            value <= u32::from(u8::MAX) && is_standard_font_type(low_u8(value))
        }
        (ObjectType::FontAttributes, 4) => obj.get_font_attributes_body().is_ok_and(|body| {
            value <= u32::from(u8::MAX)
                && is_standard_font_size_for_style(body.font_size, low_u8(value))
        }),
        (ObjectType::FillAttributes, 1) => value <= 3,
        (ObjectType::Line | ObjectType::Rectangle | ObjectType::Ellipse, 1) => {
            pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::Polygon, 3) => {
            reference != ObjectID::NULL
                && pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::Line, 4) => value <= 1,
        (ObjectType::Rectangle, 4) => value <= 0x0F,
        (ObjectType::Rectangle, 5) | (ObjectType::Ellipse, 7) | (ObjectType::Polygon, 4) => {
            pool_reference_has_type(pool, reference, ObjectType::FillAttributes)
        }
        (ObjectType::Ellipse, 4) | (ObjectType::Polygon, 5) => value <= 3,
        (ObjectType::Ellipse, 5 | 6)
        | (ObjectType::Meter, 7 | 8)
        | (ObjectType::ArchedBarGraph, 6 | 7) => value <= 180,
        (ObjectType::Meter, 5) => value <= 0x01,
        (ObjectType::Meter, 9 | 10) => value <= u32::from(u16::MAX),
        (ObjectType::LinearBarGraph, 5) => value <= 0x3F,
        (ObjectType::LinearBarGraph, 7 | 8) => value <= u32::from(u16::MAX),
        (ObjectType::ArchedBarGraph, 5) => value <= 0x1F,
        (ObjectType::ArchedBarGraph, 9 | 10) => value <= u32::from(u16::MAX),
        (ObjectType::Button, 6) => value <= 0x3F,
        (ObjectType::PictureGraphic, 2) => value <= 0x07,
        (ObjectType::ScaledGraphic, 3) => {
            value <= u32::from(u8::MAX) && scaled_graphic_scale_type_is_valid(low_u8(value))
        }
        (ObjectType::ScaledGraphic, 4) => value <= 0x01,
        (ObjectType::ScaledGraphic, 5) => scaled_graphic_value_source_is_valid(pool, reference),
        (ObjectType::Animation, 4 | 6 | 7 | 8) => {
            animation_attribute_index_is_valid(obj, attribute_id, value)
        }
        (ObjectType::Animation, 9) => value <= 0x07,
        (ObjectType::GraphicContext, 1 | 2) => value <= 32767,
        (ObjectType::GraphicContext, 3 | 4 | 8 | 9) => value <= u32::from(u16::MAX),
        (ObjectType::GraphicContext, 7) => {
            let zoom = f32::from_bits(value);
            zoom.is_finite() && (-32.0..=32.0).contains(&zoom)
        }
        (ObjectType::GraphicContext, 12) => {
            reference == ObjectID::NULL
                || pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
        }
        (ObjectType::GraphicContext, 13) => {
            reference == ObjectID::NULL
                || pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::GraphicContext, 14) => {
            reference == ObjectID::NULL
                || pool_reference_has_type(pool, reference, ObjectType::FillAttributes)
        }
        (ObjectType::GraphicContext, 15) => value <= 2,
        (ObjectType::GraphicContext, 16) => value <= 0x03,
        (ObjectType::ColourPalette, 1) => value == 0,
        (ObjectType::WorkingSetSpecialControls, 2) => {
            pool_reference_has_type(pool, reference, ObjectType::ColourMap)
        }
        (ObjectType::WorkingSetSpecialControls, 3) => {
            pool_reference_has_type(pool, reference, ObjectType::ColourPalette)
        }
        _ => true,
    }
}

fn animation_attribute_index_is_valid(obj: &VTObject, attribute_id: u8, value: u32) -> bool {
    let child_count = obj.children_pos.len();
    if child_count == 0 || value > u32::from(u8::MAX) {
        return false;
    }
    let Ok(body) = obj.get_animation_body() else {
        return false;
    };
    let candidate = low_u8(value);
    match attribute_id {
        4 => candidate == u8::MAX || usize::from(candidate) < child_count,
        6 => candidate <= body.last_child_index && usize::from(body.last_child_index) < child_count,
        7 => usize::from(candidate) < child_count,
        8 => body.first_child_index <= candidate && usize::from(candidate) < child_count,
        _ => true,
    }
}

fn animation_numeric_value_is_valid(obj: &VTObject, value: u32) -> bool {
    if value == u32::from(u8::MAX) {
        return true;
    }
    value <= u32::from(u8::MAX) && (value as usize) < obj.children_pos.len()
}

fn apply_string_value_to_pool(pool: &mut ObjectPool, id: ObjectID, text: &str) -> Result<bool> {
    if pool.set_string_variable_value(id, text.as_bytes().to_vec()) {
        return Ok(true);
    }
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    match obj.r#type {
        ObjectType::OutputString => {
            let mut body = obj.get_output_string_body()?;
            if body.variable_reference != ObjectID::NULL {
                return Ok(false);
            }
            let max_len = body.value.len();
            if text.len() > max_len {
                return Ok(false);
            }
            body.value = padded_vt_string_bytes(text, max_len);
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputAttributes => {
            let mut body = obj.get_input_attributes_body()?;
            let max_len = body.validation_string.len();
            if text.len() > max_len {
                return Ok(false);
            }
            body.validation_string = padded_vt_string_bytes(text, max_len);
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        _ => Ok(false),
    }
}

fn padded_vt_string_bytes(text: &str, max_len: usize) -> Vec<u8> {
    let mut value = text.as_bytes().to_vec();
    value.resize(max_len, b' ');
    value
}

fn replace_body_if_changed(obj: &mut VTObject, body: impl Into<Vec<u8>>) -> bool {
    let body = body.into();
    if obj.body == body {
        false
    } else {
        obj.body = body;
        true
    }
}

fn apply_child_position_to_pool(
    pool: &mut ObjectPool,
    parent: ObjectID,
    child: ObjectID,
    x: i16,
    y: i16,
) -> Result<bool> {
    if pool.find(child).is_none() {
        return Ok(false);
    }
    let Some(parent_obj) = pool.find_mut(parent) else {
        return Ok(false);
    };
    if let Some(pos) = parent_obj
        .children_pos
        .iter_mut()
        .find(|pos| pos.id == child)
    {
        if pos.x == x && pos.y == y {
            return Ok(false);
        }
        pos.x = x;
        pos.y = y;
        return Ok(true);
    }
    if parent_obj.children.contains(&child) {
        parent_obj.children_pos.push(ChildRef::new(child, x, y));
        return Ok(true);
    }
    Ok(false)
}

fn apply_soft_key_mask_to_pool(
    pool: &mut ObjectPool,
    data_mask: ObjectID,
    soft_key_mask: ObjectID,
) -> Result<bool> {
    if soft_key_mask != ObjectID::NULL {
        match pool.find(soft_key_mask).map(|obj| obj.r#type) {
            Some(ObjectType::SoftKeyMask) => {}
            _ => return Ok(false),
        }
    }
    let Some(obj) = pool.find_mut(data_mask) else {
        return Ok(false);
    };
    match obj.r#type {
        ObjectType::DataMask => {
            let mut body = obj.get_data_mask_body()?;
            if body.soft_key_mask == soft_key_mask {
                return Ok(false);
            }
            body.soft_key_mask = soft_key_mask;
            obj.body = body.encode();
            Ok(true)
        }
        ObjectType::AlarmMask => {
            let mut body = obj.get_alarm_mask_body()?;
            if body.soft_key_mask == soft_key_mask {
                return Ok(false);
            }
            body.soft_key_mask = soft_key_mask;
            obj.body = body.encode()?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn apply_background_to_pool(pool: &mut ObjectPool, id: ObjectID, colour: u8) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    match obj.r#type {
        ObjectType::DataMask => {
            let mut body = obj.get_data_mask_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::AlarmMask => {
            let mut body = obj.get_alarm_mask_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::SoftKeyMask => {
            let mut body = obj.get_soft_key_mask_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::Key => {
            let mut body = obj.get_key_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::WindowMask => {
            let mut body = obj.get_window_mask_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Button => {
            let mut body = obj.get_button_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::InputBoolean => {
            let mut body = obj.get_input_boolean_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputString => {
            let mut body = obj.get_input_string_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputNumber => {
            let mut body = obj.get_input_number_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::OutputString => {
            let mut body = obj.get_output_string_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::OutputNumber => {
            let mut body = obj.get_output_number_body()?;
            body.background_color = colour;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        _ => Ok(false),
    }
}

fn apply_end_point_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    width: u16,
    height: u16,
    line_direction: u8,
) -> Result<bool> {
    if line_direction > 1 {
        return Ok(false);
    }
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::Line {
        return Ok(false);
    }
    let mut body = obj.get_output_line_body()?;
    if body.width == width && body.height == height && body.line_direction == line_direction {
        return Ok(false);
    }
    body.width = width;
    body.height = height;
    body.line_direction = line_direction;
    obj.body = body.encode()?;
    Ok(true)
}

fn apply_size_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    width: u16,
    height: u16,
) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    match obj.r#type {
        ObjectType::Container => {
            let mut body = obj.get_container_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::Button => {
            let mut body = obj.get_button_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()))
        }
        ObjectType::OutputString => {
            let mut body = obj.get_output_string_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::OutputNumber => {
            let mut body = obj.get_output_number_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputString => {
            let mut body = obj.get_input_string_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputNumber => {
            let mut body = obj.get_input_number_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputList => {
            let mut body = obj.get_input_list_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::OutputList => {
            let mut body = obj.get_output_list_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Line => {
            let mut body = obj.get_output_line_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Rectangle => {
            let mut body = obj.get_output_rectangle_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Ellipse => {
            let mut body = obj.get_output_ellipse_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Polygon => {
            let mut body = obj.get_output_polygon_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::LinearBarGraph => {
            let mut body = obj.get_linear_bar_graph_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::ArchedBarGraph => {
            let mut body = obj.get_arched_bar_graph_body()?;
            body.width = width;
            body.height = height;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::InputBoolean => {
            let mut body = obj.get_input_boolean_body()?;
            body.width = width;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        ObjectType::Meter => {
            let mut body = obj.get_meter_body()?;
            body.width = width;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        _ => Ok(false),
    }
}

fn apply_attribute_ref_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    kind: AttributeRefKind,
    attr: ObjectID,
) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    match (kind, obj.r#type) {
        (AttributeRefKind::Font, ObjectType::OutputString) => {
            let mut body = obj.get_output_string_body()?;
            body.font_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Font, ObjectType::OutputNumber) => {
            let mut body = obj.get_output_number_body()?;
            body.font_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Font, ObjectType::InputString) => {
            let mut body = obj.get_input_string_body()?;
            body.font_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Font, ObjectType::InputNumber) => {
            let mut body = obj.get_input_number_body()?;
            body.font_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Font, ObjectType::InputBoolean) => {
            let mut body = obj.get_input_boolean_body()?;
            body.foreground = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Line, ObjectType::Line) => {
            let mut body = obj.get_output_line_body()?;
            body.line_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Line, ObjectType::Rectangle) => {
            let mut body = obj.get_output_rectangle_body()?;
            body.line_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Line, ObjectType::Ellipse) => {
            let mut body = obj.get_output_ellipse_body()?;
            body.line_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Line, ObjectType::Polygon) => {
            let mut body = obj.get_output_polygon_body()?;
            body.line_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Fill, ObjectType::Rectangle) => {
            let mut body = obj.get_output_rectangle_body()?;
            body.fill_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Fill, ObjectType::Ellipse) => {
            let mut body = obj.get_output_ellipse_body()?;
            body.fill_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        (AttributeRefKind::Fill, ObjectType::Polygon) => {
            let mut body = obj.get_output_polygon_body()?;
            body.fill_attributes = attr;
            Ok(replace_body_if_changed(obj, body.encode()?))
        }
        _ => Ok(false),
    }
}

fn apply_font_attribute_values_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    colour: u8,
    size: u8,
    font_type: u8,
    style: u8,
) -> Result<bool> {
    if !is_standard_font_size_for_style(size, style) || !is_standard_font_type(font_type) {
        return Ok(false);
    }
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::FontAttributes {
        return Ok(false);
    }
    let mut body = obj.get_font_attributes_body()?;
    body.font_color = colour;
    body.font_size = size;
    body.font_type = font_type;
    body.font_style = style;
    Ok(replace_body_if_changed(obj, body.encode()))
}

fn apply_line_attribute_values_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    colour: u8,
    width: u8,
    line_art: u16,
) -> Result<bool> {
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::LineAttributes {
        return Ok(false);
    }
    let mut body = obj.get_line_attributes_body()?;
    body.line_color = colour;
    body.line_width = width;
    body.line_art = line_art;
    Ok(replace_body_if_changed(obj, body.encode()))
}

fn apply_fill_attribute_values_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    fill_type: u8,
    colour: u8,
    pattern: ObjectID,
) -> Result<bool> {
    if fill_type > 3 {
        return Ok(false);
    }
    if pattern != ObjectID::NULL
        && !pool
            .find(pattern)
            .is_some_and(|object| object.r#type == ObjectType::PictureGraphic)
    {
        return Ok(false);
    }
    if fill_type == 3
        && pattern != ObjectID::NULL
        && !pool
            .find(pattern)
            .and_then(|object| object.get_picture_graphic_body().ok())
            .is_some_and(|body| picture_graphic_fill_pattern_buffer_is_valid(&body))
    {
        return Ok(false);
    }
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    if obj.r#type != ObjectType::FillAttributes {
        return Ok(false);
    }
    let mut body = obj.get_fill_attributes_body()?;
    body.fill_type = fill_type;
    body.fill_color = colour;
    body.fill_pattern = pattern;
    Ok(replace_body_if_changed(obj, body.encode()?))
}
