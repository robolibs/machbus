use alloc::collections::BTreeMap as HashMap;

use crate::isobus::vt::commands::KeyActivationCode;
use crate::isobus::vt::objects::is_select_input_open_target_type;
use crate::isobus::vt::objects::{
    change_attribute_targets_one_byte_field, change_attribute_targets_two_byte_field,
    change_soft_key_mask_type_matches, external_object_pointer_default_is_valid_for_context,
    is_enable_disable_object_type, is_standard_font_size_for_style, is_standard_font_type,
    key_group_icon_reference_is_valid, key_group_name_reference_is_valid,
    object_pointer_numeric_value_is_valid_for_context, output_list_item_reference_is_valid,
    picture_graphic_fill_pattern_buffer_is_valid, scaled_graphic_scale_type_is_valid,
    scaled_graphic_value_source_is_valid, text_justification_is_valid,
    vt_change_attribute_id_is_supported,
    window_mask_icon_reference_is_valid, window_mask_required_object_types,
    window_mask_text_reference_is_valid,
};
use crate::isobus::vt::render::gtui::{GraphicsContextCopySource, GtuiRenderer, RenderCommand};
use crate::isobus::vt::render::bus_message::{
    VtBusMessage, validate_vt_to_ecu_envelope,
};
use crate::isobus::vt::render::input::{InputRuntime, OperatorEvent, VtEvent};
use crate::isobus::vt::render::layout::{
    LayoutConfig, LayoutEngine, PlacementMap, RuntimeOverrides, window_mask_cell_span,
};
use crate::isobus::vt::render::macros::{MacroEffect, MacroTriggerIndex, decode_macro_effects};
use crate::isobus::vt::render::scene::{
    FillPattern, NodeKind, Rect, Scene, SceneNode, SoftKeyKind,
};
use crate::isobus::vt::render::style::{
    Colour, FillType, FontDecoration, FontMetrics, Palette, ResolvedStyle,
};
use crate::isobus::vt::render::text::{self as text_layout, HorizontalAlign, VerticalAlign};
use crate::isobus::vt::server_working_set::{
    GraphicsContextCommand, MaskLockState, ObjectLabelState, ServerObjectState, ServerRenderEffect,
    graphics_context_payload_is_canonical,
};
use crate::isobus::vt::{
    ChildRef, ObjectID, ObjectPool, ObjectType, PolygonPoint, ServerWorkingSet, VTObject,
};
use crate::net::{Address, Error, Message, Result};

