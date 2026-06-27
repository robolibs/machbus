/// Free a VT pool handle. Accepts `NULL`. Do not call after the handle has
/// been consumed by [`machbus_session_new_with_content`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_free(h: *mut MachbusVtPool) {
    if h.is_null() {
        return;
    }
    // SAFETY: pointer originated from Box::into_raw in a vt-pool constructor.
    unsafe { drop(Box::from_raw(h)) };
}

/// Number of objects currently in the pool, or 0 for a null handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_object_count(h: *const MachbusVtPool) -> usize {
    if h.is_null() {
        return 0;
    }
    // SAFETY: validated non-null.
    unsafe { (*h).0.size() }
}

/// Serialize the pool to ISO 11783-6 object-pool bytes (length-query
/// convention: returns the full length; copies into `out` only when
/// `cap >= length`; pass `out = NULL` to query the size). Returns 0 on error.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_serialize(
    h: *const MachbusVtPool,
    out: *mut u8,
    cap: usize,
) -> usize {
    if h.is_null() {
        set_last_error("null VT pool handle");
        return 0;
    }
    // SAFETY: validated non-null.
    match unsafe { (*h).0.serialize() } {
        Ok(bytes) => {
            clear_last_error();
            copy_bytes_out(&bytes, out, cap)
        }
        Err(e) => {
            set_last_error(e.to_string());
            0
        }
    }
}

/// Add a fully built [`VTObject`] to the pool, mapping errors to `false`.
fn vt_pool_push(p: *mut MachbusVtPool, obj: crate::isobus::vt::VTObject) -> bool {
    let Some(pool) = vt_pool_mut(p) else {
        return false;
    };
    bool_result(pool.add(obj))
}

/// Same as [`vt_pool_push`] but for adders whose body encoder is fallible.
fn vt_pool_push_result(
    p: *mut MachbusVtPool,
    obj: crate::net::error::Result<crate::isobus::vt::VTObject>,
) -> bool {
    match obj {
        Ok(o) => vt_pool_push(p, o),
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

/// Add a Working Set object (Type 0): background colour, `selectable`
/// (0/1), and the initial Data/Alarm Mask object id (`0xFFFF` = none).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_working_set(
    h: *mut MachbusVtPool,
    id: u16,
    background_color: u8,
    selectable: u8,
    active_mask: u16,
) -> bool {
    let body = crate::isobus::vt::WorkingSetBody {
        background_colour: background_color,
        selectable,
        active_mask: ObjectID(active_mask),
        ..Default::default()
    };
    vt_pool_push(h, crate::isobus::vt::create_working_set(id, &body))
}

/// Add a Data Mask object (Type 1): background colour and soft-key mask id
/// (`0xFFFF` = none).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_data_mask(
    h: *mut MachbusVtPool,
    id: u16,
    background_color: u8,
    soft_key_mask: u16,
) -> bool {
    let body = crate::isobus::vt::DataMaskBody {
        background_color,
        soft_key_mask: ObjectID(soft_key_mask),
    };
    vt_pool_push(h, crate::isobus::vt::create_data_mask(id, &body))
}

/// Add a Container object (Type 3): width, height, and `hidden` (0/1).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_container(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    height: u16,
    hidden: bool,
) -> bool {
    let body = crate::isobus::vt::ContainerBody {
        width,
        height,
        hidden,
    };
    vt_pool_push(h, crate::isobus::vt::create_container(id, &body))
}

/// Add a Soft Key Mask object (Type 4): background colour.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_soft_key_mask(
    h: *mut MachbusVtPool,
    id: u16,
    background_color: u8,
) -> bool {
    let body = crate::isobus::vt::SoftKeyMaskBody { background_color };
    vt_pool_push(h, crate::isobus::vt::create_soft_key_mask(id, &body))
}

/// Add a Key object (Type 5): background colour and key code.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_key(
    h: *mut MachbusVtPool,
    id: u16,
    background_color: u8,
    key_code: u8,
) -> bool {
    let body = crate::isobus::vt::KeyBody {
        background_color,
        key_code,
    };
    vt_pool_push(h, crate::isobus::vt::create_key(id, &body))
}

