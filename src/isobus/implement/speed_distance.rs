//! ISO 11783-7 wheel- / ground-based speed + distance, plus hitch /
//! PTO status feedback codecs.
//!
//! Mirrors the C++ `machbus::isobus::implement::speed_distance.hpp`.
//! PGNs covered:
//!
//! - `PGN_WHEEL_BASED_SPEED_DIST` (0xFE48) — TECU broadcast (Class 1+).
//! - `PGN_GROUND_BASED_SPEED_DIST` (0xFE49) — TECU broadcast (Class 2+).
//! - `PGN_MACHINE_SELECTED_SPEED` (0xF022) — full layout with
//!   distance + exit code (`MachineSelectedSpeedFull`).
//! - `PGN_FRONT_HITCH` (0xFE08) / `PGN_REAR_HITCH` (0xF005) — hitch
//!   status feedback (Class 2+).
//! - `PGN_FRONT_PTO` (0xFE54) / `PGN_REAR_PTO` (0xF003) — PTO status
//!   feedback (Class 2+).
//!
//! The C++ `TECUSpeedDistance` (IsoNet-coupled wrapper) is
//! intentionally not ported. `MachineDirection` and `SpeedSource` are
//! re-exported from `machine_speed_cmd`.
//!
//! ## `MachineSelectedSpeed` duplication
//!
//! The C++ defines `MachineSelectedSpeed` in *both*
//! `machine_speed_cmd.hpp` and `speed_distance.hpp` with subtly
//! different wire layouts (status flags packed into byte 4 vs. byte
//! 7, distance bytes 2..6 omitted in the former). This is a C++
//! inconsistency. The Rust port keeps both layouts but renames the
//! speed_distance variant `MachineSelectedSpeedFull` to make the
//! divergence explicit. See `book/src/reference/behavior-differences.md`.

use super::machine_speed_cmd::{MachineDirection, SpeedSource};
use crate::net::pgn_defs::{PGN_FRONT_HITCH, PGN_FRONT_PTO, PGN_REAR_HITCH, PGN_REAR_PTO};
use crate::net::types::Pgn;

const VALID_MAX_U16_SIGNAL_RAW: u16 = 0xFAFF;
const VALID_MAX_U32_SIGNAL_RAW: u32 = 0xFAFF_FFFF;

fn has_ff_tail(data: &[u8], used: usize) -> bool {
    data[used..].iter().all(|&byte| byte == 0xFF)
}

fn scaled_u16_non_na(value: f64, resolution: f64) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    let raw = value / resolution;
    if raw <= 0.0 {
        0
    } else if raw >= f64::from(VALID_MAX_U16_SIGNAL_RAW) {
        VALID_MAX_U16_SIGNAL_RAW
    } else {
        raw as u16
    }
}

fn u16_signal_raw_is_valid(raw: u16) -> bool {
    raw <= VALID_MAX_U16_SIGNAL_RAW
}

fn scaled_u32_bounded(value: f64, resolution: f64) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let raw = value / resolution;
    if raw <= 0.0 {
        0
    } else if raw >= f64::from(VALID_MAX_U32_SIGNAL_RAW) {
        VALID_MAX_U32_SIGNAL_RAW
    } else {
        raw as u32
    }
}

fn u32_signal_raw_is_valid(raw: u32) -> bool {
    raw <= VALID_MAX_U32_SIGNAL_RAW
}

fn percent_raw_is_available_or_not_available(raw: u8) -> bool {
    raw <= 250 || raw == 0xFF
}

fn offset_scaled_u16_bounded(value: f64, offset: f64, resolution: f64) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    let raw = (value + offset) / resolution;
    if raw <= 0.0 {
        0
    } else if raw >= f64::from(VALID_MAX_U16_SIGNAL_RAW) {
        VALID_MAX_U16_SIGNAL_RAW
    } else {
        raw as u16
    }
}

