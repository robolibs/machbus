use std::collections::HashMap;

use crate::isobus::vt::objects::{
    change_attribute_targets_one_byte_field, change_attribute_targets_two_byte_field,
    change_soft_key_mask_type_matches, external_object_pointer_default_is_valid_for_context,
    is_enable_disable_object_type, is_object_label_graphic_representation_type,
    is_select_input_object_type, is_select_input_open_target_type, is_standard_font_size_for_style,
    is_standard_font_type, key_group_icon_reference_is_valid, key_group_name_reference_is_valid,
    object_pointer_numeric_value_is_valid_for_context, output_list_item_reference_is_valid,
    picture_graphic_fill_pattern_buffer_is_valid, scaled_graphic_scale_type_is_valid,
    scaled_graphic_value_source_is_valid, text_justification_is_valid,
    vt_change_attribute_id_is_supported,
    window_mask_icon_reference_is_valid, window_mask_required_object_types,
    window_mask_text_reference_is_valid,
};
use crate::isobus::vt::server_working_set::AudioSignalState;
use crate::isobus::vt::{
    ChildRef, MacroBody, ObjectID, ObjectLabelState, ObjectPool, ObjectType, PolygonPoint, cmd,
};

/// A decoded, typed macro command effect.
///
/// Only the command subset whose parameter layout is already established
/// in the machbus VT command path (every object-targeting command places
/// the object id in the two bytes after the command code) is modelled;
/// everything else decodes to [`MacroEffect::Unsupported`] rather than
/// being guessed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MacroEffect {
    /// Hide/Show Object (`cmd::HIDE_SHOW`).
    HideShow { object: ObjectID, show: bool },
    /// Enable/Disable Object (`cmd::ENABLE_DISABLE`).
    EnableDisable { object: ObjectID, enable: bool },
    /// Select Input Object (`cmd::SELECT_INPUT_OBJECT_COMMAND`).
    SelectInputObject { object: ObjectID, option: u8 },
    /// Control Audio Signal (`cmd::CONTROL_AUDIO_SIGNAL`): terminal side
    /// effect, not a retained draw command.
    ControlAudioSignal { audio: AudioSignalState },
    /// Set Audio Volume (`cmd::SET_AUDIO_VOLUME`): terminal side effect, not
    /// a retained draw command.
    SetAudioVolume { percent: u8 },
    /// Change Numeric Value (`cmd::CHANGE_NUMERIC_VALUE`).
    ChangeNumericValue { object: ObjectID, value: u32 },
    /// Change String Value (`cmd::CHANGE_STRING_VALUE`). The macro-embedded
    /// layout is `[id:u16][len:u16][value bytes]`, as parsed by
    /// `MacroBody::decode`.
    ChangeStringValue { object: ObjectID, value: Vec<u8> },
    /// Change Child Location (`cmd::CHANGE_CHILD_LOCATION`): parent id,
    /// child id, and one-byte x/y location.
    ChangeChildLocation {
        parent: ObjectID,
        child: ObjectID,
        x: u8,
        y: u8,
    },
    /// Change Child Position (`cmd::CHANGE_CHILD_POSITION`): parent id,
    /// child id, and signed 16-bit x/y position.
    ChangeChildPosition {
        parent: ObjectID,
        child: ObjectID,
        x: i16,
        y: i16,
    },
    /// Change Size (`cmd::CHANGE_SIZE`): object id plus width/height.
    ChangeSize {
        object: ObjectID,
        width: u16,
        height: u16,
    },
    /// Change Background Colour (`cmd::CHANGE_BACKGROUND_COLOUR`).
    ChangeBackgroundColour { object: ObjectID, colour: u8 },
    /// Change Font Attributes (`cmd::CHANGE_FONT_ATTRIBUTES`): mutate a
    /// Font Attributes object directly.
    ChangeFontAttributes {
        object: ObjectID,
        colour: u8,
        size: u8,
        font_type: u8,
        style: u8,
    },
    /// Change Line Attributes (`cmd::CHANGE_LINE_ATTRIBUTES`): mutate a
    /// Line Attributes object directly.
    ChangeLineAttributes {
        object: ObjectID,
        colour: u8,
        width: u8,
        line_art: u16,
    },
    /// Change Fill Attributes (`cmd::CHANGE_FILL_ATTRIBUTES`): mutate a
    /// Fill Attributes object directly.
    ChangeFillAttributes {
        object: ObjectID,
        fill_type: u8,
        colour: u8,
        pattern: ObjectID,
    },
    /// Change End Point (`cmd::CHANGE_END_POINT`): Output Line width,
    /// height, and line direction.
    ChangeEndPoint {
        object: ObjectID,
        width: u16,
        height: u16,
        line_direction: u8,
    },
    /// Change Soft Key Mask (`cmd::CHANGE_SOFT_KEY_MASK`): Data/Alarm Mask
    /// soft-key-mask reference.
    ChangeSoftKeyMask {
        mask_type: u8,
        data_mask: ObjectID,
        soft_key_mask: ObjectID,
    },
    /// Change List Item (`cmd::CHANGE_LIST_ITEM`): replace a list slot.
    ChangeListItem {
        list: ObjectID,
        index: u8,
        item: ObjectID,
    },
    /// Delete Object Pool (`cmd::DELETE_OBJECT_POOL`): lifecycle command that
    /// clears the active pool rather than mutating one object.
    DeleteObjectPool,
    /// Change Priority (`cmd::CHANGE_PRIORITY`): Alarm Mask priority.
    ChangePriority { object: ObjectID, priority: u8 },
    /// Change Object Label (`cmd::CHANGE_OBJECT_LABEL`): metadata label
    /// state for an object admitted by the pool's Object Label Reference List.
    ChangeObjectLabel {
        object: ObjectID,
        label: ObjectLabelState,
    },
    /// Lock/Unlock Mask (`cmd::LOCK_UNLOCK_MASK`): Data/User Layout mask
    /// refresh hold state.
    LockUnlockMask {
        object: ObjectID,
        locked: bool,
        timeout_ms: u16,
    },
    /// Execute Macro (`cmd::EXECUTE_MACRO`): run another Macro object by id.
    ExecuteMacro { object: ObjectID },
    /// Change Polygon Point (`cmd::CHANGE_POLYGON_POINT`).
    ChangePolygonPoint {
        object: ObjectID,
        index: u8,
        x: u16,
        y: u16,
    },
    /// Change Polygon Scale (`cmd::CHANGE_POLYGON_SCALE`).
    ChangePolygonScale {
        object: ObjectID,
        width: u16,
        height: u16,
    },
    /// Change Generic Attribute (`cmd::CHANGE_ATTRIBUTE`).
    ChangeGenericAttribute {
        object: ObjectID,
        attribute_id: u8,
        value: u32,
    },
    /// Select Colour Map or Colour Palette (`cmd::SELECT_COLOUR_MAP`).
    SelectColourMap { object: ObjectID },
    /// Change Active Mask (`cmd::CHANGE_ACTIVE_MASK`, ISO 11783-6 F.34):
    /// `[working_set:u16][mask:u16][FF×3]`.
    ChangeActiveMask {
        working_set: ObjectID,
        mask: ObjectID,
    },
    /// A command decoded structurally (its bytes were walked) but not
    /// modelled as a typed effect — too-short parameters, or a command
    /// the runtime does not yet interpret.
    Unsupported { command_type: u8 },
}

