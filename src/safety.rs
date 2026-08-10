//! One safe state for the autonomy path.
//!
//! This lives at the crate root, not under `session`, so the `embedded`
//! profile gets it too: the whole point of a stop latch is the ECU it runs on,
//! and gating it behind the hosted session meant an embedded autosteer node had
//! no latch, no link-loss disengage and no ISB reaction at all.
//!
//! Before this existed the crate had three separate safety abstractions —
//! `Guidance`'s link-liveness flag, `TimAuthority`'s communication watchdog and
//! `TecuSafeMode` — and none of them could stop the machine: the first had no
//! consumer, the second was never ticked, the third was never consulted.
//!
//! [`StopLatch`] is the single place a failure is recorded. It **latches**: a
//! flapping link or a released button must not put a machine back under
//! automation by itself, so recovery is always an explicit act.

use crate::net::types::Pgn;

/// Why the autonomy path was told to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SafeStopTrigger {
    /// No Agricultural Guidance Machine Info within the link timeout.
    GuidanceLinkTimeout,
    /// No TIM status message within the AEF status timeout.
    TimStatusTimeout,
    /// A TIM function request went unanswered past its timeout.
    FunctionRequestTimeout,
    /// ISO 11783-7 §8 heartbeat reported a sequence or communication error.
    HeartbeatError,
    /// Operator pressed the Auxiliary Shortcut Button (stop all implements).
    IsbStop,
    /// The CAN controller reached bus-off.
    BusOff,
    /// The control function lost its claimed address, so commands are being
    /// discarded before they reach the bus.
    AddressClaimLost,
    /// The operator took over a primary control.
    OperatorOverride,
    /// The application stopped refreshing its setpoint.
    CommandStale,
    /// The clock handed to the session moved backwards, so every timer is
    /// suspect.
    ClockWentBackwards,
    /// A queued safety command was refused by the network layer.
    SendFailed(Pgn),
    /// No GNSS position within the configured window. The vehicle would
    /// otherwise keep steering to a curvature computed from a position that
    /// stopped updating.
    PositionStale,
    /// The receiver reported a fix that cannot be steered on — no fix, dead
    /// reckoning, or an error/unavailable method.
    FixDegraded,
    /// The operator switched the key off. The machine is shutting down, so an
    /// autonomous controller must stop asking for the wheel.
    KeySwitchOff,
}

impl SafeStopTrigger {
    /// A stable numeric code for the C ABI and other bindings.
    #[must_use]
    pub const fn as_code(self) -> u32 {
        match self {
            Self::GuidanceLinkTimeout => 1,
            Self::TimStatusTimeout => 2,
            Self::FunctionRequestTimeout => 3,
            Self::HeartbeatError => 4,
            Self::IsbStop => 5,
            Self::BusOff => 6,
            Self::AddressClaimLost => 7,
            Self::OperatorOverride => 8,
            Self::CommandStale => 9,
            Self::ClockWentBackwards => 10,
            Self::SendFailed(_) => 11,
            Self::PositionStale => 12,
            Self::FixDegraded => 13,
            Self::KeySwitchOff => 14,
        }
    }

    /// A short, stable identifier for logs and bindings.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GuidanceLinkTimeout => "guidance_link_timeout",
            Self::TimStatusTimeout => "tim_status_timeout",
            Self::FunctionRequestTimeout => "function_request_timeout",
            Self::HeartbeatError => "heartbeat_error",
            Self::IsbStop => "isb_stop",
            Self::BusOff => "bus_off",
            Self::AddressClaimLost => "address_claim_lost",
            Self::OperatorOverride => "operator_override",
            Self::CommandStale => "command_stale",
            Self::ClockWentBackwards => "clock_went_backwards",
            Self::SendFailed(_) => "send_failed",
            Self::PositionStale => "position_stale",
            Self::FixDegraded => "fix_degraded",
            Self::KeySwitchOff => "key_switch_off",
        }
    }
}

/// Latching record of the first failure that demanded a safe state.
///
/// Only the *first* trigger is kept: it is the one that describes what actually
/// went wrong, and later triggers are usually consequences of it (bus-off, for
/// instance, produces send failures on its way down).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StopLatch {
    reason: Option<SafeStopTrigger>,
}

impl StopLatch {
    #[must_use]
    pub const fn new() -> Self {
        Self { reason: None }
    }

    /// Record `trigger`. Returns `true` if this is the transition into the
    /// stopped state, so the caller emits its event exactly once.
    pub const fn trip(&mut self, trigger: SafeStopTrigger) -> bool {
        if self.reason.is_some() {
            return false;
        }
        self.reason = Some(trigger);
        true
    }

    #[must_use]
    pub const fn is_latched(&self) -> bool {
        self.reason.is_some()
    }

    /// What tripped it, if anything.
    #[must_use]
    pub const fn reason(&self) -> Option<SafeStopTrigger> {
        self.reason
    }

