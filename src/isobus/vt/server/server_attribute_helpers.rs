fn validate_server_advertisement(
    screen_width: u16,
    screen_height: u16,
    vt_version: u16,
) -> Result<()> {
    VTServerConfig {
        screen_width,
        screen_height,
        vt_version,
        ..VTServerConfig::default()
    }
    .validate()
}

fn parse_label(data: &[u8]) -> String {
    let mut s = String::new();
    for &b in data.iter().skip(1).take(7) {
        if b != b' ' && b != 0 {
            s.push(b as char);
        }
    }
    s
}

#[inline]
fn u16_le(buf: &[u8]) -> u16 {
    (buf[0] as u16) | ((buf[1] as u16) << 8)
}

#[inline]
fn u32_le(buf: &[u8]) -> u32 {
    (buf[0] as u32) | ((buf[1] as u32) << 8) | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24)
}

#[inline]
fn is_fixed_vt_payload(data: &[u8]) -> bool {
    data.len() == 8
}

#[inline]
fn has_ff_tail(data: &[u8], used: usize) -> bool {
    used <= data.len() && data[used..].iter().all(|&byte| byte == 0xFF)
}

#[inline]
fn is_parameterless_vt_request(data: &[u8]) -> bool {
    is_fixed_vt_payload(data) && has_ff_tail(data, 1)
}

#[inline]
fn is_canonical_bool(byte: u8) -> bool {
    byte <= 1
}

fn numeric_value_width_for_type(object_type: ObjectType) -> Option<usize> {
    match object_type {
        ObjectType::InputBoolean
        | ObjectType::InputList
        | ObjectType::OutputList
        | ObjectType::Animation => Some(1),
        ObjectType::Meter
        | ObjectType::LinearBarGraph
        | ObjectType::ArchedBarGraph
        | ObjectType::ObjectPointer
        | ObjectType::ScaledGraphic => Some(2),
        ObjectType::ExternalObjectPointer => Some(4),
        ObjectType::InputNumber | ObjectType::OutputNumber | ObjectType::NumberVariable => Some(4),
        _ => None,
    }
}

/// The attribute ID that carries an object's value for the types whose value
/// is settable via *both* Change Numeric Value and Change Attribute. Used to
/// keep the two coherent (Change Numeric Value clears any prior Change
/// Attribute overlay for this AID).
fn value_attribute_id_for_type(object_type: ObjectType) -> Option<u8> {
    match object_type {
        ObjectType::InputBoolean => Some(5),
        ObjectType::InputList | ObjectType::OutputList => Some(4),
        ObjectType::Meter | ObjectType::LinearBarGraph => Some(12),
        _ => None,
    }
}

fn numeric_value_is_valid(pool: &ObjectPool, object: &VTObject, value: u32) -> bool {
    match object.r#type {
        ObjectType::InputBoolean => value <= 1,
        ObjectType::Animation => animation_numeric_value_is_valid(object, value),
        ObjectType::ObjectPointer => object_pointer_numeric_value_is_valid_for_context(
            pool,
            object.id,
            ObjectID(value as u16),
        ),
        ObjectType::ExternalObjectPointer => {
            let external_reference_name = ObjectID(value as u16);
            external_reference_name == ObjectID::NULL
                || pool_reference_has_type(
                    pool,
                    external_reference_name,
                    ObjectType::ExternalReferenceName,
                )
        }
        ObjectType::ScaledGraphic => {
            scaled_graphic_value_source_is_valid(pool, ObjectID(value as u16))
        }
        _ => true,
    }
}

fn build_select_input_object_response(id: ObjectID, response_code: u8, error_bits: u8) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    data[0] = cmd::SELECT_INPUT_OBJECT_COMMAND;
    data[1..3].copy_from_slice(&id.to_le_bytes());
    data[3] = if error_bits == 0 { response_code } else { 0 };
    data[4] = error_bits;
    data
}

fn select_input_object_effective_enabled(object: &VTObject, state: &ServerObjectState) -> bool {
    state
        .enable_state
        .get(&object.id)
        .copied()
        .or_else(|| select_input_object_base_enabled(object))
        .unwrap_or(true)
}

fn select_input_object_base_enabled(object: &VTObject) -> Option<bool> {
    match object.r#type {
        ObjectType::InputBoolean => object
            .get_input_boolean_body()
            .ok()
            .map(|body| body.enabled != 0),
        ObjectType::InputString | ObjectType::Key => Some(true),
        ObjectType::InputNumber => object
            .get_input_number_body()
            .ok()
            .map(|body| body.options2 & 0x01 != 0),
        ObjectType::InputList => object
            .get_input_list_body()
            .ok()
            .map(|body| body.options & 0x01 != 0),
        ObjectType::Button => object
            .get_button_body()
            .ok()
            .map(|body| body.options & 0x10 == 0),
        _ => None,
    }
}

