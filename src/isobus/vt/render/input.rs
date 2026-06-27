//! Input / focus / edit / soft-key runtime.
//!
//! The layout engine produces a static [`Scene`] snapshot. The input
//! runtime layers operator interaction on top of it: it tracks which
//! input field currently has focus, what edit state an in-progress
//! string/number edit is in, which soft key is latched, and translates
//! high-level operator events into VT-style semantic events that a VT
//! server bridges back onto the bus.
//!
//! The runtime is intentionally **backend-agnostic**: it does not own a
//! windowing system or a keyboard. A host terminal feeds it
//! [`OperatorEvent`]s and reads back [`VtEvent`]s to forward.
//! Button pointer activations are sequenced as activation-code transitions:
//! press emits `Pressed`, matching release emits `Released`, and drag-off
//! release emits `Aborted`.
//! Input Number and Input List objects distinguish normal editing from
//! real-time editing: normal editing previews until commit, while real-time
//! editing emits complete value-change events as the operator changes values.
//!
//! [`Scene`]: crate::isobus::vt::render::scene::Scene

pub use crate::isobus::vt::render::bus_message::{
    VtBusMessage, VtBusMessageKind, validate_vt_to_ecu_envelope,
};

use crate::isobus::vt::commands::KeyActivationCode;
use crate::isobus::vt::render::scene::{NodeKind, Scene, SceneNode, SoftKeyKind};
use crate::isobus::vt::{ObjectID, ObjectType};

/// An operator-facing event delivered by the host terminal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperatorEvent {
    /// Move focus to the next / previous interactive field.
    FocusNext,
    FocusPrev,
    /// Focus a specific object (e.g. via touch).
    Tap(i32, i32),
    /// Pointer/button press at device coordinates.
    PointerDown(i32, i32),
    /// Pointer movement at device coordinates.
    PointerMove(i32, i32),
    /// Pointer/button release at device coordinates.
    PointerUp(i32, i32),
    /// Hardware key activation from a host terminal/keypad.
    HardwareKey(u8),
    /// Activate a zero-based physical soft-key cell.
    ///
    /// Hosted terminals with real soft-key hardware should use this instead
    /// of manufacturing pointer coordinates. Application cells can be routed
    /// by [`InputRuntime`] directly; navigation cells need
    /// [`VtRenderRuntime`](crate::isobus::vt::render::runtime::VtRenderRuntime)
    /// because page changes rebuild the scene.
    PhysicalSoftKey(u8),
    /// Press a zero-based physical soft-key cell without releasing it yet.
    ///
    /// Use this for hardware/backlit-button hosts that need standard
    /// `Pressed`/held-repeat/`Released` activation sequencing.
    PhysicalSoftKeyDown(u8),
    /// Release a zero-based physical soft-key cell previously pressed with
    /// [`OperatorEvent::PhysicalSoftKeyDown`].
    PhysicalSoftKeyUp(u8),
    /// Activate the currently focused / hovered soft key by id.
    SoftKeyActivate(ObjectID),
    /// Activate a host-reserved soft-key navigation cell.
    SoftKeyNavigation(SoftKeyKind),
    /// A character typed into the focused input field.
    Char(char),
    /// A backspace on the focused input field.
    Backspace,
    /// Commit the in-progress edit (Enter / accept soft key).
    Commit,
    /// Cancel the in-progress edit (Esc / cancel soft key).
    Cancel,
}

/// A semantic VT event produced by the runtime, ready to be bridged
/// onto the bus by a VT server (value changes, focus changes, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VtEvent {
    /// Input Boolean `value` changed for the given object id.
    BooleanValueChanged { id: ObjectID, value: bool },
    /// Input String content changed for the given object id.
    StringValueChanged { id: ObjectID, text: String },
    /// In-progress string edit preview; not yet committed to the bus value.
    StringEditPreview { id: ObjectID, text: String },
    /// Input Number value changed for the given object id.
    NumberValueChanged { id: ObjectID, raw: u32 },
    /// In-progress number edit preview; not yet committed to the bus value.
    NumberEditPreview { id: ObjectID, raw: u32 },
    /// Input List selection changed for the given object id.
    ListSelectionChanged { id: ObjectID, index: usize },
    /// In-progress Input List selection preview; not yet committed.
    ListSelectionPreview { id: ObjectID, index: usize },
    /// An open input transaction was aborted by ESC/cancel.
    InputEsc {
        id: ObjectID,
        error_code: u8,
        transfer_sequence_number: Option<u8>,
    },
    /// A soft key was activated.
    SoftKeyActivated { id: ObjectID },
    /// A soft-key activation state transition was reported.
    SoftKeyActivation {
        id: ObjectID,
        code: KeyActivationCode,
    },
    /// Soft-key page changed via a host-reserved navigation cell.
    SoftKeyPageChanged { page: u16, page_count: u16 },
    /// A button object was activated.
    ButtonActivated { id: ObjectID },
    /// A button activation state transition was reported.
    ButtonActivation {
        id: ObjectID,
        code: KeyActivationCode,
    },
    /// A user-layout Window Mask / Key Group visibility notification.
    UserLayoutHideShow {
        first: (ObjectID, bool),
        second: Option<(ObjectID, bool)>,
        transfer_sequence_number: Option<u8>,
    },
    /// A touch/click/drag pointing event on a Data Mask or free-form Window Mask.
    PointingEvent {
        x: u16,
        y: u16,
        touch_state: KeyActivationCode,
        parent_mask: ObjectID,
        transfer_sequence_number: Option<u8>,
    },
    /// Focus moved to a new object.
    FocusChanged { id: ObjectID },
    /// The event could not be applied (no focus, disabled field, …).
    Ignored { reason: &'static str },
}

const fn is_input_object_type(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
    )
}

const fn is_select_input_node_type(object_type: ObjectType) -> bool {
    is_input_object_type(object_type) || matches!(object_type, ObjectType::Button)
}

const fn is_edit_open_input_type(object_type: ObjectType) -> bool {
    matches!(
        object_type,
        ObjectType::InputString | ObjectType::InputNumber | ObjectType::InputList
    )
}

/// Source of a held Key Group key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum KeyGroupPressSource {
    Pointer,
    Physical,
}

/// In-progress Key Group key activation state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct KeyGroupPressState {
    id: ObjectID,
    source: KeyGroupPressSource,
}

impl KeyGroupPressState {
    #[inline]
    const fn pointer(id: ObjectID) -> Self {
        Self {
            id,
            source: KeyGroupPressSource::Pointer,
        }
    }

    #[inline]
    const fn physical(id: ObjectID) -> Self {
        Self {
            id,
            source: KeyGroupPressSource::Physical,
        }
    }

    #[inline]
    pub(crate) const fn id(self) -> ObjectID {
        self.id
    }

    #[inline]
    const fn is_physical(self) -> bool {
        matches!(self.source, KeyGroupPressSource::Physical)
    }
}

/// In-progress edit state for the focused input field.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum EditState {
    #[default]
    Idle,
    /// Editing a string field, accumulating characters.
    String { buffer: String },
    /// Editing a number field, accumulating digits / sign.
    Number { buffer: String, real_time: bool },
    /// Editing a list field, holding the selected item index until commit.
    List { index: usize },
}

