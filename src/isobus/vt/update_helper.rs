//! Ergonomic VT-client update wrapper with deduplication + batching.
//!
//! Mirrors the C++ `machbus::isobus::vt::VTClientUpdateHelper`. The
//! C++ helper holds a `VTClient&` and dispatches sends inline. The
//! Rust port decouples from the client: setters return [`UpdateOp`]
//! (enum of pending work) which the caller hands to a [`VTClient`]
//! send. Batch mode queues ops in a [`Vec`] you drain with
//! [`VTClientUpdateHelper::end_batch`].
//!
//! [`VTClient`]: super::client

use alloc::{string::String, vec::Vec};
use core::mem;

use super::client::{ClientOutbound, VTClient};
use super::commands::VT_STRING_VALUE_MAX_LEN;
use super::objects::{ObjectID, ObjectPool, ObjectType};
use super::state_tracker::VTClientStateTracker;
use crate::net::error::{Error, Result};

/// Pending VT mutation produced by the helper. The caller dispatches
/// each variant through their `VTClient` instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOp {
    Numeric {
        id: ObjectID,
        value: u32,
    },
    String {
        id: ObjectID,
        value: String,
    },
    Visibility {
        id: ObjectID,
        visible: bool,
    },
    Enable {
        id: ObjectID,
        enabled: bool,
    },
    ActiveMask {
        working_set_id: ObjectID,
        mask_id: ObjectID,
    },
}

impl UpdateOp {
    /// Convert this pending helper operation into the canonical ECU→VT command
    /// bytes produced by [`VTClient`].
    ///
    /// The caller still owns transport scheduling; this helper centralizes the
    /// `UpdateOp`→wire-command mapping so batch users do not accidentally emit a
    /// stale or non-protocol layout for common VT mutations.
    pub fn to_client_outbound(&self, client: &VTClient) -> Result<ClientOutbound> {
        match self {
            Self::Numeric { id, value } => client.change_numeric_value(*id, *value),
            Self::String { id, value } => client.change_string_value(*id, value),
            Self::Visibility { id, visible } => client.hide_show(*id, *visible),
            Self::Enable { id, enabled } => client.enable_disable(*id, *enabled),
            Self::ActiveMask {
                working_set_id,
                mask_id,
            } => client.change_active_mask(*working_set_id, *mask_id),
        }
    }
}

/// Deduplicating + batching update helper.
///
/// Holds a mutable reference to the [`VTClientStateTracker`] so it can
/// (a) skip sends when the cached value already matches, and (b)
/// optimistically pre-update the tracker once the caller confirms an
/// op was successfully sent (via [`Self::confirm`]).
pub struct VTClientUpdateHelper<'a> {
    tracker: &'a mut VTClientStateTracker,
    pool: Option<&'a ObjectPool>,
    batch_mode: bool,
    pending: Vec<UpdateOp>,
}

