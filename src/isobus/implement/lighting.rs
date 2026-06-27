//! ISO 11783-7 §4.5 lighting messages.
//!
//! Mirrors the C++ `machbus::isobus::implement::lighting.hpp`. Two
//! PGNs share one wire format (16 lights × 2 bits, packed 4-per-byte
//! into bytes 0–3, bytes 4–7 reserved):
//!
//! - `PGN_LIGHTING_DATA` — broadcast from the tractor at 100 ms.
//! - `PGN_LIGHTING_COMMAND` — broadcast from the TECU to implements.
//!
//! The C++ `LightingInterface` (IsoNet-coupled) is intentionally
//! not ported — users dispatch via `IsoNet::register_pgn_callback`
//! and `IsoNet::send` directly.

use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_LIGHTING_COMMAND, PGN_LIGHTING_DATA};

/// 2-bit per-light state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum LightState {
    Off = 0,
    On = 1,
    Error = 2,
    #[default]
    NotAvailable = 3,
}

impl LightState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Off,
            1 => Self::On,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            1 => Some(Self::On),
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
}

/// Snapshot of every lighting channel an ISO 11783 tractor and
/// implement can broadcast or receive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LightingState {
    pub left_turn: LightState,
    pub right_turn: LightState,
    pub low_beam: LightState,
    pub high_beam: LightState,
    pub front_fog: LightState,
    pub rear_fog: LightState,
    pub beacon: LightState,
    /// Daytime running lights.
    pub running: LightState,
    pub rear_work: LightState,
    pub front_work: LightState,
    /// Side work light (typically implement-mounted).
    pub side_work: LightState,
    pub hazard: LightState,
    /// Back-up / reverse.
    pub backup: LightState,
    pub center_stop: LightState,
    pub left_stop: LightState,
    pub right_stop: LightState,
}

impl LightingState {
    /// Encode to the 8-byte wire format.
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.left_turn.as_u8()
            | (self.right_turn.as_u8() << 2)
            | (self.low_beam.as_u8() << 4)
            | (self.high_beam.as_u8() << 6);
        data[1] = self.front_fog.as_u8()
            | (self.rear_fog.as_u8() << 2)
            | (self.beacon.as_u8() << 4)
            | (self.running.as_u8() << 6);
        data[2] = self.rear_work.as_u8()
            | (self.front_work.as_u8() << 2)
            | (self.side_work.as_u8() << 4)
            | (self.hazard.as_u8() << 6);
        data[3] = self.backup.as_u8()
            | (self.center_stop.as_u8() << 2)
            | (self.left_stop.as_u8() << 4)
            | (self.right_stop.as_u8() << 6);
        data
    }

    /// Decode from a classic 8-byte payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || data[4..].iter().any(|&byte| byte != 0xFF) {
            return None;
        }
        Some(Self {
            left_turn: LightState::try_from_u8(data[0] & 0x03)?,
            right_turn: LightState::try_from_u8((data[0] >> 2) & 0x03)?,
            low_beam: LightState::try_from_u8((data[0] >> 4) & 0x03)?,
            high_beam: LightState::try_from_u8(data[0] >> 6)?,
            front_fog: LightState::try_from_u8(data[1] & 0x03)?,
            rear_fog: LightState::try_from_u8((data[1] >> 2) & 0x03)?,
            beacon: LightState::try_from_u8((data[1] >> 4) & 0x03)?,
            running: LightState::try_from_u8(data[1] >> 6)?,
            rear_work: LightState::try_from_u8(data[2] & 0x03)?,
            front_work: LightState::try_from_u8((data[2] >> 2) & 0x03)?,
            side_work: LightState::try_from_u8((data[2] >> 4) & 0x03)?,
            hazard: LightState::try_from_u8(data[2] >> 6)?,
            backup: LightState::try_from_u8(data[3] & 0x03)?,
            center_stop: LightState::try_from_u8((data[3] >> 2) & 0x03)?,
            left_stop: LightState::try_from_u8((data[3] >> 4) & 0x03)?,
            right_stop: LightState::try_from_u8(data[3] >> 6)?,
        })
    }

    #[inline]
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !matches!(msg.pgn, PGN_LIGHTING_DATA | PGN_LIGHTING_COMMAND)
            || !msg.has_usable_envelope_for_pgn(msg.pgn)
        {
            return None;
        }
        Self::decode(&msg.data)
    }

    /// Every channel `Off` — the safe lighting output (e.g. while the tractor
    /// key switch is off).
    #[must_use]
    pub const fn all_off() -> Self {
        Self {
            left_turn: LightState::Off,
            right_turn: LightState::Off,
            low_beam: LightState::Off,
            high_beam: LightState::Off,
            front_fog: LightState::Off,
            rear_fog: LightState::Off,
            beacon: LightState::Off,
            running: LightState::Off,
            rear_work: LightState::Off,
            front_work: LightState::Off,
            side_work: LightState::Off,
            hazard: LightState::Off,
            backup: LightState::Off,
            center_stop: LightState::Off,
            left_stop: LightState::Off,
            right_stop: LightState::Off,
        }
    }
}