#[inline]
fn object_id(p: &[u8]) -> ObjectID {
    ObjectID::new(u16::from_le_bytes([p[0], p[1]]))
}

/// Decode a Macro object's command stream into typed effects, preserving
/// command order. This is the interpreter half of the macro runtime;
/// applying the effects to a pool/scene is a separate step.
#[must_use]
pub fn decode_macro_effects(body: &MacroBody) -> Vec<MacroEffect> {
    body.commands
        .iter()
        .map(|command| {
            let p = &command.parameters;
            match command.command_type {
                cmd::HIDE_SHOW if p.len() >= 3 => MacroEffect::HideShow {
                    object: object_id(p),
                    show: p[2] != 0,
                },
                cmd::ENABLE_DISABLE if p.len() >= 3 => MacroEffect::EnableDisable {
                    object: object_id(p),
                    enable: p[2] != 0,
                },
                cmd::SELECT_INPUT_OBJECT_COMMAND if p.len() >= 3 => {
                    MacroEffect::SelectInputObject {
                        object: object_id(p),
                        option: p[2],
                    }
                }
                cmd::CONTROL_AUDIO_SIGNAL if p.len() >= 7 => MacroEffect::ControlAudioSignal {
                    audio: AudioSignalState {
                        activations: p[0],
                        frequency_hz: u16::from_le_bytes([p[1], p[2]]),
                        duration_ms: u16::from_le_bytes([p[3], p[4]]),
                        off_time_ms: u16::from_le_bytes([p[5], p[6]]),
                    },
                },
                cmd::SET_AUDIO_VOLUME if p.len() >= 7 && p[0] <= 100 => {
                    MacroEffect::SetAudioVolume { percent: p[0] }
                }
                cmd::CHANGE_NUMERIC_VALUE if p.len() >= 7 => MacroEffect::ChangeNumericValue {
                    object: object_id(p),
                    value: u32::from_le_bytes([p[3], p[4], p[5], p[6]]),
                },
                cmd::CHANGE_STRING_VALUE if p.len() >= 4 => {
                    let len = u16::from_le_bytes([p[2], p[3]]) as usize;
                    if p.len() >= 4 + len {
                        MacroEffect::ChangeStringValue {
                            object: object_id(p),
                            value: p[4..4 + len].to_vec(),
                        }
                    } else {
                        MacroEffect::Unsupported {
                            command_type: command.command_type,
                        }
                    }
                }
                cmd::CHANGE_CHILD_LOCATION if p.len() >= 6 => MacroEffect::ChangeChildLocation {
                    parent: object_id(p),
                    child: ObjectID::new(u16::from_le_bytes([p[2], p[3]])),
                    x: p[4],
                    y: p[5],
                },
                cmd::CHANGE_CHILD_POSITION if p.len() >= 8 => MacroEffect::ChangeChildPosition {
                    parent: object_id(p),
                    child: ObjectID::new(u16::from_le_bytes([p[2], p[3]])),
                    x: i16::from_le_bytes([p[4], p[5]]),
                    y: i16::from_le_bytes([p[6], p[7]]),
                },
                cmd::CHANGE_SIZE if p.len() >= 6 => MacroEffect::ChangeSize {
                    object: object_id(p),
                    width: u16::from_le_bytes([p[2], p[3]]),
                    height: u16::from_le_bytes([p[4], p[5]]),
                },
                cmd::CHANGE_BACKGROUND_COLOUR if p.len() >= 3 => {
                    MacroEffect::ChangeBackgroundColour {
                        object: object_id(p),
                        colour: p[2],
                    }
                }
                cmd::CHANGE_FONT_ATTRIBUTES if p.len() >= 6 => MacroEffect::ChangeFontAttributes {
                    object: object_id(p),
                    colour: p[2],
                    size: p[3],
                    font_type: p[4],
                    style: p[5],
                },
                cmd::CHANGE_LINE_ATTRIBUTES if p.len() >= 6 => MacroEffect::ChangeLineAttributes {
                    object: object_id(p),
                    colour: p[2],
                    width: p[3],
                    line_art: u16::from_le_bytes([p[4], p[5]]),
                },
                cmd::CHANGE_FILL_ATTRIBUTES if p.len() >= 6 => MacroEffect::ChangeFillAttributes {
                    object: object_id(p),
                    fill_type: p[2],
                    colour: p[3],
                    pattern: ObjectID::new(u16::from_le_bytes([p[4], p[5]])),
                },
                cmd::CHANGE_END_POINT if p.len() >= 7 => MacroEffect::ChangeEndPoint {
                    object: object_id(p),
                    width: u16::from_le_bytes([p[2], p[3]]),
                    height: u16::from_le_bytes([p[4], p[5]]),
                    line_direction: p[6],
                },
                cmd::CHANGE_SOFT_KEY_MASK if p.len() >= 5 => MacroEffect::ChangeSoftKeyMask {
                    mask_type: p[0],
                    data_mask: ObjectID::new(u16::from_le_bytes([p[1], p[2]])),
                    soft_key_mask: ObjectID::new(u16::from_le_bytes([p[3], p[4]])),
                },
                cmd::CHANGE_LIST_ITEM if p.len() >= 5 => MacroEffect::ChangeListItem {
                    list: object_id(p),
                    index: p[2],
                    item: ObjectID::new(u16::from_le_bytes([p[3], p[4]])),
                },
                cmd::DELETE_OBJECT_POOL if p.iter().all(|&b| b == 0xFF) => {
                    MacroEffect::DeleteObjectPool
                }
                cmd::CHANGE_PRIORITY if p.len() >= 3 => MacroEffect::ChangePriority {
                    object: object_id(p),
                    priority: p[2],
                },
                cmd::CHANGE_OBJECT_LABEL if p.len() >= 7 => MacroEffect::ChangeObjectLabel {
                    object: object_id(p),
                    label: ObjectLabelState {
                        string_variable: ObjectID::new(u16::from_le_bytes([p[2], p[3]])),
                        font_type: p[4],
                        graphic_designator: ObjectID::new(u16::from_le_bytes([p[5], p[6]])),
                    },
                },
                cmd::LOCK_UNLOCK_MASK if p.len() >= 5 && p[0] <= 1 => MacroEffect::LockUnlockMask {
                    object: ObjectID::new(u16::from_le_bytes([p[1], p[2]])),
                    locked: p[0] == 1,
                    timeout_ms: u16::from_le_bytes([p[3], p[4]]),
                },
                cmd::EXECUTE_MACRO if p.len() >= 2 => MacroEffect::ExecuteMacro {
                    object: object_id(p),
                },
                cmd::CHANGE_POLYGON_POINT if p.len() >= 7 => MacroEffect::ChangePolygonPoint {
                    object: object_id(p),
                    index: p[2],
                    x: u16::from_le_bytes([p[3], p[4]]),
                    y: u16::from_le_bytes([p[5], p[6]]),
                },
                cmd::CHANGE_POLYGON_SCALE if p.len() >= 6 => MacroEffect::ChangePolygonScale {
                    object: object_id(p),
                    width: u16::from_le_bytes([p[2], p[3]]),
                    height: u16::from_le_bytes([p[4], p[5]]),
                },
                cmd::CHANGE_ATTRIBUTE if p.len() >= 7 => MacroEffect::ChangeGenericAttribute {
                    object: object_id(p),
                    attribute_id: p[2],
                    value: u32::from_le_bytes([p[3], p[4], p[5], p[6]]),
                },
                cmd::SELECT_COLOUR_MAP if p.len() >= 2 => MacroEffect::SelectColourMap {
                    object: object_id(p),
                },
                cmd::CHANGE_ACTIVE_MASK if p.len() >= 4 => MacroEffect::ChangeActiveMask {
                    working_set: object_id(p),
                    mask: ObjectID::new(u16::from_le_bytes([p[2], p[3]])),
                },
                other => MacroEffect::Unsupported {
                    command_type: other,
                },
            }
        })
        .collect()
}