impl<'a> VTClientUpdateHelper<'a> {
    #[must_use]
    pub fn new(tracker: &'a mut VTClientStateTracker) -> Self {
        Self {
            tracker,
            pool: None,
            batch_mode: false,
            pending: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_pool(mut self, pool: &'a ObjectPool) -> Self {
        self.pool = Some(pool);
        self
    }

    // ─── Numeric ──────────────────────────────────────────────────────

    /// Returns the [`UpdateOp`] to send, or `None` if the value is
    /// already what the tracker has cached. In batch mode, the op is
    /// queued internally and `None` is returned; drain via
    /// [`Self::end_batch`].
    pub fn set_numeric_value(&mut self, id: impl Into<ObjectID>, value: u32) -> Option<UpdateOp> {
        let id = id.into();
        if self.tracker.numeric_value(id) == Some(value) {
            self.remove_pending_slot(&UpdateOpSlot::Numeric(id));
            return None;
        }
        self.maybe_emit(UpdateOp::Numeric { id, value })
    }

    /// Checked `(value + offset) * scale` conversion to the raw VT numeric
    /// value.
    ///
    /// Rejects non-finite inputs/results and values outside the `u32` wire
    /// domain instead of silently emitting Rust's saturating float-to-integer
    /// cast result.
    pub fn try_set_numeric_scaled(
        &mut self,
        id: impl Into<ObjectID>,
        value: f64,
        scale: f64,
        offset: f64,
    ) -> Result<Option<UpdateOp>> {
        let raw = scaled_numeric_raw(value, scale, offset)?;
        Ok(self.set_numeric_value(id, raw))
    }

    /// Compatibility wrapper around [`Self::try_set_numeric_scaled`].
    ///
    /// Invalid scaled values are dropped (`None`) because this legacy helper
    /// returns `Option` rather than `Result`. New code should prefer the
    /// fallible variant when bad scaling input needs to be reported.
    pub fn set_numeric_scaled(
        &mut self,
        id: impl Into<ObjectID>,
        value: f64,
        scale: f64,
        offset: f64,
    ) -> Option<UpdateOp> {
        self.try_set_numeric_scaled(id, value, scale, offset)
            .ok()
            .flatten()
    }

    pub fn set_numeric_clamped(
        &mut self,
        id: impl Into<ObjectID>,
        value: u32,
        min_val: u32,
        max_val: u32,
    ) -> Option<UpdateOp> {
        self.set_numeric_value(id, value.clamp(min_val, max_val))
    }

    // ─── String ───────────────────────────────────────────────────────

    pub fn set_string_value(
        &mut self,
        id: impl Into<ObjectID>,
        value: impl Into<String>,
    ) -> Option<UpdateOp> {
        self.try_set_string_value(id, value).ok().flatten()
    }

    /// Checked string update helper.
    ///
    /// Rejects values that cannot fit the VT `Change String Value` u16 length
    /// field before they can enter a batch. The legacy [`Self::set_string_value`]
    /// wrapper drops that error as `None` for compatibility.
    pub fn try_set_string_value(
        &mut self,
        id: impl Into<ObjectID>,
        value: impl Into<String>,
    ) -> Result<Option<UpdateOp>> {
        let id = id.into();
        let value = value.into();
        if value.len() > VT_STRING_VALUE_MAX_LEN {
            return Err(Error::invalid_data(
                "VT string-value payload exceeds u16 length field",
            ));
        }
        if self.tracker.string_value(id) == Some(value.as_str()) {
            self.remove_pending_slot(&UpdateOpSlot::String(id));
            return Ok(None);
        }
        Ok(self.maybe_emit(UpdateOp::String { id, value }))
    }

    // ─── Visibility ───────────────────────────────────────────────────

    pub fn show(&mut self, id: impl Into<ObjectID>) -> Option<UpdateOp> {
        self.set_visibility(id, true)
    }

    pub fn hide(&mut self, id: impl Into<ObjectID>) -> Option<UpdateOp> {
        self.set_visibility(id, false)
    }

    pub fn set_visibility(&mut self, id: impl Into<ObjectID>, visible: bool) -> Option<UpdateOp> {
        let id = id.into();
        if self.tracker.is_visible(id) == Some(visible) {
            self.remove_pending_slot(&UpdateOpSlot::Visibility(id));
            return None;
        }
        self.maybe_emit(UpdateOp::Visibility { id, visible })
    }

    // ─── Enable / disable ─────────────────────────────────────────────

    pub fn enable(&mut self, id: impl Into<ObjectID>) -> Option<UpdateOp> {
        self.set_enable(id, true)
    }

    pub fn disable(&mut self, id: impl Into<ObjectID>) -> Option<UpdateOp> {
        self.set_enable(id, false)
    }

    pub fn set_enable(&mut self, id: impl Into<ObjectID>, enabled: bool) -> Option<UpdateOp> {
        let id = id.into();
        if self.tracker.is_enabled(id) == Some(enabled) {
            self.remove_pending_slot(&UpdateOpSlot::Enable(id));
            return None;
        }
        self.maybe_emit(UpdateOp::Enable { id, enabled })
    }

    // ─── Active mask ──────────────────────────────────────────────────

    /// Returns an op to switch the active mask. Validates against the
    /// pool (if attached) that `mask_id` exists and is a Data/Alarm
    /// Mask.
    pub fn change_active_mask(
        &mut self,
        working_set_id: impl Into<ObjectID>,
        mask_id: impl Into<ObjectID>,
    ) -> Result<Option<UpdateOp>> {
        let working_set_id = working_set_id.into();
        let mask_id = mask_id.into();
        if self.tracker.active_data_mask() == mask_id {
            self.remove_pending_slot(&UpdateOpSlot::ActiveMask(working_set_id));
            return Ok(None);
        }
        if let Some(pool) = self.pool {
            let mask_obj = pool
                .find(mask_id)
                .ok_or_else(|| Error::invalid_state("mask object not in pool"))?;
            if !matches!(
                mask_obj.r#type,
                ObjectType::DataMask | ObjectType::AlarmMask
            ) {
                return Err(Error::invalid_state("object is not a mask type"));
            }
        }
        Ok(self.maybe_emit(UpdateOp::ActiveMask {
            working_set_id,
            mask_id,
        }))
    }

    // ─── Batch ────────────────────────────────────────────────────────

    pub fn begin_batch(&mut self) {
        self.batch_mode = true;
        self.pending.clear();
    }

    /// Drain all pending ops; subsequent setters are non-batched. The
    /// caller dispatches each op and then signals successful sends via
    /// [`Self::confirm`].
    #[must_use]
    pub fn end_batch(&mut self) -> Vec<UpdateOp> {
        self.batch_mode = false;
        mem::take(&mut self.pending)
    }

    pub fn cancel_batch(&mut self) {
        self.batch_mode = false;
        self.pending.clear();
    }

    #[inline]
    #[must_use]
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    #[inline]
    #[must_use]
    pub const fn is_batching(&self) -> bool {
        self.batch_mode
    }

    /// Update the tracker's cache to reflect a successfully sent op.
    /// The C++ does this inline after a successful `client.send`; Rust
    /// makes the optimistic write explicit.
    pub fn confirm(&mut self, op: &UpdateOp) {
        match op {
            UpdateOp::Numeric { id, value } => self.tracker.set_numeric_value(*id, *value),
            UpdateOp::String { id, value } => self.tracker.set_string_value(*id, value),
            UpdateOp::Visibility { id, visible } => self.tracker.set_visibility(*id, *visible),
            UpdateOp::Enable { id, enabled } => self.tracker.set_enable_state(*id, *enabled),
            UpdateOp::ActiveMask { .. } => {
                // The active-mask change comes back as a CHANGE_ACTIVE_MASK
                // event from the VT, so the tracker updates from there. The
                // helper does not pre-update.
            }
        }
    }

    fn maybe_emit(&mut self, op: UpdateOp) -> Option<UpdateOp> {
        if self.batch_mode {
            self.upsert_pending(op);
            None
        } else {
            Some(op)
        }
    }

    fn upsert_pending(&mut self, op: UpdateOp) {
        let slot = UpdateOpSlot::from(&op);
        if let Some(existing) = self
            .pending
            .iter_mut()
            .find(|existing| slot.matches(existing))
        {
            *existing = op;
        } else {
            self.pending.push(op);
        }
    }

    fn remove_pending_slot(&mut self, slot: &UpdateOpSlot) {
        if self.batch_mode {
            self.pending.retain(|op| !slot.matches(op));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum UpdateOpSlot {
    Numeric(ObjectID),
    String(ObjectID),
    Visibility(ObjectID),
    Enable(ObjectID),
    ActiveMask(ObjectID),
}

impl UpdateOpSlot {
    fn from(op: &UpdateOp) -> Self {
        match op {
            UpdateOp::Numeric { id, .. } => Self::Numeric(*id),
            UpdateOp::String { id, .. } => Self::String(*id),
            UpdateOp::Visibility { id, .. } => Self::Visibility(*id),
            UpdateOp::Enable { id, .. } => Self::Enable(*id),
            UpdateOp::ActiveMask { working_set_id, .. } => Self::ActiveMask(*working_set_id),
        }
    }

    fn matches(self, op: &UpdateOp) -> bool {
        match (self, op) {
            (Self::Numeric(slot_id), UpdateOp::Numeric { id, .. })
            | (Self::String(slot_id), UpdateOp::String { id, .. })
            | (Self::Visibility(slot_id), UpdateOp::Visibility { id, .. })
            | (Self::Enable(slot_id), UpdateOp::Enable { id, .. }) => slot_id == *id,
            (
                Self::ActiveMask(slot_working_set_id),
                UpdateOp::ActiveMask { working_set_id, .. },
            ) => slot_working_set_id == *working_set_id,
            _ => false,
        }
    }
}

fn scaled_numeric_raw(value: f64, scale: f64, offset: f64) -> Result<u32> {
    if !value.is_finite() || !scale.is_finite() || !offset.is_finite() {
        return Err(Error::invalid_data(
            "VT numeric scaling input is not finite",
        ));
    }
    let raw = (value + offset) * scale;
    if !raw.is_finite() {
        return Err(Error::invalid_data(
            "VT numeric scaled result is not finite",
        ));
    }
    if raw < 0.0 || raw > f64::from(u32::MAX) {
        return Err(Error::invalid_data(
            "VT numeric scaled result is outside u32 wire range",
        ));
    }
    Ok(trunc_f64(raw) as u32)
}

fn trunc_f64(value: f64) -> f64 {
    value as i64 as f64
}

#[cfg(test)]
mod tests {
    use super::super::client::{VTClientConfig, VTState};
    use super::super::commands::{VT_STRING_VALUE_MAX_LEN, cmd};
    use super::super::objects::{
        ContainerBody, DataMaskBody, WorkingSetBody, create_container, create_data_mask,
        create_working_set,
    };
    use super::*;
    use crate::net::Message;
    use crate::net::pgn_defs::{PGN_ECU_TO_VT, PGN_VT_TO_ECU};
    use proptest::prelude::*;
    use std::collections::HashSet;

    #[test]
    fn set_numeric_skips_when_unchanged() {
        let mut tracker = VTClientStateTracker::new();
        tracker.set_numeric_value(5, 42);
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        assert!(h.set_numeric_value(5, 42).is_none());
        let op = h.set_numeric_value(5, 100).unwrap();
        assert_eq!(
            op,
            UpdateOp::Numeric {
                id: ObjectID(5),
                value: 100
            }
        );
        h.confirm(&op);
        assert_eq!(h.set_numeric_value(5, 100), None);
    }

    #[test]
    fn try_set_numeric_scaled_rejects_bad_inputs_without_queueing() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();

        assert!(h.try_set_numeric_scaled(1, f64::NAN, 1.0, 0.0).is_err());
        assert!(
            h.try_set_numeric_scaled(1, 1.0, f64::INFINITY, 0.0)
                .is_err()
        );
        assert!(
            h.try_set_numeric_scaled(1, 1.0, 1.0, f64::NEG_INFINITY)
                .is_err()
        );
        assert!(h.try_set_numeric_scaled(1, -2.0, 1.0, 0.0).is_err());
        assert!(
            h.try_set_numeric_scaled(1, f64::from(u32::MAX), 2.0, 0.0)
                .is_err()
        );

        assert_eq!(h.pending_count(), 0);
        assert!(h.end_batch().is_empty());
    }

    #[test]
    fn try_set_numeric_scaled_uses_checked_legacy_formula_for_valid_values() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        let op = h.try_set_numeric_scaled(9, 10.75, 2.0, 0.5).unwrap();
        assert_eq!(
            op,
            Some(UpdateOp::Numeric {
                id: ObjectID(9),
                value: 22,
            })
        );
    }

    #[test]
    fn set_numeric_scaled_compat_wrapper_drops_invalid_scaled_values() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        assert_eq!(h.set_numeric_scaled(1, f64::NAN, 1.0, 0.0), None);
        assert_eq!(h.set_numeric_scaled(1, -1.0, 1.0, 0.0), None);
    }

    #[test]
    fn set_string_skips_when_unchanged() {
        let mut tracker = VTClientStateTracker::new();
        tracker.set_string_value(7, "hi");
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        assert!(h.set_string_value(7, "hi").is_none());
        let op = h.set_string_value(7, "world").unwrap();
        assert_eq!(
            op,
            UpdateOp::String {
                id: ObjectID(7),
                value: "world".into()
            }
        );
    }

    #[test]
    fn try_set_string_value_rejects_oversized_values_without_queueing() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();

        let oversized = "x".repeat(VT_STRING_VALUE_MAX_LEN + 1);
        let err = h.try_set_string_value(7, oversized).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert_eq!(h.pending_count(), 0);
        assert!(h.end_batch().is_empty());
    }

    #[test]
    fn set_string_value_compat_wrapper_drops_oversized_values() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();

        let oversized = "x".repeat(VT_STRING_VALUE_MAX_LEN + 1);
        assert_eq!(h.set_string_value(7, oversized), None);
        assert_eq!(h.pending_count(), 0);
    }