/// The input runtime. Owns focus + edit state for the currently
/// displayed scene.
#[derive(Debug, Clone)]
pub struct InputRuntime {
    focus_order: Vec<ObjectID>,
    focus_index: Option<usize>,
    selected_input: Option<ObjectID>,
    open_input: Option<ObjectID>,
    edit: EditState,
    soft_key_latched: Option<ObjectID>,
    pointer_down_button: Option<ObjectID>,
    pointer_down_button_point: Option<(i32, i32)>,
    pointer_down_key_group: Option<KeyGroupPressState>,
}

impl Default for InputRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl InputRuntime {
    #[must_use]
    pub fn new() -> Self {
        Self {
            focus_order: Vec::new(),
            focus_index: None,
            selected_input: None,
            open_input: None,
            edit: EditState::Idle,
            soft_key_latched: None,
            pointer_down_button: None,
            pointer_down_button_point: None,
            pointer_down_key_group: None,
        }
    }

    /// Re-bind the runtime to a freshly built scene: rebuilds the tab
    /// order from visible, enabled interactive nodes and clears any
    /// edit state that no longer applies.
    pub fn bind(&mut self, scene: &Scene) {
        let previous_selected = self.selected_input;
        let previous_open = self.open_input;
        self.focus_order = scene
            .nodes
            .iter()
            .filter(|n| n.visible && n.enabled && n.is_interactive())
            .map(|n| n.id)
            .collect();

        // Preserve focus by object identity across scene rebuilds. Rebuilds
        // can reorder nodes when masks, user-layout placements, or external
        // object materialisation change; preserving only the old focus index
        // would move an in-progress edit to whichever object occupies that
        // slot after the rebuild.
        if let Some(id) = previous_selected {
            if let Some(index) = self
                .focus_order
                .iter()
                .position(|&candidate| candidate == id)
            {
                self.focus_index = Some(index);
                self.selected_input = Some(id);
                let still_open = previous_open == Some(id)
                    && scene
                        .find(id)
                        .is_some_and(|node| is_edit_open_input_type(node.object_type));
                if still_open {
                    self.open_input = Some(id);
                } else {
                    self.open_input = None;
                    self.edit = EditState::Idle;
                }
            } else if scene
                .soft_keys
                .iter()
                .any(|key| key.id == id && key.visible && key.enabled)
                || scene_has_visible_key_group_key(scene, id)
            {
                self.focus_index = None;
                self.selected_input = Some(id);
                self.open_input = None;
                self.edit = EditState::Idle;
            } else {
                self.focus_index = None;
                self.selected_input = None;
                self.open_input = None;
                self.edit = EditState::Idle;
            }
        } else if self.focus_index.is_some() || self.open_input.is_some() {
            self.focus_index = None;
            self.open_input = None;
            self.edit = EditState::Idle;
        }
        self.soft_key_latched = None;
        self.pointer_down_button = None;
        self.pointer_down_button_point = None;
        self.pointer_down_key_group = None;
    }

    /// Currently focused object id, if any.
    #[must_use]
    pub fn focused(&self) -> Option<ObjectID> {
        self.focus_index
            .and_then(|i| self.focus_order.get(i).copied())
    }

    /// Currently selected input object.
    #[inline]
    #[must_use]
    pub const fn selected_input(&self) -> Option<ObjectID> {
        self.selected_input
    }

    /// Currently open input object with an in-progress edit transaction.
    #[inline]
    #[must_use]
    pub const fn open_input(&self) -> Option<ObjectID> {
        self.open_input
    }

    #[inline]
    #[must_use]
    pub const fn edit_state(&self) -> &EditState {
        &self.edit
    }

    #[inline]
    #[must_use]
    pub const fn soft_key_latched(&self) -> Option<ObjectID> {
        self.soft_key_latched
    }

    /// Button object that currently owns a pointer press, if any.
    #[inline]
    #[must_use]
    pub const fn pointer_down_button(&self) -> Option<ObjectID> {
        self.pointer_down_button
    }

    /// Device-pixel point that pressed the current Button, if any.
    #[inline]
    #[must_use]
    pub const fn pointer_down_button_point(&self) -> Option<(i32, i32)> {
        self.pointer_down_button_point
    }

    pub(crate) fn restore_pointer_down_button(&mut self, id: ObjectID, point: (i32, i32)) {
        self.pointer_down_button = Some(id);
        self.pointer_down_button_point = Some(point);
    }

    /// Key object from a placed Key Group that currently owns a pointer press.
    #[inline]
    #[must_use]
    pub const fn pointer_down_key_group_key(&self) -> Option<ObjectID> {
        match self.pointer_down_key_group {
            Some(state) => Some(state.id),
            None => None,
        }
    }

    #[inline]
    pub(crate) const fn pointer_down_key_group_press(&self) -> Option<KeyGroupPressState> {
        self.pointer_down_key_group
    }

    pub(crate) fn restore_pointer_down_key_group_press(&mut self, state: KeyGroupPressState) {
        self.pointer_down_key_group = Some(state);
    }

    pub(crate) fn restore_physical_key_group_key_press(&mut self, id: ObjectID) {
        self.pointer_down_key_group = Some(KeyGroupPressState::physical(id));
    }

    pub(crate) fn take_physical_key_group_key_press(&mut self) -> Option<ObjectID> {
        if self
            .pointer_down_key_group
            .is_some_and(KeyGroupPressState::is_physical)
        {
            return self
                .pointer_down_key_group
                .take()
                .map(KeyGroupPressState::id);
        }
        None
    }

    /// Apply an ECU-requested input selection to the current scene.
    ///
    /// Returns `true` when the runtime input/focus state changed. The command
    /// is interaction state, not draw state; hosts that visualize focus can use
    /// the changed flag to schedule their own repaint.
    pub fn select_input_object(
        &mut self,
        scene: &Scene,
        id: ObjectID,
        open_for_input: bool,
    ) -> bool {
        if id == ObjectID::NULL {
            let changed = self.focus_index.is_some()
                || self.selected_input.is_some()
                || self.open_input.is_some()
                || !matches!(self.edit, EditState::Idle)
                || self.pointer_down_button.is_some()
                || self.pointer_down_key_group.is_some();
            self.focus_index = None;
            self.selected_input = None;
            self.open_input = None;
            self.edit = EditState::Idle;
            self.pointer_down_button = None;
            self.pointer_down_button_point = None;
            self.pointer_down_key_group = None;
            return changed;
        }

        let (index, can_open) = if let Some(node) = scene.find(id) {
            if !node.visible || !node.enabled || !is_select_input_node_type(node.object_type) {
                return false;
            }
            if open_for_input && !is_input_object_type(node.object_type) {
                return false;
            }
            (
                self.focus_order
                    .iter()
                    .position(|&candidate| candidate == id),
                open_for_input && is_edit_open_input_type(node.object_type),
            )
        } else if scene
            .soft_keys
            .iter()
            .any(|key| key.id == id && key.visible && key.enabled)
            || scene_has_visible_key_group_key(scene, id)
        {
            if open_for_input {
                return false;
            }
            (None, false)
        } else {
            return false;
        };
        let changed = self.focus_index != index
            || self.selected_input != Some(id)
            || self.open_input != can_open.then_some(id)
            || !matches!(self.edit, EditState::Idle)
            || self.pointer_down_button.is_some()
            || self.pointer_down_key_group.is_some();
        self.focus_index = index;
        self.selected_input = Some(id);
        self.open_input = can_open.then_some(id);
        self.edit = EditState::Idle;
        self.pointer_down_button = None;
        self.pointer_down_button_point = None;
        self.pointer_down_key_group = None;
        changed
    }

