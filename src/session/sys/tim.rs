//! `stack.tim()` — Tractor Implement Management authority + wire workflow.
//!
//! The pure TIM codecs and local authority/interlock guard live in
//! [`crate::isobus::tim`]. This module wires that guard to real ISOBUS/J1939
//! PGNs so tests and applications can prove a TIM-style lifecycle across the
//! virtual bus:
//!
//! - local authority requests/grants/revocations are explicit,
//! - guarded PTO/hitch command helpers refuse to emit frames unless authority
//!   is granted and interlocks are clear,
//! - PTO/hitch/TIM aux-valve status/command PGNs are decoded into the unified
//!   stack event stream.

use crate::isobus::implement::tractor_commands::{HitchCommandMsg, PtoCommandMsg};
use crate::isobus::tim::{
    AuxValveCommand, HitchState, PtoState, TimAuthorityState, TimCommand, TimValidationError,
};
use crate::net::types::Address;

use super::imp::{Hitch, Pto};

/// TIM events on the unified stack event queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimEvent {
    /// Local authority state changed due to request/grant/deny/revoke/interlock.
    AuthorityStateChanged(TimAuthorityState),
    /// A guarded command was refused before any CAN frame was emitted.
    CommandBlocked {
        command: TimCommand,
        error: TimValidationError,
    },
    /// PTO status observed on the bus.
    PtoStatus {
        pto: Pto,
        source: Address,
        state: PtoState,
    },
    /// Hitch status observed on the bus.
    HitchStatus {
        hitch: Hitch,
        source: Address,
        state: HitchState,
    },
    /// PTO command observed on the bus.
    PtoCommand {
        pto: Pto,
        source: Address,
        msg: PtoCommandMsg,
    },
    /// Hitch command observed on the bus.
    HitchCommand {
        hitch: Hitch,
        source: Address,
        msg: HitchCommandMsg,
    },
    /// TIM aux-valve command/status observed on the bus.
    AuxValveCommand {
        source: Address,
        command: AuxValveCommand,
    },
}
