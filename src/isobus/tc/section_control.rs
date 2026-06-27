//! Section-control runtime (ISO 11783-10 TC-SC).
//!
//! A boom is split into sections (see
//! [`crate::isobus::tc::ddop_helpers::SectionInfo`]). Section control
//! decides, each control step, which sections are commanded ON. A section
//! applies only when **all** of the following hold:
//!
//! - the task is active (no application off-task);
//! - the section-control master switch is on;
//! - that individual section is requested on (by the operator or by
//!   automatic coverage/boundary logic the caller supplies).
//!
//! The runtime tracks the last commanded state per section and reports
//! whether anything changed, so a caller emits a new section-command
//! setpoint only on change. It is topology-agnostic (index-based) and
//! decoupled from the task runtime (the active flag is passed in).

use alloc::{vec, vec::Vec};

/// Per-step section-control runtime over a fixed number of sections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SectionControl {
    requested: Vec<bool>,
    commanded: Vec<bool>,
}

impl SectionControl {
    /// A runtime for `count` sections, all initially requested off.
    #[must_use]
    pub fn new(count: usize) -> Self {
        Self {
            requested: vec![false; count],
            commanded: vec![false; count],
        }
    }

    #[must_use]
    pub fn count(&self) -> usize {
        self.requested.len()
    }

    /// Request a single section on/off. Returns `false` if the index is
    /// out of range (no change).
    pub fn request(&mut self, section: usize, on: bool) -> bool {
        match self.requested.get_mut(section) {
            Some(slot) => {
                *slot = on;
                true
            }
            None => false,
        }
    }

    /// Request all sections on/off.
    pub fn request_all(&mut self, on: bool) {
        for slot in &mut self.requested {
            *slot = on;
        }
    }

    /// Recompute commanded section states: a section is ON only while the
    /// task is active, the master switch is on, and the section is
    /// requested. Returns `true` if any commanded state changed.
    pub fn update(&mut self, task_active: bool, master_on: bool) -> bool {
        let mut changed = false;
        let gate = task_active && master_on;
        for (cmd, req) in self.commanded.iter_mut().zip(&self.requested) {
            let next = gate && *req;
            if *cmd != next {
                *cmd = next;
                changed = true;
            }
        }
        changed
    }

    /// The commanded ON/OFF state per section after the last `update`.
    #[must_use]
    pub fn commanded(&self) -> &[bool] {
        &self.commanded
    }

    /// Count of sections currently commanded ON.
    #[must_use]
    pub fn active_count(&self) -> usize {
        self.commanded.iter().filter(|&&on| on).count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_apply_only_when_active_master_on_and_requested() {
        let mut sc = SectionControl::new(4);
        assert_eq!(sc.count(), 4);
        sc.request(0, true);
        sc.request(2, true);

        // Task active + master on → requested sections command on.
        assert!(sc.update(true, true));
        assert_eq!(sc.commanded(), &[true, false, true, false]);
        assert_eq!(sc.active_count(), 2);

        // No change on a second identical update.
        assert!(!sc.update(true, true));

        // Master off → all off (change).
        assert!(sc.update(true, false));
        assert_eq!(sc.active_count(), 0);

        // Master back on → sections resume.
        assert!(sc.update(true, true));
        assert_eq!(sc.commanded(), &[true, false, true, false]);

        // Task inactive → all off regardless of master/requested.
        assert!(sc.update(false, true));
        assert_eq!(sc.active_count(), 0);
    }

    #[test]
    fn request_all_and_out_of_range() {
        let mut sc = SectionControl::new(3);
        sc.request_all(true);
        assert!(sc.update(true, true));
        assert_eq!(sc.active_count(), 3);

        // Out-of-range request is rejected without panicking.
        assert!(!sc.request(99, true));
        assert!(sc.request(1, false));
        assert!(sc.update(true, true));
        assert_eq!(sc.commanded(), &[true, false, true]);
    }
}
