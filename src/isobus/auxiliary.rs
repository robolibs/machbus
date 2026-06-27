//! ISO 11783-11 Auxiliary Functions (AUX-O / AUX-N).
//!
//! Mirrors the C++ `machbus::isobus::auxiliary.hpp`. Two related
//! function styles share the same wire frame layout:
//!
//! ```text
//! byte 0    function_number
//! byte 1    type (0=on/off, 1=variable speed, 2=variable position)
//! byte 2    state (0=off, 1=on, 2=variable)
//! byte 3..4 setpoint (LE u16)
//! byte 5..7 reserved (0xFF)
//! ```
//!
//! - **AUX-O** (old-style): `PGN_AUX_INPUT_STATUS`, setpoint range `0..=10000` (0.0–100.0%).
//! - **AUX-N** (new-style, ISO 11783-6 Annex G): `PGN_AUX_INPUT_TYPE2`, setpoint range `0..=65535`.
//!
//! The C++ `AuxOInterface` / `AuxNInterface` (IsoNet-coupled) are
//! intentionally not ported. `AuxConfig` (auto-send + interval) is
//! similarly elided since it only configures those interfaces.

use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum AuxFunctionType {
    /// Boolean on/off.
    #[default]
    Type0 = 0,
    /// Variable-speed (analog).
    Type1 = 1,
    /// Variable-position (analog).
    Type2 = 2,
}

impl AuxFunctionType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Type1,
            2 => Self::Type2,
            _ => Self::Type0,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Type0),
            1 => Some(Self::Type1),
            2 => Some(Self::Type2),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum AuxFunctionState {
    #[default]
    Off = 0,
    On = 1,
    Variable = 2,
}

impl AuxFunctionState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::On,
            2 => Self::Variable,
            _ => Self::Off,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            1 => Some(Self::On),
            2 => Some(Self::Variable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// AUX-O function descriptor — old-style (`PGN_AUX_INPUT_STATUS`).
/// Setpoint range: `0..=10000` (0.0%–100.0%).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuxOFunction {
    pub function_number: u8,
    pub r#type: AuxFunctionType,
    pub state: AuxFunctionState,
    pub setpoint: u16,
}

/// AUX-N function descriptor — new-style (`PGN_AUX_INPUT_TYPE2`).
/// Setpoint range: `0..=65535`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuxNFunction {
    pub function_number: u8,
    pub r#type: AuxFunctionType,
    pub state: AuxFunctionState,
    pub setpoint: u16,
}

/// Encode an AUX-O / AUX-N function to the 8-byte wire format.
/// Both styles share the same layout, so a single helper covers both.
#[must_use]
pub fn encode(
    function_number: u8,
    ty: AuxFunctionType,
    state: AuxFunctionState,
    setpoint: u16,
) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    data[0] = function_number;
    data[1] = ty.as_u8();
    data[2] = state.as_u8();
    data[3] = (setpoint & 0xFF) as u8;
    data[4] = ((setpoint >> 8) & 0xFF) as u8;
    data
}

/// Derive the [`AuxFunctionState`] from a setpoint and type:
/// - `Type0` (boolean): `Off` if `setpoint == 0`, else `On`.
/// - `Type1` / `Type2`: always `Variable`.
#[must_use]
pub fn derive_state(ty: AuxFunctionType, setpoint: u16) -> AuxFunctionState {
    match ty {
        AuxFunctionType::Type0 => {
            if setpoint > 0 {
                AuxFunctionState::On
            } else {
                AuxFunctionState::Off
            }
        }
        _ => AuxFunctionState::Variable,
    }
}

fn decode_fields(data: &[u8]) -> Option<(u8, AuxFunctionType, AuxFunctionState, u16)> {
    if data.len() != 8 {
        return None;
    }
    if data[5..].iter().any(|&byte| byte != 0xFF) {
        return None;
    }
    Some((
        data[0],
        AuxFunctionType::try_from_u8(data[1])?,
        AuxFunctionState::try_from_u8(data[2])?,
        (data[3] as u16) | ((data[4] as u16) << 8),
    ))
}