/// Outcome of applying decoded macro effects to a pool.
///
/// Number Variable writes are pool-owned and applied directly.
/// Visibility / enabled / colour-selection / active-mask changes are runtime
/// state (they do not alter the object pool definition), so they are reported
/// back for the caller's scene/runtime layer to apply rather than mutated here.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MacroApplyReport {
    /// Number Variable writes applied to the pool.
    pub numeric_applied: usize,
    /// String Variable writes applied to the pool.
    pub string_applied: usize,
    /// Pool-owned layout/geometry/style writes applied to the pool.
    pub pool_applied: usize,
    /// `(object, show)` visibility changes to apply to runtime state.
    pub visibility_changes: Vec<(ObjectID, bool)>,
    /// `(object, enable)` enabled-state changes to apply to runtime state.
    pub enable_changes: Vec<(ObjectID, bool)>,
    /// Whether a Delete Object Pool lifecycle command appeared.
    pub delete_object_pool: bool,
    /// Input object to select, if any (last one wins).
    pub selected_input_change: Option<ObjectID>,
    /// Last Control Audio Signal terminal side effect, if any.
    pub audio_signal: Option<AudioSignalState>,
    /// Last Set Audio Volume side effect, if any.
    pub audio_volume_percent: Option<u8>,
    /// The new active mask requested by a Change Active Mask command, if
    /// any (last one wins). The scene layer rebuilds with this mask.
    pub active_mask_change: Option<ObjectID>,
    /// The Colour Map or Colour Palette requested by a Select Colour Map
    /// command, if any (last one wins). Runtime layers validate and apply
    /// this selection because it is render state, not an object-pool field.
    pub colour_selection_change: Option<ObjectID>,
    /// `(object, locked, timeout_ms)` mask lock changes to apply to runtime
    /// state. The standalone helper reports these because mask refresh holds
    /// are not object-pool fields.
    pub mask_lock_changes: Vec<(ObjectID, bool, u16)>,
    /// Macro ids requested by Execute Macro effects.
    ///
    /// The standalone helper reports these instead of recursively mutating the
    /// pool. `VtRenderRuntime` owns nested execution ordering and recursion
    /// guards because those depend on runtime state.
    pub macro_executions: Vec<ObjectID>,
    /// `(object, label)` metadata changes to apply to runtime state.
    ///
    /// Labels are intentionally retained as runtime/backend metadata rather
    /// than rendered scene nodes; hosted runtimes resolve the referenced
    /// String Variable only when a backend asks for the object label text.
    pub object_label_changes: Vec<(ObjectID, ObjectLabelState)>,
    /// Generic Change Attribute effects decoded from the macro stream.
    ///
    /// The standalone helper reports these rather than guessing which
    /// object-family attributes a host wants to apply. `VtRenderRuntime`
    /// consumes the same decoded effect through its validated generic
    /// attribute replay path.
    pub generic_attribute_changes: Vec<(ObjectID, u8, u32)>,
    /// Effects that could not be applied (unknown target / unsupported).
    pub skipped: usize,
}

