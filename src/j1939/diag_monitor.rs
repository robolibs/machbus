//! Active-fault monitor for J1939 / ISO 11783-12 diagnostics.
//!
//! The codecs in [`crate::j1939::diagnostic`] turn DM1 (active DTC list)
//! frames into [`DmDtcList`] values, but nothing tracks fault *state* over
//! time. This monitor does: it ingests successive DM1 snapshots, maintains
//! the current active-fault set and lamp state, and reports the delta
//! (newly-set and cleared faults) on each update — the core a service-tool
//! or operator-alert workflow needs.
//!
//! DTC identity is by `(spn, fmi)` (see [`Dtc::matches`]); a changing
//! occurrence count on an already-active fault is an update, not a new
//! fault.

use alloc::vec::Vec;

use crate::j1939::diagnostic::{DiagnosticLamps, DmDtcList, Dtc, Fmi};

/// What changed between two DM1 snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DtcDelta {
    /// Faults present now that were not active before.
    pub newly_active: Vec<Dtc>,
    /// Faults that were active before and are now gone.
    pub cleared: Vec<Dtc>,
}

impl DtcDelta {
    /// `true` if neither set nor cleared any fault.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.newly_active.is_empty() && self.cleared.is_empty()
    }
}

/// Tracks the current active-fault set and lamp state across DM1 updates.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DiagnosticMonitor {
    active: Vec<Dtc>,
    lamps: DiagnosticLamps,
}

impl DiagnosticMonitor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Ingest a DM1 (active DTC list) snapshot. Updates the active set and
    /// lamp state and returns the delta versus the previous snapshot.
    pub fn ingest_dm1(&mut self, dm: &DmDtcList) -> DtcDelta {
        let newly_active = dm
            .dtcs
            .iter()
            .filter(|n| !self.active.iter().any(|a| a.matches(n)))
            .copied()
            .collect();
        let cleared = self
            .active
            .iter()
            .filter(|a| !dm.dtcs.iter().any(|n| n.matches(a)))
            .copied()
            .collect();
        self.active = dm.dtcs.clone();
        self.lamps = dm.lamps;
        DtcDelta {
            newly_active,
            cleared,
        }
    }

    /// Current active faults (as of the last ingested DM1).
    #[must_use]
    pub fn active(&self) -> &[Dtc] {
        &self.active
    }

    /// Current lamp state (as of the last ingested DM1).
    #[must_use]
    pub fn lamps(&self) -> DiagnosticLamps {
        self.lamps
    }

    /// `true` if a fault with this `(spn, fmi)` is currently active.
    #[must_use]
    pub fn is_active(&self, spn: u32, fmi: Fmi) -> bool {
        self.active.iter().any(|d| d.spn == spn && d.fmi == fmi)
    }

    #[must_use]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    /// Forget all tracked faults and reset lamps (e.g. after a DM3/DM11
    /// clear-all). Does not itself emit a clear request.
    pub fn clear(&mut self) {
        self.active.clear();
        self.lamps = DiagnosticLamps::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dtc(spn: u32, fmi: Fmi, oc: u8) -> Dtc {
        Dtc {
            spn,
            fmi,
            occurrence_count: oc,
        }
    }

    fn dm1(dtcs: Vec<Dtc>) -> DmDtcList {
        DmDtcList {
            lamps: DiagnosticLamps::default(),
            dtcs,
        }
    }

    #[test]
    fn tracks_newly_active_and_cleared_faults() {
        let mut mon = DiagnosticMonitor::new();

        // First snapshot: two faults, both new.
        let d = mon.ingest_dm1(&dm1(vec![
            dtc(100, Fmi::AboveNormal, 1),
            dtc(200, Fmi::BelowNormal, 1),
        ]));
        assert_eq!(d.newly_active.len(), 2);
        assert!(d.cleared.is_empty());
        assert_eq!(mon.active_count(), 2);
        assert!(mon.is_active(100, Fmi::AboveNormal));

        // Second snapshot: 100 persists (occurrence bumped), 200 cleared,
        // 300 newly active.
        let d = mon.ingest_dm1(&dm1(vec![
            dtc(100, Fmi::AboveNormal, 2),
            dtc(300, Fmi::Erratic, 1),
        ]));
        assert_eq!(d.newly_active, vec![dtc(300, Fmi::Erratic, 1)]);
        assert_eq!(d.cleared, vec![dtc(200, Fmi::BelowNormal, 1)]);
        assert!(!mon.is_active(200, Fmi::BelowNormal));
        assert!(mon.is_active(300, Fmi::Erratic));
    }

    #[test]
    fn identical_snapshot_yields_empty_delta() {
        let mut mon = DiagnosticMonitor::new();
        let snap = dm1(vec![dtc(50, Fmi::Erratic, 1)]);
        mon.ingest_dm1(&snap);
        // Same faults again (even with a bumped occurrence count) → no delta.
        let d = mon.ingest_dm1(&dm1(vec![dtc(50, Fmi::Erratic, 9)]));
        assert!(d.is_empty());
        assert_eq!(mon.active_count(), 1);
    }

    #[test]
    fn clear_forgets_all_faults() {
        let mut mon = DiagnosticMonitor::new();
        mon.ingest_dm1(&dm1(vec![dtc(1, Fmi::Erratic, 1)]));
        mon.clear();
        assert_eq!(mon.active_count(), 0);
        assert!(!mon.is_active(1, Fmi::Erratic));
    }
}