    /// Abort the current open input transaction because the ECU sent ESC Input.
    ///
    /// Returns the aborted input object when there was one.
    pub fn abort_open_input(&mut self) -> Option<ObjectID> {
        let aborted = self.open_input.take();
        self.edit = EditState::Idle;
        self.pointer_down_button = None;
        self.pointer_down_button_point = None;
        self.pointer_down_key_group = None;
        aborted
    }

    fn clear_focus_and_edit_state(&mut self) {
        self.focus_index = None;
        self.selected_input = None;
        self.open_input = None;
        self.edit = EditState::Idle;
    }

    fn focused_available_node<'a>(
        &mut self,
        scene: &'a Scene,
        no_focus_reason: &'static str,
        vanished_reason: &'static str,
    ) -> Result<(ObjectID, &'a SceneNode), VtEvent> {
        let Some(focused) = self.focused() else {
            return Err(VtEvent::Ignored {
                reason: no_focus_reason,
            });
        };
        let Some(node) = scene.find(focused) else {
            self.clear_focus_and_edit_state();
            return Err(VtEvent::Ignored {
                reason: vanished_reason,
            });
        };
        if !node.visible || !node.enabled {
            self.clear_focus_and_edit_state();
            return Err(VtEvent::Ignored {
                reason: "focused node is disabled or hidden",
            });
        }
        Ok((focused, node))
    }

    /// Dispatch one operator event against the given scene. Returns the
    /// semantic VT event(s) the host should forward.
    #[must_use]
    pub fn handle(&mut self, scene: &Scene, event: &OperatorEvent) -> Vec<VtEvent> {
        match event {
            OperatorEvent::FocusNext => vec![self.move_focus(scene, 1)],
            OperatorEvent::FocusPrev => vec![self.move_focus(scene, -1)],
            OperatorEvent::Tap(px, py) => vec![self.tap(scene, *px, *py)],
            OperatorEvent::PointerDown(px, py) => vec![self.pointer_down(scene, *px, *py)],
            OperatorEvent::PointerMove(px, py) => vec![self.pointer_move(scene, *px, *py)],
            OperatorEvent::PointerUp(px, py) => vec![self.pointer_up(scene, *px, *py)],
            OperatorEvent::HardwareKey(code) => self.hardware_key(scene, *code),
            OperatorEvent::PhysicalSoftKey(cell) => self.physical_soft_key(scene, *cell),
            OperatorEvent::PhysicalSoftKeyDown(_) | OperatorEvent::PhysicalSoftKeyUp(_) => {
                vec![VtEvent::Ignored {
                    reason: "physical soft-key press/release requires render runtime",
                }]
            }
            OperatorEvent::SoftKeyActivate(id) => {
                if !scene.soft_keys.iter().any(|key| {
                    key.id == *id
                        && key.visible
                        && key.enabled
                        && key.kind == SoftKeyKind::Application
                }) && !scene_has_visible_key_group_key(scene, *id)
                {
                    return vec![VtEvent::Ignored {
                        reason: "soft-key activation target is not visible",
                    }];
                }
                self.soft_key_latched = Some(*id);
                vec![VtEvent::SoftKeyActivated { id: *id }]
            }
            OperatorEvent::SoftKeyNavigation(_) => vec![VtEvent::Ignored {
                reason: "soft-key navigation requires render runtime",
            }],
            OperatorEvent::Char(c) => vec![self.type_char(scene, *c)],
            OperatorEvent::Backspace => vec![self.backspace(scene)],
            OperatorEvent::Commit => self.commit(scene),
            OperatorEvent::Cancel => {
                let aborted_input = self.open_input;
                self.edit = EditState::Idle;
                self.open_input = None;
                self.pointer_down_button = None;
                self.pointer_down_button_point = None;
                self.pointer_down_key_group = None;
                if let Some(id) = aborted_input {
                    vec![VtEvent::InputEsc {
                        id,
                        error_code: 0,
                        transfer_sequence_number: None,
                    }]
                } else {
                    vec![VtEvent::Ignored {
                        reason: "no open input to cancel",
                    }]
                }
            }
        }
    }

    fn move_focus(&mut self, _scene: &Scene, delta: i32) -> VtEvent {
        if self.focus_order.is_empty() {
            return VtEvent::Ignored {
                reason: "no interactive fields available",
            };
        }
        let next = match self.focus_index {
            None => 0usize,
            Some(i) => {
                let len = self.focus_order.len() as i32;
                let raw = i as i32 + delta;
                let wrapped = ((raw % len) + len) % len;
                wrapped as usize
            }
        };
        self.focus_index = Some(next);
        self.selected_input = Some(self.focus_order[next]);
        self.open_input = None;
        self.edit = EditState::Idle;
        VtEvent::FocusChanged {
            id: self.focus_order[next],
        }
    }

    fn tap(&mut self, scene: &Scene, px: i32, py: i32) -> VtEvent {
        if let Some(key) = scene.soft_key_hit_test(px, py) {
            return match key.kind {
                SoftKeyKind::Application => {
                    self.soft_key_latched = Some(key.id);
                    VtEvent::SoftKeyActivated { id: key.id }
                }
                SoftKeyKind::NavigationNext | SoftKeyKind::NavigationPrevious => VtEvent::Ignored {
                    reason: "soft-key navigation requires render runtime",
                },
            };
        }
        if let Some(node) = scene.disabled_interactive_hit_test(px, py) {
            let reason = if matches!(node.kind, NodeKind::Button { .. }) {
                "button is disabled"
            } else {
                "input field is disabled"
            };
            return VtEvent::Ignored { reason };
        }
        if let Some(node) = scene.hit_test(px, py) {
            let id = node.id;
            if let NodeKind::KeyGroup {
                available: true,
                key_ids,
                ..
            } = &node.kind
            {
                let Some(&key_id) = key_group_key_at(node, key_ids, px, py) else {
                    return VtEvent::Ignored {
                        reason: "key group has no key under cursor",
                    };
                };
                self.soft_key_latched = Some(key_id);
                return VtEvent::SoftKeyActivated { id: key_id };
            }
            self.focus_index = self.focus_order.iter().position(|&i| i == id);
            self.selected_input = self.focused();
            self.open_input = None;
            self.edit = EditState::Idle;
            return match &node.kind {
                NodeKind::InputBoolean { value, .. } => {
                    VtEvent::BooleanValueChanged { id, value: !*value }
                }
                NodeKind::InputList {
                    selected,
                    item_count,
                    selectable_indices,
                    real_time_editing,
                    ..
                } => {
                    if *item_count == 0 || selectable_indices.is_empty() {
                        VtEvent::Ignored {
                            reason: "input list has no selectable items",
                        }
                    } else {
                        let selected_position = selectable_indices
                            .iter()
                            .position(|index| index == selected);
                        let index = selected_position.map_or(selectable_indices[0], |position| {
                            selectable_indices[(position + 1) % selectable_indices.len()]
                        });
                        if *real_time_editing {
                            self.open_input = None;
                            self.edit = EditState::Idle;
                            return VtEvent::ListSelectionChanged { id, index };
                        }
                        self.open_input = Some(id);
                        self.edit = EditState::List { index };
                        VtEvent::ListSelectionPreview { id, index }
                    }
                }
                NodeKind::Button { enabled: true, .. } => VtEvent::ButtonActivated { id },
                NodeKind::Button { enabled: false, .. } => VtEvent::Ignored {
                    reason: "button is disabled",
                },
                _ => VtEvent::FocusChanged { id },
            };
        }
        VtEvent::Ignored {
            reason: "tap missed all interactive nodes",
        }
    }

    fn type_char(&mut self, scene: &Scene, c: char) -> VtEvent {
        let (focused, node) = match self.focused_available_node(
            scene,
            "typing with no focused field",
            "focused node vanished",
        ) {
            Ok(focused) => focused,
            Err(event) => return event,
        };
        if node.object_type == ObjectType::InputNumber && !c.is_ascii_digit() {
            return VtEvent::Ignored {
                reason: "input number accepts only decimal digits",
            };
        }
        // InputString character-set enforcement (InputAttributes rule).
        if node.object_type == ObjectType::InputString
            && let NodeKind::InputString {
                validation: Some(rule),
                ..
            } = &node.kind
            && !rule.accepts(c)
        {
            return VtEvent::Ignored {
                reason: "character rejected by input validation",
            };
        }
        let max_length = match &node.kind {
            NodeKind::InputString { max_length, .. } => *max_length,
            _ => 0,
        };

        match (&mut self.edit, node.object_type) {
            (EditState::String { buffer }, ObjectType::InputString) => {
                if !input_string_can_accept(buffer, c, max_length) {
                    return VtEvent::Ignored {
                        reason: "input string maximum length reached",
                    };
                }
                self.open_input = Some(focused);
                buffer.push(c);
                VtEvent::StringEditPreview {
                    id: focused,
                    text: buffer.clone(),
                }
            }
            (EditState::Number { buffer, real_time }, ObjectType::InputNumber) => {
                let mut next = buffer.clone();
                next.push(c);
                let raw = next.parse::<u32>().unwrap_or(0);
                if *real_time && !input_number_node_accepts_raw(node, raw) {
                    return VtEvent::Ignored {
                        reason: "input number value outside range",
                    };
                }
                self.open_input = Some(focused);
                *buffer = next;
                if *real_time {
                    VtEvent::NumberValueChanged { id: focused, raw }
                } else {
                    VtEvent::NumberEditPreview { id: focused, raw }
                }
            }
            (_, ObjectType::InputString) => {
                if !input_string_can_accept("", c, max_length) {
                    return VtEvent::Ignored {
                        reason: "input string maximum length reached",
                    };
                }
                self.open_input = Some(focused);
                self.edit = EditState::String {
                    buffer: c.to_string(),
                };
                VtEvent::StringEditPreview {
                    id: focused,
                    text: c.to_string(),
                }
            }
            (_, ObjectType::InputNumber) => {
                let buf = c.to_string();
                let raw = buf.parse::<u32>().unwrap_or(0);
                let real_time = input_number_node_real_time(node);
                if real_time && !input_number_node_accepts_raw(node, raw) {
                    return VtEvent::Ignored {
                        reason: "input number value outside range",
                    };
                }
                self.open_input = Some(focused);
                self.edit = EditState::Number {
                    buffer: buf,
                    real_time,
                };
                if real_time {
                    VtEvent::NumberValueChanged { id: focused, raw }
                } else {
                    VtEvent::NumberEditPreview { id: focused, raw }
                }
            }
            _ => VtEvent::Ignored {
                reason: "focused node is not an editable text/number field",
            },
        }
    }

    fn backspace(&mut self, scene: &Scene) -> VtEvent {
        let (focused, node) = match self.focused_available_node(
            scene,
            "backspace with no focused field",
            "focused node vanished",
        ) {
            Ok(focused) => focused,
            Err(event) => return event,
        };
        match (&mut self.edit, node.object_type) {
            (EditState::String { buffer }, ObjectType::InputString) => {
                buffer.pop();
                VtEvent::StringEditPreview {
                    id: focused,
                    text: buffer.clone(),
                }
            }
            (EditState::Number { buffer, real_time }, ObjectType::InputNumber) => {
                let mut next = buffer.clone();
                next.pop();
                let raw = next.parse::<u32>().unwrap_or(0);
                if *real_time && !input_number_node_accepts_raw(node, raw) {
                    return VtEvent::Ignored {
                        reason: "input number value outside range",
                    };
                }
                *buffer = next;
                if *real_time {
                    VtEvent::NumberValueChanged { id: focused, raw }
                } else {
                    VtEvent::NumberEditPreview { id: focused, raw }
                }
            }
            _ => VtEvent::Ignored {
                reason: "focused node is not an editable text/number field",
            },
        }
    }

    fn commit(&mut self, scene: &Scene) -> Vec<VtEvent> {
        let (focused, node) = match self.focused_available_node(
            scene,
            "commit with no focused field",
            "focused node vanished",
        ) {
            Ok(focused) => focused,
            Err(event) => return vec![event],
        };
        let events = match &self.edit {
            EditState::String { buffer } => vec![VtEvent::StringValueChanged {
                id: focused,
                text: buffer.clone(),
            }],
            EditState::Number { buffer, real_time } => {
                if *real_time {
                    vec![VtEvent::Ignored {
                        reason: "real-time input value already sent",
                    }]
                } else {
                    let raw = buffer.parse::<u32>().unwrap_or(0);
                    if let NodeKind::InputNumber {
                        min_value,
                        max_value,
                        ..
                    } = &node.kind
                        && !input_number_in_range(raw, *min_value, *max_value)
                    {
                        return vec![VtEvent::Ignored {
                            reason: "input number value outside range",
                        }];
                    }
                    vec![VtEvent::NumberValueChanged { id: focused, raw }]
                }
            }
            EditState::List { index } => vec![VtEvent::ListSelectionChanged {
                id: focused,
                index: *index,
            }],
            EditState::Idle if matches!(node.kind, NodeKind::Button { enabled: true, .. }) => {
                vec![VtEvent::ButtonActivated { id: focused }]
            }
            EditState::Idle => vec![VtEvent::Ignored {
                reason: "commit with nothing being edited",
            }],
        };
        self.edit = EditState::Idle;
        self.open_input = None;
        events
    }

    fn pointer_move(&mut self, scene: &Scene, px: i32, py: i32) -> VtEvent {
        if let Some(state) = self.pointer_down_key_group {
            if state.is_physical() {
                return VtEvent::Ignored {
                    reason: "pointer move ignored for physical key-group press",
                };
            }
            let id = state.id;
            if let Some(node) = scene.hit_test(px, py)
                && let NodeKind::KeyGroup {
                    available: true,
                    key_ids,
                    ..
                } = &node.kind
                && key_group_key_at(node, key_ids, px, py).copied() == Some(id)
            {
                return VtEvent::Ignored {
                    reason: "pointer remains on pressed key",
                };
            }
            self.pointer_down_key_group = None;
            return VtEvent::SoftKeyActivation {
                id,
                code: KeyActivationCode::Aborted,
            };
        }

        if let Some(id) = self.pointer_down_button {
            if let Some(node) = scene.hit_test(px, py)
                && node.id == id
                && matches!(node.kind, NodeKind::Button { enabled: true, .. })
            {
                return VtEvent::Ignored {
                    reason: "pointer remains on pressed button",
                };
            }
            self.pointer_down_button = None;
            self.pointer_down_button_point = None;
            return VtEvent::ButtonActivation {
                id,
                code: KeyActivationCode::Aborted,
            };
        }

        if let Some(node) = scene.hit_test(px, py)
            && node.is_interactive()
        {
            return VtEvent::FocusChanged { id: node.id };
        }
        VtEvent::Ignored {
            reason: "pointer move outside interactive nodes",
        }
    }

    fn pointer_down(&mut self, scene: &Scene, px: i32, py: i32) -> VtEvent {
        if self.pointer_down_button.is_some() || self.pointer_down_key_group.is_some() {
            return VtEvent::Ignored {
                reason: "simultaneous soft-key/button activation is not supported",
            };
        }
        self.pointer_down_button = None;
        self.pointer_down_button_point = None;
        self.pointer_down_key_group = None;
        if let Some(node) = scene.hit_test(px, py) {
            let id = node.id;
            if let NodeKind::Button { enabled, .. } = &node.kind {
                if !enabled {
                    return VtEvent::Ignored {
                        reason: "button is disabled",
                    };
                }
                self.focus_index = self.focus_order.iter().position(|&i| i == id);
                self.selected_input = self.focused();
                self.open_input = None;
                self.edit = EditState::Idle;
                self.pointer_down_button = Some(id);
                self.pointer_down_button_point = Some((px, py));
                return VtEvent::ButtonActivation {
                    id,
                    code: KeyActivationCode::Pressed,
                };
            }
            if let NodeKind::KeyGroup {
                available: true,
                key_ids,
                ..
            } = &node.kind
            {
                let Some(&key_id) = key_group_key_at(node, key_ids, px, py) else {
                    return VtEvent::Ignored {
                        reason: "key group has no key under cursor",
                    };
                };
                self.soft_key_latched = Some(key_id);
                self.open_input = None;
                self.edit = EditState::Idle;
                self.pointer_down_key_group = Some(KeyGroupPressState::pointer(key_id));
                return VtEvent::SoftKeyActivation {
                    id: key_id,
                    code: KeyActivationCode::Pressed,
                };
            }
        }
        self.tap(scene, px, py)
    }

    fn pointer_up(&mut self, scene: &Scene, px: i32, py: i32) -> VtEvent {
        if self
            .pointer_down_key_group
            .is_some_and(KeyGroupPressState::is_physical)
        {
            return VtEvent::Ignored {
                reason: "pointer release ignored for physical key-group press",
            };
        }
        let pressed_key_group = self
            .pointer_down_key_group
            .take()
            .map(KeyGroupPressState::id);
        if let Some(id) = pressed_key_group {
            if let Some(node) = scene.hit_test(px, py)
                && let NodeKind::KeyGroup {
                    available: true,
                    key_ids,
                    ..
                } = &node.kind
                && key_group_key_at(node, key_ids, px, py).copied() == Some(id)
            {
                return VtEvent::SoftKeyActivation {
                    id,
                    code: KeyActivationCode::Released,
                };
            }
            return VtEvent::SoftKeyActivation {
                id,
                code: KeyActivationCode::Aborted,
            };
        }

        let pressed = self.pointer_down_button.take();
        self.pointer_down_button_point = None;
        if let Some(node) = scene.hit_test(px, py)
            && Some(node.id) == pressed
            && matches!(node.kind, NodeKind::Button { enabled: true, .. })
        {
            return VtEvent::ButtonActivation {
                id: node.id,
                code: KeyActivationCode::Released,
            };
        }
        if let Some(id) = pressed {
            return VtEvent::ButtonActivation {
                id,
                code: KeyActivationCode::Aborted,
            };
        }
        VtEvent::Ignored {
            reason: "pointer release did not activate a button",
        }
    }

    fn hardware_key(&mut self, scene: &Scene, code: u8) -> Vec<VtEvent> {
        match code {
            b'\t' => vec![self.move_focus(scene, 1)],
            0x1B => self.handle(scene, &OperatorEvent::Cancel),
            b'\r' | b'\n' => self.commit(scene),
            0x08 | 0x7F => vec![self.backspace(scene)],
            byte if byte.is_ascii() => vec![self.type_char(scene, char::from(byte))],
            _ => vec![VtEvent::Ignored {
                reason: "unsupported hardware key",
            }],
        }
    }

    fn physical_soft_key(&mut self, scene: &Scene, cell_index: u8) -> Vec<VtEvent> {
        let Some(key) = scene.soft_key_cell(cell_index) else {
            return vec![VtEvent::Ignored {
                reason: "physical soft-key cell is not available",
            }];
        };
        match key.kind {
            SoftKeyKind::Application => {
                self.soft_key_latched = Some(key.id);
                vec![VtEvent::SoftKeyActivated { id: key.id }]
            }
            SoftKeyKind::NavigationNext | SoftKeyKind::NavigationPrevious => {
                vec![VtEvent::Ignored {
                    reason: "soft-key navigation requires render runtime",
                }]
            }
        }
    }
}

