//! ISO 11783-7 auxiliary-valve flow status (Class 2 TECU).
//!
//! Mirrors the C++ `machbus::isobus::implement::aux_valve_status.hpp`.
//! Reports estimated and measured hydraulic valve flow for up to 16
//! valves at 100 ms per valve. PGN bases:
//! `PGN_AUX_VALVE_ESTIMATED_FLOW_BASE + index` (0..16) and the
//! corresponding `_MEASURED_` base.
//!
//! The C++ `AuxValveStatusInterface` (IsoNet-coupled) is not ported.

use crate::net::pgn_defs::{PGN_AUX_VALVE_ESTIMATED_FLOW_BASE, PGN_AUX_VALVE_MEASURED_FLOW_BASE};
use crate::net::types::Pgn;

pub const MAX_AUX_VALVES: u8 = 16;

/// Convert a flow percentage (`0..=100`) to the `0.4 %`-per-bit raw byte
/// (`0..=250`), clamping out-of-range and mapping non-finite input to `0`.
#[must_use]
fn flow_percent_to_raw(percent: f64) -> u8 {
    if !percent.is_finite() {
        return 0;
    }
    let scaled = percent.clamp(0.0, 100.0) / 0.4;
    (scaled + 0.5) as u8
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ValveState {
    #[default]
    Blocked = 0,
    Extending = 1,
    Retracting = 2,
    FloatPosition = 3,
}

impl ValveState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Blocked,
            1 => Self::Extending,
            2 => Self::Retracting,
            _ => Self::FloatPosition,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Blocked),
            1 => Some(Self::Extending),
            2 => Some(Self::Retracting),
            3 => Some(Self::FloatPosition),
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
pub enum ValveLimitStatus {
    NotLimited = 0,
    OperatorLimited = 1,
    SystemLimited = 2,
    Reserved3 = 3,
    Reserved4 = 4,
    Reserved5 = 5,
    Error = 6,
    #[default]
    NotAvailable = 7,
}

impl ValveLimitStatus {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            0 => Self::NotLimited,
            1 => Self::OperatorLimited,
            2 => Self::SystemLimited,
            3 => Self::Reserved3,
            4 => Self::Reserved4,
            5 => Self::Reserved5,
            6 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotLimited),
            1 => Some(Self::OperatorLimited),
            2 => Some(Self::SystemLimited),
            6 => Some(Self::Error),
            7 => Some(Self::NotAvailable),
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
pub enum ValveFailSafe {
    #[default]
    Block = 0,
    Float = 1,
    Extend = 2,
    Retract = 3,
}

impl ValveFailSafe {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Block,
            1 => Self::Float,
            2 => Self::Extend,
            _ => Self::Retract,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Block),
            1 => Some(Self::Float),
            2 => Some(Self::Extend),
            3 => Some(Self::Retract),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Auxiliary valve flow message — same wire format used for both
/// "estimated" and "measured" PGN bases.
///
/// - byte 0: extend flow (`0.4 %` per bit, `0..=250` = 0–100 %).
/// - byte 1: retract flow (same scaling).
/// - byte 2: bits 0–1 state, bits 2–4 limit, bits 5–6 fail-safe,
///   bit 7 reserved (= `1` on encode for parity with C++).
/// - bytes 3–7: reserved (`0xFF`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxValveFlowMsg {
    pub valve_index: u8,
    pub extend_flow_percent: u8,
    pub retract_flow_percent: u8,
    pub state: ValveState,
    pub limit_status: ValveLimitStatus,
    pub fail_safe: ValveFailSafe,
}

impl Default for AuxValveFlowMsg {
    fn default() -> Self {
        Self {
            valve_index: 0,
            extend_flow_percent: 0xFF,
            retract_flow_percent: 0xFF,
            state: ValveState::Blocked,
            limit_status: ValveLimitStatus::NotAvailable,
            fail_safe: ValveFailSafe::Block,
        }
    }
}

impl AuxValveFlowMsg {
    /// Extend flow as percent (`0.0..=100.0`). Returns `0.0` for the
    /// `0xFF` "not available" sentinel.
    #[must_use]
    pub fn extend_flow(&self) -> f64 {
        if self.extend_flow_percent == 0xFF {
            0.0
        } else {
            self.extend_flow_percent as f64 * 0.4
        }
    }