/// Result of a render-runtime mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderUpdate {
    /// No scene-affecting state changed.
    Unchanged,
    /// The command is valid VT state, but this retained renderer does not draw it.
    NotRenderAffecting { reason: &'static str },
    /// The scene graph is unchanged, but the backend command stream changed.
    CommandStreamChanged { reason: &'static str },
    /// The scene was rebuilt for a new or refreshed active mask.
    SceneRebuilt { active_mask: ObjectID },
}

/// Render-facing form of ECU→VT commands.
///
/// `VTServer` remains the bus decoder/validator. This enum is the renderer's
/// semantic command surface: accepted ECU commands either mutate the retained
/// scene state or report that they are not render-affecting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtRuntimeCommand {
    HideShow {
        id: ObjectID,
        visible: bool,
    },
    EnableDisable {
        id: ObjectID,
        enabled: bool,
    },
    SelectInputObject {
        id: ObjectID,
        open_for_input: bool,
    },
    Esc,
    ChangeChildLocation {
        parent: ObjectID,
        child: ObjectID,
        x: u8,
        y: u8,
    },
    ChangeChildPosition {
        parent: ObjectID,
        child: ObjectID,
        x: u16,
        y: u16,
    },
    ChangeSize {
        id: ObjectID,
        width: u16,
        height: u16,
    },
    ChangeEndPoint {
        id: ObjectID,
        width: u16,
        height: u16,
        line_direction: u8,
    },
    ChangeBackgroundColour {
        id: ObjectID,
        colour: u8,
    },
    ChangeNumericValue {
        id: ObjectID,
        value: u32,
    },
    ChangeStringValue {
        id: ObjectID,
        text: String,
    },
    ChangeFontAttributes {
        id: ObjectID,
        attributes: ObjectID,
    },
    ChangeFontAttributeValues {
        id: ObjectID,
        colour: u8,
        size: u8,
        font_type: u8,
        style: u8,
    },
    ChangeLineAttributes {
        id: ObjectID,
        attributes: ObjectID,
    },
    ChangeLineAttributeValues {
        id: ObjectID,
        colour: u8,
        width: u8,
        line_art: u16,
    },
    ChangeFillAttributes {
        id: ObjectID,
        attributes: ObjectID,
    },
    ChangeFillAttributeValues {
        id: ObjectID,
        fill_type: u8,
        colour: u8,
        pattern: ObjectID,
    },
    ChangeActiveMask {
        mask: ObjectID,
    },
    ChangeSoftKeyMask {
        data_mask: ObjectID,
        soft_key_mask: ObjectID,
    },
    ChangeGenericAttribute {
        id: ObjectID,
        attribute_id: u8,
        value: u32,
    },
    ChangePriority {
        id: ObjectID,
        priority: u8,
    },
    ChangeListItem {
        list: ObjectID,
        index: u8,
        item: ObjectID,
    },
    LockUnlockMask {
        id: ObjectID,
        locked: bool,
        timeout_ms: u16,
    },
    ExecuteMacro {
        id: ObjectID,
    },
    ChangeObjectLabel {
        id: ObjectID,
        label: ObjectLabelState,
    },
    ChangePolygonPoint {
        id: ObjectID,
        index: u8,
        x: u16,
        y: u16,
    },
    ChangePolygonScale {
        id: ObjectID,
        width: u16,
        height: u16,
    },
    SelectColourMap {
        id: ObjectID,
    },
    GraphicsContext {
        id: ObjectID,
        subcommand: u8,
        payload: Vec<u8>,
    },
    AudioSignal,
    SetAudioVolume {
        percent: u8,
    },
}

/// Host-persistable VT user-layout placement.
///
/// The render runtime keeps these records separate from the uploaded object
/// pool: they model the VT/operator-selected user-layout mapping that ISO
/// expects the terminal to recall from non-volatile storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserLayoutPlacement {
    WindowMask { id: ObjectID, column: u8, row: u8 },
    KeyGroup { id: ObjectID, first_cell: u8 },
}