/// Add a Button object (Type 6): width, height, background/border colour,
/// key code, and options.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_button(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    height: u16,
    background_color: u8,
    border_color: u8,
    key_code: u8,
    options: u8,
) -> bool {
    let body = crate::isobus::vt::ButtonBody {
        width,
        height,
        background_color,
        border_color,
        key_code,
        options,
    };
    vt_pool_push(h, crate::isobus::vt::create_button(id, &body))
}

/// Add an Input Number object (Type 9): geometry, font attributes id, Options 1,
/// variable reference id (`0xFFFF` = use raw `value`), value, min/max/offset,
/// scale, decimals, format, justification, and Options 2.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_input_number(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    height: u16,
    background_color: u8,
    font_attributes: u16,
    options: u8,
    variable_reference: u16,
    value: u32,
    min_value: i32,
    max_value: i32,
    offset: i32,
    scale: f32,
    number_of_decimals: u8,
    format: u8,
    justification: u8,
    options2: u8,
) -> bool {
    let body = crate::isobus::vt::InputNumberBody {
        width,
        height,
        background_color,
        font_attributes: ObjectID(font_attributes),
        options,
        variable_reference: ObjectID(variable_reference),
        value,
        min_value,
        max_value,
        offset,
        scale,
        number_of_decimals,
        format,
        justification,
        options2,
    };
    vt_pool_push_result(h, crate::isobus::vt::create_input_number(id, &body))
}

/// Add an Input List object (Type 10): geometry, variable reference id,
/// inline selected value, options, and the list of item object ids
/// (`items` / `item_count`; may be null/0 for an empty list).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_input_list(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    height: u16,
    variable_reference: u16,
    value: u8,
    options: u8,
    items: *const u16,
    item_count: usize,
) -> bool {
    let item_ids: Vec<ObjectID> = if item_count == 0 {
        Vec::new()
    } else if items.is_null() {
        set_last_error("null input-list item pointer with non-zero count");
        return false;
    } else {
        // SAFETY: caller supplied a pointer valid for `item_count` u16s.
        let slice = unsafe { std::slice::from_raw_parts(items, item_count) };
        slice.iter().map(|&v| ObjectID(v)).collect()
    };
    let body = crate::isobus::vt::InputListBody {
        width,
        height,
        variable_reference: ObjectID(variable_reference),
        value,
        options,
        items: item_ids,
    };
    vt_pool_push_result(h, crate::isobus::vt::create_input_list(id, &body))
}

/// Add an Output Number object (Type 12): geometry, font attributes id,
/// options, variable reference id, value, offset, scale, decimals, format,
/// justification.
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_output_number(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    height: u16,
    background_color: u8,
    font_attributes: u16,
    options: u8,
    variable_reference: u16,
    value: u32,
    offset: i32,
    scale: f32,
    number_of_decimals: u8,
    format: u8,
    justification: u8,
) -> bool {
    let body = crate::isobus::vt::OutputNumberBody {
        width,
        height,
        background_color,
        font_attributes: ObjectID(font_attributes),
        options,
        variable_reference: ObjectID(variable_reference),
        value,
        offset,
        scale,
        number_of_decimals,
        format,
        justification,
    };
    vt_pool_push_result(h, crate::isobus::vt::create_output_number(id, &body))
}

/// Add an Output String object (Type 11): geometry, font attributes id,
/// options, variable reference id, justification, and the literal string
/// `value` (NUL-terminated; ignored when a variable reference is set).
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_output_string(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    height: u16,
    background_color: u8,
    font_attributes: u16,
    options: u8,
    variable_reference: u16,
    justification: u8,
    value: *const c_char,
) -> bool {
    let value_bytes = read_c_str(value).unwrap_or_default().into_bytes();
    let body = crate::isobus::vt::OutputStringBody {
        width,
        height,
        background_color,
        font_attributes: ObjectID(font_attributes),
        options,
        variable_reference: ObjectID(variable_reference),
        justification,
        value: value_bytes,
    };
    vt_pool_push_result(h, crate::isobus::vt::create_output_string(id, &body))
}

