//! `stack.tc()` — Task Controller client handle.
//!
//! Wraps [`crate::isobus::tc::TaskControllerClient`] (pump-style).
//! Inbound `PGN_TC_TO_ECU` frames are routed through
//! `TaskControllerClient::handle_tc_message`; the FSM-driven outbound
//! frames are auto-shipped on [`Stack::tick`].

use crate::isobus::tc::TCState;

/// TC client events on the unified event queue.
#[derive(Debug, Clone, PartialEq)]
pub enum TcEvent {
    /// FSM transitioned (`Disconnected` → `Connected`, etc.).
    StateChanged(TCState),
}
