//! ISO 11783 Shortcut Button / ISB status (PGN `PGN_SHORTCUT_BUTTON`).
//!
//! Mirrors the C++ `ShortcutButtonInterface` wire layout in AgIsoStack++:
//! bytes 0..=5 are `0xFF`, byte 6 is the transition counter, and byte 7
//! carries the two-bit stop/permitted state. The full C++ interface is
//! intentionally not ported — users compose `decode` / `encode` directly with
//! `IsoNet::register_pgn_callback` and `IsoNet::send`.

use crate::net::message::Message;
use crate::net::pgn_defs::PGN_SHORTCUT_BUTTON;

/// Two-bit shortcut-button state in byte 7, bits 0–1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ShortcutButtonState {
    /// Stop all implement operations.
    #[default]
    StopImplementOperations = 0,
    /// Permit all implements to operate.
    PermitAllImplementsToOperate = 1,
    Error = 2,
    NotAvailable = 3,
}

impl ShortcutButtonState {
    #[inline]
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::StopImplementOperations,
            1 => Self::PermitAllImplementsToOperate,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[inline]
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::StopImplementOperations),
            1 => Some(Self::PermitAllImplementsToOperate),
            2 => Some(Self::Error),
            3 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    #[must_use]
    pub const fn is_stop_requested(self) -> bool {
        matches!(self, Self::StopImplementOperations)
    }
}

/// Decoded Shortcut Button message, including the transition counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortcutButtonMessage {
    pub state: ShortcutButtonState,
    pub transition_count: u8,
}

/// Encode to a single 8-byte CAN payload with transition counter zero.
#[must_use]
pub fn encode(state: ShortcutButtonState) -> [u8; 8] {
    encode_with_transition_count(state, 0)
}

/// Encode to a single 8-byte CAN payload.
#[must_use]
pub fn encode_with_transition_count(state: ShortcutButtonState, transition_count: u8) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    data[6] = transition_count;
    data[7] = 0xFC | (state.as_u8() & 0x03);
    data
}

/// Decode from a classic 8-byte message payload and preserve the transition
/// counter. Returns [`None`] for malformed fixed-size payloads.
#[must_use]
pub fn decode_message(msg: &Message) -> Option<ShortcutButtonMessage> {
    if !msg.has_usable_envelope_for_pgn(PGN_SHORTCUT_BUTTON) {
        return None;
    }
    if msg.data.len() != 8 {
        return None;
    }
    if msg.data[..6].iter().any(|&byte| byte != 0xFF) {
        return None;
    }
    let reserved = msg.data[7] & 0xFC;
    if reserved != 0x00 && reserved != 0xFC {
        return None;
    }
    Some(ShortcutButtonMessage {
        state: ShortcutButtonState::try_from_u8(msg.data[7] & 0x03)?,
        transition_count: msg.data[6],
    })
}

/// Decode from a classic 8-byte message payload. Returns only the state.
#[must_use]
pub fn decode(msg: &Message) -> Option<ShortcutButtonState> {
    decode_message(msg).map(|decoded| decoded.state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_SHORTCUT_BUTTON;

    #[test]
    fn state_round_trips() {
        for s in [
            ShortcutButtonState::StopImplementOperations,
            ShortcutButtonState::PermitAllImplementsToOperate,
            ShortcutButtonState::Error,
            ShortcutButtonState::NotAvailable,
        ] {
            assert_eq!(ShortcutButtonState::from_u8(s.as_u8()), s);
        }
    }

    #[test]
    fn encode_sets_isb_counter_and_state_fields() {
        let payload = encode(ShortcutButtonState::StopImplementOperations);
        assert_eq!(payload, [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0xFC]);

        let payload =
            encode_with_transition_count(ShortcutButtonState::PermitAllImplementsToOperate, 9);
        assert_eq!(payload, [0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x09, 0xFD]);
    }

    #[test]
    fn decode_returns_state() {
        let msg = Message::new(
            PGN_SHORTCUT_BUTTON,
            encode(ShortcutButtonState::StopImplementOperations).to_vec(),
            0x10,
        );
        assert_eq!(
            decode(&msg),
            Some(ShortcutButtonState::StopImplementOperations)
        );
    }

    #[test]
    fn decode_preserves_transition_counter_and_accepts_agisostack_zero_reserved_bits() {
        let msg = Message::new(
            PGN_SHORTCUT_BUTTON,
            vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x09, 0x01],
            0x10,
        );
        assert_eq!(
            decode_message(&msg),
            Some(ShortcutButtonMessage {
                state: ShortcutButtonState::PermitAllImplementsToOperate,
                transition_count: 9,
            })
        );
    }

    #[test]
    fn decode_empty_returns_none() {
        let msg = Message::new(PGN_SHORTCUT_BUTTON, vec![], 0x10);
        assert_eq!(decode(&msg), None);
    }

    #[test]
    fn decode_oversized_returns_none() {
        let msg = Message::new(PGN_SHORTCUT_BUTTON, vec![0xFF; 9], 0x10);
        assert_eq!(decode(&msg), None);
    }

    #[test]
    fn decode_rejects_reserved_bits_and_tails() {
        let mut payload = encode(ShortcutButtonState::StopImplementOperations);
        payload[7] = 0x04;
        assert_eq!(
            decode(&Message::new(PGN_SHORTCUT_BUTTON, payload.to_vec(), 0x10,)),
            None
        );

        let mut payload = encode(ShortcutButtonState::StopImplementOperations);
        payload[0] = 0x00;
        assert_eq!(
            decode(&Message::new(PGN_SHORTCUT_BUTTON, payload.to_vec(), 0x10,)),
            None
        );
    }
}
