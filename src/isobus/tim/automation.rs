//! AEF 023 per-function automation state and version negotiation
//! (D.2.2 Table 45, B.2.2 / §5.3.4.1).
//!
//! `TimAuthorityState` (`Idle | Requested | Granted | Denied | Revoked`) maps
//! onto none of the AEF automation states and never crossed the wire, and one
//! global authority could not hold per-function state at all — so a client
//! steering while its hitch request was still pending was unrepresentable.

use alloc::vec::Vec;

use super::functions::{TimExitReason, TimFunctionId};

/// The 4-bit automation status SLOT (D.2.2 Table 45).
///
/// This mirrors [`crate::session::sys::AutomationStatus`], which is the
/// session-facing view; this one is the wire encoding used inside TIM messages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum AutomationState {
    Unavailable = 0x0,
    NotReady = 0x1,
    ReadyToEnable = 0x2,
    Enabled = 0x3,
    Pending = 0x4,
    ActiveNotLimited = 0x5,
    ActiveLimitedHigh = 0x6,
    ActiveLimitedLow = 0x7,
    Fault = 0xD,
    Error = 0xE,
    #[default]
    NotAvailable = 0xF,
}

impl AutomationState {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x0F {
            0x0 => Some(Self::Unavailable),
            0x1 => Some(Self::NotReady),
            0x2 => Some(Self::ReadyToEnable),
            0x3 => Some(Self::Enabled),
            0x4 => Some(Self::Pending),
            0x5 => Some(Self::ActiveNotLimited),
            0x6 => Some(Self::ActiveLimitedHigh),
            0x7 => Some(Self::ActiveLimitedLow),
            0xD => Some(Self::Fault),
            0xE => Some(Self::Error),
            0xF => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn is_active(self) -> bool {
        matches!(
            self,
            Self::ActiveNotLimited | Self::ActiveLimitedHigh | Self::ActiveLimitedLow
        )
    }

    /// The highest state among several functions. A client reports the highest
    /// state of the functions it is using in its status message (§5.2.2).
    #[must_use]
    pub fn highest(states: &[Self]) -> Self {
        // Ordering follows Table 45's progression, with fault states ranked
        // above active ones so a fault is never masked by an active sibling.
        fn rank(s: AutomationState) -> u8 {
            match s {
                AutomationState::NotAvailable => 0,
                AutomationState::Unavailable => 1,
                AutomationState::NotReady => 2,
                AutomationState::ReadyToEnable => 3,
                AutomationState::Enabled => 4,
                AutomationState::Pending => 5,
                AutomationState::ActiveNotLimited => 6,
                AutomationState::ActiveLimitedHigh | AutomationState::ActiveLimitedLow => 7,
                AutomationState::Error => 8,
                AutomationState::Fault => 9,
            }
        }
        states
            .iter()
            .copied()
            .max_by_key(|s| rank(*s))
            .unwrap_or(Self::NotAvailable)
    }
}

/// State of one TIM function, with the reason it last left automation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionState {
    pub function: TimFunctionId,
    pub state: AutomationState,
    pub exit_reason: TimExitReason,
}

impl FunctionState {
    #[must_use]
    pub const fn new(function: TimFunctionId) -> Self {
        Self {
            function,
            state: AutomationState::NotAvailable,
            exit_reason: TimExitReason::AllClear,
        }
    }
}

/// Per-function automation state for a TIM couple.
#[derive(Debug, Default)]
pub struct AutomationStates {
    functions: Vec<FunctionState>,
}

impl AutomationStates {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// State of `function`, defaulting to not-available.
    #[must_use]
    pub fn state(&self, function: TimFunctionId) -> AutomationState {
        self.functions
            .iter()
            .find(|f| f.function == function)
            .map_or(AutomationState::NotAvailable, |f| f.state)
    }

    /// Why `function` last stopped accepting commands.
    #[must_use]
    pub fn exit_reason(&self, function: TimFunctionId) -> TimExitReason {
        self.functions
            .iter()
            .find(|f| f.function == function)
            .map_or(TimExitReason::AllClear, |f| f.exit_reason)
    }

    /// Set a function's state. Returns `true` if it changed.
    pub fn set(&mut self, function: TimFunctionId, state: AutomationState) -> bool {
        if let Some(entry) = self.functions.iter_mut().find(|f| f.function == function) {
            let changed = entry.state != state;
            entry.state = state;
            if state.is_active() {
                // Entering automation clears the previous exit reason so a
                // stale one cannot be read as current.
                entry.exit_reason = TimExitReason::AllClear;
            }
            changed
        } else {
            let mut entry = FunctionState::new(function);
            entry.state = state;
            self.functions.push(entry);
            true
        }
    }

