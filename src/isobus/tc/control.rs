//! Prescription / rate-control runtime (ISO 11783-10).
//!
//! The `geo` module resolves the prescribed application rate at a position
//! ([`crate::isobus::tc::TCGEOInterface::get_rate_at_position`]); the task
//! runtime ([`crate::isobus::tc::TaskSession`]) tracks whether a task is
//! active. This module is the controller that ties them together: it
//! decides the rate to actually command — follow the prescription while
//! the task is active, shut off otherwise — and reports only when the
//! commanded rate changes, so a caller emits a setpoint only on change.
//!
//! It is decoupled from `geo`: the caller passes the prescription rate it
//! already resolved (or `None` when the position is outside every zone).

use crate::isobus::tc::task::TaskSession;

/// The controller's decision for one control step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateCommand {
    /// Rate to apply: `Some(rate)` while applying, `None` = shut off
    /// (task not active, or no prescription at the current position).
    pub rate: Option<i32>,
    /// `true` if the commanded rate differs from the previous step (the
    /// caller should emit a setpoint), `false` if unchanged.
    pub changed: bool,
}

/// Stateful prescription/rate controller. Remembers the last commanded
/// rate so it can flag changes for change-driven setpoint messaging.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrescriptionController {
    last_commanded: Option<i32>,
}

impl PrescriptionController {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The most recently commanded rate (`None` = shut off / never set).
    #[must_use]
    pub const fn last_commanded(&self) -> Option<i32> {
        self.last_commanded
    }

    /// Compute the commanded rate for one control step given the task
    /// session and the prescription rate at the current position.
    ///
    /// Application happens only while the task is active; otherwise the
    /// rate is shut off (`None`). The result flags whether the commanded
    /// rate changed since the previous step.
    pub fn command(
        &mut self,
        session: &TaskSession,
        prescription_rate: Option<i32>,
    ) -> RateCommand {
        let commanded = if session.is_active() {
            prescription_rate
        } else {
            None
        };
        let changed = commanded != self.last_commanded;
        self.last_commanded = commanded;
        RateCommand {
            rate: commanded,
            changed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active_session() -> TaskSession {
        let mut s = TaskSession::new();
        s.start().unwrap();
        s
    }

    #[test]
    fn applies_prescription_rate_only_while_active() {
        let mut ctrl = PrescriptionController::new();
        let session = active_session();

        let c = ctrl.command(&session, Some(120));
        assert_eq!(c.rate, Some(120));
        assert!(c.changed);
        assert_eq!(ctrl.last_commanded(), Some(120));

        // Same rate again → no change.
        let c = ctrl.command(&session, Some(120));
        assert_eq!(c.rate, Some(120));
        assert!(!c.changed);

        // New zone rate → change.
        let c = ctrl.command(&session, Some(80));
        assert_eq!(c.rate, Some(80));
        assert!(c.changed);

        // Outside any zone → shut off (change).
        let c = ctrl.command(&session, None);
        assert_eq!(c.rate, None);
        assert!(c.changed);
    }

    #[test]
    fn shuts_off_when_task_not_active() {
        let mut ctrl = PrescriptionController::new();
        let idle = TaskSession::new();
        // Even with a prescription rate available, an inactive task applies nothing.
        let c = ctrl.command(&idle, Some(200));
        assert_eq!(c.rate, None);
        assert!(!c.changed); // last was None (default) → unchanged.

        // Pausing mid-application shuts off and flags the change.
        let mut session = active_session();
        assert_eq!(ctrl.command(&session, Some(50)).rate, Some(50));
        session.pause().unwrap();
        let c = ctrl.command(&session, Some(50));
        assert_eq!(c.rate, None);
        assert!(c.changed);
    }
}