fn input_string_can_accept(buffer: &str, c: char, max_length: u8) -> bool {
    max_length == 0 || buffer.len().saturating_add(c.len_utf8()) <= max_length as usize
}

fn input_number_in_range(raw: u32, min_value: i32, max_value: i32) -> bool {
    if min_value >= max_value {
        return true;
    }
    let raw = i64::from(raw);
    raw >= i64::from(min_value) && raw <= i64::from(max_value)
}

fn input_number_node_real_time(node: &SceneNode) -> bool {
    matches!(
        node.kind,
        NodeKind::InputNumber {
            real_time_editing: true,
            ..
        }
    )
}

fn input_number_node_accepts_raw(node: &SceneNode, raw: u32) -> bool {
    match &node.kind {
        NodeKind::InputNumber {
            min_value,
            max_value,
            ..
        } => input_number_in_range(raw, *min_value, *max_value),
        _ => true,
    }
}

fn key_group_key_at<'a>(
    node: &SceneNode,
    key_ids: &'a [ObjectID],
    pointer_x: i32,
    pointer_y: i32,
) -> Option<&'a ObjectID> {
    if key_ids.is_empty() || node.rect.w == 0 || node.rect.h == 0 {
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
    key_ids.get(index).filter(|&&id| id != ObjectID::NULL)
}