impl UserLayoutPlacement {
    #[inline]
    #[must_use]
    pub const fn object_id(self) -> ObjectID {
        match self {
            Self::WindowMask { id, .. } | Self::KeyGroup { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SoftKeyPressState {
    id: ObjectID,
    kind: SoftKeyKind,
    source: SoftKeyPressSource,
}

impl SoftKeyPressState {
    const fn pointer(id: ObjectID, kind: SoftKeyKind) -> Self {
        Self {
            id,
            kind,
            source: SoftKeyPressSource::Pointer,
        }
    }

    const fn physical(id: ObjectID, kind: SoftKeyKind) -> Self {
        Self {
            id,
            kind,
            source: SoftKeyPressSource::Physical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoftKeyPressSource {
    Pointer,
    Physical,
}

/// Backend-neutral live runtime for one VT working set.
#[derive(Debug, Clone)]
pub struct VtRenderRuntime {
    pool: ObjectPool,
    active_mask: ObjectID,
    selected_colour_map: ObjectID,
    selected_colour_palette: Option<ObjectID>,
    object_labels: HashMap<ObjectID, ObjectLabelState>,
    graphics_contexts: Vec<GraphicsContextCommand>,
    mask_locks: HashMap<ObjectID, MaskLockState>,
    locked_scene_dirty: bool,
    locked_scene_changed_objects: Vec<ObjectID>,
    soft_key_page: u16,
    soft_key_pointer_down: Option<SoftKeyPressState>,
    pointing_parent: Option<ObjectID>,
    activation_hold: Option<ActivationHoldState>,
    pending_activation_events: Vec<VtEvent>,
    animation_elapsed_ms: u32,
    animation_elapsed_by_object: HashMap<ObjectID, u32>,
    overrides: RuntimeOverrides,
    user_layout_placements: PlacementMap,
    user_layout_selection: HashMap<ObjectID, UserLayoutPlacement>,
    engine: LayoutEngine,
    scene: Scene,
    input: InputRuntime,
    dirty: bool,
}

#[derive(Debug, Clone)]
struct RenderInitialState {
    requested_mask: ObjectID,
    selected_colour_map: ObjectID,
    selected_colour_palette: Option<ObjectID>,
    object_labels: HashMap<ObjectID, ObjectLabelState>,
    graphics_contexts: Vec<GraphicsContextCommand>,
    mask_locks: HashMap<ObjectID, MaskLockState>,
    selected_input_object: ObjectID,
    open_input_object: ObjectID,
    overrides: RuntimeOverrides,
    user_layout_placements: PlacementMap,
    user_layout_selection: HashMap<ObjectID, UserLayoutPlacement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SoftKeyPointerEvent {
    Tap,
    Down,
    Move,
    Up,
}

/// Timing policy for repeated VT `Held` activation events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationHoldTiming {
    /// Delay from `Pressed` to the first generated `Held` event.
    pub initial_delay_ms: u32,
    /// Interval between subsequent generated `Held` events.
    pub repeat_interval_ms: u32,
}

/// Result of advancing the runtime-owned animation clock.
///
/// Hosted event loops can use this as a compact "tick" result: redraw only
/// when `update` rebuilds the scene, and schedule the next timer from
/// `next_refresh_interval_ms`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationTick {
    /// Render update caused by the elapsed-time advance.
    pub update: RenderUpdate,
    /// Smallest visible Animation refresh interval after applying the tick.
    pub next_refresh_interval_ms: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ActivationHoldState {
    target: ActivationHoldTarget,
    elapsed_ms: u32,
    next_due_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActivationHoldTarget {
    SoftKey(ObjectID),
    Button(ObjectID),
}

impl Default for ActivationHoldTiming {
    fn default() -> Self {
        Self {
            initial_delay_ms: Self::DEFAULT_INITIAL_DELAY_MS,
            repeat_interval_ms: Self::DEFAULT_REPEAT_INTERVAL_MS,
        }
    }
}

impl ActivationHoldTiming {
    pub const DEFAULT_INITIAL_DELAY_MS: u32 = 500;
    pub const DEFAULT_REPEAT_INTERVAL_MS: u32 = 200;

    #[inline]
    #[must_use]
    pub const fn new(initial_delay_ms: u32, repeat_interval_ms: u32) -> Self {
        Self {
            initial_delay_ms,
            repeat_interval_ms,
        }
    }
}

impl ActivationHoldState {
    #[inline]
    #[must_use]
    const fn new(target: ActivationHoldTarget) -> Self {
        Self {
            target,
            elapsed_ms: 0,
            next_due_ms: ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS,
        }
    }
}

impl Default for RenderInitialState {
    fn default() -> Self {
        Self {
            requested_mask: ObjectID::NULL,
            selected_colour_map: ObjectID::NULL,
            selected_colour_palette: None,
            object_labels: HashMap::new(),
            graphics_contexts: Vec::new(),
            mask_locks: HashMap::new(),
            selected_input_object: ObjectID::NULL,
            open_input_object: ObjectID::NULL,
            overrides: RuntimeOverrides::new(),
            user_layout_placements: PlacementMap::new(),
            user_layout_selection: HashMap::new(),
        }
    }
}

impl RenderInitialState {
    fn with_working_set_special_controls(mut self, pool: &ObjectPool) -> Self {
        if self.selected_colour_palette.is_some() || self.selected_colour_map != ObjectID::NULL {
            return self;
        }
        let Some(body) = pool
            .objects()
            .iter()
            .find(|obj| obj.r#type == ObjectType::WorkingSetSpecialControls)
            .and_then(|obj| obj.get_working_set_special_controls_body().ok())
        else {
            return self;
        };
        if body.colour_map != ObjectID::NULL {
            self.selected_colour_map = body.colour_map;
        }
        if body.colour_palette != ObjectID::NULL {
            self.selected_colour_palette = Some(body.colour_palette);
        }
        self
    }
}