    #[must_use]
    pub fn retract_flow(&self) -> f64 {
        if self.retract_flow_percent == 0xFF {
            0.0
        } else {
            self.retract_flow_percent as f64 * 0.4
        }
    }

    /// Set the extend flow from a percentage (`0.0..=100.0`), converting to the
    /// `0.4 %`-per-bit raw byte and clamping out-of-range/non-finite input. The
    /// inverse of [`extend_flow`](Self::extend_flow).
    #[must_use]
    pub fn with_extend_flow_percent(mut self, percent: f64) -> Self {
        self.extend_flow_percent = flow_percent_to_raw(percent);
        self
    }

    /// Set the retract flow from a percentage (`0.0..=100.0`). See
    /// [`with_extend_flow_percent`](Self::with_extend_flow_percent).
    #[must_use]
    pub fn with_retract_flow_percent(mut self, percent: f64) -> Self {
        self.retract_flow_percent = flow_percent_to_raw(percent);
        self
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.extend_flow_percent;
        data[1] = self.retract_flow_percent;
        // bit 7 reserved (= 1) preserves parity with C++ encoding.
        data[2] = (self.state.as_u8() & 0x03)
            | ((self.limit_status.as_u8() & 0x07) << 2)
            | ((self.fail_safe.as_u8() & 0x03) << 5)
            | (1 << 7);
        data
    }

    /// `valve_index` is supplied by the caller because it's encoded
    /// in the PGN (`base + index`), not in the payload.
    #[must_use]
    pub fn decode(data: &[u8], valve_index: u8) -> Option<Self> {
        if data.len() != 8
            || valve_index >= MAX_AUX_VALVES
            || data[2] & 0x80 != 0x80
            || data[3..].iter().any(|&byte| byte != 0xFF)
        {
            return None;
        }
        let limit_status = ValveLimitStatus::try_from_u8((data[2] >> 2) & 0x07)?;
        if matches!(data[0], 251..=254) || matches!(data[1], 251..=254) {
            return None;
        }
        Some(Self {
            valve_index,
            extend_flow_percent: data[0],
            retract_flow_percent: data[1],
            state: ValveState::try_from_u8(data[2] & 0x03)?,
            limit_status,
            fail_safe: ValveFailSafe::try_from_u8((data[2] >> 5) & 0x03)?,
        })
    }
}

/// PGN for the *estimated* flow message of `valve_index`
/// (`0..=MAX_AUX_VALVES − 1`). Returns `None` if out of range.
#[inline]
#[must_use]
pub fn estimated_flow_pgn(valve_index: u8) -> Option<Pgn> {
    if valve_index >= MAX_AUX_VALVES {
        None
    } else {
        Some(PGN_AUX_VALVE_ESTIMATED_FLOW_BASE + valve_index as Pgn)
    }
}

