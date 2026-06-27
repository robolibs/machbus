//! Stack-owned Shortcut Button / ISB workflow.
//!
//! The pure PGN `0xFD02` codec lives in [`crate::j1939::shortcut_button`].
//! This module wires it into [`Stack`] so applications can send
//! local Shortcut Button state and receive/cache peer state through the unified
//! event queue.

use crate::j1939::shortcut_button::ShortcutButtonMessage;
use crate::net::types::Address;

/// Shortcut Button event emitted on the unified stack queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShortcutButtonEvent {
    pub source: Address,
    pub message: ShortcutButtonMessage,
}
