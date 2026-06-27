//! CAN fault confinement → fail-safe mapping (ISO 11783-2/-3 over ISO 11898).
//!
//! A CAN controller derives an error-confinement state from its transmit/receive
//! error counters (ISO 11898 thresholds: error-passive at 128, bus-off at 256):
//! error-active → error-passive → bus-off. A safety-relevant ECU must react —
//! degrade while the link is error-passive and enter a fail-safe state on bus-off.
//!
//! This is the repo-owned mapping plus an edge monitor; the counters and the
//! [`BusState`] itself come from the CAN adapter / driver. Wire the monitor's edges into
//! [`SafetyPolicy::trigger_emergency`](super::policy::SafetyPolicy::trigger_emergency)
//! / [`reset_to_normal`](super::policy::SafetyPolicy::reset_to_normal).

use super::can_adapter::can::BusState;

/// The action an application should take for a given CAN error-confinement state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FaultConfinementAction {
    /// Error-active — normal operation.
    #[default]
    Normal,
    /// Error-passive — the bus is degraded; reduce reliance on the link and warn.
    Degrade,
    /// Bus-off — the controller is disconnected; enter the fail-safe state.
    FailSafe,
}

/// Map a CAN error-confinement state to the required application action.
#[must_use]
pub fn fault_confinement_action(state: BusState) -> FaultConfinementAction {
    match state {
        BusState::ErrorActive => FaultConfinementAction::Normal,
        BusState::ErrorPassive => FaultConfinementAction::Degrade,
        BusState::BusOff => FaultConfinementAction::FailSafe,
    }
}

/// Tracks the CAN error-confinement state and reports the required action only
/// when it *changes*, so the caller drives the safety policy on transitions
/// (trigger fail-safe on entry to bus-off, clear it on recovery to active).
#[derive(Debug, Clone, Default)]
pub struct FaultConfinementMonitor {
    action: FaultConfinementAction,
}

impl FaultConfinementMonitor {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The most recently observed action.
    #[must_use]
    pub fn action(&self) -> FaultConfinementAction {
        self.action
    }

    /// `true` once the CAN controller has reached bus-off (fail-safe required).
    #[must_use]
    pub fn is_fail_safe(&self) -> bool {
        matches!(self.action, FaultConfinementAction::FailSafe)
    }

    /// Feed the latest [`BusState`]. Returns `Some(new_action)` only when the
    /// required action changed from the previous observation, otherwise `None`.
    pub fn observe(&mut self, state: BusState) -> Option<FaultConfinementAction> {
        let next = fault_confinement_action(state);
        if next == self.action {
            None
        } else {
            self.action = next;
            Some(next)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_maps_to_action() {
        assert_eq!(
            fault_confinement_action(BusState::ErrorActive),
            FaultConfinementAction::Normal
        );
        assert_eq!(
            fault_confinement_action(BusState::ErrorPassive),
            FaultConfinementAction::Degrade
        );
        assert_eq!(
            fault_confinement_action(BusState::BusOff),
            FaultConfinementAction::FailSafe
        );
    }

    #[test]
    fn monitor_reports_only_on_transitions() {
        let mut m = FaultConfinementMonitor::new();
        assert_eq!(m.action(), FaultConfinementAction::Normal);
        // Same state repeated ⇒ no edge.
        assert_eq!(m.observe(BusState::ErrorActive), None);
        // Degrade edge.
        assert_eq!(
            m.observe(BusState::ErrorPassive),
            Some(FaultConfinementAction::Degrade)
        );
        assert_eq!(m.observe(BusState::ErrorPassive), None);
        // Fail-safe edge on bus-off.
        assert_eq!(
            m.observe(BusState::BusOff),
            Some(FaultConfinementAction::FailSafe)
        );
        assert!(m.is_fail_safe());
        // Recovery clears back to normal (an edge).
        assert_eq!(
            m.observe(BusState::ErrorActive),
            Some(FaultConfinementAction::Normal)
        );
        assert!(!m.is_fail_safe());
    }
}
