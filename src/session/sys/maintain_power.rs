//! Stack-owned Maintain Power workflow.
//!
//! The pure ISO 11783-9/J1939 codec and lifecycle state machine live in
//! [`crate::j1939::maintain_power`]. This module wires that state machine into
//! [`Stack`] so TECU and implement control functions can exchange
//! `PGN_MAINTAIN_POWER` frames through the normal stack tick loop, with peer
//! status and lifecycle transitions surfaced on the unified event queue.

use crate::j1939::{MaintainPowerData, PowerState};
use crate::net::types::Address;

/// Event emitted for stack-owned Maintain Power workflow activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaintainPowerEvent {
    /// A valid peer Maintain Power payload was observed and cached.
    Status {
        source: Address,
        data: MaintainPowerData,
    },
    /// Local `MaintainPower` lifecycle state changed.
    StateChanged(PowerState),
    /// The local `MaintainPower` reached its terminal power-off decision.
    PowerOff,
}