/// Add an Output Rectangle object (Type 14): width, height, line attributes
/// id, line-suppression bitmask (bits 0..=3), and fill attributes id.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_output_rectangle(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    height: u16,
    line_attributes: u16,
    line_suppression: u8,
    fill_attributes: u16,
) -> bool {
    let body = crate::isobus::vt::OutputRectangleBody {
        width,
        height,
        line_attributes: ObjectID(line_attributes),
        line_suppression,
        fill_attributes: ObjectID(fill_attributes),
    };
    vt_pool_push_result(h, crate::isobus::vt::create_output_rectangle(id, &body))
}

/// Add a Font Attributes object (Type 23): colour, size, type, style.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_font_attributes(
    h: *mut MachbusVtPool,
    id: u16,
    font_color: u8,
    font_size: u8,
    font_type: u8,
    font_style: u8,
) -> bool {
    let body = crate::isobus::vt::FontAttributesBody {
        font_color,
        font_size,
        font_type,
        font_style,
    };
    vt_pool_push(h, crate::isobus::vt::create_font_attributes(id, &body))
}

/// Add a Fill Attributes object (Type 25): fill type (0..=3), fill colour,
/// and the fill-pattern Picture Graphic id (`0xFFFF` = none).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_fill_attributes(
    h: *mut MachbusVtPool,
    id: u16,
    fill_type: u8,
    fill_color: u8,
    fill_pattern: u16,
) -> bool {
    let body = crate::isobus::vt::FillAttributesBody {
        fill_type,
        fill_color,
        fill_pattern: ObjectID(fill_pattern),
    };
    vt_pool_push_result(h, crate::isobus::vt::create_fill_attributes(id, &body))
}

/// Add a Picture Graphic object (Type 20): display width, actual bitmap
/// width/height, colour format (0=1-bit,1=4-bit,2=8-bit), options
/// (bit 2 = RLE), transparency colour index, and the raw bitmap bytes
/// (`data` / `data_len`; may be null/0).
#[allow(clippy::too_many_arguments)]
#[unsafe(no_mangle)]
pub extern "C" fn machbus_vt_pool_add_picture_graphic(
    h: *mut MachbusVtPool,
    id: u16,
    width: u16,
    actual_width: u16,
    actual_height: u16,
    format: u8,
    options: u8,
    transparency: u8,
    data: *const u8,
    data_len: usize,
) -> bool {
    let bitmap = match read_bytes(data, data_len) {
        Ok(b) => b.to_vec(),
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let body = crate::isobus::vt::PictureGraphicBody {
        width,
        actual_width,
        actual_height,
        format,
        options,
        transparency,
        data: bitmap,
    };
    vt_pool_push_result(h, crate::isobus::vt::create_picture_graphic(id, &body))
}

// ── MachbusDdop ──

/// Owned, opaque ISO 11783-10 Device Descriptor Object Pool (DDOP). Build with
/// [`machbus_ddop_new`], populate with the `machbus_ddop_add_*` adders, and
/// release with [`machbus_ddop_free`] (or transfer to a session). Each adder
/// returns the assigned ObjectID (>= 0) or -1 on error.
pub struct MachbusDdop(DDOP);

fn ddop_mut<'a>(p: *mut MachbusDdop) -> Option<&'a mut DDOP> {
    if p.is_null() {
        set_last_error("null DDOP handle");
        return None;
    }
    // SAFETY: validated non-null; caller owns the box.
    Some(unsafe { &mut (*p).0 })
}

/// Map an `add_*` result (new ObjectID) to the C return: the id as `i32`, or
/// -1 on error.
fn ddop_add_result(r: crate::net::error::Result<crate::isobus::tc::ObjectID>) -> i32 {
    match r {
        Ok(id) => {
            clear_last_error();
            i32::from(id.raw())
        }
        Err(e) => {
            set_last_error(e.to_string());
            -1
        }
    }
}

/// Create an empty DDOP. Free with [`machbus_ddop_free`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_new() -> *mut MachbusDdop {
    clear_last_error();
    Box::into_raw(Box::new(MachbusDdop(DDOP::default())))
}

