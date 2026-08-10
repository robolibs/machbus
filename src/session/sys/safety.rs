//! Session-facing view of the crate-level [`crate::safety`] types.
//!
//! The state machine itself is at the crate root so the `embedded` profile can
//! use it; this module adds the part that depends on the session event surface.

pub use crate::safety::{SafeStopTrigger, StopLatch};

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
            Event::Imp(super::ImplementEvent::WheelSpeed(w)) if w.is_key_off() => {
                Some(Self::KeySwitchOff)
            }
            Event::Gnss(super::GnssEvent::PositionStale { .. }) => Some(Self::PositionStale),
            Event::Gnss(super::GnssEvent::FixDegraded { .. }) => Some(Self::FixDegraded),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every event that `from_event` is expected to classify, with the stop it
    /// must map to. Shared so the G8 reachability test derives its coverage by
    /// running the real mapping rather than restating it.
    fn event_mapping_cases() -> Vec<(crate::session::sys::Event, Option<SafeStopTrigger>)> {
        use crate::net::fault_confinement::FaultConfinementAction;
        use crate::session::sys::{BusEvent, ClaimEvent, Event, HeartbeatEvent};

        vec![
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
            (
                Event::Gnss(crate::session::sys::GnssEvent::PositionStale {
                    silent_for_ms: 1600,
                }),
                Some(SafeStopTrigger::PositionStale),
            ),
            (
                Event::Gnss(crate::session::sys::GnssEvent::FixDegraded {
                    fix_type: crate::nmea::GNSSFixType::DeadReckon,
                }),
                Some(SafeStopTrigger::FixDegraded),
            ),
            (
                Event::Imp(crate::session::sys::ImplementEvent::WheelSpeed(
                    crate::isobus::implement::WheelBasedSpeedDist {
                        key_switch_state: 0,
                        ..Default::default()
                    },
                )),
                Some(SafeStopTrigger::KeySwitchOff),
            ),
            // An unknown key state is not evidence of a shutdown; stopping on a
            // decode gap would be its own hazard.
            (
                Event::Imp(crate::session::sys::ImplementEvent::WheelSpeed(
                    crate::isobus::implement::WheelBasedSpeedDist::default(),
                )),
                None,
            ),
            // Recovery is informational: it must not itself be a stop.
            (
                Event::Gnss(crate::session::sys::GnssEvent::FixRestored {
                    fix_type: crate::nmea::GNSSFixType::RTKFixed,
                }),
                None,
            ),
        ]
    }

    /// The event-driven half of that mapping, checked against real events rather
    /// than a comment.
    #[test]
    fn session_events_map_to_the_stop_they_demand() {
        for (event, expected) in event_mapping_cases() {
            assert_eq!(
                SafeStopTrigger::from_event(&event),
                expected,
                "{event:?} mapped to the wrong stop"
            );
        }
    }

    /// G8 — every stop trigger must have a real producer.
    ///
    /// The old version of this test matched each variant to a hard-coded string
    /// and asserted the string was non-empty, so it passed for triggers nothing
    /// could ever trip; two dead variants lived behind it for three rounds. This
    /// one unions the `PRODUCES` lists the producing modules declare next to
    /// their own code, plus the events `from_event` maps, and requires the union
    /// to be exactly the enum. Deleting a producer now fails here.
    #[test]
    fn g8_every_trigger_is_reachable() {
        use crate::net::pgn_defs::PGN_GUIDANCE_SYSTEM_CMD;

        // Everything `from_event` can return, derived by running it rather than
        // by restating the match — a dropped arm shows up as a missing trigger.
        let from_events: Vec<SafeStopTrigger> = event_mapping_cases()
            .iter()
            .filter_map(|(event, _)| SafeStopTrigger::from_event(event))
            .collect();

        let mut reachable: Vec<SafeStopTrigger> = Vec::new();
        for trigger in crate::session::plugins::autodrive::PRODUCES
            .iter()
            .chain(crate::session::plugins::guidance::PRODUCES)
            .copied()
            .chain(from_events)
        {
            if !reachable.contains(&trigger) {
                reachable.push(trigger);
            }
        }

        let all = [
            SafeStopTrigger::GuidanceLinkTimeout,
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
            assert!(
                reachable.contains(&trigger),
                "{trigger:?} has no producer: either wire one up or delete the variant (G8)"
            );
        }
        for trigger in &reachable {
            assert!(
                all.contains(trigger),
                "{trigger:?} is produced but missing from the G8 coverage list"
            );
        }
    }
}