    /// Record that a function left automation, with the reason.
    pub fn exit(&mut self, function: TimFunctionId, reason: TimExitReason) {
        let target = if reason == TimExitReason::AllClear {
            AutomationState::ReadyToEnable
        } else {
            AutomationState::NotReady
        };
        self.set(function, target);
        if let Some(entry) = self.functions.iter_mut().find(|f| f.function == function) {
            entry.exit_reason = reason;
        }
    }

    /// The highest state across every tracked function — what a client reports
    /// in its status message (§5.2.2).
    #[must_use]
    pub fn highest(&self) -> AutomationState {
        let states: Vec<_> = self.functions.iter().map(|f| f.state).collect();
        AutomationState::highest(&states)
    }

    /// `true` when any function is actively controlling the machine.
    #[must_use]
    pub fn any_active(&self) -> bool {
        self.functions.iter().any(|f| f.state.is_active())
    }
}

/// TIM version pair (B.2.2). A peer advertises the lowest version it can work
/// with and the version it implements; the couple runs the highest common one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimVersion {
    pub minimum: u8,
    pub implemented: u8,
}

impl TimVersion {
    #[must_use]
    pub const fn new(minimum: u8, implemented: u8) -> Self {
        Self {
            minimum,
            implemented,
        }
    }

    /// The highest version both peers can run, or `None` when their ranges do
    /// not overlap and the couple must not proceed (§5.3.4.1).
    #[must_use]
    pub const fn negotiate(self, peer: Self) -> Option<u8> {
        let highest_common = if self.implemented < peer.implemented {
            self.implemented
        } else {
            peer.implemented
        };
        if highest_common >= self.minimum && highest_common >= peer.minimum {
            Some(highest_common)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_is_tracked_per_function() {
        // The defect this replaces: one global authority state for the whole
        // couple, so steering and the hitch could not differ.
        let mut states = AutomationStates::new();
        states.set(
            TimFunctionId::ExternalGuidance,
            AutomationState::ActiveNotLimited,
        );
        states.set(TimFunctionId::RearHitch, AutomationState::Pending);

        assert_eq!(
            states.state(TimFunctionId::ExternalGuidance),
            AutomationState::ActiveNotLimited
        );
        assert_eq!(
            states.state(TimFunctionId::RearHitch),
            AutomationState::Pending
        );
        // A function never mentioned is not-available, not "ready".
        assert_eq!(
            states.state(TimFunctionId::FrontPto),
            AutomationState::NotAvailable
        );
    }

    #[test]
    fn a_fault_is_not_masked_by_an_active_sibling() {
        let mut states = AutomationStates::new();
        states.set(
            TimFunctionId::ExternalGuidance,
            AutomationState::ActiveNotLimited,
        );
        states.set(TimFunctionId::VehicleSpeed, AutomationState::Fault);

        assert_eq!(
            states.highest(),
            AutomationState::Fault,
            "the client reports the highest state, and a fault must win"
        );
    }

    #[test]
    fn exit_records_the_reason_and_clears_it_on_re_entry() {
        let mut states = AutomationStates::new();
        states.set(
            TimFunctionId::VehicleSpeed,
            AutomationState::ActiveNotLimited,
        );

        states.exit(TimFunctionId::VehicleSpeed, TimExitReason::OperatorOverride);
        assert_eq!(
            states.exit_reason(TimFunctionId::VehicleSpeed),
            TimExitReason::OperatorOverride
        );
        assert!(!states.any_active());

        // Re-entering automation clears the stale reason.
        states.set(
            TimFunctionId::VehicleSpeed,
            AutomationState::ActiveNotLimited,
        );
        assert_eq!(
            states.exit_reason(TimFunctionId::VehicleSpeed),
            TimExitReason::AllClear
        );
        assert!(states.any_active());
    }

    #[test]
    fn version_negotiation_picks_the_highest_common() {
        let a = TimVersion::new(1, 3);
        let b = TimVersion::new(2, 5);
        assert_eq!(a.negotiate(b), Some(3));
        assert_eq!(b.negotiate(a), Some(3));

        // Non-overlapping ranges must not produce a couple.
        let old = TimVersion::new(1, 1);
        let new = TimVersion::new(4, 6);
        assert_eq!(old.negotiate(new), None);
        assert_eq!(new.negotiate(old), None);

        // Exactly touching ranges are fine.
        assert_eq!(
            TimVersion::new(2, 2).negotiate(TimVersion::new(2, 9)),
            Some(2)
        );
    }

    #[test]
    fn automation_state_rejects_the_reserved_band() {
        for raw in 0x8u8..=0xC {
            assert_eq!(AutomationState::from_u8(raw), None, "{raw:#x}");
        }
        for raw in [0x0u8, 0x5, 0x7, 0xD, 0xF] {
            assert_eq!(AutomationState::from_u8(raw).unwrap().as_u8(), raw);
        }
    }
}