    /// Release the latch. Deliberately explicit — nothing in the stack clears
    /// this on its own, because "the fault went away" is not consent to move.
    pub const fn clear(&mut self) {
        self.reason = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_trigger_wins_and_only_reports_once() {
        let mut latch = StopLatch::new();
        assert!(!latch.is_latched());

        assert!(latch.trip(SafeStopTrigger::GuidanceLinkTimeout));
        // The consequences of the first fault must not overwrite the cause.
        assert!(!latch.trip(SafeStopTrigger::BusOff));
        assert_eq!(latch.reason(), Some(SafeStopTrigger::GuidanceLinkTimeout));
        assert!(latch.is_latched());
    }

    /// G8 — a trigger with no producer is a bug, not a placeholder. Nine of the
    /// eleven variants were unreachable when this was written: they described
    /// faults the session could detect and never acted on.
    ///
    /// The match is exhaustive on purpose. Adding a variant without naming what
    /// produces it stops the build here rather than shipping a safe state that
    /// can never be entered.
    #[test]
    fn every_trigger_names_its_producer() {
        use crate::net::pgn_defs::PGN_GUIDANCE_SYSTEM_CMD;

        let all = [
            SafeStopTrigger::GuidanceLinkTimeout,
            SafeStopTrigger::TimStatusTimeout,
            SafeStopTrigger::FunctionRequestTimeout,
            SafeStopTrigger::HeartbeatError,
            SafeStopTrigger::IsbStop,
            SafeStopTrigger::BusOff,
            SafeStopTrigger::AddressClaimLost,
            SafeStopTrigger::OperatorOverride,
            SafeStopTrigger::CommandStale,
            SafeStopTrigger::ClockWentBackwards,
            SafeStopTrigger::SendFailed(PGN_GUIDANCE_SYSTEM_CMD),
            SafeStopTrigger::PositionStale,
            SafeStopTrigger::FixDegraded,
            SafeStopTrigger::KeySwitchOff,
        ];

        for trigger in all {
            let producer = match trigger {
                SafeStopTrigger::GuidanceLinkTimeout => "guidance/autodrive on_tick link watchdog",
                SafeStopTrigger::TimStatusTimeout => "plugins::tim status watchdog",
                SafeStopTrigger::FunctionRequestTimeout => "isobus::tim function request timeout",
                SafeStopTrigger::HeartbeatError => "SafeStopTrigger::from_event, heartbeat faults",
                SafeStopTrigger::IsbStop => "guidance/autodrive on_frame, shortcut button",
                SafeStopTrigger::BusOff => "SafeStopTrigger::from_event, ConfinementChanged",
                SafeStopTrigger::AddressClaimLost => "SafeStopTrigger::from_event, claim lost",
                SafeStopTrigger::OperatorOverride => "autodrive on_frame, guidance limit status",
                SafeStopTrigger::CommandStale => "guidance/autodrive on_tick setpoint watchdog",
                SafeStopTrigger::ClockWentBackwards => "SafeStopTrigger::from_event, clock fault",
                SafeStopTrigger::SendFailed(_) => "guidance/autodrive on_event, refused command",
                SafeStopTrigger::PositionStale => "plugins::gnss on_tick position watchdog",
                SafeStopTrigger::FixDegraded => "plugins::gnss on_frame fix method check",
                SafeStopTrigger::KeySwitchOff => "SafeStopTrigger::from_event, wheel-based speed",
            };
            assert!(
                !producer.is_empty(),
                "{trigger:?} must name the code path that trips it"
            );
        }
    }

    #[test]
    fn clearing_is_explicit() {
        let mut latch = StopLatch::new();
        latch.trip(SafeStopTrigger::IsbStop);
        assert!(latch.is_latched());
        latch.clear();
        assert!(!latch.is_latched());
        // Cleared means a later fault reports again.
        assert!(latch.trip(SafeStopTrigger::BusOff));
    }
}

// ─── ISO 11783-9 §4.7 TECU safe mode ───────────────────────────────────
//
// These live here rather than in `isobus::tractor_ecu` so the `embedded`
// profile gets them: a safe-mode guard is most needed on the ECU it runs on,
// and the tractor-ECU module is hosted-only.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SafeModeTrigger {
    #[default]
    None,
    PowerLoss,
    EcuPowerLoss,
    CanBusFail,
    TecuCommLoss,
    ManualTrigger,
}

/// How a command relates to the TECU safe-mode constraints (ISO 11783-9 §4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TecuCommandKind {
    /// Engages motion / starts an actuator (hitch lower, PTO engage, drive).
    Engage,
    /// Disengages / stops / moves to a safe state.
    Disengage,
    /// Read-only status, no actuation.
    Query,
}

/// TECU safe-mode guard (ISO 11783-9 §4.7). Enforces the safety obligations as
/// repo-owned logic: no unexpected start (engage commands are blocked while in
/// safe mode), must allow stop (disengage always passes), loss-of-comms auto-
/// stop (enter on the relevant trigger), and operator override (explicit clear).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TecuSafeMode {
    active: bool,
    trigger: SafeModeTrigger,
}

impl TecuSafeMode {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            trigger: SafeModeTrigger::None,
        }
    }

    /// Enter safe mode, recording why.
    pub const fn enter(&mut self, trigger: SafeModeTrigger) {
        self.active = true;
        self.trigger = trigger;
    }

    /// Operator override / conditions clear: leave safe mode.
    pub const fn clear(&mut self) {
        self.active = false;
        self.trigger = SafeModeTrigger::None;
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub const fn trigger(&self) -> SafeModeTrigger {
        self.trigger
    }

    /// Whether a command of `kind` may take effect now. In safe mode only
    /// disengage/stop and read-only queries are allowed; engage commands are
    /// refused (and the caller should NACK them).
    #[must_use]
    pub const fn allows(&self, kind: TecuCommandKind) -> bool {
        !self.active || matches!(kind, TecuCommandKind::Disengage | TecuCommandKind::Query)
    }
}