/// Wheel slip as a percentage, derived from wheel-based and ground-based speed
/// (`(wheel − ground) / wheel × 100`). Returns `None` when the wheel speed is
/// not positive (slip is undefined while stationary). Positive values indicate
/// driven-wheel slip; a negative value indicates overrun (e.g. downhill).
#[must_use]
pub fn wheel_slip_percent(wheel_speed_mps: f64, ground_speed_mps: f64) -> Option<f64> {
    if !wheel_speed_mps.is_finite() || !ground_speed_mps.is_finite() || wheel_speed_mps <= 0.0 {
        return None;
    }
    Some((wheel_speed_mps - ground_speed_mps) / wheel_speed_mps * 100.0)
}

// ─── Wheel-Based Speed and Distance (PGN 0xFE48) ───────────────────────

/// Wheel-based speed + accumulated distance broadcast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelBasedSpeedDist {
    /// `0.001 m/s` per bit (2 bytes).
    pub speed_mps: f64,
    /// `0.001 m` per bit (4 bytes, total accumulated).
    pub distance_m: f64,
    pub direction: MachineDirection,
    /// Maximum tractor power-on time, minutes; `0xFF` = N/A.
    pub max_power_time_min: u8,
    /// 2 bits in byte 7 bits 2..3: 0 = key off, 1 = not off, 2 = error,
    /// 3 = N/A.
    pub key_switch_state: u8,
    /// 2 bits in byte 7 bits 4..5: implement start/stop operation state.
    pub implement_start_stop_operations_state: u8,
    /// 2 bits in byte 7 bits 6..7: operator direction reversed state.
    pub operator_direction_reversed_state: u8,
}

impl Default for WheelBasedSpeedDist {
    fn default() -> Self {
        Self {
            speed_mps: 0.0,
            distance_m: 0.0,
            direction: MachineDirection::NotAvailable,
            max_power_time_min: 0xFF,
            key_switch_state: 0x03,
            implement_start_stop_operations_state: 0x03,
            operator_direction_reversed_state: 0x03,
        }
    }
}

impl WheelBasedSpeedDist {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let spd = scaled_u16_non_na(self.speed_mps, 0.001);
        data[0] = (spd & 0xFF) as u8;
        data[1] = ((spd >> 8) & 0xFF) as u8;
        let dist = scaled_u32_bounded(self.distance_m, 0.001);
        data[2] = (dist & 0xFF) as u8;
        data[3] = ((dist >> 8) & 0xFF) as u8;
        data[4] = ((dist >> 16) & 0xFF) as u8;
        data[5] = ((dist >> 24) & 0xFF) as u8;
        data[6] = self.max_power_time_min;
        data[7] = (self.direction.as_u8() & 0x03)
            | ((self.key_switch_state & 0x03) << 2)
            | ((self.implement_start_stop_operations_state & 0x03) << 4)
            | ((self.operator_direction_reversed_state & 0x03) << 6);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let spd = (data[0] as u16) | ((data[1] as u16) << 8);
        let dist = (data[2] as u32)
            | ((data[3] as u32) << 8)
            | ((data[4] as u32) << 16)
            | ((data[5] as u32) << 24);
        if !u16_signal_raw_is_valid(spd) || !u32_signal_raw_is_valid(dist) {
            return None;
        }
        Some(Self {
            speed_mps: spd as f64 * 0.001,
            distance_m: dist as f64 * 0.001,
            direction: MachineDirection::try_from_u8(data[7] & 0x03)?,
            max_power_time_min: data[6],
            key_switch_state: (data[7] >> 2) & 0x03,
            implement_start_stop_operations_state: (data[7] >> 4) & 0x03,
            operator_direction_reversed_state: (data[7] >> 6) & 0x03,
        })
    }
}

// ─── Ground-Based Speed and Distance (PGN 0xFE49) ──────────────────────

/// Ground-based speed + accumulated distance broadcast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundBasedSpeedDist {
    /// `0.001 m/s` per bit.
    pub speed_mps: f64,
    /// `0.001 m` per bit (total accumulated).
    pub distance_m: f64,
    pub direction: MachineDirection,
}

impl Default for GroundBasedSpeedDist {
    fn default() -> Self {
        Self {
            speed_mps: 0.0,
            distance_m: 0.0,
            direction: MachineDirection::NotAvailable,
        }
    }
}