fn select_input_object_visible_on_active_mask(
    pool: &ObjectPool,
    state: &ServerObjectState,
    target: ObjectID,
) -> bool {
    let Some(target_object) = pool.find(target) else {
        return false;
    };
    if target_object.r#type == ObjectType::Key
        && select_input_key_visible_on_active_soft_key_mask(pool, state, target)
    {
        return true;
    }
    let active_mask = select_input_active_mask(pool, state);
    active_mask != ObjectID::NULL
        && select_input_reachable_from(pool, state, active_mask, target, &mut Vec::new())
}

fn select_input_active_mask(pool: &ObjectPool, state: &ServerObjectState) -> ObjectID {
    if state.active_data_mask != ObjectID::default()
        && pool.find(state.active_data_mask).is_some_and(|object| {
            matches!(object.r#type, ObjectType::DataMask | ObjectType::AlarmMask)
        })
    {
        return state.active_data_mask;
    }
    if let Some(ws) = pool
        .objects()
        .iter()
        .find(|obj| obj.r#type == ObjectType::WorkingSet)
    {
        if let Ok(body) = ws.get_working_set_body()
            && body.active_mask != ObjectID::NULL
            && pool.find(body.active_mask).is_some_and(|object| {
                matches!(object.r#type, ObjectType::DataMask | ObjectType::AlarmMask)
            })
        {
            return body.active_mask;
        }
        if let Some(mask) = ws.children.iter().copied().find(|&child| {
            pool.find(child).is_some_and(|object| {
                matches!(object.r#type, ObjectType::DataMask | ObjectType::AlarmMask)
            })
        }) {
            return mask;
        }
    }
    ObjectID::NULL
}

fn select_input_active_soft_key_mask(
    pool: &ObjectPool,
    state: &ServerObjectState,
) -> Option<ObjectID> {
    let active_mask = select_input_active_mask(pool, state);
    if let Some(soft_key_mask) = state.soft_key_masks.get(&active_mask).copied() {
        return (soft_key_mask != ObjectID::NULL).then_some(soft_key_mask);
    }
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

fn select_input_key_visible_on_active_soft_key_mask(
    pool: &ObjectPool,
    state: &ServerObjectState,
    target: ObjectID,
) -> bool {
    let Some(soft_key_mask) = select_input_active_soft_key_mask(pool, state) else {
        return false;
    };
    select_input_reachable_from(pool, state, soft_key_mask, target, &mut Vec::new())
}

fn select_input_reachable_from(
    pool: &ObjectPool,
    state: &ServerObjectState,
    current: ObjectID,
    target: ObjectID,
    path: &mut Vec<ObjectID>,
) -> bool {
    if current == target {
        return true;
    }
    if path.contains(&current) {
        return false;
    }
    let Some(object) = pool.find(current) else {
        return false;
    };
    if current != target
        && object.r#type == ObjectType::Container
        && !select_input_container_visible(object, state)
    {
        return false;
    }
    path.push(current);
    let reachable = object
        .children
        .iter()
        .copied()
        .chain(object.children_pos.iter().map(|child| child.id))
        .any(|child| select_input_reachable_from(pool, state, child, target, path))
        || (object.r#type == ObjectType::ObjectPointer
            && object.get_object_pointer_body().is_ok_and(|body| {
                body.value != ObjectID::NULL
                    && select_input_reachable_from(pool, state, body.value, target, path)
            }));
    path.pop();
    reachable
}

fn select_input_container_visible(object: &VTObject, state: &ServerObjectState) -> bool {
    state
        .visibility
        .get(&object.id)
        .copied()
        .or_else(|| object.get_container_body().ok().map(|body| !body.hidden))
        .unwrap_or(true)
}

fn numeric_value_payload_width_is_canonical(data: &[u8], value_width: usize) -> bool {
    match value_width {
        1 => data.len() == 8 && data[5..8].iter().all(|&byte| byte == 0),
        2 => data.len() == 8 && data[6..8].iter().all(|&byte| byte == 0),
        4 => data.len() == 8,
        _ => false,
    }
}

fn initialise_working_set_special_controls(pool: &ObjectPool, state: &mut ServerObjectState) {
    let Some(body) = pool
        .objects()
        .iter()
        .find(|obj| obj.r#type == ObjectType::WorkingSetSpecialControls)
        .and_then(|obj| obj.get_working_set_special_controls_body().ok())
    else {
        return;
    };
    state.selected_colour_map = body.colour_map;
    state.selected_colour_palette = Some(body.colour_palette);
}

fn vt_change_attribute_value_is_valid(
    pool: &ObjectPool,
    state: &ServerObjectState,
    object: &VTObject,
    attribute_id: u8,
    value: u32,
) -> bool {
    if change_attribute_targets_one_byte_field(object.r#type, attribute_id)
        && value > u32::from(u8::MAX)
    {
        return false;
    }
    if change_attribute_targets_two_byte_field(object.r#type, attribute_id)
        && value > u32::from(u16::MAX)
    {
        return false;
    }
    let reference = ObjectID(value as u16);
    match (object.r#type, attribute_id) {
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
                && object.get_window_mask_body().is_ok_and(|body| {
                    vt_window_mask_required_objects_match(pool, &body, value as u8)
                })
        }
        (ObjectType::WindowMask, 5) | (ObjectType::KeyGroup, 1) => value <= 0x03,
        (ObjectType::WindowMask, 6 | 7) => window_mask_text_reference_is_valid(pool, reference),
        (ObjectType::WindowMask, 8) => window_mask_icon_reference_is_valid(pool, reference),
        (ObjectType::KeyGroup, 2) => key_group_name_reference_is_valid(pool, reference),
        (ObjectType::KeyGroup, 3) => key_group_icon_reference_is_valid(pool, reference),
        (ObjectType::ExternalObjectPointer, 1) => {
            external_object_pointer_default_is_valid_for_context(pool, object.id, reference)
        }
        (ObjectType::ExternalObjectDefinition | ObjectType::ExternalReferenceName, 1) => {
            value <= 0x01
        }
        (ObjectType::ExternalObjectPointer, 2) => {
            reference == ObjectID::NULL
                || pool_reference_has_type(pool, reference, ObjectType::ExternalReferenceName)
        }
        (ObjectType::OutputString, 5) => value <= 0x03,
        (ObjectType::OutputString, 7) | (ObjectType::InputString, 8) => {
            value <= u32::from(u8::MAX) && text_justification_is_valid(value as u8)
        }
        (ObjectType::OutputNumber, 5) | (ObjectType::InputNumber, 5) => value <= 0x0F,
        (ObjectType::OutputNumber, 9) | (ObjectType::InputNumber, 11) => value <= 7,
        (ObjectType::OutputNumber, 10) | (ObjectType::InputNumber, 12) => value <= 1,
        (ObjectType::OutputNumber, 11) | (ObjectType::InputNumber, 13) => {
            value <= u32::from(u8::MAX) && text_justification_is_valid(value as u8)
        }
        (ObjectType::OutputNumber, 8) | (ObjectType::InputNumber, 10) => {
            f32::from_bits(value).is_finite()
        }
        (ObjectType::OutputString | ObjectType::OutputNumber | ObjectType::InputString, 4)
        | (ObjectType::InputNumber, 4) => {
            reference != ObjectID::NULL
                && pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
        }
        (ObjectType::InputString, 5) => pool_reference_has_any_type(
            pool,
            reference,
            &[
                ObjectType::InputAttributes,
                ObjectType::ExtendedInputAttributes,
            ],
        ),
        (ObjectType::OutputString, 6) => {
            pool_reference_has_type(pool, reference, ObjectType::StringVariable)
        }
        (ObjectType::InputString, 7) => {
            pool_reference_has_type(pool, reference, ObjectType::StringVariable)
        }
        (ObjectType::OutputNumber, 6)
        | (ObjectType::OutputList, 3)
        | (ObjectType::InputNumber, 6)
        | (ObjectType::InputList, 3)
        | (ObjectType::InputBoolean, 4)
        | (ObjectType::Meter, 11)
        | (ObjectType::LinearBarGraph, 9)
        | (ObjectType::LinearBarGraph, 10)
        | (ObjectType::ArchedBarGraph, 11)
        | (ObjectType::ArchedBarGraph, 12) => {
            pool_reference_has_type(pool, reference, ObjectType::NumberVariable)
        }
        (ObjectType::InputBoolean, 3) => {
            reference != ObjectID::NULL
                && pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
        }
        (ObjectType::FillAttributes, 3) => {
            fill_attribute_pattern_change_is_valid(pool, state, object, reference)
        }
        (ObjectType::PictureGraphic, 2) => value <= 0x07,
        (ObjectType::InputString, 6) => value <= 0x07,
        (ObjectType::InputNumber, 7) => object.get_input_number_body().is_ok_and(|body| {
            let current_max = current_i32_attribute(state, object.id, 8, body.max_value);
            (value as i32) <= current_max
        }),
        (ObjectType::InputNumber, 8) => object.get_input_number_body().is_ok_and(|body| {
            let current_min = current_i32_attribute(state, object.id, 7, body.min_value);
            current_min <= value as i32
        }),
        (ObjectType::FontAttributes, 2) => object.get_font_attributes_body().is_ok_and(|body| {
            let current_style = current_u8_attribute(state, object.id, 4, body.font_style);
            value <= u32::from(u8::MAX)
                && is_standard_font_size_for_style(value as u8, current_style)
        }),
        (ObjectType::FontAttributes, 3) => {
            value <= u32::from(u8::MAX) && is_standard_font_type(value as u8)
        }
        (ObjectType::FontAttributes, 4) => object.get_font_attributes_body().is_ok_and(|body| {
            let current_size = current_u8_attribute(state, object.id, 2, body.font_size);
            value <= u32::from(u8::MAX)
                && is_standard_font_size_for_style(current_size, value as u8)
        }),
        (ObjectType::FillAttributes, 1) => {
            fill_attribute_type_change_is_valid(pool, state, object, value)
        }
        (ObjectType::Line | ObjectType::Rectangle | ObjectType::Ellipse, 1) => {
            pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::Polygon, 3) => {
            reference != ObjectID::NULL
                && pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::Line, 4) => value <= 1,
        (ObjectType::Rectangle, 4) => value <= 0x0F,
        (ObjectType::Ellipse, 4) => value <= 3,
        (ObjectType::Ellipse, 5 | 6) => value <= 180,
        (ObjectType::Rectangle, 5) | (ObjectType::Ellipse, 7) | (ObjectType::Polygon, 4) => {
            pool_reference_has_type(pool, reference, ObjectType::FillAttributes)
        }
        (ObjectType::Polygon, 5) => value <= 3,
        (ObjectType::Meter, 5) => value <= 0x01,
        (ObjectType::Meter, 7 | 8) => value <= 180,
        (ObjectType::Meter, 9 | 10) => value <= u32::from(u16::MAX),
        (ObjectType::LinearBarGraph, 5) => value <= 0x3F,
        (ObjectType::LinearBarGraph, 7 | 8) => value <= u32::from(u16::MAX),
        (ObjectType::ArchedBarGraph, 5) => value <= 0x1F,
        (ObjectType::ArchedBarGraph, 6 | 7) => value <= 180,
        (ObjectType::ArchedBarGraph, 9 | 10) => value <= u32::from(u16::MAX),
        (ObjectType::Button, 6) => value <= 0x3F,
        (ObjectType::ScaledGraphic, 3) => {
            value <= u32::from(u8::MAX) && scaled_graphic_scale_type_is_valid(value as u8)
        }
        (ObjectType::ScaledGraphic, 4) => value <= 0x01,
        (ObjectType::ScaledGraphic, 5) => scaled_graphic_value_source_is_valid(pool, reference),
        (ObjectType::Animation, 4 | 6 | 7 | 8) => {
            vt_animation_attribute_index_is_valid(state, object, attribute_id, value)
        }
        (ObjectType::Animation, 5) => value <= 1,
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

fn vt_retained_change_attribute_value(
    object: &VTObject,
    attribute_id: u8,
    value: u32,
) -> Option<u32> {
    match (object.r#type, attribute_id) {
        (ObjectType::Button, 6) => object.get_button_body().ok().map(|body| {
            // ISO 11783-6 Table B.14: Button Options bit 0 ("latchable")
            // cannot be changed at runtime by Change Attribute. Preserve the
            // object-pool definition while retaining the mutable state/border
            // option bits from the command.
            u32::from((body.options & 0x01) | (value.to_le_bytes()[0] & !0x01))
        }),
        (ObjectType::PictureGraphic, 2) => object.get_picture_graphic_body().ok().map(|body| {
            // ISO 11783-6 Table B.41: Picture Graphic Options bit 2 selects
            // raw vs RLE bitmap data and cannot be changed at runtime by
            // Change Attribute. Preserve the uploaded data-shape bit while
            // retaining the mutable transparent/flashing bits.
            u32::from((body.options & 0x04) | (value.to_le_bytes()[0] & !0x04))
        }),
        _ => Some(value),
    }
}

fn vt_get_attribute_value(
    pool: &ObjectPool,
    state: &ServerObjectState,
    object: &VTObject,
    attribute_id: u8,
) -> Result<Option<u32>> {
    if let Some(value) = state.attributes.get(&(object.id, attribute_id))
        && !matches!(
            (object.r#type, attribute_id),
            (ObjectType::InputNumber, 15) | (ObjectType::InputList, 5)
        )
    {
        return Ok(Some(*value));
    }

    let value = match object.r#type {
        ObjectType::DataMask => {
            let body = object.get_data_mask_body()?;
            match attribute_id {
                1 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                2 => state
                    .soft_key_masks
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.soft_key_mask)
                    .raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::AlarmMask => {
            let body = object.get_alarm_mask_body()?;
            match attribute_id {
                1 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                2 => state
                    .soft_key_masks
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.soft_key_mask)
                    .raw() as u32,
                3 => state
                    .priorities
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.priority) as u32,
                4 => body.acoustic_signal as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::WindowMask => {
            let body = object.get_window_mask_body()?;
            match attribute_id {
                1 => body.width_cells as u32,
                2 => body.height_cells as u32,
                3 => body.window_type as u32,
                4 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                5 => body.options as u32,
                6 => body.name.raw() as u32,
                7 => body.window_title.raw() as u32,
                8 => body.window_icon.raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Container => {
            let body = object.get_container_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => body.hidden as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::SoftKeyMask => {
            let body = object.get_soft_key_mask_body()?;
            match attribute_id {
                1 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Key => {
            let body = object.get_key_body()?;
            match attribute_id {
                1 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                2 => body.key_code as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::KeyGroup => {
            let body = object.get_key_group_body()?;
            match attribute_id {
                1 => body.options as u32,
                2 => body.name.raw() as u32,
                3 => body.key_group_icon.raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::ObjectPointer => {
            let body = object.get_object_pointer_body()?;
            match attribute_id {
                1 => state
                    .numeric_values
                    .get(&object.id)
                    .copied()
                    .unwrap_or_else(|| body.value.raw() as u32),
                _ => return Ok(None),
            }
        }
        ObjectType::ExternalObjectDefinition => {
            let body = object.get_external_object_definition_body()?;
            match attribute_id {
                1 => body.options as u32,
                2 => body.name0,
                3 => body.name1,
                _ => return Ok(None),
            }
        }
        ObjectType::ExternalReferenceName => {
            let body = object.get_external_reference_name_body()?;
            match attribute_id {
                1 => body.options as u32,
                2 => body.name0,
                3 => body.name1,
                _ => return Ok(None),
            }
        }
        ObjectType::ExternalObjectPointer => {
            let body = object.get_external_object_pointer_body()?;
            let numeric_pointer = state.numeric_values.get(&object.id).copied();
            match attribute_id {
                1 => body.default_object_id.raw() as u32,
                2 => numeric_pointer
                    .map(|value| u32::from(value as u16))
                    .unwrap_or_else(|| body.external_reference_name.raw() as u32),
                3 => numeric_pointer
                    .map(|value| u32::from((value >> 16) as u16))
                    .unwrap_or_else(|| body.external_object_id.raw() as u32),
                _ => return Ok(None),
            }
        }
        ObjectType::OutputString => {
            let body = object.get_output_string_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                4 => state
                    .font_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.font_attributes)
                    .raw() as u32,
                5 => body.options as u32,
                6 => body.variable_reference.raw() as u32,
                7 => body.justification as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::OutputNumber => {
            let body = object.get_output_number_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                4 => state
                    .font_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.font_attributes)
                    .raw() as u32,
                5 => body.options as u32,
                6 => body.variable_reference.raw() as u32,
                7 => body.offset as u32,
                8 => body.scale.to_bits(),
                9 => body.number_of_decimals as u32,
                10 => body.format as u32,
                11 => body.justification as u32,
                12 => state
                    .numeric_values
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.value),
                _ => return Ok(None),
            }
        }
        ObjectType::OutputList => {
            let body = object.get_output_list_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => body.variable_reference.raw() as u32,
                4 => vt_get_numeric_value_or_inline(
                    pool,
                    state,
                    object.id,
                    body.variable_reference,
                    u32::from(body.value),
                ),
                _ => return Ok(None),
            }
        }
        ObjectType::InputBoolean => {
            let body = object.get_input_boolean_body()?;
            match attribute_id {
                1 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                2 => state
                    .sizes
                    .get(&object.id)
                    .map(|(width, _)| *width as u32)
                    .unwrap_or(body.width as u32),
                3 => body.foreground.raw() as u32,
                4 => body.variable_reference.raw() as u32,
                5 => state
                    .numeric_values
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.value as u32),
                6 => state
                    .enable_state
                    .get(&object.id)
                    .copied()
                    .map(u32::from)
                    .unwrap_or(body.enabled as u32),
                _ => return Ok(None),
            }
        }
        ObjectType::InputString => {
            let body = object.get_input_string_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                4 => state
                    .font_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.font_attributes)
                    .raw() as u32,
                5 => body.input_attributes.raw() as u32,
                6 => body.options as u32,
                7 => body.variable_reference.raw() as u32,
                8 => body.justification as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::InputNumber => {
            let body = object.get_input_number_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                4 => state
                    .font_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.font_attributes)
                    .raw() as u32,
                5 => body.options as u32,
                6 => body.variable_reference.raw() as u32,
                7 => body.min_value as u32,
                8 => body.max_value as u32,
                9 => body.offset as u32,
                10 => body.scale.to_bits(),
                11 => body.number_of_decimals as u32,
                12 => body.format as u32,
                13 => body.justification as u32,
                14 => state
                    .numeric_values
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.value),
                15 => apply_optional_enabled_bit(
                    state
                        .attributes
                        .get(&(object.id, attribute_id))
                        .copied()
                        .unwrap_or(body.options2 as u32),
                    state.enable_state.get(&object.id).copied(),
                ),
                _ => return Ok(None),
            }
        }
        ObjectType::InputList => {
            let body = object.get_input_list_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => body.variable_reference.raw() as u32,
                4 => vt_get_numeric_value_or_inline(
                    pool,
                    state,
                    object.id,
                    body.variable_reference,
                    u32::from(body.value),
                ),
                5 => apply_optional_enabled_bit(
                    state
                        .attributes
                        .get(&(object.id, attribute_id))
                        .copied()
                        .unwrap_or(body.options as u32),
                    state.enable_state.get(&object.id).copied(),
                ),
                _ => return Ok(None),
            }
        }
        ObjectType::InputAttributes => {
            let body = object.get_input_attributes_body()?;
            match attribute_id {
                1 => body.validation_type as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::ExtendedInputAttributes => {
            let body = object.get_extended_input_attributes_body()?;
            match attribute_id {
                1 => body.validation_type as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::FontAttributes => {
            let body = object.get_font_attributes_body()?;
            match attribute_id {
                1 => body.font_color as u32,
                2 => body.font_size as u32,
                3 => body.font_type as u32,
                4 => body.font_style as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::LineAttributes => {
            let body = object.get_line_attributes_body()?;
            match attribute_id {
                1 => body.line_color as u32,
                2 => body.line_width as u32,
                3 => body.line_art as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::FillAttributes => {
            let body = object.get_fill_attributes_body()?;
            match attribute_id {
                1 => body.fill_type as u32,
                2 => body.fill_color as u32,
                3 => body.fill_pattern.raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Line => {
            let body = object.get_output_line_body()?;
            let endpoint = state.endpoints.get(&object.id).copied();
            match attribute_id {
                1 => state
                    .line_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.line_attributes)
                    .raw() as u32,
                2 => endpoint.map(|(width, _, _)| width).unwrap_or(body.width) as u32,
                3 => endpoint.map(|(_, height, _)| height).unwrap_or(body.height) as u32,
                4 => endpoint
                    .map(|(_, _, line_direction)| line_direction)
                    .unwrap_or(body.line_direction) as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Rectangle => {
            let body = object.get_output_rectangle_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => state
                    .line_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.line_attributes)
                    .raw() as u32,
                2 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                3 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                4 => body.line_suppression as u32,
                5 => state
                    .fill_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.fill_attributes)
                    .raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Ellipse => {
            let body = object.get_output_ellipse_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => state
                    .line_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.line_attributes)
                    .raw() as u32,
                2 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                3 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                4 => body.ellipse_type as u32,
                5 => body.start_angle as u32,
                6 => body.end_angle as u32,
                7 => state
                    .fill_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.fill_attributes)
                    .raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Polygon => {
            let body = object.get_output_polygon_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => state
                    .line_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.line_attributes)
                    .raw() as u32,
                4 => state
                    .fill_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.fill_attributes)
                    .raw() as u32,
                5 => body.polygon_type as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Meter => {
            let body = object.get_meter_body()?;
            match attribute_id {
                1 => body.width as u32,
                2 => body.needle_color as u32,
                3 => body.border_color as u32,
                4 => body.arc_and_tick_color as u32,
                5 => body.options as u32,
                6 => body.number_of_ticks as u32,
                7 => body.start_angle as u32,
                8 => body.end_angle as u32,
                9 => body.min_value as u32,
                10 => body.max_value as u32,
                11 => body.variable_reference.raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::LinearBarGraph => {
            let body = object.get_linear_bar_graph_body()?;
            match attribute_id {
                1 => body.width as u32,
                2 => body.height as u32,
                3 => body.color as u32,
                4 => body.target_line_color as u32,
                5 => body.options as u32,
                6 => body.number_of_ticks as u32,
                7 => body.min_value as u32,
                8 => body.max_value as u32,
                9 => body.variable_reference.raw() as u32,
                10 => body.target_value_variable_reference.raw() as u32,
                11 => body.target_value as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::ArchedBarGraph => {
            let body = object.get_arched_bar_graph_body()?;
            match attribute_id {
                1 => body.width as u32,
                2 => body.height as u32,
                3 => body.color as u32,
                4 => body.target_line_color as u32,
                5 => body.options as u32,
                6 => body.start_angle as u32,
                7 => body.end_angle as u32,
                8 => body.bar_width as u32,
                9 => body.min_value as u32,
                10 => body.max_value as u32,
                11 => body.variable_reference.raw() as u32,
                12 => body.target_value_variable_reference.raw() as u32,
                13 => body.target_value as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::Button => {
            let body = object.get_button_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_color) as u32,
                4 => body.border_color as u32,
                5 => body.key_code as u32,
                6 => body.options as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::PictureGraphic => {
            let body = object.get_picture_graphic_body()?;
            match attribute_id {
                1 => body.width as u32,
                2 => body.options as u32,
                3 => body.transparency as u32,
                4 => body.actual_width as u32,
                5 => body.actual_height as u32,
                6 => body.format as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::GraphicData => return Ok(None),
        ObjectType::ScaledGraphic => {
            let body = object.get_scaled_graphic_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => body.scale_type as u32,
                4 => body.options as u32,
                5 => state
                    .numeric_values
                    .get(&object.id)
                    .copied()
                    .unwrap_or_else(|| body.value.raw() as u32),
                _ => return Ok(None),
            }
        }
        ObjectType::Animation => {
            let body = object.get_animation_body()?;
            let size = state.sizes.get(&object.id).copied();
            match attribute_id {
                1 => size.map(|(width, _)| width).unwrap_or(body.width) as u32,
                2 => size.map(|(_, height)| height).unwrap_or(body.height) as u32,
                3 => body.refresh_interval_ms as u32,
                4 => state
                    .numeric_values
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.value as u32),
                5 => state
                    .enable_state
                    .get(&object.id)
                    .copied()
                    .map(u32::from)
                    .unwrap_or(body.enabled as u32),
                6 => body.first_child_index as u32,
                7 => body.default_child_index as u32,
                8 => body.last_child_index as u32,
                9 => body.options as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::GraphicContext => {
            let body = object.get_graphic_context_body()?;
            match attribute_id {
                1 => body.viewport_width as u32,
                2 => body.viewport_height as u32,
                3 => body.viewport_x as u16 as u32,
                4 => body.viewport_y as u16 as u32,
                5 => body.canvas_width as u32,
                6 => body.canvas_height as u32,
                7 => body.viewport_zoom_raw,
                8 => body.cursor_x as u16 as u32,
                9 => body.cursor_y as u16 as u32,
                10 => body.foreground_colour as u32,
                11 => state
                    .background_colours
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.background_colour) as u32,
                12 => state
                    .font_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.font_attributes)
                    .raw() as u32,
                13 => state
                    .line_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.line_attributes)
                    .raw() as u32,
                14 => state
                    .fill_attributes
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.fill_attributes)
                    .raw() as u32,
                15 => body.format as u32,
                16 => body.options as u32,
                17 => body.transparency_colour as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::ColourPalette => {
            let body = object.get_colour_palette_body()?;
            match attribute_id {
                1 => body.options as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::WorkingSetSpecialControls => {
            let body = object.get_working_set_special_controls_body()?;
            match attribute_id {
                1 => body.bytes_to_follow()? as u32,
                2 => state.selected_colour_map.raw() as u32,
                3 => state
                    .selected_colour_palette
                    .unwrap_or(body.colour_palette)
                    .raw() as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::NumberVariable => {
            let body = object.get_number_variable_body()?;
            match attribute_id {
                1 => state
                    .numeric_values
                    .get(&object.id)
                    .copied()
                    .unwrap_or(body.value),
                _ => return Ok(None),
            }
        }
        ObjectType::StringVariable => {
            let body = object.get_string_variable_body()?;
            match attribute_id {
                1 => body.length as u32,
                _ => return Ok(None),
            }
        }
        ObjectType::AuxFunction
        | ObjectType::AuxInput
        | ObjectType::AuxFunction2
        | ObjectType::AuxInput2
        | ObjectType::AuxControlDesig
        | ObjectType::WorkingSet
        | ObjectType::Macro
        | ObjectType::ColourMap
        | ObjectType::ObjectLabelRef
        | ObjectType::ScaledBitmap
        | ObjectType::GraphicsContext => return Ok(None),
    };

    Ok(Some(value))
}

fn vt_get_numeric_value_or_inline(
    pool: &ObjectPool,
    state: &ServerObjectState,
    object_id: ObjectID,
    variable_reference: ObjectID,
    inline_value: u32,
) -> u32 {
    state
        .numeric_values
        .get(&object_id)
        .copied()
        .or_else(|| {
            (variable_reference != ObjectID::NULL)
                .then(|| {
                    pool.find(variable_reference)
                        .filter(|object| object.r#type == ObjectType::NumberVariable)
                        .and_then(|object| object.get_number_variable_body().ok())
                        .map(|body| body.value)
                })
                .flatten()
        })
        .unwrap_or(inline_value)
}

fn current_i32_attribute(
    state: &ServerObjectState,
    object_id: ObjectID,
    attribute_id: u8,
    fallback: i32,
) -> i32 {
    state
        .attributes
        .get(&(object_id, attribute_id))
        .map_or(fallback, |value| *value as i32)
}

fn current_u8_attribute(
    state: &ServerObjectState,
    object_id: ObjectID,
    attribute_id: u8,
    fallback: u8,
) -> u8 {
    state
        .attributes
        .get(&(object_id, attribute_id))
        .map_or(fallback, |value| *value as u8)
}

fn vt_animation_attribute_index_is_valid(
    state: &ServerObjectState,
    object: &VTObject,
    attribute_id: u8,
    value: u32,
) -> bool {
    let child_count = object.children_pos.len();
    if child_count == 0 || value > u32::from(u8::MAX) {
        return false;
    }
    let Ok(body) = object.get_animation_body() else {
        return false;
    };
    let candidate = value as u8;
    match attribute_id {
        4 => candidate == u8::MAX || usize::from(candidate) < child_count,
        6 => {
            let current_last = current_u8_attribute(state, object.id, 8, body.last_child_index);
            candidate <= current_last && usize::from(current_last) < child_count
        }
        7 => usize::from(candidate) < child_count,
        8 => {
            let current_first = current_u8_attribute(state, object.id, 6, body.first_child_index);
            current_first <= candidate && usize::from(candidate) < child_count
        }
        _ => true,
    }
}

fn fill_attribute_type_change_is_valid(
    pool: &ObjectPool,
    state: &ServerObjectState,
    object: &VTObject,
    value: u32,
) -> bool {
    if value > 3 {
        return false;
    }
    if value != 3 {
        return true;
    }
    let Ok(body) = object.get_fill_attributes_body() else {
        return false;
    };
    let pattern = state
        .attributes
        .get(&(object.id, 3))
        .map_or(body.fill_pattern, |value| ObjectID(*value as u16));
    pattern == ObjectID::NULL || fill_pattern_reference_has_valid_buffer(pool, pattern)
}

fn fill_attribute_pattern_change_is_valid(
    pool: &ObjectPool,
    state: &ServerObjectState,
    object: &VTObject,
    reference: ObjectID,
) -> bool {
    if !pool_reference_has_type(pool, reference, ObjectType::PictureGraphic) {
        return false;
    }
    if reference == ObjectID::NULL {
        return true;
    }
    let Ok(body) = object.get_fill_attributes_body() else {
        return false;
    };
    let fill_type = current_u8_attribute(state, object.id, 1, body.fill_type);
    fill_type != 3 || fill_pattern_reference_has_valid_buffer(pool, reference)
}

fn fill_pattern_reference_has_valid_buffer(pool: &ObjectPool, reference: ObjectID) -> bool {
    pool.find(reference)
        .filter(|object| object.r#type == ObjectType::PictureGraphic)
        .and_then(|object| object.get_picture_graphic_body().ok())
        .is_some_and(|body| picture_graphic_fill_pattern_buffer_is_valid(&body))
}

fn animation_numeric_value_is_valid(object: &VTObject, value: u32) -> bool {
    if value == u32::from(u8::MAX) {
        return true;
    }
    value <= u32::from(u8::MAX) && (value as usize) < object.children_pos.len()
}

fn apply_optional_enabled_bit(options: u32, enabled: Option<bool>) -> u32 {
    enabled.map_or(options, |enabled| (options & !0x01) | u32::from(enabled))
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

fn vt_window_mask_required_objects_match(
    pool: &ObjectPool,
    body: &super::objects::WindowMaskBody,
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

fn graphics_context_reference_is_valid(
    pool: &ObjectPool,
    source: ObjectID,
    subcommand: u8,
    payload: &[u8],
) -> bool {
    let Some(reference) = graphics_context_payload_object_id(payload) else {
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
                && !graphics_context_object_contains(pool, reference, source, &mut Vec::new())
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

fn graphics_context_payload_object_id(payload: &[u8]) -> Option<ObjectID> {
    (payload.len() >= 2).then(|| ObjectID(u16_le(payload)))
}

fn graphics_context_object_contains(
    pool: &ObjectPool,
    root: ObjectID,
    needle: ObjectID,
    path: &mut Vec<ObjectID>,
) -> bool {
    if root == needle {
        return true;
    }
    if path.contains(&root) {
        return false;
    }
    path.push(root);
    let contains = pool.find(root).is_some_and(|object| {
        object
            .children_pos
            .iter()
            .any(|child| graphics_context_object_contains(pool, child.id, needle, path))
            || object
                .children
                .iter()
                .copied()
                .any(|child| graphics_context_object_contains(pool, child, needle, path))
    });
    path.pop();
    contains
}

#[inline]
const fn valid_vt_peer_address(address: Address) -> bool {
    address != NULL_ADDRESS && address != BROADCAST_ADDRESS
}

#[inline]
const fn change_background_colour_target_is_valid(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::DataMask
            | ObjectType::AlarmMask
            | ObjectType::SoftKeyMask
            | ObjectType::Key
            | ObjectType::WindowMask
            | ObjectType::Button
            | ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::OutputString
            | ObjectType::OutputNumber
            | ObjectType::GraphicContext
    )
}

#[inline]
const fn change_size_target_is_valid(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::Container
            | ObjectType::Button
            | ObjectType::OutputString
            | ObjectType::OutputNumber
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
            | ObjectType::OutputList
            | ObjectType::Line
            | ObjectType::Rectangle
            | ObjectType::Ellipse
            | ObjectType::Polygon
            | ObjectType::LinearBarGraph
            | ObjectType::ArchedBarGraph
            | ObjectType::InputBoolean
            | ObjectType::Meter
    )
}

#[inline]
const fn aux_state_matches_type(ty: AuxFunctionType, state: AuxFunctionState) -> bool {
    match ty {
        AuxFunctionType::Type0 => matches!(state, AuxFunctionState::Off | AuxFunctionState::On),
        AuxFunctionType::Type1 | AuxFunctionType::Type2 => {
            matches!(state, AuxFunctionState::Variable)
        }
    }
}
