//! `stack.vt()` — Virtual Terminal client handle.
//!
//! Wraps [`crate::isobus::vt::VTClient`] (pump-style) plus a state
//! tracker. Inbound `PGN_VT_TO_ECU` frames are routed both into the
//! `VTClient` connect FSM and into the [`VTClientStateTracker`] so
//! `latest_*` accessors stay current. Native client events get
//! re-emitted as [`VtEvent`] entries on the unified queue.

use crate::isobus::vt::{ActivationCode, AuxCapabilities, LanguageCode, ObjectID, VTState};
use crate::net::types::Address;

/// VT client events on the unified event queue.
#[derive(Debug, Clone, PartialEq)]
pub enum VtEvent {
    /// Client transitioned (e.g. `Disconnected` → `Connected`).
    StateChanged(VTState),
    /// Soft-key activation arrived from the VT.
    SoftKey { id: ObjectID, code: ActivationCode },
    /// Button activation.
    Button { id: ObjectID, code: ActivationCode },
    /// VT pushed a numeric value change for `id`.
    NumericValueChanged { id: ObjectID, value: u32 },
    /// VT pushed a string value change for `id`.
    StringValueChanged { id: ObjectID, value: String },
    /// VT reported a pool error (code is the VT-specific error byte).
    PoolError(u8),
    /// `(old_lang, new_lang)`.
    LanguageChanged {
        from: LanguageCode,
        to: LanguageCode,
    },
    /// Whether *this* WS is the active one on the VT.
    ActiveWorkingSet(bool),
    /// VT v5 auxiliary-channel capabilities returned by Get Supported Objects.
    AuxCapabilities {
        source: Address,
        capabilities: AuxCapabilities,
    },
}