impl GroundBasedSpeedDist {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let spd = scaled_u16_non_na(self.speed_mps, 0.001);
        data[0] = (spd & 0xFF) as u8;
        data[1] = ((spd >> 8) & 0xFF) as u8;
        let dist = scaled_u32_bounded(self.distance_m, 0.001);
        data[2] = (dist & 0xFF) as u8;
        data[3] = ((dist >> 8) & 0xFF) as u8;
        data[4] = ((dist >> 16) & 0xFF) as u8;
        data[5] = ((dist >> 24) & 0xFF) as u8;
        data[7] = self.direction.as_u8() & 0x03;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || data[6] != 0xFF || data[7] & 0xFC != 0 {
            return None;
        }
        let spd = (data[0] as u16) | ((data[1] as u16) << 8);
        let dist = (data[2] as u32)
            | ((data[3] as u32) << 8)
            | ((data[4] as u32) << 16)
            | ((data[5] as u32) << 24);
        if !u16_signal_raw_is_valid(spd) || !u32_signal_raw_is_valid(dist) {
            return None;
        }
        Some(Self {
            speed_mps: spd as f64 * 0.001,
            distance_m: dist as f64 * 0.001,
            direction: MachineDirection::try_from_u8(data[7] & 0x03)?,
        })
    }
}

// ─── Machine Selected Speed — full variant (PGN 0xF022) ────────────────

/// Full ISO 11783-7 wire layout for `PGN_MACHINE_SELECTED_SPEED`.
/// Includes accumulated distance + exit code, with the status nibble
/// packed into byte 7 (3-bit `limit_status`, not 2-bit). Use
/// `super::MachineSelectedSpeedMsg` for the simpler legacy layout.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachineSelectedSpeedFull {
    /// `0.001 m/s` per bit.
    pub speed_mps: f64,
    /// `0.001 m` per bit (total accumulated).
    pub distance_m: f64,
    pub direction: MachineDirection,
    pub source: SpeedSource,
    /// 3 bits: 0 = not limited, …, 7 = N/A.
    pub limit_status: u8,
    pub exit_code: u8,
}

impl Default for MachineSelectedSpeedFull {
    fn default() -> Self {
        Self {
            speed_mps: 0.0,
            distance_m: 0.0,
            direction: MachineDirection::NotAvailable,
            source: SpeedSource::WheelBased,
            limit_status: 0x07,
            exit_code: 0xFF,
        }
    }
}

impl MachineSelectedSpeedFull {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let spd = scaled_u16_non_na(self.speed_mps, 0.001);
        data[0] = (spd & 0xFF) as u8;
        data[1] = ((spd >> 8) & 0xFF) as u8;
        let dist = scaled_u32_bounded(self.distance_m, 0.001);
        data[2] = (dist & 0xFF) as u8;
        data[3] = ((dist >> 8) & 0xFF) as u8;
        data[4] = ((dist >> 16) & 0xFF) as u8;
        data[5] = ((dist >> 24) & 0xFF) as u8;
        data[6] = self.exit_code;
        data[7] = (self.direction.as_u8() & 0x03)
            | ((self.source.as_u8() & 0x03) << 2)
            | ((self.limit_status & 0x07) << 4);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || data[7] & 0x80 != 0 {
            return None;
        }
        let spd = (data[0] as u16) | ((data[1] as u16) << 8);
        let dist = (data[2] as u32)
            | ((data[3] as u32) << 8)
            | ((data[4] as u32) << 16)
            | ((data[5] as u32) << 24);
        if !u16_signal_raw_is_valid(spd) || !u32_signal_raw_is_valid(dist) {
            return None;
        }
        Some(Self {
            speed_mps: spd as f64 * 0.001,
            distance_m: dist as f64 * 0.001,
            direction: MachineDirection::try_from_u8(data[7] & 0x03)?,
            source: SpeedSource::try_from_u8((data[7] >> 2) & 0x03)?,
            limit_status: (data[7] >> 4) & 0x07,
            exit_code: data[6],
        })
    }
}

// ─── Hitch / PTO status feedback ───────────────────────────────────────

/// 2-bit limit status (status feedback variant).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum LimitStatus {
    NotLimited = 0,
    OperatorLimited = 1,
    SystemLimited = 2,
    #[default]
    NotAvailable = 3,
}