/// TECU lighting controller (ISO 11783-9 §4.5): gates the commanded lighting
/// output on the key-switch state. With the key off, all lights are forced off;
/// with the key on, the command passes through. (Region rules and the §4.5
/// power-failure flashing fallback are not modeled here.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightingController {
    key_switch_on: bool,
}

impl Default for LightingController {
    fn default() -> Self {
        Self::new()
    }
}

impl LightingController {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            key_switch_on: true,
        }
    }

    pub const fn set_key_switch(&mut self, on: bool) {
        self.key_switch_on = on;
    }

    #[must_use]
    pub const fn key_switch_on(&self) -> bool {
        self.key_switch_on
    }

    /// The lighting output to emit for `command`, gated by the key switch.
    #[must_use]
    pub const fn effective(&self, command: LightingState) -> LightingState {
        if self.key_switch_on {
            command
        } else {
            LightingState::all_off()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lighting_controller_gates_on_key_switch() {
        let command = LightingState {
            low_beam: LightState::On,
            beacon: LightState::On,
            ..LightingState::default()
        };
        let mut ctl = LightingController::new();
        // Key on: command passes through.
        assert!(ctl.key_switch_on());
        assert_eq!(ctl.effective(command), command);
        // Key off: every channel is forced off.
        ctl.set_key_switch(false);
        let out = ctl.effective(command);
        assert_eq!(out, LightingState::all_off());
        assert_eq!(out.low_beam, LightState::Off);
        assert_eq!(out.beacon, LightState::Off);
    }

    #[test]
    fn light_state_round_trips() {
        for s in [
            LightState::Off,
            LightState::On,
            LightState::Error,
            LightState::NotAvailable,
        ] {
            assert_eq!(LightState::from_u8(s.as_u8()), s);
        }
    }

    #[test]
    fn round_trip_typical_lighting() {
        let s = LightingState {
            left_turn: LightState::On,
            right_turn: LightState::Off,
            low_beam: LightState::On,
            high_beam: LightState::Off,
            beacon: LightState::On,
            front_work: LightState::On,
            rear_work: LightState::On,
            backup: LightState::Off,
            ..Default::default()
        };
        let bytes = s.encode();
        let decoded = LightingState::decode(&bytes).unwrap();
        assert_eq!(decoded, s);
    }

    #[test]
    fn default_encodes_all_not_available() {
        let s = LightingState::default();
        let bytes = s.encode();
        // NotAvailable = 0b11; four channels per byte → 0xFF.
        assert_eq!(&bytes[0..4], &[0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(&bytes[4..], &[0xFFu8; 4]);
    }

    #[test]
    fn from_message_works_for_both_pgns() {
        let s = LightingState {
            beacon: LightState::On,
            hazard: LightState::On,
            ..Default::default()
        };
        for pgn in [PGN_LIGHTING_DATA, PGN_LIGHTING_COMMAND] {
            let msg = Message::new(pgn, s.encode().to_vec(), 0x10);
            assert_eq!(LightingState::from_message(&msg).unwrap(), s);
        }
    }

    #[test]
    fn decode_short_payload_returns_none() {
        assert!(LightingState::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn decode_overlong_payload_returns_none() {
        assert!(LightingState::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn decode_rejects_bad_reserved_tail() {
        let mut bytes = LightingState::default().encode();
        bytes[4] = 0x00;
        assert!(LightingState::decode(&bytes).is_none());
    }
}
