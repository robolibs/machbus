//! AEF 023 TIM status messages and the heartbeat counter (Annexes A.2, C.2, C.3).
//!
//! Neither TIM PGN existed in this crate: a repo-wide search for 8960 / 9216 /
//! 0x2300 / 0x2400 found nothing, so every version, support, authentication,
//! assignment and status message was absent. `TimAuthority` guarded commands
//! locally and never crossed the wire.
//!
//! This module adds the two status messages and the counter they both carry —
//! the layer that lets a TIM couple notice each other going away.

use super::automation::AutomationState;
use crate::net::types::Pgn;

/// TIM server → TIM client, PDU1, default priority 6 (A.2.2).
pub const PGN_TIM_SERVER_TO_CLIENT: Pgn = 0x2300;
/// TIM client → TIM server, PDU1, default priority 6 (A.2.2).
pub const PGN_TIM_CLIENT_TO_SERVER: Pgn = 0x2400;

/// Status messages are transmitted at priority 4 (C.2, C.3) — above the
/// default for their PGN, because losing them is a safety event.
pub const TIM_STATUS_PRIORITY: u8 = 4;

/// Status repetition rate (C.2, C.3).
pub const TIM_STATUS_INTERVAL_MS: u32 = 100;
/// Three missed status messages is a communication error.
///
/// No in-crate watchdog reads this: the crate implements no receive-side TIM
/// client, so nothing here observes a peer's status stream. The command path is
/// guarded instead by `TimAuthority`'s `DEFAULT_COMMS_TIMEOUT_MS`, which is the
/// same 300 ms and *is* ticked.
pub const TIM_STATUS_TIMEOUT_MS: u32 = 300;

/// Message code for `TIM_ServerStatus_Msg` (A.2.3).
pub const MSG_CODE_SERVER_STATUS: u8 = 0xFA;
/// Message code for `TIM_ClientStatus_Msg` (A.2.3).
pub const MSG_CODE_CLIENT_STATUS: u8 = 0xF9;

/// The 1-byte heartbeat counter carried by both status messages (§5.3.3.1).
///
/// It is not a plain counter: four of its values are conditions rather than
/// positions in a sequence, and the rollover is at 0xFA rather than 0xFF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeartbeatCounter {
    /// A normal position in the rolling sequence, 0x00..=0xFA.
    Count(u8),
    /// Counter reset (0xFB).
    Reset,
    /// Sender reports an error condition (0xFE).
    SenderError,
    /// Sender is shutting down in an orderly way (0xFF).
    GracefulShutdown,
}

/// Highest normal counter value; the sequence rolls over here, not at 0xFF.
pub const HEARTBEAT_ROLLOVER: u8 = 0xFA;

impl HeartbeatCounter {
    /// Decode a raw counter byte. `0xFC`/`0xFD` are invalid in TIM V1 and are
    /// the only values that fail.
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0x00..=HEARTBEAT_ROLLOVER => Some(Self::Count(raw)),
            0xFB => Some(Self::Reset),
            0xFE => Some(Self::SenderError),
            0xFF => Some(Self::GracefulShutdown),
            // 0xFC..=0xFD: "Invalid, shall not be used in TIM V1".
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Count(n) => n,
            Self::Reset => 0xFB,
            Self::SenderError => 0xFE,
            Self::GracefulShutdown => 0xFF,
        }
    }

    /// The next counter in the normal sequence, rolling over at 0xFA.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Count(n) if n < HEARTBEAT_ROLLOVER => Self::Count(n + 1),
            // Anything else restarts the sequence.
            _ => Self::Count(0),
        }
    }

    /// Judge **sequence continuity only** (§5.3.3.1.2).
    ///
    /// A counter that repeats, goes backwards, or jumps by more than 3 is a
    /// severe communication error rather than a missed frame.
    ///
    /// The special values are conditions rather than sequence positions, so
    /// they are not sequence errors and this returns `true` for them. That is
    /// not the same as "healthy": `0xFE` and `0xFF` carry their own obligations
    /// and are classified by [`is_severe_comms_error`](Self::is_severe_comms_error)
    /// and [`is_graceful_shutdown_edge`](Self::is_graceful_shutdown_edge). Using
    /// `follows` alone as the health check treated a peer explicitly declaring
    /// its own fault as a peer in good order.
    #[must_use]
    pub fn follows(self, previous: Self) -> bool {
        let (Self::Count(now), Self::Count(before)) = (self, previous) else {
            return true;
        };
        let span = u16::from(HEARTBEAT_ROLLOVER) + 1;
        let delta = (u16::from(now) + span - u16::from(before)) % span;
        (1..=3).contains(&delta)
    }

    /// §5.3.3.1.2: "If the Heartbeat counter value in the currently received
    /// message is 'Invalid' or 'Error condition on sender', then the recipient
    /// shall treat it as a severe communication error and handle it as defined
    /// in 5.3.3."
    #[must_use]
    pub const fn is_severe_comms_error(self) -> bool {
        matches!(self, Self::SenderError)
    }

    /// §5.3.3.1.2: a transition *into* "Graceful shutdown" — the peer is
    /// stopping deliberately, so communication stops with it rather than
    /// escalating as a fault. A repeat of the same value is not a new edge.
    #[must_use]
    pub const fn is_graceful_shutdown_edge(self, previous: Self) -> bool {
        matches!(self, Self::GracefulShutdown) && !matches!(previous, Self::GracefulShutdown)
    }
}