impl LimitStatus {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::NotLimited,
            1 => Self::OperatorLimited,
            2 => Self::SystemLimited,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotLimited),
            1 => Some(Self::OperatorLimited),
            2 => Some(Self::SystemLimited),
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

/// 3-bit exit-reason code for hitch status. The PTO variant only
/// uses 2 bits and silently masks; values above 3 will collapse to
/// `NotAvailable`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ExitReasonCode {
    NoExit = 0,
    OperatorCmd = 1,
    SystemCmd = 2,
    Fault = 3,
    #[default]
    NotAvailable = 7,
}

impl ExitReasonCode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            0 => Self::NoExit,
            1 => Self::OperatorCmd,
            2 => Self::SystemCmd,
            3 => Self::Fault,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_hitch_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoExit),
            1 => Some(Self::OperatorCmd),
            2 => Some(Self::SystemCmd),
            3 => Some(Self::Fault),
            7 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[must_use]
    pub const fn try_from_pto_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoExit),
            1 => Some(Self::OperatorCmd),
            2 => Some(Self::SystemCmd),
            3 => Some(Self::Fault),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Hitch status feedback (front or rear). Position is `0..=250`
/// for `0..=100 %` (0.4 % per bit).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HitchStatus {
    pub position_percent: u8,
    /// 2 bits: 0 = not in work, 1 = in work, 2 = error, 3 = N/A.
    pub in_work_indication: u8,
    pub limit_status: LimitStatus,
    pub exit_code: ExitReasonCode,
    /// Draft force in N (Class 2 TECU; 10 N per bit, offset −320 000).
    pub draft_force_n: f64,
    /// `true` = `PGN_REAR_HITCH` (0xF005), `false` = `PGN_FRONT_HITCH`.
    pub is_rear: bool,
}

impl Default for HitchStatus {
    fn default() -> Self {
        Self {
            position_percent: 0xFF,
            in_work_indication: 0x03,
            limit_status: LimitStatus::NotAvailable,
            exit_code: ExitReasonCode::NotAvailable,
            draft_force_n: 0.0,
            is_rear: true,
        }
    }
}

impl HitchStatus {
    #[must_use]
    pub const fn pgn(&self) -> Pgn {
        if self.is_rear {
            PGN_REAR_HITCH
        } else {
            PGN_FRONT_HITCH
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.position_percent;
        data[1] = (self.in_work_indication & 0x03)
            | ((self.limit_status.as_u8() & 0x03) << 2)
            | ((self.exit_code.as_u8() & 0x07) << 4);
        let force_raw = offset_scaled_u16_bounded(self.draft_force_n, 320_000.0, 10.0);
        data[2] = (force_raw & 0xFF) as u8;
        data[3] = ((force_raw >> 8) & 0xFF) as u8;
        data
    }

    /// Decode from a payload. Caller specifies whether the payload
    /// was received on the front or rear hitch PGN.
    #[must_use]
    pub fn decode(data: &[u8], is_rear: bool) -> Option<Self> {
        if data.len() != 8 || data[1] & 0x80 != 0 || !has_ff_tail(data, 4) {
            return None;
        }
        if !percent_raw_is_available_or_not_available(data[0]) {
            return None;
        }
        let force_raw = (data[2] as u16) | ((data[3] as u16) << 8);
        if !u16_signal_raw_is_valid(force_raw) {
            return None;
        }
        Some(Self {
            position_percent: data[0],
            in_work_indication: data[1] & 0x03,
            limit_status: LimitStatus::try_from_u8((data[1] >> 2) & 0x03)?,
            exit_code: ExitReasonCode::try_from_hitch_u8(data[1] >> 4)?,
            draft_force_n: f64::from(force_raw) * 10.0 - 320_000.0,
            is_rear,
        })
    }
}

/// PTO status feedback (front or rear).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PtoStatus {
    /// Shaft speed; 0.125 rpm per bit.
    pub shaft_speed_rpm: f64,
    /// 2 bits: 0 = disengaged, 1 = engaged, 2 = error, 3 = N/A.
    pub engagement: u8,
    pub limit_status: LimitStatus,
    /// Note: only the low 2 bits are used on the wire here (PTO
    /// layout, unlike the hitch which uses 3).
    pub exit_code: ExitReasonCode,
    /// 2 bits: 0 = not active, 1 = active, 2 = error, 3 = N/A.
    pub economy_mode: u8,
    /// `true` = `PGN_REAR_PTO` (0xF003), `false` = `PGN_FRONT_PTO`.
    pub is_rear: bool,
}