    #[test]
    fn show_hide_short_circuit() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        let op = h.show(1).unwrap();
        h.confirm(&op);
        assert!(h.show(1).is_none());
        let _ = h.hide(1).unwrap();
    }

    #[test]
    fn batch_collects_then_drains() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();
        assert!(h.set_numeric_value(1, 10).is_none());
        assert!(h.set_string_value(2, "x").is_none());
        assert!(h.show(3).is_none());
        assert_eq!(h.pending_count(), 3);
        let ops = h.end_batch();
        assert_eq!(ops.len(), 3);
    }

    #[test]
    fn batch_coalesces_duplicate_slots_to_last_value() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();
        h.set_numeric_value(1, 10);
        h.set_numeric_value(1, 20);
        h.set_string_value(2, "old");
        h.set_string_value(2, "new");
        h.show(3);
        h.hide(3);
        h.enable(4);
        h.disable(4);
        h.change_active_mask(0x1000, 5).unwrap();
        h.change_active_mask(0x1000, 6).unwrap();

        assert_eq!(h.pending_count(), 5);
        assert_eq!(
            h.end_batch(),
            vec![
                UpdateOp::Numeric {
                    id: ObjectID(1),
                    value: 20,
                },
                UpdateOp::String {
                    id: ObjectID(2),
                    value: "new".into(),
                },
                UpdateOp::Visibility {
                    id: ObjectID(3),
                    visible: false,
                },
                UpdateOp::Enable {
                    id: ObjectID(4),
                    enabled: false,
                },
                UpdateOp::ActiveMask {
                    working_set_id: ObjectID(0x1000),
                    mask_id: ObjectID(6),
                },
            ]
        );
    }

    #[test]
    fn batch_revert_to_cached_value_removes_pending_slot() {
        let mut tracker = VTClientStateTracker::new();
        tracker.set_numeric_value(1, 10);
        tracker.set_string_value(2, "old");
        tracker.set_visibility(3, true);
        tracker.set_enable_state(4, true);
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();

        h.set_numeric_value(1, 20);
        h.set_numeric_value(1, 10);
        h.set_string_value(2, "new");
        h.set_string_value(2, "old");
        h.hide(3);
        h.show(3);
        h.disable(4);
        h.enable(4);
        h.change_active_mask(0x1000, 5).unwrap();
        h.change_active_mask(0x1000, ObjectID::NULL).unwrap();

        assert_eq!(h.pending_count(), 0);
        assert!(h.end_batch().is_empty());
    }

    #[test]
    fn batch_revert_active_mask_to_cached_value_removes_only_matching_working_set_slot() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();
        h.change_active_mask(0x1000, 5).unwrap();
        h.change_active_mask(0x2000, 6).unwrap();
        h.change_active_mask(0x1000, ObjectID::NULL).unwrap();

        assert_eq!(
            h.end_batch(),
            vec![UpdateOp::ActiveMask {
                working_set_id: ObjectID(0x2000),
                mask_id: ObjectID(6),
            }]
        );
    }

    #[test]
    fn cancel_batch_drops_pending() {
        let mut tracker = VTClientStateTracker::new();
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        h.begin_batch();
        h.set_numeric_value(1, 5);
        h.cancel_batch();
        assert_eq!(h.pending_count(), 0);
    }

    #[test]
    fn change_active_mask_validates_pool() {
        let mut tracker = VTClientStateTracker::new();
        let pool = ObjectPool::default()
            .with_object(create_data_mask(1, &DataMaskBody::default()))
            .with_object(create_container(2, &ContainerBody::default()));
        let mut h = VTClientUpdateHelper::new(&mut tracker).with_pool(&pool);
        // Switch to a valid Data Mask.
        let op = h.change_active_mask(0, 1).unwrap().unwrap();
        assert_eq!(
            op,
            UpdateOp::ActiveMask {
                working_set_id: ObjectID(0),
                mask_id: ObjectID(1)
            }
        );
        // Container is not a mask type.
        assert!(h.change_active_mask(0, 2).is_err());
        // Unknown mask_id.
        assert!(h.change_active_mask(0, 99).is_err());
    }

    #[test]
    fn change_active_mask_skips_when_already_active() {
        let mut tracker = VTClientStateTracker::new();
        // Force tracker into an "active mask" state via a vt_status
        // simulation isn't strictly necessary — set_numeric/etc. don't
        // expose this. Bypass: call helper with mask_id matching the
        // tracker's initial 0xFFFF (default).
        let mut h = VTClientUpdateHelper::new(&mut tracker);
        assert!(h.change_active_mask(0, 0xFFFF).unwrap().is_none());
    }

    #[test]
    fn update_op_to_client_outbound_uses_canonical_command_layouts() {
        let mut client = VTClient::new(VTClientConfig::default());
        force_connected(&mut client);

        assert_outbound(
            UpdateOp::Numeric {
                id: ObjectID(0x1234),
                value: 0xDEAD_BEEF,
            }
            .to_client_outbound(&client)
            .unwrap(),
            [0xA8, 0x34, 0x12, 0xFF, 0xEF, 0xBE, 0xAD, 0xDE],
        );
        assert_outbound(
            UpdateOp::String {
                id: ObjectID(5),
                value: "hi".into(),
            }
            .to_client_outbound(&client)
            .unwrap(),
            [0xB3, 0x05, 0x00, 0x02, 0x00, b'h', b'i', 0xFF],
        );
        assert_outbound(
            UpdateOp::Visibility {
                id: ObjectID(0x1234),
                visible: false,
            }
            .to_client_outbound(&client)
            .unwrap(),
            [0xA0, 0x34, 0x12, 0x00, 0xFF, 0xFF, 0xFF, 0xFF],
        );
        assert_outbound(
            UpdateOp::Enable {
                id: ObjectID(0x1234),
                enabled: true,
            }
            .to_client_outbound(&client)
            .unwrap(),
            [0xA1, 0x34, 0x12, 0x01, 0xFF, 0xFF, 0xFF, 0xFF],
        );
        assert_outbound(
            UpdateOp::ActiveMask {
                working_set_id: ObjectID(1),
                mask_id: ObjectID(2),
            }
            .to_client_outbound(&client)
            .unwrap(),
            [0xAD, 0x01, 0x00, 0x02, 0x00, 0xFF, 0xFF, 0xFF],
        );

        let disconnected = VTClient::new(VTClientConfig::default());
        assert!(
            UpdateOp::Numeric {
                id: ObjectID(1),
                value: 0,
            }
            .to_client_outbound(&disconnected)
            .is_err()
        );

        assert!(
            UpdateOp::String {
                id: ObjectID(7),
                value: "x".repeat(VT_STRING_VALUE_MAX_LEN + 1),
            }
            .to_client_outbound(&client)
            .is_err()
        );
    }

    proptest! {
        #[test]
        fn proptest_scaled_numeric_is_fallible_not_saturating(
            value in any::<f64>(),
            scale in any::<f64>(),
            offset in any::<f64>(),
        ) {
            let raw = (value + offset) * scale;
            match scaled_numeric_raw(value, scale, offset) {
                Ok(encoded) => {
                    prop_assert!(value.is_finite());
                    prop_assert!(scale.is_finite());
                    prop_assert!(offset.is_finite());
                    prop_assert!(raw.is_finite());
                    prop_assert!(raw >= 0.0);
                    prop_assert!(raw <= f64::from(u32::MAX));
                    prop_assert_eq!(encoded, raw.trunc() as u32);
                }
                Err(_) => {
                    prop_assert!(
                        !value.is_finite()
                            || !scale.is_finite()
                            || !offset.is_finite()
                            || !raw.is_finite()
                            || raw < 0.0
                            || raw > f64::from(u32::MAX)
                    );
                }
            }
        }

        #[test]
        fn proptest_batch_coalescing_keeps_pending_slots_bounded(
            operations in proptest::collection::vec(
                (0u8..=4, any::<u16>(), any::<u16>(), any::<u32>(), any::<bool>()),
                0..=512,
            ),
        ) {
            let mut tracker = VTClientStateTracker::new();
            let mut helper = VTClientUpdateHelper::new(&mut tracker);
            let mut expected_slots = HashSet::<UpdateOpSlot>::new();
            helper.begin_batch();

            for (kind, id_raw, aux_raw, value, flag) in operations {
                let id = ObjectID(id_raw);
                match kind {
                    0 => {
                        helper.set_numeric_value(id, value);
                        expected_slots.insert(UpdateOpSlot::Numeric(id));
                    }
                    1 => {
                        helper.set_string_value(id, format!("{value:08X}"));
                        expected_slots.insert(UpdateOpSlot::String(id));
                    }
                    2 => {
                        helper.set_visibility(id, flag);
                        expected_slots.insert(UpdateOpSlot::Visibility(id));
                    }
                    3 => {
                        helper.set_enable(id, flag);
                        expected_slots.insert(UpdateOpSlot::Enable(id));
                    }
                    _ => {
                        let mask = ObjectID(aux_raw);
                        helper.change_active_mask(id, mask).unwrap();
                        if mask == ObjectID::NULL {
                            expected_slots.remove(&UpdateOpSlot::ActiveMask(id));
                        } else {
                            expected_slots.insert(UpdateOpSlot::ActiveMask(id));
                        }
                    }
                }
                prop_assert_eq!(helper.pending_count(), expected_slots.len());
                prop_assert!(helper.pending_count() <= 5 * (usize::from(u16::MAX) + 1));
            }
            let drained = helper.end_batch();
            prop_assert_eq!(drained.len(), expected_slots.len());
            prop_assert!(!helper.is_batching());
            prop_assert_eq!(helper.pending_count(), 0);
        }
    }

    fn force_connected(c: &mut VTClient) {
        c.set_object_pool(dummy_pool());
        c.connect().unwrap();
        let mut data = vec![cmd::VT_STATUS];
        data.resize(8, 0xFF);
        data[6] = 4;
        c.handle_vt_message(&Message::new(PGN_VT_TO_ECU, data, 0x80));
        let _ = c.update(1);
        let _ = c.update(1);
        let mut memory_response = [0xFFu8; 8];
        memory_response[0] = cmd::GET_MEMORY_RESPONSE;
        memory_response[1] = 0x00;
        c.handle_vt_message(&Message::new(PGN_VT_TO_ECU, memory_response.to_vec(), 0x80));
        let _ = c.update(1);
        let _ = c.update(1_000);
        let mut end_of_pool_response = [0xFFu8; 8];
        end_of_pool_response[0] = cmd::END_OF_POOL;
        end_of_pool_response[1] = 0x00;
        end_of_pool_response[6] = 0x00;
        c.handle_vt_message(&Message::new(
            PGN_VT_TO_ECU,
            end_of_pool_response.to_vec(),
            0x80,
        ));
        assert_eq!(c.state(), VTState::Connected);
    }

    fn assert_outbound(out: ClientOutbound, expected_data: [u8; 8]) {
        assert_eq!(out.pgn, PGN_ECU_TO_VT);
        assert_eq!(out.dest, Some(0x80));
        assert_eq!(out.data.as_slice(), &expected_data);
    }

    fn dummy_pool() -> ObjectPool {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
    }
}