/// Apply decoded macro effects: write pool-owned value/geometry/style changes
/// into `pool` and collect visibility/enabled/colour/mask runtime-state changes
/// into the returned report.
pub fn apply_macro_effects(pool: &mut ObjectPool, effects: &[MacroEffect]) -> MacroApplyReport {
    let mut report = MacroApplyReport::default();
    for effect in effects {
        match effect {
            MacroEffect::HideShow { object, show } => {
                if pool
                    .find(*object)
                    .is_some_and(|object| object.r#type == ObjectType::Container)
                {
                    report.visibility_changes.push((*object, *show));
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::EnableDisable { object, enable } => {
                if pool
                    .find(*object)
                    .is_some_and(|object| is_enable_disable_object_type(object.r#type))
                {
                    report.enable_changes.push((*object, *enable));
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::SelectInputObject { object, option } => {
                if !select_input_report_is_valid(pool, *object, *option) {
                    report.skipped += 1;
                    continue;
                }
                report.selected_input_change = Some(*object);
            }
            MacroEffect::ControlAudioSignal { audio } => {
                report.audio_signal = Some(*audio);
            }
            MacroEffect::SetAudioVolume { percent } => {
                report.audio_volume_percent = Some(*percent);
            }
            MacroEffect::ChangeNumericValue { object, value } => {
                if apply_numeric_value(pool, *object, *value) {
                    report.numeric_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeStringValue { object, value } => {
                if apply_string_value(pool, *object, value.clone()) {
                    report.string_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeChildLocation {
                parent,
                child,
                x,
                y,
            } => {
                if apply_child_position(pool, *parent, *child, i16::from(*x), i16::from(*y)) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeChildPosition {
                parent,
                child,
                x,
                y,
            } => {
                if apply_child_position(pool, *parent, *child, *x, *y) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeSize {
                object,
                width,
                height,
            } => {
                if apply_size(pool, *object, *width, *height) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeBackgroundColour { object, colour } => {
                if apply_background(pool, *object, *colour) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeFontAttributes {
                object,
                colour,
                size,
                font_type,
                style,
            } => {
                if apply_font_attributes(pool, *object, *colour, *size, *font_type, *style) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeLineAttributes {
                object,
                colour,
                width,
                line_art,
            } => {
                if apply_line_attributes(pool, *object, *colour, *width, *line_art) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeFillAttributes {
                object,
                fill_type,
                colour,
                pattern,
            } => {
                if apply_fill_attributes(pool, *object, *fill_type, *colour, *pattern) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeEndPoint {
                object,
                width,
                height,
                line_direction,
            } => {
                if apply_end_point(pool, *object, *width, *height, *line_direction) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeSoftKeyMask {
                mask_type,
                data_mask,
                soft_key_mask,
            } => {
                if apply_soft_key_mask(pool, *mask_type, *data_mask, *soft_key_mask) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeListItem { list, index, item } => {
                if apply_list_item(pool, *list, usize::from(*index), *item) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::DeleteObjectPool => {
                pool.clear();
                report.delete_object_pool = true;
                report.pool_applied += 1;
            }
            MacroEffect::ChangePriority { object, priority } => {
                if apply_priority(pool, *object, *priority) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeObjectLabel { object, label } => {
                if object_label_target_is_valid(pool, *object)
                    && object_label_state_is_valid(pool, *label)
                {
                    report.object_label_changes.push((*object, *label));
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::LockUnlockMask {
                object,
                locked,
                timeout_ms,
            } => {
                if pool.find(*object).is_some_and(|object| {
                    matches!(object.r#type, ObjectType::DataMask | ObjectType::WindowMask)
                }) {
                    report
                        .mask_lock_changes
                        .push((*object, *locked, *timeout_ms));
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ExecuteMacro { object } => {
                if pool
                    .find(*object)
                    .is_some_and(|object| object.r#type == ObjectType::Macro)
                {
                    report.macro_executions.push(*object);
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangePolygonPoint {
                object,
                index,
                x,
                y,
            } => {
                if apply_polygon_point(pool, *object, usize::from(*index), *x, *y) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangePolygonScale {
                object,
                width,
                height,
            } => {
                if apply_polygon_scale(pool, *object, *width, *height) {
                    report.pool_applied += 1;
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeGenericAttribute {
                object,
                attribute_id,
                value,
            } => {
                if generic_attribute_report_is_valid(pool, *object, *attribute_id, *value) {
                    report
                        .generic_attribute_changes
                        .push((*object, *attribute_id, *value));
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::SelectColourMap { object } => {
                if colour_selection_report_is_valid(pool, *object) {
                    report.colour_selection_change = Some(*object);
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::ChangeActiveMask { working_set, mask } => {
                if active_mask_report_is_valid(pool, *working_set, *mask) {
                    report.active_mask_change = Some(*mask);
                } else {
                    report.skipped += 1;
                }
            }
            MacroEffect::Unsupported { .. } => report.skipped += 1,
        }
    }
    report
}

fn apply_child_position(
    pool: &mut ObjectPool,
    parent: ObjectID,
    child: ObjectID,
    x: i16,
    y: i16,
) -> bool {
    if pool.find(child).is_none() {
        return false;
    }
    let Some(parent_obj) = pool.find_mut(parent) else {
        return false;
    };
    if let Some(pos) = parent_obj
        .children_pos
        .iter_mut()
        .find(|pos| pos.id == child)
    {
        pos.x = x;
        pos.y = y;
        return true;
    }
    if parent_obj.children.contains(&child) {
        parent_obj.children_pos.push(ChildRef::new(child, x, y));
        return true;
    }
    false
}

fn apply_background(pool: &mut ObjectPool, id: ObjectID, colour: u8) -> bool {
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    match obj.r#type {
        ObjectType::DataMask => {
            let Ok(mut body) = obj.get_data_mask_body() else {
                return false;
            };
            body.background_color = colour;
            obj.body = body.encode();
            true
        }
        ObjectType::AlarmMask => {
            let Ok(mut body) = obj.get_alarm_mask_body() else {
                return false;
            };
            body.background_color = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::SoftKeyMask => {
            let Ok(mut body) = obj.get_soft_key_mask_body() else {
                return false;
            };
            body.background_color = colour;
            obj.body = body.encode().to_vec();
            true
        }
        ObjectType::WindowMask => {
            let Ok(mut body) = obj.get_window_mask_body() else {
                return false;
            };
            body.background_color = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Button => {
            let Ok(mut body) = obj.get_button_body() else {
                return false;
            };
            body.background_color = colour;
            obj.body = body.encode();
            true
        }
        ObjectType::InputBoolean => {
            let Ok(mut body) = obj.get_input_boolean_body() else {
                return false;
            };
            body.background_color = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputString => {
            let Ok(mut body) = obj.get_input_string_body() else {
                return false;
            };
            body.background_color = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputNumber => {
            let Ok(mut body) = obj.get_input_number_body() else {
                return false;
            };
            body.background_color = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::OutputString => {
            let Ok(mut body) = obj.get_output_string_body() else {
                return false;
            };
            body.background_color = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::OutputNumber => {
            let Ok(mut body) = obj.get_output_number_body() else {
                return false;
            };
            body.background_color = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::GraphicContext => {
            let Ok(mut body) = obj.get_graphic_context_body() else {
                return false;
            };
            body.background_colour = colour;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        _ => false,
    }
}

fn apply_font_attributes(
    pool: &mut ObjectPool,
    id: ObjectID,
    colour: u8,
    size: u8,
    font_type: u8,
    style: u8,
) -> bool {
    if !is_standard_font_size_for_style(size, style) || !is_standard_font_type(font_type) {
        return false;
    }
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    if obj.r#type != ObjectType::FontAttributes {
        return false;
    }
    let Ok(mut body) = obj.get_font_attributes_body() else {
        return false;
    };
    body.font_color = colour;
    body.font_size = size;
    body.font_type = font_type;
    body.font_style = style;
    let encoded = body.encode();
    if obj.body == encoded {
        false
    } else {
        obj.body = encoded;
        true
    }
}

fn apply_line_attributes(
    pool: &mut ObjectPool,
    id: ObjectID,
    colour: u8,
    width: u8,
    line_art: u16,
) -> bool {
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    if obj.r#type != ObjectType::LineAttributes {
        return false;
    }
    let Ok(mut body) = obj.get_line_attributes_body() else {
        return false;
    };
    body.line_color = colour;
    body.line_width = width;
    body.line_art = line_art;
    let encoded = body.encode();
    if obj.body == encoded {
        false
    } else {
        obj.body = encoded;
        true
    }
}

fn apply_fill_attributes(
    pool: &mut ObjectPool,
    id: ObjectID,
    fill_type: u8,
    colour: u8,
    pattern: ObjectID,
) -> bool {
    if fill_type > 3 {
        return false;
    }
    if pattern != ObjectID::NULL
        && !pool
            .find(pattern)
            .is_some_and(|object| object.r#type == ObjectType::PictureGraphic)
    {
        return false;
    }
    if fill_type == 3
        && pattern != ObjectID::NULL
        && !pool
            .find(pattern)
            .and_then(|object| object.get_picture_graphic_body().ok())
            .is_some_and(|body| picture_graphic_fill_pattern_buffer_is_valid(&body))
    {
        return false;
    }
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    if obj.r#type != ObjectType::FillAttributes {
        return false;
    }
    let Ok(mut body) = obj.get_fill_attributes_body() else {
        return false;
    };
    body.fill_type = fill_type;
    body.fill_color = colour;
    body.fill_pattern = pattern;
    let Ok(encoded) = body.encode() else {
        return false;
    };
    if obj.body == encoded {
        false
    } else {
        obj.body = encoded;
        true
    }
}

fn apply_size(pool: &mut ObjectPool, id: ObjectID, width: u16, height: u16) -> bool {
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    match obj.r#type {
        ObjectType::Container => {
            let Ok(mut body) = obj.get_container_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            obj.body = body.encode();
            true
        }
        ObjectType::Button => {
            let Ok(mut body) = obj.get_button_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            obj.body = body.encode();
            true
        }
        ObjectType::OutputString => {
            let Ok(mut body) = obj.get_output_string_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::OutputNumber => {
            let Ok(mut body) = obj.get_output_number_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputString => {
            let Ok(mut body) = obj.get_input_string_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputNumber => {
            let Ok(mut body) = obj.get_input_number_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputList => {
            let Ok(mut body) = obj.get_input_list_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::OutputList => {
            let Ok(mut body) = obj.get_output_list_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Line => {
            let Ok(mut body) = obj.get_output_line_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Rectangle => {
            let Ok(mut body) = obj.get_output_rectangle_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Ellipse => {
            let Ok(mut body) = obj.get_output_ellipse_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Polygon => {
            let Ok(mut body) = obj.get_output_polygon_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::LinearBarGraph => {
            let Ok(mut body) = obj.get_linear_bar_graph_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::ArchedBarGraph => {
            let Ok(mut body) = obj.get_arched_bar_graph_body() else {
                return false;
            };
            body.width = width;
            body.height = height;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::InputBoolean => {
            let Ok(mut body) = obj.get_input_boolean_body() else {
                return false;
            };
            body.width = width;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Meter => {
            let Ok(mut body) = obj.get_meter_body() else {
                return false;
            };
            body.width = width;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        _ => false,
    }
}

fn apply_priority(pool: &mut ObjectPool, id: ObjectID, priority: u8) -> bool {
    if priority > 2 {
        return false;
    }
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    if obj.r#type != ObjectType::AlarmMask {
        return false;
    }
    let Ok(mut body) = obj.get_alarm_mask_body() else {
        return false;
    };
    body.priority = priority;
    let Ok(encoded) = body.encode() else {
        return false;
    };
    obj.body = encoded;
    true
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

fn object_label_state_is_valid(pool: &ObjectPool, label: ObjectLabelState) -> bool {
    if !is_standard_font_type(label.font_type) {
        return false;
    }
    if label.string_variable != ObjectID::NULL
        && !pool
            .find(label.string_variable)
            .is_some_and(|object| object.r#type == ObjectType::StringVariable)
    {
        return false;
    }
    label.graphic_designator == ObjectID::NULL
        || pool
            .find(label.graphic_designator)
            .is_some_and(|object| is_object_label_graphic_representation_type(object.r#type))
}

fn select_input_report_is_valid(pool: &ObjectPool, id: ObjectID, option: u8) -> bool {
    match option {
        0xFF => {
            id == ObjectID::NULL
                || pool
                    .find(id)
                    .is_some_and(|object| is_select_input_object_type(object.r#type, 4))
        }
        0x00 => {
            id != ObjectID::NULL
                && pool
                    .find(id)
                    .is_some_and(|object| is_select_input_open_target_type(object.r#type))
        }
        _ => false,
    }
}

fn colour_selection_report_is_valid(pool: &ObjectPool, id: ObjectID) -> bool {
    id == ObjectID::NULL
        || pool.find(id).is_some_and(|object| {
            matches!(
                object.r#type,
                ObjectType::ColourMap | ObjectType::ColourPalette
            )
        })
}

fn active_mask_report_is_valid(pool: &ObjectPool, working_set: ObjectID, mask: ObjectID) -> bool {
    pool.find(working_set)
        .is_some_and(|object| object.r#type == ObjectType::WorkingSet)
        && pool.find(mask).is_some_and(|object| {
            matches!(object.r#type, ObjectType::DataMask | ObjectType::AlarmMask)
        })
}

fn apply_end_point(
    pool: &mut ObjectPool,
    id: ObjectID,
    width: u16,
    height: u16,
    line_direction: u8,
) -> bool {
    if line_direction > 1 {
        return false;
    }
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    if obj.r#type != ObjectType::Line {
        return false;
    }
    let Ok(mut body) = obj.get_output_line_body() else {
        return false;
    };
    body.width = width;
    body.height = height;
    body.line_direction = line_direction;
    let Ok(encoded) = body.encode() else {
        return false;
    };
    obj.body = encoded;
    true
}

fn generic_attribute_report_is_valid(
    pool: &ObjectPool,
    id: ObjectID,
    attribute_id: u8,
    value: u32,
) -> bool {
    let Some(obj) = pool.find(id) else {
        return false;
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
    let reference = ObjectID::new(value as u16);
    match (obj.r#type, attribute_id) {
        (ObjectType::DataMask | ObjectType::AlarmMask, 2) => {
            reference == ObjectID::NULL
                || macro_pool_reference_has_type(pool, reference, ObjectType::SoftKeyMask)
        }
        (ObjectType::AlarmMask, 3) => value <= 2,
        (ObjectType::AlarmMask, 4) => value <= 3,
        (ObjectType::WindowMask, 1) => (1..=2).contains(&value),
        (ObjectType::WindowMask, 2) => (1..=6).contains(&value),
        (ObjectType::WindowMask, 3) => {
            value <= 18
                && obj.get_window_mask_body().is_ok_and(|body| {
                    macro_window_mask_required_objects_match(pool, &body, value as u8)
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
                || macro_pool_reference_has_type(pool, reference, ObjectType::ExternalReferenceName)
        }
        (ObjectType::OutputString, 4)
        | (ObjectType::OutputNumber, 4)
        | (ObjectType::InputString, 4)
        | (ObjectType::InputNumber, 4) => {
            reference != ObjectID::NULL
                && macro_pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
        }
        (ObjectType::OutputString, 5) => value <= 0x03,
        (ObjectType::OutputString, 6) | (ObjectType::InputString, 7) => {
            macro_pool_reference_has_type(pool, reference, ObjectType::StringVariable)
        }
        (ObjectType::OutputString, 7) | (ObjectType::InputString, 8) => {
            value <= u32::from(u8::MAX) && text_justification_is_valid(value as u8)
        }
        (ObjectType::InputString, 5) => macro_pool_reference_has_any_type(
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
                && macro_pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
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
            macro_pool_reference_has_type(pool, reference, ObjectType::NumberVariable)
        }
        (ObjectType::FillAttributes, 1 | 3) => {
            macro_fill_attribute_generic_report_is_valid(pool, obj, attribute_id, value)
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
        (ObjectType::InputNumber, 7) => obj
            .get_input_number_body()
            .is_ok_and(|body| (value as i32) <= body.max_value),
        (ObjectType::InputNumber, 8) => obj
            .get_input_number_body()
            .is_ok_and(|body| body.min_value <= value as i32),
        (ObjectType::FontAttributes, 2) => obj.get_font_attributes_body().is_ok_and(|body| {
            value <= u32::from(u8::MAX)
                && is_standard_font_size_for_style(value as u8, body.font_style)
        }),
        (ObjectType::FontAttributes, 3) => {
            value <= u32::from(u8::MAX) && is_standard_font_type(value as u8)
        }
        (ObjectType::FontAttributes, 4) => obj.get_font_attributes_body().is_ok_and(|body| {
            value <= u32::from(u8::MAX)
                && is_standard_font_size_for_style(body.font_size, value as u8)
        }),
        (ObjectType::Line | ObjectType::Rectangle | ObjectType::Ellipse, 1) => {
            macro_pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::Polygon, 3) => {
            reference != ObjectID::NULL
                && macro_pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::Line, 4) => value <= 1,
        (ObjectType::Rectangle, 4) => value <= 0x0F,
        (ObjectType::Rectangle, 5) | (ObjectType::Ellipse, 7) | (ObjectType::Polygon, 4) => {
            macro_pool_reference_has_type(pool, reference, ObjectType::FillAttributes)
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
            value <= u32::from(u8::MAX) && scaled_graphic_scale_type_is_valid(value as u8)
        }
        (ObjectType::ScaledGraphic, 4) => value <= 0x01,
        (ObjectType::ScaledGraphic, 5) => scaled_graphic_value_source_is_valid(pool, reference),
        (ObjectType::Animation, 4 | 6 | 7 | 8) => {
            macro_animation_attribute_index_is_valid(obj, attribute_id, value)
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
                || macro_pool_reference_has_type(pool, reference, ObjectType::FontAttributes)
        }
        (ObjectType::GraphicContext, 13) => {
            reference == ObjectID::NULL
                || macro_pool_reference_has_type(pool, reference, ObjectType::LineAttributes)
        }
        (ObjectType::GraphicContext, 14) => {
            reference == ObjectID::NULL
                || macro_pool_reference_has_type(pool, reference, ObjectType::FillAttributes)
        }
        (ObjectType::GraphicContext, 15) => value <= 2,
        (ObjectType::GraphicContext, 16) => value <= 0x03,
        (ObjectType::ColourPalette, 1) => value == 0,
        (ObjectType::WorkingSetSpecialControls, 2) => {
            macro_pool_reference_has_type(pool, reference, ObjectType::ColourMap)
        }
        (ObjectType::WorkingSetSpecialControls, 3) => {
            macro_pool_reference_has_type(pool, reference, ObjectType::ColourPalette)
        }
        _ => true,
    }
}

fn macro_fill_attribute_generic_report_is_valid(
    pool: &ObjectPool,
    object: &crate::isobus::vt::VTObject,
    attribute_id: u8,
    value: u32,
) -> bool {
    let Ok(body) = object.get_fill_attributes_body() else {
        return false;
    };
    match attribute_id {
        1 => {
            value <= 3
                && (value != 3
                    || body.fill_pattern == ObjectID::NULL
                    || macro_fill_pattern_reference_has_valid_buffer(pool, body.fill_pattern))
        }
        3 => {
            let pattern = ObjectID(value as u16);
            (pattern == ObjectID::NULL
                || pool
                    .find(pattern)
                    .is_some_and(|object| object.r#type == ObjectType::PictureGraphic))
                && (body.fill_type != 3
                    || pattern == ObjectID::NULL
                    || macro_fill_pattern_reference_has_valid_buffer(pool, pattern))
        }
        _ => true,
    }
}

fn macro_fill_pattern_reference_has_valid_buffer(pool: &ObjectPool, reference: ObjectID) -> bool {
    pool.find(reference)
        .filter(|object| object.r#type == ObjectType::PictureGraphic)
        .and_then(|object| object.get_picture_graphic_body().ok())
        .is_some_and(|body| picture_graphic_fill_pattern_buffer_is_valid(&body))
}

fn macro_pool_reference_has_type(
    pool: &ObjectPool,
    reference: ObjectID,
    expected: ObjectType,
) -> bool {
    reference == ObjectID::NULL
        || pool
            .find(reference)
            .is_some_and(|object| object.r#type == expected)
}

fn macro_pool_reference_has_any_type(
    pool: &ObjectPool,
    reference: ObjectID,
    expected: &[ObjectType],
) -> bool {
    reference == ObjectID::NULL
        || pool
            .find(reference)
            .is_some_and(|object| expected.contains(&object.r#type))
}

fn macro_animation_attribute_index_is_valid(
    obj: &crate::isobus::vt::VTObject,
    attribute_id: u8,
    value: u32,
) -> bool {
    let child_count = obj.children_pos.len();
    if child_count == 0 || value > u32::from(u8::MAX) {
        return false;
    }
    match attribute_id {
        4 => value == 255 || (value as usize) < child_count,
        6..=8 => (value as usize) < child_count,
        _ => false,
    }
}

fn macro_window_mask_required_objects_match(
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
                    && pool
                        .find(reference)
                        .is_some_and(|object| object.r#type == expected_type)
            })
}

fn apply_soft_key_mask(
    pool: &mut ObjectPool,
    mask_type: u8,
    data_mask: ObjectID,
    soft_key_mask: ObjectID,
) -> bool {
    if soft_key_mask != ObjectID::NULL
        && !pool
            .find(soft_key_mask)
            .is_some_and(|obj| obj.r#type == ObjectType::SoftKeyMask)
    {
        return false;
    }
    let Some(obj) = pool.find_mut(data_mask) else {
        return false;
    };
    if !change_soft_key_mask_type_matches(mask_type, obj.r#type) {
        return false;
    }
    match obj.r#type {
        ObjectType::DataMask => {
            let Ok(mut body) = obj.get_data_mask_body() else {
                return false;
            };
            body.soft_key_mask = soft_key_mask;
            obj.body = body.encode();
            true
        }
        ObjectType::AlarmMask => {
            let Ok(mut body) = obj.get_alarm_mask_body() else {
                return false;
            };
            body.soft_key_mask = soft_key_mask;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        _ => false,
    }
}

fn apply_list_item(pool: &mut ObjectPool, list: ObjectID, index: usize, item: ObjectID) -> bool {
    if item != ObjectID::NULL && pool.find(item).is_none() {
        return false;
    }
    if pool
        .find(list)
        .is_some_and(|obj| obj.r#type == ObjectType::OutputList)
        && !output_list_item_reference_is_valid(pool, item)
    {
        return false;
    }
    let Some(obj) = pool.find_mut(list) else {
        return false;
    };
    match obj.r#type {
        ObjectType::InputList => {
            let Ok(mut body) = obj.get_input_list_body() else {
                return false;
            };
            let Some(slot) = body.items.get_mut(index) else {
                return false;
            };
            *slot = item;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::OutputList => {
            let Ok(mut body) = obj.get_output_list_body() else {
                return false;
            };
            let Some(slot) = body.items.get_mut(index) else {
                return false;
            };
            *slot = item;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::ExternalObjectDefinition => {
            let Ok(mut body) = obj.get_external_object_definition_body() else {
                return false;
            };
            let Some(slot) = body.object_ids.get_mut(index) else {
                return false;
            };
            *slot = item;
            let Ok(encoded) = body.encode() else {
                return false;
            };
            obj.body = encoded;
            true
        }
        ObjectType::Animation => {
            let Some(slot) = obj.children_pos.get_mut(index) else {
                return false;
            };
            slot.id = item;
            obj.children = obj.children_pos.iter().map(|child| child.id).collect();
            true
        }
        _ => false,
    }
}

fn apply_polygon_point(pool: &mut ObjectPool, id: ObjectID, index: usize, x: u16, y: u16) -> bool {
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    if obj.r#type != ObjectType::Polygon {
        return false;
    }
    let Ok(mut body) = obj.get_output_polygon_body() else {
        return false;
    };
    let Some(point) = body.points.get_mut(index) else {
        return false;
    };
    *point = PolygonPoint { x, y };
    let Ok(encoded) = body.encode() else {
        return false;
    };
    obj.body = encoded;
    true
}

fn apply_polygon_scale(
    pool: &mut ObjectPool,
    id: ObjectID,
    new_width: u16,
    new_height: u16,
) -> bool {
    let Some(obj) = pool.find_mut(id) else {
        return false;
    };
    if obj.r#type != ObjectType::Polygon {
        return false;
    }
    let Ok(mut body) = obj.get_output_polygon_body() else {
        return false;
    };
    if body.width == 0 || body.height == 0 {
        return false;
    }
    let old_width = u32::from(body.width);
    let old_height = u32::from(body.height);
    let next_width = u32::from(new_width);
    let next_height = u32::from(new_height);
    for point in &mut body.points {
        point.x = scale_polygon_coordinate(point.x, next_width, old_width);
        point.y = scale_polygon_coordinate(point.y, next_height, old_height);
    }
    body.width = new_width;
    body.height = new_height;
    let Ok(encoded) = body.encode() else {
        return false;
    };
    obj.body = encoded;
    true
}

fn scale_polygon_coordinate(old: u16, new_extent: u32, old_extent: u32) -> u16 {
    (((u32::from(old) * new_extent) + (old_extent / 2)) / old_extent).min(u32::from(u16::MAX))
        as u16
}
