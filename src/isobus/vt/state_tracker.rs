//! Client-side mirror of the VT's state, updated by feeding inbound
//! `PGN_VT_TO_ECU` messages.
//!
//! Mirrors the C++ `machbus::isobus::vt::VTClientStateTracker`. The
//! C++ class subscribes itself to the network manager; the Rust port
//! is pump-style — call [`VTClientStateTracker::handle_vt_message`]
//! when a `PGN_VT_TO_ECU` message arrives.

use super::commands::cmd;
use super::objects::ObjectID;
use super::wire::{decode_vt_string_value, vt_string_payload_is_canonical};
use crate::net::constants::NULL_ADDRESS;
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::PGN_VT_TO_ECU;
use crate::net::types::Address;
use alloc::{borrow::ToOwned, collections::BTreeMap as HashMap, string::String, vec::Vec};
use core::cmp::Ordering;

/// Tracked attribute snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrackedAttribute {
    pub object_id: ObjectID,
    pub attribute_id: u8,
    pub value: u32,
}

/// Alarm priority levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(u8)]
pub enum AlarmPriority {
    /// Highest priority.
    Critical = 0,
    Warning = 1,
    /// Lowest priority.
    #[default]
    Information = 2,
}

impl AlarmPriority {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match Self::try_from_u8(v) {
            Some(priority) => priority,
            None => Self::Information,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Critical),
            1 => Some(Self::Warning),
            2 => Some(Self::Information),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// One entry in the active-alarm stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmEntry {
    pub alarm_mask_id: ObjectID,
    pub priority: AlarmPriority,
    pub activation_timestamp_ms: u32,
}

impl Default for AlarmEntry {
    fn default() -> Self {
        Self {
            alarm_mask_id: ObjectID::NULL,
            priority: AlarmPriority::Information,
            activation_timestamp_ms: 0,
        }
    }
}

impl PartialOrd for AlarmEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AlarmEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority (lower numeric value) sorts first; ties broken
        // by older timestamp first.
        self.priority.cmp(&other.priority).then_with(|| {
            self.activation_timestamp_ms
                .cmp(&other.activation_timestamp_ms)
        })
    }
}

/// Client-side mirror of VT state.
pub struct VTClientStateTracker {
    active_data_mask: ObjectID,
    active_soft_key_mask: ObjectID,
    active_alarm_mask: ObjectID,

    numeric_values: HashMap<ObjectID, u32>,
    string_values: HashMap<ObjectID, String>,
    visibility: HashMap<ObjectID, bool>,
    enable_state: HashMap<ObjectID, bool>,
    /// `data_mask_id -> soft_key_mask_id`.
    soft_key_mask_assignments: HashMap<ObjectID, ObjectID>,

    active_alarms: Vec<AlarmEntry>,
    alarm_priorities: HashMap<ObjectID, AlarmPriority>,

    vt_busy_code: u8,
    vt_function_code: u8,
    vt_address: Address,

    pub on_active_mask_changed: Event<ObjectID>,
    pub on_numeric_value_changed: Event<(ObjectID, u32)>,
    pub on_string_value_changed: Event<(ObjectID, String)>,
    pub on_visibility_changed: Event<(ObjectID, bool)>,
    pub on_enable_state_changed: Event<(ObjectID, bool)>,
    pub on_alarm_activated: Event<(ObjectID, AlarmPriority)>,
    pub on_alarm_deactivated: Event<ObjectID>,
}