/// TIM server master indication, byte 3 bits 8-5 of `TIM_ServerStatus_Msg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TimServerMaster {
    /// Acting as TIM server master, but automation is not allowed.
    ActingAutomationNotAllowed = 0x0,
    /// Acting as TIM server master with automation allowed.
    ActingAutomationAllowed = 0x1,
    Error = 0xE,
    #[default]
    NotAvailable = 0xF,
}

impl TimServerMaster {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x0F {
            0x0 => Some(Self::ActingAutomationNotAllowed),
            0x1 => Some(Self::ActingAutomationAllowed),
            0xE => Some(Self::Error),
            0xF => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// A client must ignore server messages from a source not indicating that
    /// it is acting as TIM server master (§5.3.1.2).
    #[must_use]
    pub const fn is_acting_master(self) -> bool {
        matches!(
            self,
            Self::ActingAutomationNotAllowed | Self::ActingAutomationAllowed
        )
    }
}

/// TIM system state, byte 3 bits 4-1 of `TIM_ServerStatus_Msg`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TimSystemState {
    NoAutomationActive = 0x1,
    AutomationActive = 0x5,
    Error = 0xE,
    #[default]
    NotAvailable = 0xF,
}

impl TimSystemState {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x0F {
            0x1 => Some(Self::NoAutomationActive),
            0x5 => Some(Self::AutomationActive),
            0xE => Some(Self::Error),
            0xF => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Annex C.2 byte 4 bits 4-1 — whether the system's preconditions for TIM
/// operation are met.
///
/// "All TIM participants shall use the TIM system operation state information
/// of the TIM server master to determine if for TIM automation the TIM system
/// is in the standstill state." The whole byte was absent from the codec, so a
/// client had no way to learn it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TimSystemOperationState {
    /// Conditions are not fulfilled for any TIM system operation state.
    NotFulfilled = 0x0,
    /// Speed at or above standstill and the operator is present.
    NormalOperation = 0x1,
    /// Speed below standstill and the operator is present.
    StandstillOperation = 0x2,
    /// Speed below standstill and the operator voluntarily activated the mode.
    StationaryOperation = 0x3,
    Error = 0xE,
    #[default]
    NotAvailable = 0xF,
}

impl TimSystemOperationState {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x0F {
            0x0 => Some(Self::NotFulfilled),
            0x1 => Some(Self::NormalOperation),
            0x2 => Some(Self::StandstillOperation),
            0x3 => Some(Self::StationaryOperation),
            0xE => Some(Self::Error),
            0xF => Some(Self::NotAvailable),
            // 0x4..=0xD are reserved.
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// `true` only for a state a TIM client may automate in.
    #[must_use]
    pub const fn permits_automation(self) -> bool {
        matches!(
            self,
            Self::NormalOperation | Self::StandstillOperation | Self::StationaryOperation
        )
    }
}

/// `TIM_ServerStatus_Msg` — Annex C.2. Broadcast every 100 ms at priority 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimServerStatus {
    pub counter: HeartbeatCounter,
    pub master: TimServerMaster,
    pub system_state: TimSystemState,
    /// Byte 4 bits 8-5, "related to values of D.2.2".
    pub server_state: AutomationState,
    /// Byte 4 bits 4-1.
    pub operation_state: TimSystemOperationState,
}

impl Default for TimServerStatus {
    fn default() -> Self {
        Self {
            counter: HeartbeatCounter::Reset,
            master: TimServerMaster::NotAvailable,
            system_state: TimSystemState::NotAvailable,
            server_state: AutomationState::NotAvailable,
            operation_state: TimSystemOperationState::NotAvailable,
        }
    }
}

impl TimServerStatus {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = MSG_CODE_SERVER_STATUS;
        data[1] = self.counter.as_u8();
        data[2] = ((self.master.as_u8() & 0x0F) << 4) | (self.system_state.as_u8() & 0x0F);
        data[3] = ((self.server_state.as_u8() & 0x0F) << 4) | (self.operation_state.as_u8() & 0x0F);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 4 || data[0] != MSG_CODE_SERVER_STATUS {
            return None;
        }
        Some(Self {
            counter: HeartbeatCounter::from_u8(data[1])?,
            master: TimServerMaster::from_u8(data[2] >> 4)?,
            system_state: TimSystemState::from_u8(data[2] & 0x0F)?,
            server_state: AutomationState::from_u8(data[3] >> 4)?,
            operation_state: TimSystemOperationState::from_u8(data[3] & 0x0F)?,
        })
    }
}