fn scene_has_visible_key_group_key(scene: &Scene, id: ObjectID) -> bool {
    id != ObjectID::NULL
        && scene.nodes.iter().any(|node| {
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

/// Helper to construct an interactive scene node for tests / examples.
pub fn make_test_node(id: ObjectID, kind: NodeKind, object_type: ObjectType) -> SceneNode {
    SceneNode {
        id,
        object_type,
        parent: ObjectID::NULL,
        rect: crate::isobus::vt::render::scene::Rect::new(0, 0, 100, 40),
        clip: None,
        style: crate::isobus::vt::render::style::ResolvedStyle::default(),
        visible: true,
        enabled: true,
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::vt::render::scene::{InputValidation, InputValidationRange, Rect};

    fn scene_with(nodes: Vec<SceneNode>) -> Scene {
        let mut s = Scene::new(ObjectID::new(1), (200, 200));
        s.nodes = nodes;
        s
    }

    #[test]
    fn focus_cycles_through_interactive_nodes_only() {
        let mut scene = scene_with(vec![
            make_test_node(
                ObjectID::new(2),
                NodeKind::OutputString {
                    text: "x".into(),
                    transparent_bg: false,
                    justification: 0,
                },
                ObjectType::OutputString,
            ),
            make_test_node(
                ObjectID::new(3),
                NodeKind::Button {
                    label: "OK".into(),
                    enabled: true,
                    transparent_bg: false,
                    draw_border: true,
                    key_number: 1,
                },
                ObjectType::Button,
            ),
            make_test_node(
                ObjectID::new(4),
                NodeKind::Button {
                    label: "NO".into(),
                    enabled: true,
                    transparent_bg: false,
                    draw_border: true,
                    key_number: 2,
                },
                ObjectType::Button,
            ),
        ]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let e1 = rt.handle(&scene, &OperatorEvent::FocusNext);
        assert!(matches!(e1[0], VtEvent::FocusChanged { id } if id == ObjectID::new(3)));
        assert_eq!(rt.selected_input(), Some(ObjectID::new(3)));
        assert_eq!(rt.open_input(), None);
        let e2 = rt.handle(&scene, &OperatorEvent::FocusNext);
        assert!(matches!(e2[0], VtEvent::FocusChanged { id } if id == ObjectID::new(4)));
        // Wrap around.
        let e3 = rt.handle(&scene, &OperatorEvent::FocusNext);
        assert!(matches!(e3[0], VtEvent::FocusChanged { id } if id == ObjectID::new(3)));

        // Re-bind with no interactive nodes -> focus cleared.
        scene.nodes.clear();
        rt.bind(&scene);
        let e4 = rt.handle(&scene, &OperatorEvent::FocusNext);
        assert!(matches!(e4[0], VtEvent::Ignored { .. }));
    }

    #[test]
    fn bind_preserves_open_input_by_object_id_when_scene_reorders() {
        let input_string = make_test_node(
            ObjectID::new(6),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 0,
                validation: None,
            },
            ObjectType::InputString,
        );
        let input_number = make_test_node(
            ObjectID::new(7),
            NodeKind::InputNumber {
                enabled: true,
                real_time_editing: false,
                text: String::new(),
                transparent_bg: false,
                justification: 0,
                min_value: 0,
                max_value: 999,
            },
            ObjectType::InputNumber,
        );
        let mut scene = scene_with(vec![input_string.clone(), input_number.clone()]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);

        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let first = rt.handle(&scene, &OperatorEvent::Char('A'));
        assert!(matches!(&first[0], VtEvent::StringEditPreview { text, .. } if text == "A"));
        assert_eq!(rt.selected_input(), Some(ObjectID::new(6)));
        assert_eq!(rt.open_input(), Some(ObjectID::new(6)));

        scene.nodes = vec![input_number, input_string];
        rt.bind(&scene);
        assert_eq!(rt.selected_input(), Some(ObjectID::new(6)));
        assert_eq!(rt.open_input(), Some(ObjectID::new(6)));

        let second = rt.handle(&scene, &OperatorEvent::Char('B'));
        assert!(matches!(&second[0], VtEvent::StringEditPreview { text, .. } if text == "AB"));
    }

    #[test]
    fn tap_toggles_input_boolean_and_focuses_field() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(5),
            NodeKind::InputBoolean {
                enabled: true,
                value: false,
            },
            ObjectType::InputBoolean,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let ev = rt.handle(&scene, &OperatorEvent::Tap(10, 10));
        assert!(matches!(
            ev[0],
            VtEvent::BooleanValueChanged { id, value: true } if id == ObjectID::new(5)
        ));
    }

    #[test]
    fn typing_accumulates_into_edit_buffer() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(6),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 0,
                validation: None,
            },
            ObjectType::InputString,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let e1 = rt.handle(&scene, &OperatorEvent::Char('A'));
        let e2 = rt.handle(&scene, &OperatorEvent::Char('B'));
        let e3 = rt.handle(&scene, &OperatorEvent::Backspace);
        assert!(matches!(&e1[0], VtEvent::StringEditPreview { text, .. } if text == "A"));
        assert!(matches!(&e2[0], VtEvent::StringEditPreview { text, .. } if text == "AB"));
        assert!(matches!(&e3[0], VtEvent::StringEditPreview { text, .. } if text == "A"));
        assert_eq!(rt.open_input(), Some(ObjectID::new(6)));
        let committed = rt.handle(&scene, &OperatorEvent::Commit);
        assert!(matches!(&committed[0], VtEvent::StringValueChanged { text, .. } if text == "A"));
        assert_eq!(rt.open_input(), None);
    }

    #[test]
    fn input_string_whitelist_validation_filters_characters() {
        // Whitelist: only digits are valid.
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(6),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 0,
                validation: Some(InputValidation {
                    allow_listed: true,
                    byte_oriented: true,
                    chars: b"0123456789".to_vec(),
                    ranges: Vec::new(),
                }),
            },
            ObjectType::InputString,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let accepted = rt.handle(&scene, &OperatorEvent::Char('5'));
        let rejected = rt.handle(&scene, &OperatorEvent::Char('A'));
        assert!(matches!(&accepted[0], VtEvent::StringEditPreview { text, .. } if text == "5"));
        assert!(matches!(&rejected[0], VtEvent::Ignored { .. }));
        // The rejected character left the edit buffer unchanged.
        let next = rt.handle(&scene, &OperatorEvent::Char('7'));
        assert!(matches!(&next[0], VtEvent::StringEditPreview { text, .. } if text == "57"));
    }

    #[test]
    fn input_string_blacklist_validation_filters_characters() {
        // Classic byte blacklist: spaces are invalid, other one-byte
        // characters are allowed, and non-byte characters cannot enter the
        // 8-bit validation path.
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(6),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 0,
                validation: Some(InputValidation {
                    allow_listed: false,
                    byte_oriented: true,
                    chars: b" ".to_vec(),
                    ranges: Vec::new(),
                }),
            },
            ObjectType::InputString,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let accepted = rt.handle(&scene, &OperatorEvent::Char('A'));
        let rejected = rt.handle(&scene, &OperatorEvent::Char(' '));
        let rejected_wide = rt.handle(&scene, &OperatorEvent::Char('😀'));
        assert!(matches!(&accepted[0], VtEvent::StringEditPreview { text, .. } if text == "A"));
        assert!(matches!(&rejected[0], VtEvent::Ignored { .. }));
        assert!(matches!(&rejected_wide[0], VtEvent::Ignored { .. }));
    }

    #[test]
    fn input_string_max_length_rejects_overflow_without_mutating_buffer() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(6),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 3,
                validation: None,
            },
            ObjectType::InputString,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let a = rt.handle(&scene, &OperatorEvent::Char('A'));
        let b = rt.handle(&scene, &OperatorEvent::Char('B'));
        let c = rt.handle(&scene, &OperatorEvent::Char('C'));
        let rejected = rt.handle(&scene, &OperatorEvent::Char('D'));
        assert!(matches!(&a[0], VtEvent::StringEditPreview { text, .. } if text == "A"));
        assert!(matches!(&b[0], VtEvent::StringEditPreview { text, .. } if text == "AB"));
        assert!(matches!(&c[0], VtEvent::StringEditPreview { text, .. } if text == "ABC"));
        assert!(
            matches!(&rejected[0], VtEvent::Ignored { reason } if *reason == "input string maximum length reached")
        );
        let committed = rt.handle(&scene, &OperatorEvent::Commit);
        assert!(matches!(&committed[0], VtEvent::StringValueChanged { text, .. } if text == "ABC"));
    }

    #[test]
    fn input_string_wide_range_validation_filters_characters() {
        // ExtendedInputAttributes-style whitelist: Latin-1 supplement
        // and emoji plane 1 range are valid; ASCII letters are not.
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(6),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 0,
                validation: Some(InputValidation {
                    allow_listed: true,
                    byte_oriented: false,
                    chars: Vec::new(),
                    ranges: vec![
                        InputValidationRange {
                            plane: 0,
                            first: 0x00E0,
                            last: 0x00FF,
                        },
                        InputValidationRange {
                            plane: 1,
                            first: 0xF600,
                            last: 0xF64F,
                        },
                    ],
                }),
            },
            ObjectType::InputString,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let accepted_accent = rt.handle(&scene, &OperatorEvent::Char('é'));
        let accepted_emoji = rt.handle(&scene, &OperatorEvent::Char('😀'));
        let rejected_ascii = rt.handle(&scene, &OperatorEvent::Char('A'));
        assert!(
            matches!(&accepted_accent[0], VtEvent::StringEditPreview { text, .. } if text == "é")
        );
        assert!(
            matches!(&accepted_emoji[0], VtEvent::StringEditPreview { text, .. } if text == "é😀")
        );
        assert!(matches!(&rejected_ascii[0], VtEvent::Ignored { .. }));
    }

    #[test]
    fn number_edit_rejects_non_digits() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(7),
            NodeKind::InputNumber {
                enabled: true,
                real_time_editing: false,
                text: String::new(),
                transparent_bg: false,
                justification: 0,
                min_value: 0,
                max_value: 0,
            },
            ObjectType::InputNumber,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let e1 = rt.handle(&scene, &OperatorEvent::Char('4'));
        let e2 = rt.handle(&scene, &OperatorEvent::Char('2'));
        let e3 = rt.handle(&scene, &OperatorEvent::Char('x'));
        assert!(matches!(&e1[0], VtEvent::NumberEditPreview { raw: 4, .. }));
        assert!(matches!(&e2[0], VtEvent::NumberEditPreview { raw: 42, .. }));
        assert!(matches!(
            &e3[0],
            VtEvent::Ignored {
                reason: "input number accepts only decimal digits"
            }
        ));
        let committed = rt.handle(&scene, &OperatorEvent::Commit);
        assert!(matches!(
            &committed[0],
            VtEvent::NumberValueChanged { raw: 42, .. }
        ));
    }

    #[test]
    fn number_commit_rejects_values_outside_field_range() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(7),
            NodeKind::InputNumber {
                enabled: true,
                real_time_editing: false,
                text: String::new(),
                transparent_bg: false,
                justification: 0,
                min_value: 0,
                max_value: 45,
            },
            ObjectType::InputNumber,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let _ = rt.handle(&scene, &OperatorEvent::Char('7'));
        let _ = rt.handle(&scene, &OperatorEvent::Char('3'));
        let rejected = rt.handle(&scene, &OperatorEvent::Commit);
        assert!(matches!(
            rejected[0],
            VtEvent::Ignored {
                reason: "input number value outside range"
            }
        ));
        assert_eq!(rt.open_input(), Some(ObjectID::new(7)));
        assert!(matches!(rt.edit_state(), EditState::Number { .. }));
    }

    #[test]
    fn real_time_number_edit_emits_value_changes_immediately() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(7),
            NodeKind::InputNumber {
                enabled: true,
                real_time_editing: true,
                text: String::new(),
                transparent_bg: false,
                justification: 0,
                min_value: 0,
                max_value: 45,
            },
            ObjectType::InputNumber,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);

        let first = rt.handle(&scene, &OperatorEvent::Char('4'));
        assert!(matches!(
            first[0],
            VtEvent::NumberValueChanged {
                id,
                raw: 4
            } if id == ObjectID::new(7)
        ));
        assert_eq!(rt.open_input(), Some(ObjectID::new(7)));
        assert!(matches!(
            rt.edit_state(),
            EditState::Number {
                buffer,
                real_time: true
            } if buffer == "4"
        ));

        let rejected = rt.handle(&scene, &OperatorEvent::Char('9'));
        assert!(matches!(
            rejected[0],
            VtEvent::Ignored {
                reason: "input number value outside range"
            }
        ));
        assert!(matches!(
            rt.edit_state(),
            EditState::Number {
                buffer,
                real_time: true
            } if buffer == "4"
        ));

        let close = rt.handle(&scene, &OperatorEvent::Commit);
        assert!(matches!(
            close[0],
            VtEvent::Ignored {
                reason: "real-time input value already sent"
            }
        ));
        assert_eq!(rt.open_input(), None);
    }

    #[test]
    fn tap_input_list_emits_next_selection_event() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(10),
            NodeKind::InputList {
                enabled: true,
                real_time_editing: true,
                selected: 1,
                item_count: 3,
                selectable_indices: vec![0, 1, 2],
                selected_text: None,
                selected_item_materialized: false,
            },
            ObjectType::InputList,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let event = rt.handle(&scene, &OperatorEvent::Tap(10, 10));
        assert!(matches!(
            event[0],
            VtEvent::ListSelectionChanged {
                id,
                index: 2
            } if id == ObjectID::new(10)
        ));
        assert_eq!(rt.selected_input(), Some(ObjectID::new(10)));
        assert_eq!(rt.open_input(), None);
    }

    #[test]
    fn non_real_time_input_list_previews_until_commit() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(10),
            NodeKind::InputList {
                enabled: true,
                real_time_editing: false,
                selected: 1,
                item_count: 3,
                selectable_indices: vec![0, 1, 2],
                selected_text: None,
                selected_item_materialized: false,
            },
            ObjectType::InputList,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);

        let preview = rt.handle(&scene, &OperatorEvent::Tap(10, 10));
        assert!(matches!(
            preview[0],
            VtEvent::ListSelectionPreview {
                id,
                index: 2
            } if id == ObjectID::new(10)
        ));
        assert_eq!(rt.selected_input(), Some(ObjectID::new(10)));
        assert_eq!(rt.open_input(), Some(ObjectID::new(10)));
        assert!(matches!(rt.edit_state(), EditState::List { index: 2 }));

        let committed = rt.handle(&scene, &OperatorEvent::Commit);
        assert!(matches!(
            committed[0],
            VtEvent::ListSelectionChanged {
                id,
                index: 2
            } if id == ObjectID::new(10)
        ));
        assert_eq!(rt.open_input(), None);
    }

    #[test]
    fn cancel_discards_open_edit_without_value_event() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(6),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 0,
                validation: None,
            },
            ObjectType::InputString,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
        let preview = rt.handle(&scene, &OperatorEvent::Char('A'));
        assert!(matches!(preview[0], VtEvent::StringEditPreview { .. }));
        assert_eq!(rt.open_input(), Some(ObjectID::new(6)));
        let cancelled = rt.handle(&scene, &OperatorEvent::Cancel);
        assert!(matches!(
            cancelled[0],
            VtEvent::InputEsc {
                id,
                error_code: 0,
                transfer_sequence_number: None,
            } if id == ObjectID::new(6)
        ));
        assert_eq!(rt.open_input(), None);
        assert_eq!(rt.edit_state(), &EditState::Idle);
    }

    #[test]
    fn pointer_and_hardware_events_route_to_semantic_input_events() {
        let scene = scene_with(vec![make_test_node(
            ObjectID::new(8),
            NodeKind::Button {
                label: "OK".into(),
                enabled: true,
                transparent_bg: false,
                draw_border: true,
                key_number: 1,
            },
            ObjectType::Button,
        )]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let down = rt.handle(&scene, &OperatorEvent::PointerDown(5, 5));
        assert!(matches!(
            down[0],
            VtEvent::ButtonActivation {
                id,
                code: KeyActivationCode::Pressed
            } if id == ObjectID::new(8)
        ));
        assert_eq!(rt.pointer_down_button(), Some(ObjectID::new(8)));
        let up = rt.handle(&scene, &OperatorEvent::PointerUp(5, 5));
        assert_eq!(rt.pointer_down_button(), None);
        assert!(matches!(
            up[0],
            VtEvent::ButtonActivation {
                id,
                code: KeyActivationCode::Released
            } if id == ObjectID::new(8)
        ));

        let scene = scene_with(vec![make_test_node(
            ObjectID::new(9),
            NodeKind::InputString {
                enabled: true,
                text: String::new(),
                transparent_bg: false,
                auto_wrap: false,
                justification: 0,
                max_length: 0,
                validation: None,
            },
            ObjectType::InputString,
        )]);
        rt.bind(&scene);
        let _ = rt.handle(&scene, &OperatorEvent::HardwareKey(b'\t'));
        let preview = rt.handle(&scene, &OperatorEvent::HardwareKey(b'Q'));
        let commit = rt.handle(&scene, &OperatorEvent::HardwareKey(b'\r'));
        assert!(matches!(&preview[0], VtEvent::StringEditPreview { text, .. } if text == "Q"));
        assert!(matches!(&commit[0], VtEvent::StringValueChanged { text, .. } if text == "Q"));
    }

    #[test]
    fn button_pointer_activation_requires_matching_press_and_release() {
        let scene = scene_with(vec![
            make_test_node(
                ObjectID::new(8),
                NodeKind::Button {
                    label: "OK".into(),
                    enabled: true,
                    transparent_bg: false,
                    draw_border: true,
                    key_number: 1,
                },
                ObjectType::Button,
            ),
            SceneNode {
                rect: crate::isobus::vt::render::scene::Rect::new(110, 0, 100, 40),
                ..make_test_node(
                    ObjectID::new(9),
                    NodeKind::Button {
                        label: "NO".into(),
                        enabled: true,
                        transparent_bg: false,
                        draw_border: true,
                        key_number: 2,
                    },
                    ObjectType::Button,
                )
            },
        ]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);

        let stray_up = rt.handle(&scene, &OperatorEvent::PointerUp(5, 5));
        assert!(matches!(
            stray_up[0],
            VtEvent::Ignored {
                reason: "pointer release did not activate a button"
            }
        ));

        let down = rt.handle(&scene, &OperatorEvent::PointerDown(5, 5));
        assert!(matches!(
            down[0],
            VtEvent::ButtonActivation {
                id,
                code: KeyActivationCode::Pressed
            } if id == ObjectID::new(8)
        ));
        assert_eq!(rt.pointer_down_button(), Some(ObjectID::new(8)));

        let wrong_button_up = rt.handle(&scene, &OperatorEvent::PointerUp(120, 5));
        assert!(matches!(
            wrong_button_up[0],
            VtEvent::ButtonActivation {
                id,
                code: KeyActivationCode::Aborted
            } if id == ObjectID::new(8)
        ));
        assert_eq!(rt.pointer_down_button(), None);

        let tap = rt.handle(&scene, &OperatorEvent::Tap(5, 5));
        assert!(matches!(tap[0], VtEvent::ButtonActivated { id } if id == ObjectID::new(8)));
    }

    #[test]
    fn soft_key_activation_latches_and_emits() {
        let mut scene = scene_with(vec![]);
        scene
            .soft_keys
            .push(crate::isobus::vt::render::scene::SoftKeyNode {
                id: ObjectID::new(9),
                kind: SoftKeyKind::Application,
                cell_index: 0,
                rect: Rect::new(0, 0, 64, 40),
                style: crate::isobus::vt::render::style::ResolvedStyle::default(),
                visible: true,
                enabled: true,
                key_number: 9,
                label: "9".into(),
            });
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let ev = rt.handle(&scene, &OperatorEvent::SoftKeyActivate(ObjectID::new(9)));
        assert!(matches!(ev[0], VtEvent::SoftKeyActivated { id } if id == ObjectID::new(9)));
        assert_eq!(rt.soft_key_latched(), Some(ObjectID::new(9)));

        let rejected = rt.handle(&scene, &OperatorEvent::SoftKeyActivate(ObjectID::new(10)));
        assert!(matches!(
            rejected[0],
            VtEvent::Ignored {
                reason: "soft-key activation target is not visible"
            }
        ));
        assert_eq!(rt.soft_key_latched(), Some(ObjectID::new(9)));
    }

    #[test]
    fn tap_misses_when_no_interactive_node_under_cursor() {
        let scene = scene_with(vec![]);
        let mut rt = InputRuntime::new();
        rt.bind(&scene);
        let ev = rt.handle(&scene, &OperatorEvent::Tap(0, 0));
        assert!(matches!(ev[0], VtEvent::Ignored { .. }));
    }

    #[test]
    fn make_test_node_helper_builds_interactive_node() {
        let n = make_test_node(
            ObjectID::new(1),
            NodeKind::Button {
                label: "x".into(),
                enabled: true,
                transparent_bg: false,
                draw_border: true,
                key_number: 1,
            },
            ObjectType::Button,
        );
        assert_eq!(n.rect, Rect::new(0, 0, 100, 40));
        assert!(n.is_interactive());
    }
}