/// Free a DDOP handle. Accepts `NULL`. Do not call after the handle has been
/// consumed by [`machbus_session_new_with_content`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_free(h: *mut MachbusDdop) {
    if h.is_null() {
        return;
    }
    // SAFETY: pointer originated from Box::into_raw in machbus_ddop_new.
    unsafe { drop(Box::from_raw(h)) };
}

/// Add a Device object. `id` 0 = auto-assign; `designator` is required
/// (non-empty ASCII). `software_version` / `serial_number` may be NULL.
/// Returns the assigned ObjectID, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_add_device(
    h: *mut MachbusDdop,
    id: u16,
    designator: *const c_char,
    software_version: *const c_char,
    serial_number: *const c_char,
) -> i32 {
    let Some(ddop) = ddop_mut(h) else {
        return -1;
    };
    let obj = crate::isobus::tc::DeviceObject::default()
        .with_id(id)
        .with_designator(read_c_str(designator).unwrap_or_default())
        .with_software_version(read_c_str(software_version).unwrap_or_default())
        .with_serial_number(read_c_str(serial_number).unwrap_or_default());
    ddop_add_result(ddop.add_device(obj))
}

/// Add a Device Element. `id` 0 = auto-assign. `element_type` is the ISO
/// `DeviceElementType` code (1=Device,2=Function,3=Bin,4=Section,5=Unit,
/// 6=Connector,7=NavigationReference); out-of-range falls back to Device.
/// `parent_id` 0 = no parent. `designator` may be NULL. Returns the assigned
/// ObjectID, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_add_element(
    h: *mut MachbusDdop,
    id: u16,
    element_type: u8,
    element_number: u16,
    parent_id: u16,
    designator: *const c_char,
) -> i32 {
    let Some(ddop) = ddop_mut(h) else {
        return -1;
    };
    use crate::isobus::tc::DeviceElementType;
    let ty = match element_type {
        2 => DeviceElementType::Function,
        3 => DeviceElementType::Bin,
        4 => DeviceElementType::Section,
        5 => DeviceElementType::Unit,
        6 => DeviceElementType::Connector,
        7 => DeviceElementType::NavigationReference,
        _ => DeviceElementType::Device,
    };
    let elem = crate::isobus::tc::DeviceElement::default()
        .with_id(id)
        .with_type(ty)
        .with_number(element_number)
        .with_parent(parent_id)
        .with_designator(read_c_str(designator).unwrap_or_default());
    ddop_add_result(ddop.add_element(elem))
}

/// Add a Device Process Data object. `id` 0 = auto-assign. `trigger_methods`
/// is the ISO trigger bitmask. `presentation_id` `0xFFFF` = no presentation.
/// `designator` may be NULL. Returns the assigned ObjectID, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_add_process_data(
    h: *mut MachbusDdop,
    id: u16,
    ddi: u16,
    trigger_methods: u8,
    presentation_id: u16,
    designator: *const c_char,
) -> i32 {
    let Some(ddop) = ddop_mut(h) else {
        return -1;
    };
    let pd = crate::isobus::tc::DeviceProcessData::default()
        .with_id(id)
        .with_ddi(ddi)
        .with_triggers(trigger_methods)
        .with_presentation(presentation_id)
        .with_designator(read_c_str(designator).unwrap_or_default());
    ddop_add_result(ddop.add_process_data(pd))
}

/// Add a Device Property object. `id` 0 = auto-assign. `value` is the fixed
/// property value. `presentation_id` `0xFFFF` = no presentation. `designator`
/// may be NULL. Returns the assigned ObjectID, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_add_property(
    h: *mut MachbusDdop,
    id: u16,
    ddi: u16,
    value: i32,
    presentation_id: u16,
    designator: *const c_char,
) -> i32 {
    let Some(ddop) = ddop_mut(h) else {
        return -1;
    };
    let prop = crate::isobus::tc::DeviceProperty::default()
        .with_id(id)
        .with_ddi(ddi)
        .with_value(value)
        .with_presentation(presentation_id)
        .with_designator(read_c_str(designator).unwrap_or_default());
    ddop_add_result(ddop.add_property(prop))
}