impl Default for VTClientStateTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl VTClientStateTracker {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active_data_mask: ObjectID::NULL,
            active_soft_key_mask: ObjectID::NULL,
            active_alarm_mask: ObjectID::NULL,
            numeric_values: HashMap::new(),
            string_values: HashMap::new(),
            visibility: HashMap::new(),
            enable_state: HashMap::new(),
            soft_key_mask_assignments: HashMap::new(),
            active_alarms: Vec::new(),
            alarm_priorities: HashMap::new(),
            vt_busy_code: 0,
            vt_function_code: 0xFF,
            vt_address: NULL_ADDRESS,
            on_active_mask_changed: Event::new(),
            on_numeric_value_changed: Event::new(),
            on_string_value_changed: Event::new(),
            on_visibility_changed: Event::new(),
            on_enable_state_changed: Event::new(),
            on_alarm_activated: Event::new(),
            on_alarm_deactivated: Event::new(),
        }
    }

    // ─── Accessors ────────────────────────────────────────────────────

    #[inline]
    #[must_use]
    pub const fn active_data_mask(&self) -> ObjectID {
        self.active_data_mask
    }

    #[inline]
    #[must_use]
    pub const fn active_soft_key_mask(&self) -> ObjectID {
        self.active_soft_key_mask
    }

    #[inline]
    #[must_use]
    pub const fn active_alarm_mask(&self) -> ObjectID {
        self.active_alarm_mask
    }

    #[inline]
    #[must_use]
    pub const fn vt_address(&self) -> Address {
        self.vt_address
    }

    #[inline]
    #[must_use]
    pub const fn vt_busy_code(&self) -> u8 {
        self.vt_busy_code
    }

    #[inline]
    #[must_use]
    pub const fn vt_function_code(&self) -> u8 {
        self.vt_function_code
    }

    #[must_use]
    pub fn numeric_value(&self, id: impl Into<ObjectID>) -> Option<u32> {
        self.numeric_values.get(&id.into()).copied()
    }

    #[must_use]
    pub fn string_value(&self, id: impl Into<ObjectID>) -> Option<&str> {
        self.string_values.get(&id.into()).map(String::as_str)
    }

    #[must_use]
    pub fn is_visible(&self, id: impl Into<ObjectID>) -> Option<bool> {
        self.visibility.get(&id.into()).copied()
    }

    #[must_use]
    pub fn is_enabled(&self, id: impl Into<ObjectID>) -> Option<bool> {
        self.enable_state.get(&id.into()).copied()
    }

    #[must_use]
    pub fn soft_key_mask_for(&self, data_mask_id: impl Into<ObjectID>) -> Option<ObjectID> {
        self.soft_key_mask_assignments
            .get(&data_mask_id.into())
            .copied()
    }

    // ─── Alarm priority stack ─────────────────────────────────────────

    pub fn register_alarm_priority(
        &mut self,
        alarm_mask_id: impl Into<ObjectID>,
        priority: AlarmPriority,
    ) {
        self.alarm_priorities.insert(alarm_mask_id.into(), priority);
    }

    pub fn activate_alarm(&mut self, alarm_mask_id: impl Into<ObjectID>, timestamp_ms: u32) {
        let alarm_mask_id = alarm_mask_id.into();
        if self
            .active_alarms
            .iter()
            .any(|a| a.alarm_mask_id == alarm_mask_id)
        {
            return;
        }
        let priority = self
            .alarm_priorities
            .get(&alarm_mask_id)
            .copied()
            .unwrap_or(AlarmPriority::Information);
        self.active_alarms.push(AlarmEntry {
            alarm_mask_id,
            priority,
            activation_timestamp_ms: timestamp_ms,
        });
        self.active_alarms.sort();
        if let Some(top) = self.active_alarms.first() {
            self.active_alarm_mask = top.alarm_mask_id;
            self.on_alarm_activated.emit(&(alarm_mask_id, priority));
        }
    }

    pub fn acknowledge_alarm(&mut self) {
        if self.active_alarms.is_empty() {
            return;
        }
        let deactivated = self.active_alarms.remove(0).alarm_mask_id;
        self.active_alarm_mask = self
            .active_alarms
            .first()
            .map_or(ObjectID::NULL, |a| a.alarm_mask_id);
        self.on_alarm_deactivated.emit(&deactivated);
    }

    pub fn deactivate_alarm(&mut self, alarm_mask_id: impl Into<ObjectID>) {
        let alarm_mask_id = alarm_mask_id.into();
        if let Some(pos) = self
            .active_alarms
            .iter()
            .position(|a| a.alarm_mask_id == alarm_mask_id)
        {
            self.active_alarms.remove(pos);
            self.on_alarm_deactivated.emit(&alarm_mask_id);
        }
        self.active_alarm_mask = self
            .active_alarms
            .first()
            .map_or(ObjectID::NULL, |a| a.alarm_mask_id);
    }

    #[inline]
    #[must_use]
    pub fn active_alarms(&self) -> &[AlarmEntry] {
        &self.active_alarms
    }

    #[must_use]
    pub fn highest_priority_alarm(&self) -> Option<AlarmEntry> {
        self.active_alarms.first().copied()
    }

    #[must_use]
    pub fn is_alarm_active(&self, alarm_mask_id: impl Into<ObjectID>) -> bool {
        let alarm_mask_id = alarm_mask_id.into();
        self.active_alarms
            .iter()
            .any(|a| a.alarm_mask_id == alarm_mask_id)
    }

    // ─── Manual injection (testing / initial sync) ────────────────────

    pub fn set_numeric_value(&mut self, id: impl Into<ObjectID>, value: u32) {
        self.numeric_values.insert(id.into(), value);
    }

    pub fn set_string_value(&mut self, id: impl Into<ObjectID>, value: impl Into<String>) {
        self.string_values.insert(id.into(), value.into());
    }

    pub fn set_visibility(&mut self, id: impl Into<ObjectID>, visible: bool) {
        self.visibility.insert(id.into(), visible);
    }

    pub fn set_enable_state(&mut self, id: impl Into<ObjectID>, enabled: bool) {
        self.enable_state.insert(id.into(), enabled);
    }

    pub fn reset(&mut self) {
        self.active_data_mask = ObjectID::NULL;
        self.active_soft_key_mask = ObjectID::NULL;
        self.active_alarm_mask = ObjectID::NULL;
        self.numeric_values.clear();
        self.string_values.clear();
        self.visibility.clear();
        self.enable_state.clear();
        self.soft_key_mask_assignments.clear();
        self.active_alarms.clear();
        self.alarm_priorities.clear();
        self.vt_busy_code = 0;
        self.vt_function_code = 0xFF;
        self.vt_address = NULL_ADDRESS;
    }

    // ─── Inbound dispatch ─────────────────────────────────────────────

    /// Feed an inbound `PGN_VT_TO_ECU` message; updates internal state
    /// and fires the relevant event(s).
    pub fn handle_vt_message(&mut self, msg: &Message) {
        if !msg.has_usable_envelope_for_pgn(PGN_VT_TO_ECU) || msg.data.is_empty() {
            return;
        }
        let accepted = match msg.data[0] {
            cmd::VT_STATUS => self.handle_vt_status(msg),
            cmd::NUMERIC_VALUE_CHANGE => self.handle_numeric_change(msg),
            cmd::STRING_VALUE_CHANGE => self.handle_string_change(msg),
            cmd::HIDE_SHOW => self.handle_hide_show(msg),
            cmd::ENABLE_DISABLE => self.handle_enable_disable(msg),
            cmd::CHANGE_ACTIVE_MASK => self.handle_change_active_mask(msg),
            _ => false,
        };
        if accepted {
            self.vt_address = msg.source;
        }
    }

    // ─── per-command handlers ─────────────────────────────────────────

    fn handle_vt_status(&mut self, msg: &Message) -> bool {
        if msg.data.len() != 8 {
            return false;
        }
        let new_data_mask = ObjectID(u16_le(&msg.data[2..]));
        let new_sk_mask = ObjectID(u16_le(&msg.data[4..]));
        self.vt_busy_code = msg.data[6];
        self.vt_function_code = msg.data[7];
        if new_data_mask != self.active_data_mask {
            self.active_data_mask = new_data_mask;
            self.on_active_mask_changed.emit(&self.active_data_mask);
        }
        if new_sk_mask != self.active_soft_key_mask {
            self.active_soft_key_mask = new_sk_mask;
        }
        true
    }

    fn handle_numeric_change(&mut self, msg: &Message) -> bool {
        if msg.data.len() != 8 || msg.data[3] != 0xFF {
            return false;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let value = (msg.data[4] as u32)
            | ((msg.data[5] as u32) << 8)
            | ((msg.data[6] as u32) << 16)
            | ((msg.data[7] as u32) << 24);
        self.numeric_values.insert(id, value);
        self.on_numeric_value_changed.emit(&(id, value));
        true
    }

    fn handle_string_change(&mut self, msg: &Message) -> bool {
        if msg.data.len() < 5 {
            return false;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let len = u16_le(&msg.data[3..]) as usize;
        let end = 5 + len;
        if !vt_string_payload_is_canonical(&msg.data, end) {
            return false;
        }
        let Some(s) = decode_vt_string_value(&msg.data[5..end]) else {
            return false;
        };
        let s = s.to_owned();
        self.string_values.insert(id, s.clone());
        self.on_string_value_changed.emit(&(id, s));
        true
    }

    fn handle_hide_show(&mut self, msg: &Message) -> bool {
        if msg.data.len() != 8 || !is_canonical_bool(msg.data[3]) || !has_ff_tail(&msg.data, 4) {
            return false;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let visible = msg.data[3] != 0;
        self.visibility.insert(id, visible);
        self.on_visibility_changed.emit(&(id, visible));
        true
    }

    fn handle_enable_disable(&mut self, msg: &Message) -> bool {
        if msg.data.len() != 8 || !is_canonical_bool(msg.data[3]) || !has_ff_tail(&msg.data, 4) {
            return false;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let enabled = msg.data[3] != 0;
        self.enable_state.insert(id, enabled);
        self.on_enable_state_changed.emit(&(id, enabled));
        true
    }

    fn handle_change_active_mask(&mut self, msg: &Message) -> bool {
        if msg.data.len() != 8 || !has_ff_tail(&msg.data, 5) {
            return false;
        }
        let new_mask = ObjectID(u16_le(&msg.data[3..]));
        if new_mask != self.active_data_mask {
            self.active_data_mask = new_mask;
            self.on_active_mask_changed.emit(&self.active_data_mask);
        }
        true
    }
}

#[inline]
fn u16_le(buf: &[u8]) -> u16 {
    (buf[0] as u16) | ((buf[1] as u16) << 8)
}

#[inline]
fn has_ff_tail(data: &[u8], used: usize) -> bool {
    data[used..].iter().all(|&byte| byte == 0xFF)
}

#[inline]
fn is_canonical_bool(byte: u8) -> bool {
    byte <= 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_VT_TO_ECU;
    use proptest::prelude::*;

    fn vt_status(data_mask: u16, sk_mask: u16, busy: u8) -> Message {
        let mut data = vec![cmd::VT_STATUS, 0u8];
        data.extend_from_slice(&[(data_mask & 0xFF) as u8, ((data_mask >> 8) & 0xFF) as u8]);
        data.extend_from_slice(&[(sk_mask & 0xFF) as u8, ((sk_mask >> 8) & 0xFF) as u8]);
        data.push(busy);
        data.push(0xFF); // function_code
        Message::new(PGN_VT_TO_ECU, data, 0x10)
    }

    #[test]
    fn vt_status_updates_masks_and_busy() {
        let mut t = VTClientStateTracker::new();
        t.handle_vt_message(&vt_status(100, 200, 0x42));
        assert_eq!(t.active_data_mask(), 100);
        assert_eq!(t.active_soft_key_mask(), 200);
        assert_eq!(t.vt_busy_code(), 0x42);
        assert_eq!(t.vt_address(), 0x10);
    }

    #[test]
    fn numeric_change_emits_event_and_updates_map() {
        let mut t = VTClientStateTracker::new();
        let mut data = vec![cmd::NUMERIC_VALUE_CHANGE];
        data.extend_from_slice(&[0x05, 0x00]); // id = 5
        data.push(0xFF); // padding byte
        data.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        let msg = Message::new(PGN_VT_TO_ECU, data, 0x10);
        t.handle_vt_message(&msg);
        assert_eq!(t.numeric_value(5), Some(0x1234_5678));

        let mut bad_reserved = vec![cmd::NUMERIC_VALUE_CHANGE];
        bad_reserved.extend_from_slice(&[0x05, 0x00, 0x00]);
        bad_reserved.extend_from_slice(&0x8765_4321u32.to_le_bytes());
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_reserved, 0x10));
        assert_eq!(
            t.numeric_value(5),
            Some(0x1234_5678),
            "bad reserved byte must not update cached numeric values"
        );
        assert_eq!(
            t.vt_address(),
            0x10,
            "malformed value-change must not overwrite the last accepted VT address"
        );
    }

    #[test]
    fn string_change_assembles_string() {
        let mut t = VTClientStateTracker::new();
        let payload = "hé".as_bytes();
        let mut data = vec![cmd::STRING_VALUE_CHANGE];
        data.extend_from_slice(&[0x07, 0x00]); // id = 7
        data.extend_from_slice(&(payload.len() as u16).to_le_bytes());
        data.extend_from_slice(payload);
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, data, 0x10));
        assert_eq!(t.string_value(7), Some("hé"));
    }

    #[test]
    fn string_change_rejects_truncated_declared_length() {
        let mut t = VTClientStateTracker::new();
        t.set_string_value(7, "old");
        let mut data = vec![cmd::STRING_VALUE_CHANGE];
        data.extend_from_slice(&[0x07, 0x00]); // id = 7
        data.extend_from_slice(&[0x03, 0x00]); // declares three payload bytes
        data.push(b'h'); // only one payload byte arrived
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, data, 0x10));
        assert_eq!(t.string_value(7), Some("old"));

        let mut bad_tail = vec![cmd::STRING_VALUE_CHANGE];
        bad_tail.extend_from_slice(&[0x07, 0x00]);
        bad_tail.extend_from_slice(&[0x02, 0x00]);
        bad_tail.extend_from_slice(b"hi");
        bad_tail.push(0x00);
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_tail, 0x10));
        assert_eq!(
            t.string_value(7),
            Some("old"),
            "bad trailing padding must not update cached strings"
        );
    }

    #[test]
    fn string_change_rejects_invalid_utf8_without_mutating_state() {
        let mut t = VTClientStateTracker::new();
        t.set_string_value(7, "old");
        t.handle_vt_message(&vt_status(1, 2, 0));
        assert_eq!(t.vt_address(), 0x10);

        let data = vec![
            cmd::STRING_VALUE_CHANGE,
            0x07,
            0x00,
            0x02,
            0x00,
            0xC3,
            0x28,
            0xFF,
        ];
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, data, 0x20));
        assert_eq!(t.string_value(7), Some("old"));
        assert_eq!(
            t.vt_address(),
            0x10,
            "malformed string payload must not become the accepted VT source"
        );
    }

    #[test]
    fn alarm_stack_orders_by_priority() {
        let mut t = VTClientStateTracker::new();
        t.register_alarm_priority(1, AlarmPriority::Information);
        t.register_alarm_priority(2, AlarmPriority::Critical);
        t.register_alarm_priority(3, AlarmPriority::Warning);
        t.activate_alarm(1, 100);
        t.activate_alarm(2, 200);
        t.activate_alarm(3, 300);
        // Critical should be at the top.
        assert_eq!(t.highest_priority_alarm().unwrap().alarm_mask_id, 2);
        t.acknowledge_alarm();
        assert_eq!(t.highest_priority_alarm().unwrap().alarm_mask_id, 3);
        t.acknowledge_alarm();
        assert_eq!(t.highest_priority_alarm().unwrap().alarm_mask_id, 1);
        t.acknowledge_alarm();
        assert!(t.highest_priority_alarm().is_none());
        assert_eq!(t.active_alarm_mask(), ObjectID::NULL);
    }

    #[test]
    fn duplicate_activate_is_noop() {
        let mut t = VTClientStateTracker::new();
        t.activate_alarm(1, 0);
        t.activate_alarm(1, 0);
        assert_eq!(t.active_alarms().len(), 1);
    }

    #[test]
    fn deactivate_specific_alarm() {
        let mut t = VTClientStateTracker::new();
        t.register_alarm_priority(1, AlarmPriority::Critical);
        t.register_alarm_priority(2, AlarmPriority::Warning);
        t.activate_alarm(1, 0);
        t.activate_alarm(2, 1);
        t.deactivate_alarm(1);
        assert_eq!(t.active_alarms().len(), 1);
        assert_eq!(t.active_alarm_mask(), 2);
    }

    #[test]
    fn hide_show_and_enable_disable_round_trip() {
        let mut t = VTClientStateTracker::new();
        let mut hide = vec![cmd::HIDE_SHOW];
        hide.extend_from_slice(&[0x10, 0x00, 0x01]);
        hide.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, hide, 0x10));
        assert_eq!(t.is_visible(0x10), Some(true));

        let mut en = vec![cmd::ENABLE_DISABLE];
        en.extend_from_slice(&[0x20, 0x00, 0x00]);
        en.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, en, 0x10));
        assert_eq!(t.is_enabled(0x20), Some(false));

        let mut malformed_hide = vec![cmd::HIDE_SHOW];
        malformed_hide.extend_from_slice(&[0x11, 0x00, 0x01, 0x00, 0xFF, 0xFF, 0xFF]);
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, malformed_hide, 0x10));
        assert_eq!(t.is_visible(0x11), None);
    }

    #[test]
    fn hide_show_and_enable_disable_reject_reserved_boolean_values() {
        let mut t = VTClientStateTracker::new();
        t.set_visibility(0x10, true);
        t.set_enable_state(0x20, false);
        t.handle_vt_message(&vt_status(1, 2, 0));
        assert_eq!(t.vt_address(), 0x10);

        let mut bad_hide = vec![cmd::HIDE_SHOW];
        bad_hide.extend_from_slice(&[0x10, 0x00, 0x02]);
        bad_hide.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_hide, 0x20));
        assert_eq!(t.is_visible(0x10), Some(true));
        assert_eq!(t.vt_address(), 0x10);

        let mut bad_enable = vec![cmd::ENABLE_DISABLE];
        bad_enable.extend_from_slice(&[0x20, 0x00, 0x7F]);
        bad_enable.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF]);
        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, bad_enable, 0x30));
        assert_eq!(t.is_enabled(0x20), Some(false));
        assert_eq!(t.vt_address(), 0x10);
    }

    #[test]
    fn malformed_or_unknown_messages_do_not_update_vt_address() {
        let mut t = VTClientStateTracker::new();
        t.handle_vt_message(&vt_status(10, 20, 0));
        assert_eq!(t.vt_address(), 0x10);

        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vec![0x00], 0x77));
        assert_eq!(t.vt_address(), 0x10);

        t.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vec![cmd::VT_STATUS], 0x88));
        assert_eq!(t.vt_address(), 0x10);
    }

    #[test]
    fn reset_clears_state() {
        let mut t = VTClientStateTracker::new();
        t.set_numeric_value(1, 42);
        t.handle_vt_message(&vt_status(1, 2, 0x42));
        t.activate_alarm(5, 0);
        t.reset();
        assert!(t.numeric_value(1).is_none());
        assert!(t.active_alarms().is_empty());
        assert_eq!(t.vt_function_code(), 0xFF);
        assert_eq!(t.vt_address(), NULL_ADDRESS);
    }

    proptest! {
        #[test]
        fn proptest_state_tracker_accepts_arbitrary_vt_messages_without_panics(
            messages in proptest::collection::vec(
                (any::<u8>(), proptest::collection::vec(any::<u8>(), 0..=32)),
                0..=256,
            ),
        ) {
            let mut tracker = VTClientStateTracker::new();
            for (source, data) in messages {
                let previous_address = tracker.vt_address();
                tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, data, source));

                prop_assert!(tracker.numeric_values.len() <= usize::from(u16::MAX) + 1);
                prop_assert!(tracker.string_values.len() <= usize::from(u16::MAX) + 1);
                prop_assert!(tracker.visibility.len() <= usize::from(u16::MAX) + 1);
                prop_assert!(tracker.enable_state.len() <= usize::from(u16::MAX) + 1);
                prop_assert!(tracker.soft_key_mask_assignments.len() <= usize::from(u16::MAX) + 1);
                prop_assert!(tracker.active_alarms.len() <= usize::from(u16::MAX) + 1);
                prop_assert!(tracker.vt_address() == previous_address || tracker.vt_address() == source);
            }
        }
    }
}
