//! Stack-owned ISO 11783 AUX-O / AUX-N input workflow.
//!
//! The pure codecs live in [`crate::isobus::auxiliary`]. This module wires the
//! AUX-O (`PGN_AUX_INPUT_STATUS`) and AUX-N (`PGN_AUX_INPUT_TYPE2`) status PGNs
//! into [`Stack`] so applications can broadcast local auxiliary-function state,
//! cache peer state, and observe updates through the unified event queue.

use crate::isobus::auxiliary::{AuxNFunction, AuxOFunction};
use crate::net::types::Address;

/// AUX-O/AUX-N status event emitted on the unified stack queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxiliaryEvent {
    /// Old-style AUX-O input status.
    AuxO {
        source: Address,
        function: AuxOFunction,
    },
    /// New-style AUX-N type-2 input status.
    AuxN {
        source: Address,
        function: AuxNFunction,
    },
}