/// Add a Device Value Presentation object. `id` 0 = auto-assign. `offset` and
/// `scale` apply as `displayed = (value + offset) * scale`. `unit_designator`
/// may be NULL. Returns the assigned ObjectID, or -1 on error.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_add_value_presentation(
    h: *mut MachbusDdop,
    id: u16,
    offset: i32,
    scale: f32,
    decimal_digits: u8,
    unit_designator: *const c_char,
) -> i32 {
    let Some(ddop) = ddop_mut(h) else {
        return -1;
    };
    let vp = crate::isobus::tc::DeviceValuePresentation::default()
        .with_id(id)
        .with_offset(offset)
        .with_scale(scale)
        .with_decimals(decimal_digits)
        .with_unit(read_c_str(unit_designator).unwrap_or_default());
    ddop_add_result(ddop.add_value_presentation(vp))
}

/// Serialize the DDOP to ISO 11783-10 object-pool bytes (length-query
/// convention: returns the full length; copies into `out` only when
/// `cap >= length`; pass `out = NULL` to query the size). Returns 0 on error.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_ddop_serialize(h: *const MachbusDdop, out: *mut u8, cap: usize) -> usize {
    if h.is_null() {
        set_last_error("null DDOP handle");
        return 0;
    }
    // SAFETY: validated non-null.
    match unsafe { (*h).0.serialize() } {
        Ok(bytes) => {
            clear_last_error();
            copy_bytes_out(&bytes, out, cap)
        }
        Err(e) => {
            set_last_error(e.to_string());
            0
        }
    }
}

// ── Session construction with prebuilt content ──

/// Create a session from `cfg` (or defaults if `cfg` is NULL) that plugs a
/// VT client carrying `vt_pool` (with a working set whose active mask is
/// `working_set_id`) and/or a TC client carrying `ddop`.
///
/// Ownership: this call **consumes** the `vt_pool` and `ddop` handles — on both
/// success and failure they are freed internally, so the caller must not use or
/// free them afterwards (set them to NULL). Pass NULL for either to skip that
/// subsystem; the corresponding `enable_vt_client` / `enable_tc_client` config
/// flags are honoured too (a NULL handle with the flag set plugs an empty
/// pool/DDOP, matching [`machbus_session_new`]).
///
/// Returns `NULL` on failure; inspect [`machbus_session_last_error`]. Free the
/// returned session with [`machbus_session_free`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_new_with_content(
    cfg: *const MachbusConfig,
    vt_pool: *mut MachbusVtPool,
    working_set_id: u16,
    ddop: *mut MachbusDdop,
) -> *mut MachbusSession {
    // Take ownership of the passed handles up front so they are always freed,
    // regardless of which path we take below.
    let vt_pool = if vt_pool.is_null() {
        None
    } else {
        // SAFETY: non-null, originated from a vt-pool constructor; consumed here.
        Some(unsafe { Box::from_raw(vt_pool) }.0)
    };
    let ddop = if ddop.is_null() {
        None
    } else {
        // SAFETY: non-null, originated from machbus_ddop_new; consumed here.
        Some(unsafe { Box::from_raw(ddop) }.0)
    };

    let cfg = if cfg.is_null() {
        MachbusConfig::default()
    } else {
        // SAFETY: non-null per the check.
        unsafe { *cfg }
    };

    let vt_content = vt_pool.map(|pool| {
        let mut ws = WorkingSet::default();
        ws.set_active_mask(working_set_id);
        (pool, ws)
    });

    match build_session_with_content(cfg, vt_content, ddop) {
        Ok(handle) => {
            clear_last_error();
            Box::into_raw(Box::new(handle))
        }
        Err(e) => {
            set_last_error(e.to_string());
            ptr::null_mut()
        }
    }
}