impl AuxOFunction {
    /// Construct from `(number, type, setpoint)` deriving the state
    /// per the standard rules.
    #[must_use]
    pub fn with_setpoint(function_number: u8, ty: AuxFunctionType, setpoint: u16) -> Self {
        Self {
            function_number,
            r#type: ty,
            state: derive_state(ty, setpoint),
            setpoint,
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        encode(self.function_number, self.r#type, self.state, self.setpoint)
    }

    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_AUX_INPUT_STATUS) {
            return None;
        }
        let (function_number, r#type, state, setpoint) = decode_fields(&msg.data)?;
        Some(Self {
            function_number,
            r#type,
            state,
            setpoint,
        })
    }
}

impl AuxNFunction {
    #[must_use]
    pub fn with_setpoint(function_number: u8, ty: AuxFunctionType, setpoint: u16) -> Self {
        Self {
            function_number,
            r#type: ty,
            state: derive_state(ty, setpoint),
            setpoint,
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        encode(self.function_number, self.r#type, self.state, self.setpoint)
    }

    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_AUX_INPUT_TYPE2) {
            return None;
        }
        let (function_number, r#type, state, setpoint) = decode_fields(&msg.data)?;
        Some(Self {
            function_number,
            r#type,
            state,
            setpoint,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::{PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2};

    #[test]
    fn aux_o_round_trip() {
        let f = AuxOFunction {
            function_number: 7,
            r#type: AuxFunctionType::Type1,
            state: AuxFunctionState::Variable,
            setpoint: 5000,
        };
        let bytes = f.encode();
        let msg = Message::new(PGN_AUX_INPUT_STATUS, bytes.to_vec(), 0x10);
        let decoded = AuxOFunction::decode(&msg).unwrap();
        assert_eq!(decoded, f);
    }

    #[test]
    fn aux_n_round_trip() {
        let f = AuxNFunction {
            function_number: 12,
            r#type: AuxFunctionType::Type2,
            state: AuxFunctionState::Variable,
            setpoint: 0xCAFE,
        };
        let bytes = f.encode();
        let msg = Message::new(PGN_AUX_INPUT_TYPE2, bytes.to_vec(), 0x10);
        let decoded = AuxNFunction::decode(&msg).unwrap();
        assert_eq!(decoded, f);
    }

    #[test]
    fn derive_state_for_type0() {
        assert_eq!(
            derive_state(AuxFunctionType::Type0, 0),
            AuxFunctionState::Off
        );
        assert_eq!(
            derive_state(AuxFunctionType::Type0, 1),
            AuxFunctionState::On
        );
    }

    #[test]
    fn derive_state_for_variable_types() {
        assert_eq!(
            derive_state(AuxFunctionType::Type1, 0),
            AuxFunctionState::Variable
        );
        assert_eq!(
            derive_state(AuxFunctionType::Type2, 5000),
            AuxFunctionState::Variable
        );
    }

    #[test]
    fn with_setpoint_sets_state_consistently() {
        let f = AuxOFunction::with_setpoint(0, AuxFunctionType::Type0, 1);
        assert_eq!(f.state, AuxFunctionState::On);
        let f = AuxOFunction::with_setpoint(0, AuxFunctionType::Type1, 0);
        assert_eq!(f.state, AuxFunctionState::Variable);
    }

    #[test]
    fn decode_short_payload_returns_none() {
        let msg = Message::new(PGN_AUX_INPUT_STATUS, vec![0u8; 4], 0);
        assert!(AuxOFunction::decode(&msg).is_none());
        let msg = Message::new(PGN_AUX_INPUT_TYPE2, vec![0u8; 4], 0);
        assert!(AuxNFunction::decode(&msg).is_none());
    }

    #[test]
    fn decode_oversized_payload_returns_none() {
        let msg = Message::new(PGN_AUX_INPUT_STATUS, vec![0xFF; 9], 0);
        assert!(AuxOFunction::decode(&msg).is_none());
        let msg = Message::new(PGN_AUX_INPUT_TYPE2, vec![0xFF; 9], 0);
        assert!(AuxNFunction::decode(&msg).is_none());
    }

    #[test]
    fn decode_rejects_reserved_enum_values_and_tail_bytes() {
        let mut aux_o = AuxOFunction {
            function_number: 7,
            r#type: AuxFunctionType::Type1,
            state: AuxFunctionState::Variable,
            setpoint: 5000,
        }
        .encode();
        aux_o[1] = 3;
        assert!(
            AuxOFunction::decode(&Message::new(PGN_AUX_INPUT_STATUS, aux_o.to_vec(), 0)).is_none()
        );

        let mut aux_o = AuxOFunction::with_setpoint(7, AuxFunctionType::Type1, 5000).encode();
        aux_o[2] = 3;
        assert!(
            AuxOFunction::decode(&Message::new(PGN_AUX_INPUT_STATUS, aux_o.to_vec(), 0)).is_none()
        );

        let mut aux_n = AuxNFunction::with_setpoint(12, AuxFunctionType::Type2, 0xCAFE).encode();
        aux_n[5] = 0x00;
        assert!(
            AuxNFunction::decode(&Message::new(PGN_AUX_INPUT_TYPE2, aux_n.to_vec(), 0)).is_none()
        );
    }
}
