//! `stack.vt_server()` — Virtual Terminal *server* handle.
//!
//! Wraps [`crate::isobus::vt::VTServer`] so that
//! `PGN_ECU_TO_VT` traffic from connecting clients is routed into
//! `VTServer::handle_ecu_message`, the FSM-driven `VT_STATUS`
//! broadcast is shipped automatically on [`Stack::tick`], and native
//! server events are bridged onto the unified queue as
//! [`VtServerEvent`].

use crate::isobus::vt::{ObjectID, VTServerState};
use crate::net::types::Address;

/// VT server-side events on the unified queue.
#[derive(Debug, Clone, PartialEq)]
pub enum VtServerEvent {
    StateChanged(VTServerState),
    ClientConnected(Address),
    ClientDisconnected(Address),
    /// `(old, new)` — active working-set address.
    ActiveWorkingSetChanged {
        from: Address,
        to: Address,
    },
    /// Soft-key activation reported by a client; `key_number` is the
    /// physical key index.
    SoftKey {
        id: ObjectID,
        key_number: u8,
    },
    /// Button activation.
    Button {
        id: ObjectID,
        key_number: u8,
    },
    /// Numeric value change pushed from a client.
    NumericValueChanged {
        id: ObjectID,
        value: u32,
    },
    /// String value change pushed from a client.
    StringValueChanged {
        id: ObjectID,
        value: String,
    },
    /// Input object selected: `(id, selected, edit_active)`.
    InputObjectSelected {
        id: ObjectID,
        selected: bool,
        edit_active: bool,
    },
}