/// PGN for the *measured* flow message of `valve_index`.
#[inline]
#[must_use]
pub fn measured_flow_pgn(valve_index: u8) -> Option<Pgn> {
    if valve_index >= MAX_AUX_VALVES {
        None
    } else {
        Some(PGN_AUX_VALVE_MEASURED_FLOW_BASE + valve_index as Pgn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_typical_flow() {
        let m = AuxValveFlowMsg {
            valve_index: 3,
            extend_flow_percent: 200,
            retract_flow_percent: 50,
            state: ValveState::Extending,
            limit_status: ValveLimitStatus::OperatorLimited,
            fail_safe: ValveFailSafe::Float,
        };
        let bytes = m.encode();
        let decoded = AuxValveFlowMsg::decode(&bytes, 3).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn flow_helpers_handle_not_available() {
        let m = AuxValveFlowMsg::default();
        assert_eq!(m.extend_flow(), 0.0);
        assert_eq!(m.retract_flow(), 0.0);
    }

    #[test]
    fn flow_helpers_compute_percent() {
        let m = AuxValveFlowMsg {
            extend_flow_percent: 100,  // 100 × 0.4% = 40%
            retract_flow_percent: 250, // 250 × 0.4% = 100%
            ..Default::default()
        };
        assert!((m.extend_flow() - 40.0).abs() < 1e-9);
        assert!((m.retract_flow() - 100.0).abs() < 1e-9);
    }

    #[test]
    fn with_flow_percent_round_trips_and_clamps() {
        // 40% → raw 100; 100% → raw 250 (round-trips through the accessors).
        let m = AuxValveFlowMsg::default()
            .with_extend_flow_percent(40.0)
            .with_retract_flow_percent(100.0);
        assert_eq!(m.extend_flow_percent, 100);
        assert_eq!(m.retract_flow_percent, 250);
        assert!((m.extend_flow() - 40.0).abs() < 1e-9);

        // Out-of-range clamps to the 0..=100% band; non-finite maps to 0.
        assert_eq!(
            AuxValveFlowMsg::default()
                .with_extend_flow_percent(150.0)
                .extend_flow_percent,
            250
        );
        assert_eq!(
            AuxValveFlowMsg::default()
                .with_extend_flow_percent(-5.0)
                .extend_flow_percent,
            0
        );
        assert_eq!(
            AuxValveFlowMsg::default()
                .with_extend_flow_percent(f64::NAN)
                .extend_flow_percent,
            0
        );
    }

    #[test]
    fn pgn_helpers_validate_index() {
        assert!(estimated_flow_pgn(0).is_some());
        assert!(estimated_flow_pgn(MAX_AUX_VALVES - 1).is_some());
        assert!(estimated_flow_pgn(MAX_AUX_VALVES).is_none());
        assert!(measured_flow_pgn(MAX_AUX_VALVES).is_none());
    }

    #[test]
    fn enums_round_trip() {
        for s in [
            ValveState::Blocked,
            ValveState::Extending,
            ValveState::Retracting,
            ValveState::FloatPosition,
        ] {
            assert_eq!(ValveState::from_u8(s.as_u8()), s);
        }
        for l in [
            ValveLimitStatus::NotLimited,
            ValveLimitStatus::OperatorLimited,
            ValveLimitStatus::SystemLimited,
            ValveLimitStatus::Error,
            ValveLimitStatus::NotAvailable,
        ] {
            assert_eq!(ValveLimitStatus::from_u8(l.as_u8()), l);
            assert_eq!(ValveLimitStatus::try_from_u8(l.as_u8()), Some(l));
        }
        for reserved in [
            ValveLimitStatus::Reserved3,
            ValveLimitStatus::Reserved4,
            ValveLimitStatus::Reserved5,
        ] {
            assert_eq!(ValveLimitStatus::from_u8(reserved.as_u8()), reserved);
            assert_eq!(ValveLimitStatus::try_from_u8(reserved.as_u8()), None);
        }
        for f in [
            ValveFailSafe::Block,
            ValveFailSafe::Float,
            ValveFailSafe::Extend,
            ValveFailSafe::Retract,
        ] {
            assert_eq!(ValveFailSafe::from_u8(f.as_u8()), f);
        }
    }

    #[test]
    fn decode_short_payload_returns_none() {
        assert!(AuxValveFlowMsg::decode(&[0u8; 7], 0).is_none());
    }

    #[test]
    fn decode_overlong_payload_returns_none() {
        assert!(AuxValveFlowMsg::decode(&[0u8; 9], 0).is_none());
    }

    #[test]
    fn decode_rejects_bad_padding_reserved_bit_and_index() {
        let mut bad_tail = AuxValveFlowMsg::default().encode();
        bad_tail[3] = 0x00;
        assert!(AuxValveFlowMsg::decode(&bad_tail, 0).is_none());

        let mut bad_reserved_bit = AuxValveFlowMsg::default().encode();
        bad_reserved_bit[2] &= 0x7F;
        assert!(AuxValveFlowMsg::decode(&bad_reserved_bit, 0).is_none());

        assert!(
            AuxValveFlowMsg::decode(&AuxValveFlowMsg::default().encode(), MAX_AUX_VALVES).is_none()
        );
    }
}
