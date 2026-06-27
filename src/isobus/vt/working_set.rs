//! Client-side VT working set: tracks the active mask and the
//! data / soft-key masks owned by this client.
//!
//! Mirrors the C++ `machbus::isobus::vt::WorkingSet` (28 LOC).

use alloc::vec::Vec;

use super::objects::ObjectID;

/// Client-side working set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkingSet {
    active_mask: ObjectID,
    masks: Vec<ObjectID>,
    soft_key_masks: Vec<ObjectID>,
}

impl WorkingSet {
    pub fn set_active_mask(&mut self, mask_id: impl Into<ObjectID>) {
        self.active_mask = mask_id.into();
    }

    #[inline]
    #[must_use]
    pub const fn active_mask(&self) -> ObjectID {
        self.active_mask
    }

    pub fn add_data_mask(&mut self, mask_id: impl Into<ObjectID>) {
        self.masks.push(mask_id.into());
    }

    pub fn add_soft_key_mask(&mut self, mask_id: impl Into<ObjectID>) {
        self.soft_key_masks.push(mask_id.into());
    }

    #[inline]
    #[must_use]
    pub fn data_masks(&self) -> &[ObjectID] {
        &self.masks
    }

    #[inline]
    #[must_use]
    pub fn soft_key_masks(&self) -> &[ObjectID] {
        &self.soft_key_masks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_masks_and_active() {
        let mut ws = WorkingSet::default();
        assert_eq!(ws.active_mask(), 0);
        ws.set_active_mask(42);
        ws.add_data_mask(10);
        ws.add_data_mask(20);
        ws.add_soft_key_mask(30);
        assert_eq!(ws.active_mask(), 42);
        assert_eq!(ws.data_masks(), &[10, 20]);
        assert_eq!(ws.soft_key_masks(), &[30]);
    }
}