/// TIM client state, byte 3 bits 8-5 of `TIM_ClientStatus_Msg` (D.2.2 values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TimClientState {
    AutomationUnavailable = 0x0,
    AutomationNotReady = 0x1,
    AutomationReadyToEnable = 0x2,
    AutomationEnabled = 0x3,
    AutomationActive = 0x5,
    AutomationFault = 0xD,
    Error = 0xE,
    #[default]
    NotAvailable = 0xF,
}

impl TimClientState {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x0F {
            0x0 => Some(Self::AutomationUnavailable),
            0x1 => Some(Self::AutomationNotReady),
            0x2 => Some(Self::AutomationReadyToEnable),
            0x3 => Some(Self::AutomationEnabled),
            0x5 => Some(Self::AutomationActive),
            0xD => Some(Self::AutomationFault),
            0xE => Some(Self::Error),
            0xF => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// `TIM_ClientStatus_Msg` — Annex C.3. Destination-specific to the server,
/// every 100 ms at priority 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimClientStatus {
    pub counter: HeartbeatCounter,
    pub state: TimClientState,
}

impl Default for TimClientStatus {
    fn default() -> Self {
        Self {
            counter: HeartbeatCounter::Reset,
            state: TimClientState::NotAvailable,
        }
    }
}

impl TimClientStatus {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = MSG_CODE_CLIENT_STATUS;
        data[1] = self.counter.as_u8();
        // Bits 4-1 are reserved and travel as ones.
        data[2] = ((self.state.as_u8() & 0x0F) << 4) | 0x0F;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 3 || data[0] != MSG_CODE_CLIENT_STATUS {
            return None;
        }
        Some(Self {
            counter: HeartbeatCounter::from_u8(data[1])?,
            state: TimClientState::from_u8(data[2] >> 4)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D6 — AEF §5.3.3.1.2: "If the Heartbeat counter value in the currently
    /// received message is 'Invalid' or 'Error condition on sender', then the
    /// recipient shall treat it as a severe communication error"; and a
    /// transition into "Graceful shutdown" stops communication with that peer.
    ///
    /// `follows` was the module's only stated implementation of that clause and
    /// returns `true` for both, because they are conditions rather than
    /// sequence positions — so a TIM server in a self-declared fault state read
    /// as healthy and kept its client commanding it.
    #[test]
    fn special_counters_are_conditions_not_healthy_sequence_positions() {
        let ok = HeartbeatCounter::Count(4);

        // Still true: they are not *sequence* errors.
        assert!(HeartbeatCounter::SenderError.follows(ok));
        assert!(HeartbeatCounter::GracefulShutdown.follows(ok));

        // But they are not healthy either.
        assert!(HeartbeatCounter::SenderError.is_severe_comms_error());
        assert!(!HeartbeatCounter::GracefulShutdown.is_severe_comms_error());
        assert!(!ok.is_severe_comms_error());
        assert!(!HeartbeatCounter::Reset.is_severe_comms_error());

        // Shutdown is an edge: the first one acts, a repeat does not.
        assert!(HeartbeatCounter::GracefulShutdown.is_graceful_shutdown_edge(ok));
        assert!(
            !HeartbeatCounter::GracefulShutdown
                .is_graceful_shutdown_edge(HeartbeatCounter::GracefulShutdown),
            "a repeated shutdown value is not a new transition"
        );
        assert!(!ok.is_graceful_shutdown_edge(HeartbeatCounter::GracefulShutdown));
    }

    #[test]
    fn heartbeat_counter_rolls_over_at_fa_not_ff() {
        assert_eq!(
            HeartbeatCounter::Count(HEARTBEAT_ROLLOVER).next(),
            HeartbeatCounter::Count(0)
        );
        assert_eq!(
            HeartbeatCounter::Count(5).next(),
            HeartbeatCounter::Count(6)
        );
        // Special values restart the sequence rather than incrementing.
        assert_eq!(HeartbeatCounter::Reset.next(), HeartbeatCounter::Count(0));
    }

    #[test]
    fn only_the_v1_invalid_band_fails_to_decode() {
        for raw in [0x00u8, 0x7F, HEARTBEAT_ROLLOVER, 0xFB, 0xFE, 0xFF] {
            let counter = HeartbeatCounter::from_u8(raw).expect("assigned value");
            assert_eq!(counter.as_u8(), raw);
        }
        // "Invalid, shall not be used in TIM V1".
        assert_eq!(HeartbeatCounter::from_u8(0xFC), None);
        assert_eq!(HeartbeatCounter::from_u8(0xFD), None);
    }

    #[test]
    fn counter_validation_catches_repeats_and_jumps() {
        let at = |n| HeartbeatCounter::Count(n);
        assert!(at(6).follows(at(5)), "increment of 1 is normal");
        assert!(at(8).follows(at(5)), "up to 3 tolerates lost frames");
        assert!(
            !at(9).follows(at(5)),
            "a jump of 4 is a communication error"
        );
        assert!(!at(5).follows(at(5)), "a repeat is a communication error");
        assert!(!at(4).follows(at(5)), "going backwards is an error");

        // Rollover is at 0xFA, so 0x00 legitimately follows 0xFA.
        assert!(at(0).follows(at(HEARTBEAT_ROLLOVER)));
        assert!(at(2).follows(at(HEARTBEAT_ROLLOVER)));
        assert!(!at(4).follows(at(HEARTBEAT_ROLLOVER)));
    }

    #[test]
    fn server_status_round_trips_with_the_annex_c2_layout() {
        let status = TimServerStatus {
            counter: HeartbeatCounter::Count(0x42),
            master: TimServerMaster::ActingAutomationAllowed,
            system_state: TimSystemState::AutomationActive,
            server_state: AutomationState::ActiveNotLimited,
            operation_state: TimSystemOperationState::StandstillOperation,
        };
        let bytes = status.encode();
        assert_eq!(
            bytes[0], MSG_CODE_SERVER_STATUS,
            "byte 1 is the message code"
        );
        assert_eq!(bytes[1], 0x42);
        assert_eq!(
            bytes[2], 0x15,
            "master in bits 8-5, system state in bits 4-1"
        );
        // H66 — byte 4 was absent entirely, so a client could not learn the
        // server state or whether the system's operating preconditions were
        // met. "All TIM participants shall use the TIM system operation state
        // information of the TIM server master to determine if for TIM
        // automation the TIM system is in the standstill state."
        assert_eq!(
            bytes[3], 0x52,
            "server state in bits 8-5, system operation state in bits 4-1"
        );
        assert_eq!(TimServerStatus::decode(&bytes), Some(status));

        // A three-byte frame is no longer a complete server status.
        assert_eq!(TimServerStatus::decode(&bytes[..3]), None);

        // Only the three fulfilled states permit automation.
        assert!(!TimSystemOperationState::NotFulfilled.permits_automation());
        assert!(!TimSystemOperationState::NotAvailable.permits_automation());
        assert!(!TimSystemOperationState::Error.permits_automation());
        assert!(TimSystemOperationState::NormalOperation.permits_automation());
        assert!(TimSystemOperationState::StationaryOperation.permits_automation());

        // 0x4..=0xD are reserved in the operation-state nibble.
        for reserved in 0x4u8..=0xD {
            assert_eq!(
                TimSystemOperationState::from_u8(reserved),
                None,
                "{reserved:#x}"
            );
        }

        // A different message code on the same PGN is a different message.
        let mut wrong_code = bytes;
        wrong_code[0] = MSG_CODE_CLIENT_STATUS;
        assert_eq!(TimServerStatus::decode(&wrong_code), None);
    }

    #[test]
    fn client_status_round_trips_and_reports_its_state() {
        let status = TimClientStatus {
            counter: HeartbeatCounter::Count(1),
            state: TimClientState::AutomationActive,
        };
        let bytes = status.encode();
        assert_eq!(bytes[0], MSG_CODE_CLIENT_STATUS);
        assert_eq!(bytes[2] >> 4, TimClientState::AutomationActive.as_u8());
        assert_eq!(bytes[2] & 0x0F, 0x0F, "reserved bits travel as ones");
        assert_eq!(TimClientStatus::decode(&bytes), Some(status));
    }

    #[test]
    fn a_server_not_acting_as_master_must_be_ignorable() {
        // Section 5.3.1.2: a client ignores server messages whose source is not
        // indicating "acting as TIM server master".
        assert!(TimServerMaster::ActingAutomationAllowed.is_acting_master());
        assert!(TimServerMaster::ActingAutomationNotAllowed.is_acting_master());
        assert!(!TimServerMaster::NotAvailable.is_acting_master());
        assert!(!TimServerMaster::Error.is_acting_master());
    }
}
