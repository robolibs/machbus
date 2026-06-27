//! Stack-owned J1939/ISOBUS Language Command workflow.
//!
//! The pure PGN `0xFE0F` codec lives in [`crate::j1939::language`]. This
//! module wires it into [`Stack`] so applications can broadcast local language
//! / unit preferences and observe peer preferences through the unified event
//! queue.

use crate::j1939::LanguageData;
use crate::net::types::Address;

/// Language Command event emitted on the unified stack queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageCommandEvent {
    pub source: Address,
    pub data: LanguageData,
}
