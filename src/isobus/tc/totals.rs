//! Task / lifetime total accumulation (ISO 11783-10).
//!
//! Total process-data values (e.g. total area, total volume) are tracked at two
//! scopes: the **task total**, which is zeroed when a task starts and counts the
//! work done during that task, and the **lifetime total**, which persists across
//! tasks. This module is the repo-owned accumulator for both; the caller feeds
//! per-DDI increments (e.g. derived from rate × distance, or from device deltas)
//! and reads back either scope. Use [`ddi_is_total`](super::ddi_is_total) to
//! decide which DDIs to route here.

use alloc::{collections::BTreeMap, vec::Vec};

/// Per-DDI task- and lifetime-total accumulator.
#[derive(Debug, Clone, Default)]
pub struct TaskTotals {
    task: BTreeMap<u16, i64>,
    lifetime: BTreeMap<u16, i64>,
}

impl TaskTotals {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Add `delta` to both the current task total and the lifetime total for
    /// `ddi`.
    pub fn accumulate(&mut self, ddi: u16, delta: i64) {
        *self.task.entry(ddi).or_insert(0) += delta;
        *self.lifetime.entry(ddi).or_insert(0) += delta;
    }

    /// Zero all task totals (call when a new task starts). Lifetime totals are
    /// untouched.
    pub fn reset_task(&mut self) {
        self.task.clear();
    }

    /// The current task total for `ddi` (0 if none accumulated).
    #[must_use]
    pub fn task_total(&self, ddi: u16) -> i64 {
        self.task.get(&ddi).copied().unwrap_or(0)
    }

    /// The lifetime total for `ddi` (0 if none accumulated).
    #[must_use]
    pub fn lifetime_total(&self, ddi: u16) -> i64 {
        self.lifetime.get(&ddi).copied().unwrap_or(0)
    }

    /// Iterate the current task totals as `(ddi, value)`, ordered by DDI.
    pub fn task_totals(&self) -> impl Iterator<Item = (u16, i64)> + '_ {
        self.task.iter().map(|(&d, &v)| (d, v))
    }

    /// Iterate the lifetime totals as `(ddi, value)`, ordered by DDI.
    pub fn lifetime_totals(&self) -> impl Iterator<Item = (u16, i64)> + '_ {
        self.lifetime.iter().map(|(&d, &v)| (d, v))
    }

    /// Seed a lifetime total (e.g. restored from persistent storage).
    pub fn set_lifetime_total(&mut self, ddi: u16, value: i64) {
        self.lifetime.insert(ddi, value);
    }

    /// Serialize the lifetime totals to bytes (10 per entry: u16 DDI LE + i64
    /// value LE) so they can be persisted across power cycles.
    #[must_use]
    pub fn export_lifetime_totals(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.lifetime.len() * 10);
        for (&ddi, &value) in &self.lifetime {
            out.extend_from_slice(&ddi.to_le_bytes());
            out.extend_from_slice(&value.to_le_bytes());
        }
        out
    }

    /// Restore lifetime totals from [`export_lifetime_totals`] output, replacing
    /// each DDI's stored value. Returns how many entries were loaded; trailing
    /// bytes that don't form a full entry are ignored.
    ///
    /// [`export_lifetime_totals`]: Self::export_lifetime_totals
    pub fn import_lifetime_totals(&mut self, data: &[u8]) -> usize {
        let mut loaded = 0;
        for chunk in data.chunks_exact(10) {
            let ddi = u16::from_le_bytes([chunk[0], chunk[1]]);
            let value = i64::from_le_bytes([
                chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7], chunk[8], chunk[9],
            ]);
            self.lifetime.insert(ddi, value);
            loaded += 1;
        }
        loaded
    }
}

#[cfg(test)]
mod tests {
    use super::super::ddi;
    use super::*;

    #[test]
    fn task_reset_keeps_lifetime_totals() {
        let mut t = TaskTotals::new();
        let d = ddi::TOTAL_AREA;

        t.accumulate(d, 100);
        t.accumulate(d, 50);
        assert_eq!(t.task_total(d), 150);
        assert_eq!(t.lifetime_total(d), 150);

        // A new task zeroes the task total but not the lifetime total.
        t.reset_task();
        assert_eq!(t.task_total(d), 0);
        assert_eq!(t.lifetime_total(d), 150);

        t.accumulate(d, 25);
        assert_eq!(t.task_total(d), 25);
        assert_eq!(t.lifetime_total(d), 175);

        // Unknown DDI reads as zero; lifetime can be seeded from storage.
        assert_eq!(t.task_total(0xFFFF), 0);
        t.set_lifetime_total(d, 1_000);
        assert_eq!(t.lifetime_total(d), 1_000);
    }

    #[test]
    fn lifetime_totals_round_trip_through_persistence() {
        let mut t = TaskTotals::new();
        t.accumulate(ddi::TOTAL_AREA, 4_200);
        t.accumulate(ddi::TOTAL_AREA, 800); // 5_000
        t.set_lifetime_total(0x0033, -17);

        let blob = t.export_lifetime_totals();
        assert_eq!(blob.len(), 20); // 2 DDIs × 10 bytes

        // Restore into a fresh accumulator (simulated power cycle).
        let mut restored = TaskTotals::new();
        assert_eq!(restored.import_lifetime_totals(&blob), 2);
        assert_eq!(restored.lifetime_total(ddi::TOTAL_AREA), 5_000);
        assert_eq!(restored.lifetime_total(0x0033), -17);
        // Task totals are not part of the persisted lifetime set.
        assert_eq!(restored.task_total(ddi::TOTAL_AREA), 0);

        // A trailing partial entry is ignored.
        let mut padded = blob.clone();
        padded.extend_from_slice(&[0xAB, 0xCD, 0xEF]);
        let mut r2 = TaskTotals::new();
        assert_eq!(r2.import_lifetime_totals(&padded), 2);
    }
}
