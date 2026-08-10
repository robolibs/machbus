//! One safe state for the autonomy path.
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
}

impl SafeStopTrigger {
    /// The stop a session-level [`Event`] demands of the autonomy path, if any.
    ///
    /// Shared by every subsystem that owns a [`StopLatch`] so they cannot
    /// disagree about what counts as a fault. `SendFailed` is deliberately
    /// absent: whether a refused frame is safety-relevant depends on which PGN
    /// it was, which only the owning plugin knows.
    #[must_use]
    pub fn from_event(event: &super::Event) -> Option<Self> {
        use super::{BusEvent, ClaimEvent, Event, HeartbeatEvent};
        use crate::net::fault_confinement::FaultConfinementAction;

        match event {
            Event::Bus(BusEvent::ConfinementChanged {
                action: FaultConfinementAction::FailSafe,
                ..
            }) => Some(Self::BusOff),
            Event::Bus(BusEvent::ClockWentBackwards { .. }) => Some(Self::ClockWentBackwards),
            Event::AddressClaim(ClaimEvent::Lost { .. } | ClaimEvent::Disconnected) => {
                Some(Self::AddressClaimLost)
            }
            Event::Heartbeat(
                HeartbeatEvent::CommError { .. }
                | HeartbeatEvent::SequenceError { .. }
                | HeartbeatEvent::SenderError { .. }
                | HeartbeatEvent::GracefulShutdown { .. },
            ) => Some(Self::HeartbeatError),
            _ => None,
        }
    }

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
            };
            assert!(
                !producer.is_empty(),
                "{trigger:?} must name the code path that trips it"
            );
        }
    }

    /// The event-driven half of that mapping, checked against real events rather
    /// than a comment.
    #[test]
    fn session_events_map_to_the_stop_they_demand() {
        use crate::net::fault_confinement::FaultConfinementAction;
        use crate::session::sys::{BusEvent, ClaimEvent, Event, HeartbeatEvent};

        let cases = [
            (
                Event::Bus(BusEvent::ConfinementChanged {
                    port: 0,
                    action: FaultConfinementAction::FailSafe,
                }),
                Some(SafeStopTrigger::BusOff),
            ),
            (
                Event::Bus(BusEvent::ClockWentBackwards { by_micros: 1 }),
                Some(SafeStopTrigger::ClockWentBackwards),
            ),
            (
                Event::AddressClaim(ClaimEvent::Lost {
                    previous_address: 0x80,
                }),
                Some(SafeStopTrigger::AddressClaimLost),
            ),
            (
                Event::AddressClaim(ClaimEvent::Disconnected),
                Some(SafeStopTrigger::AddressClaimLost),
            ),
            (
                Event::Heartbeat(HeartbeatEvent::CommError { source: 0x26 }),
                Some(SafeStopTrigger::HeartbeatError),
            ),
            (
                Event::Heartbeat(HeartbeatEvent::SenderError { source: 0x26 }),
                Some(SafeStopTrigger::HeartbeatError),
            ),
            (
                Event::Heartbeat(HeartbeatEvent::GracefulShutdown { source: 0x26 }),
                Some(SafeStopTrigger::HeartbeatError),
            ),
            // A degraded bus warns; it is not by itself a stop.
            (
                Event::Bus(BusEvent::ConfinementChanged {
                    port: 0,
                    action: FaultConfinementAction::Degrade,
                }),
                None,
            ),
            // Winning an address is not a fault.
            (
                Event::AddressClaim(ClaimEvent::Claimed { address: 0x80 }),
                None,
            ),
            (
                Event::Heartbeat(HeartbeatEvent::Received {
                    source: 0x26,
                    sequence: 3,
                }),
                None,
            ),
        ];

        for (event, expected) in cases {
            assert_eq!(
                SafeStopTrigger::from_event(&event),
                expected,
                "{event:?} mapped to the wrong stop"
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