impl Default for PtoStatus {
    fn default() -> Self {
        Self {
            shaft_speed_rpm: 0.0,
            engagement: 0x03,
            limit_status: LimitStatus::NotAvailable,
            exit_code: ExitReasonCode::NotAvailable,
            economy_mode: 0x03,
            is_rear: true,
        }
    }
}

impl PtoStatus {
    #[must_use]
    pub const fn pgn(&self) -> Pgn {
        if self.is_rear {
            PGN_REAR_PTO
        } else {
            PGN_FRONT_PTO
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let rpm = scaled_u16_non_na(self.shaft_speed_rpm, 0.125);
        data[0] = (rpm & 0xFF) as u8;
        data[1] = ((rpm >> 8) & 0xFF) as u8;
        data[2] = (self.engagement & 0x03)
            | ((self.economy_mode & 0x03) << 2)
            | ((self.limit_status.as_u8() & 0x03) << 4)
            | ((self.exit_code.as_u8() & 0x03) << 6);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8], is_rear: bool) -> Option<Self> {
        if data.len() != 8 || !has_ff_tail(data, 3) {
            return None;
        }
        let rpm = (data[0] as u16) | ((data[1] as u16) << 8);
        if !u16_signal_raw_is_valid(rpm) {
            return None;
        }
        Some(Self {
            shaft_speed_rpm: rpm as f64 * 0.125,
            engagement: data[2] & 0x03,
            economy_mode: (data[2] >> 2) & 0x03,
            limit_status: LimitStatus::try_from_u8((data[2] >> 4) & 0x03)?,
            exit_code: ExitReasonCode::try_from_pto_u8(data[2] >> 6)?,
            is_rear,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_slip_computation() {
        // 10% slip: ground covers 90% of wheel speed.
        let slip = wheel_slip_percent(5.0, 4.5).unwrap();
        assert!((slip - 10.0).abs() < 1e-9);
        // No slip when ground == wheel.
        assert!((wheel_slip_percent(5.0, 5.0).unwrap()).abs() < 1e-9);
        // Overrun (ground faster than wheel) ⇒ negative slip.
        assert!(wheel_slip_percent(4.0, 5.0).unwrap() < 0.0);
        // Undefined when stationary or for non-finite input.
        assert!(wheel_slip_percent(0.0, 0.0).is_none());
        assert!(wheel_slip_percent(-1.0, 0.5).is_none());
        assert!(wheel_slip_percent(f64::NAN, 1.0).is_none());
    }

    #[test]
    fn wheel_speed_round_trip() {
        let m = WheelBasedSpeedDist {
            speed_mps: 5.5,
            distance_m: 12_345.678,
            direction: MachineDirection::Forward,
            max_power_time_min: 60,
            key_switch_state: 1,
            implement_start_stop_operations_state: 1,
            operator_direction_reversed_state: 0,
        };
        let decoded = WheelBasedSpeedDist::decode(&m.encode()).unwrap();
        assert!((decoded.speed_mps - 5.5).abs() < 1e-3);
        assert!((decoded.distance_m - 12_345.678).abs() < 1e-3);
        assert_eq!(decoded.direction, MachineDirection::Forward);
        assert_eq!(decoded.max_power_time_min, 60);
        assert_eq!(decoded.key_switch_state, 1);
        assert_eq!(decoded.implement_start_stop_operations_state, 1);
        assert_eq!(decoded.operator_direction_reversed_state, 0);
    }

    #[test]
    fn ground_speed_round_trip() {
        let m = GroundBasedSpeedDist {
            speed_mps: 3.0,
            distance_m: 100.0,
            direction: MachineDirection::Reverse,
        };
        let decoded = GroundBasedSpeedDist::decode(&m.encode()).unwrap();
        assert!((decoded.speed_mps - 3.0).abs() < 1e-3);
        assert!((decoded.distance_m - 100.0).abs() < 1e-3);
        assert_eq!(decoded.direction, MachineDirection::Reverse);
    }

    #[test]
    fn machine_selected_speed_full_round_trip() {
        let m = MachineSelectedSpeedFull {
            speed_mps: 2.5,
            distance_m: 1000.0,
            direction: MachineDirection::Forward,
            source: SpeedSource::GroundBased,
            limit_status: 1,
            exit_code: 0x42,
        };
        let decoded = MachineSelectedSpeedFull::decode(&m.encode()).unwrap();
        assert!((decoded.speed_mps - 2.5).abs() < 1e-3);
        assert!((decoded.distance_m - 1000.0).abs() < 1e-3);
        assert_eq!(decoded.direction, MachineDirection::Forward);
        assert_eq!(decoded.source, SpeedSource::GroundBased);
        assert_eq!(decoded.limit_status, 1);
        assert_eq!(decoded.exit_code, 0x42);
    }

    #[test]
    fn hitch_status_round_trip_and_pgn() {
        let m = HitchStatus {
            position_percent: 200,
            in_work_indication: 1,
            limit_status: LimitStatus::OperatorLimited,
            exit_code: ExitReasonCode::OperatorCmd,
            draft_force_n: -100_000.0,
            is_rear: true,
        };
        let bytes = m.encode();
        let decoded = HitchStatus::decode(&bytes, true).unwrap();
        assert_eq!(decoded.position_percent, 200);
        assert_eq!(decoded.in_work_indication, 1);
        assert_eq!(decoded.limit_status, LimitStatus::OperatorLimited);
        assert_eq!(decoded.exit_code, ExitReasonCode::OperatorCmd);
        assert!((decoded.draft_force_n - -100_000.0).abs() < 10.0);
        assert_eq!(decoded.pgn(), PGN_REAR_HITCH);
        let front = HitchStatus {
            is_rear: false,
            ..m
        };
        assert_eq!(front.pgn(), PGN_FRONT_HITCH);
    }

    #[test]
    fn pto_status_round_trip_and_pgn() {
        let m = PtoStatus {
            shaft_speed_rpm: 540.0,
            engagement: 1,
            limit_status: LimitStatus::SystemLimited,
            exit_code: ExitReasonCode::Fault,
            economy_mode: 0,
            is_rear: false,
        };
        let bytes = m.encode();
        let decoded = PtoStatus::decode(&bytes, false).unwrap();
        assert!((decoded.shaft_speed_rpm - 540.0).abs() < 0.125);
        assert_eq!(decoded.engagement, 1);
        assert_eq!(decoded.limit_status, LimitStatus::SystemLimited);
        assert_eq!(decoded.exit_code, ExitReasonCode::Fault);
        assert_eq!(decoded.economy_mode, 0);
        assert_eq!(decoded.pgn(), PGN_FRONT_PTO);
    }

    #[test]
    fn limit_status_round_trip() {
        for s in [
            LimitStatus::NotLimited,
            LimitStatus::OperatorLimited,
            LimitStatus::SystemLimited,
            LimitStatus::NotAvailable,
        ] {
            assert_eq!(LimitStatus::from_u8(s.as_u8()), s);
        }
    }

    #[test]
    fn exit_reason_code_round_trip() {
        for c in [
            ExitReasonCode::NoExit,
            ExitReasonCode::OperatorCmd,
            ExitReasonCode::SystemCmd,
            ExitReasonCode::Fault,
            ExitReasonCode::NotAvailable,
        ] {
            assert_eq!(ExitReasonCode::from_u8(c.as_u8()), c);
            assert_eq!(ExitReasonCode::try_from_hitch_u8(c.as_u8()), Some(c));
        }
        for reserved in 4..=6 {
            assert_eq!(
                ExitReasonCode::from_u8(reserved),
                ExitReasonCode::NotAvailable
            );
            assert_eq!(ExitReasonCode::try_from_hitch_u8(reserved), None);
        }
    }

    #[test]
    fn numeric_encoders_clamp_instead_of_wrapping_or_emitting_speed_na() {
        let wheel_high = WheelBasedSpeedDist {
            speed_mps: f64::INFINITY,
            distance_m: f64::INFINITY,
            ..Default::default()
        }
        .encode();
        assert_eq!(&wheel_high[..6], &[0, 0, 0, 0, 0, 0]);

        let wheel_high = WheelBasedSpeedDist {
            speed_mps: 1.0e9,
            distance_m: 1.0e12,
            ..Default::default()
        }
        .encode();
        assert_eq!(&wheel_high[..6], &[0xFF, 0xFA, 0xFF, 0xFF, 0xFF, 0xFA]);

        let ground_low = GroundBasedSpeedDist {
            speed_mps: -1.0,
            distance_m: f64::NAN,
            ..Default::default()
        }
        .encode();
        assert_eq!(&ground_low[..6], &[0, 0, 0, 0, 0, 0]);

        let selected_high = MachineSelectedSpeedFull {
            speed_mps: 1.0e9,
            distance_m: 1.0e12,
            ..Default::default()
        }
        .encode();
        assert_eq!(&selected_high[..6], &[0xFF, 0xFA, 0xFF, 0xFF, 0xFF, 0xFA]);

        let pto_high = PtoStatus {
            shaft_speed_rpm: 1.0e9,
            ..Default::default()
        }
        .encode();
        assert_eq!(&pto_high[..2], &[0xFF, 0xFA]);

        let hitch_high = HitchStatus {
            draft_force_n: 1.0e12,
            ..Default::default()
        }
        .encode();
        assert_eq!(&hitch_high[2..4], &[0xFF, 0xFA]);

        let hitch_low = HitchStatus {
            draft_force_n: f64::NEG_INFINITY,
            ..Default::default()
        }
        .encode();
        assert_eq!(&hitch_low[2..4], &[0, 0]);
    }

    #[test]
    fn short_payloads_return_none() {
        assert!(WheelBasedSpeedDist::decode(&[0u8; 7]).is_none());
        assert!(GroundBasedSpeedDist::decode(&[0u8; 7]).is_none());
        assert!(MachineSelectedSpeedFull::decode(&[0u8; 7]).is_none());
        assert!(HitchStatus::decode(&[0u8; 7], true).is_none());
        assert!(PtoStatus::decode(&[0u8; 7], false).is_none());
    }

    #[test]
    fn overlong_payloads_return_none() {
        assert!(WheelBasedSpeedDist::decode(&[0u8; 9]).is_none());
        assert!(GroundBasedSpeedDist::decode(&[0u8; 9]).is_none());
        assert!(MachineSelectedSpeedFull::decode(&[0u8; 9]).is_none());
        assert!(HitchStatus::decode(&[0u8; 9], true).is_none());
        assert!(PtoStatus::decode(&[0u8; 9], false).is_none());
    }

    #[test]
    fn decoders_reject_bad_padding_and_reserved_bits() {
        let mut ground_bad_padding = GroundBasedSpeedDist::default().encode();
        ground_bad_padding[6] = 0x00;
        assert!(GroundBasedSpeedDist::decode(&ground_bad_padding).is_none());

        let mut ground_bad_reserved = GroundBasedSpeedDist::default().encode();
        ground_bad_reserved[7] |= 0xFC;
        assert!(GroundBasedSpeedDist::decode(&ground_bad_reserved).is_none());

        let mut selected_bad_reserved = MachineSelectedSpeedFull::default().encode();
        selected_bad_reserved[7] |= 0x80;
        assert!(MachineSelectedSpeedFull::decode(&selected_bad_reserved).is_none());

        let mut hitch_bad_padding = HitchStatus::default().encode();
        hitch_bad_padding[4] = 0x00;
        assert!(HitchStatus::decode(&hitch_bad_padding, true).is_none());

        let mut hitch_bad_reserved = HitchStatus::default().encode();
        hitch_bad_reserved[1] |= 0x80;
        assert!(HitchStatus::decode(&hitch_bad_reserved, true).is_none());

        let mut pto_bad_padding = PtoStatus::default().encode();
        pto_bad_padding[3] = 0x00;
        assert!(PtoStatus::decode(&pto_bad_padding, true).is_none());
    }
}
