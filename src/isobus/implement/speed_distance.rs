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

/// A J1939 measured parameter, which carries three distinct outcomes the wire
/// keeps separate: a real measurement, the transmitter reporting its own fault,
/// and the parameter simply not being provided.
///
/// Collapsing these into a decode failure — as this module used to — makes a
/// faulted sensor indistinguishable from an unplugged bus, and drops the whole
/// PG (including its other, valid fields) whenever one parameter is absent.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Signal<T> {
    /// A real measurement.
    Value(T),
    /// The transmitter reports it cannot measure this parameter (error range).
    Error,
    /// The transmitter does not provide this parameter at all.
    #[default]
    NotAvailable,
}

impl<T: Copy> Signal<T> {
    /// The measurement, or `None` for error / not-available.
    #[must_use]
    pub const fn value(self) -> Option<T> {
        match self {
            Self::Value(v) => Some(v),
            _ => None,
        }
    }

    #[must_use]
    pub const fn is_error(self) -> bool {
        matches!(self, Self::Error)
    }

    #[must_use]
    pub const fn is_not_available(self) -> bool {
        matches!(self, Self::NotAvailable)
    }

    /// The measurement, or `fallback` when it is not a real value. Use only
    /// where a substituted number cannot be mistaken for a measurement.
    #[must_use]
    pub fn unwrap_or(self, fallback: T) -> T {
        self.value().unwrap_or(fallback)
    }
}

impl<T> From<T> for Signal<T> {
    fn from(value: T) -> Self {
        Self::Value(value)
    }
}

/// Split a 16-bit raw signal into value / error / not-available.
/// Reserved raws (0xFB00..=0xFDFF) are the only genuine decode failure.
fn u16_signal(raw: u16, resolution: f64) -> Option<Signal<f64>> {
    match raw {
        0..=VALID_MAX_U16_SIGNAL_RAW => Some(Signal::Value(f64::from(raw) * resolution)),
        0xFE00..=0xFEFF => Some(Signal::Error),
        0xFF00..=0xFFFF => Some(Signal::NotAvailable),
        _ => None,
    }
}

/// Split an 8-bit raw signal into value / error / not-available
/// (ISO 11783-7:2022 Table 1, 8-bit row).
///
/// `0xFE` is the error indicator and `0xFF` not-available; `0xFB..=0xFD` are
/// reserved and are the only genuine decode failure. Treating `0xFE` as one
/// meant a hitch reporting a failed position sensor dropped the whole PG,
/// including the limit status and exit code that say *why* it failed.
fn u8_signal(raw: u8, resolution: f64) -> Option<Signal<f64>> {
    match raw {
        0..=250 => Some(Signal::Value(f64::from(raw) * resolution)),
        0xFE => Some(Signal::Error),
        0xFF => Some(Signal::NotAvailable),
        _ => None,
    }
}

fn encode_u8_signal(signal: Signal<f64>, resolution: f64) -> u8 {
    match signal {
        Signal::Value(v) => scaled_u8_bounded(v, resolution),
        Signal::Error => 0xFE,
        Signal::NotAvailable => 0xFF,
    }
}

fn scaled_u8_bounded(value: f64, resolution: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    let raw = value / resolution;
    if raw <= 0.0 {
        0
    } else if raw >= 250.0 {
        250
    } else {
        raw as u8
    }
}

fn u32_signal(raw: u32, resolution: f64) -> Option<Signal<f64>> {
    match raw {
        0..=VALID_MAX_U32_SIGNAL_RAW => Some(Signal::Value(raw as f64 * resolution)),
        0xFE00_0000..=0xFEFF_FFFF => Some(Signal::Error),
        0xFF00_0000..=0xFFFF_FFFF => Some(Signal::NotAvailable),
        _ => None,
    }
}

fn encode_u16_signal(signal: Signal<f64>, resolution: f64) -> u16 {
    match signal {
        Signal::Value(v) => scaled_u16_non_na(v, resolution),
        Signal::Error => 0xFE00,
        Signal::NotAvailable => 0xFFFF,
    }
}

fn encode_u32_signal(signal: Signal<f64>, resolution: f64) -> u32 {
    match signal {
        Signal::Value(v) => scaled_u32_bounded(v, resolution),
        Signal::Error => 0xFE00_0000,
        Signal::NotAvailable => 0xFFFF_FFFF,
    }
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
    pub speed_mps: Signal<f64>,
    /// `0.001 m` per bit (4 bytes, total accumulated).
    pub distance_m: Signal<f64>,
    pub direction: MachineDirection,
    /// Maximum tractor power-on time, minutes; `0xFF` = N/A.
    pub max_power_time_min: u8,
    /// 2 bits in byte 7 bits 2..3: 0 = key off, 1 = not off, 2 = error,
    /// 3 = N/A.
    pub key_switch_state: u8,
    // NOTE: `key_switch_state` and `max_power_time_min` were decoded here and
    // acted on nowhere. Key-off means the operator is shutting the machine
    // down; an autonomous controller that keeps steering through it is exactly
    // the unintended-motion case. See `is_key_off`.
    /// 2 bits in byte 7 bits 4..5: implement start/stop operation state.
    pub implement_start_stop_operations_state: u8,
    /// 2 bits in byte 7 bits 6..7: operator direction reversed state.
    pub operator_direction_reversed_state: u8,
}

impl WheelBasedSpeedDist {
    /// `true` when the operator has switched the key off (raw `0`).
    ///
    /// `2` (error) and `3` (not available) are deliberately *not* key-off: an
    /// unknown key state is not evidence of a shutdown, and treating it as one
    /// would stop the machine on a decode gap.
    #[must_use]
    pub const fn is_key_off(&self) -> bool {
        self.key_switch_state == 0
    }

    /// Maximum tractor power-on time in minutes, or `None` when not available.
    #[must_use]
    pub const fn max_power_time(&self) -> Option<u8> {
        if self.max_power_time_min == 0xFF {
            None
        } else {
            Some(self.max_power_time_min)
        }
    }
}

impl Default for WheelBasedSpeedDist {
    fn default() -> Self {
        Self {
            speed_mps: Signal::NotAvailable,
            distance_m: Signal::NotAvailable,
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
        let spd = encode_u16_signal(self.speed_mps, 0.001);
        data[0] = (spd & 0xFF) as u8;
        data[1] = ((spd >> 8) & 0xFF) as u8;
        let dist = encode_u32_signal(self.distance_m, 0.001);
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
        let speed_mps = u16_signal(spd, 0.001)?;
        let distance_m = u32_signal(dist, 0.001)?;
        Some(Self {
            speed_mps,
            distance_m,
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
    pub speed_mps: Signal<f64>,
    /// `0.001 m` per bit (total accumulated).
    pub distance_m: Signal<f64>,
    pub direction: MachineDirection,
}

impl Default for GroundBasedSpeedDist {
    fn default() -> Self {
        Self {
            speed_mps: Signal::NotAvailable,
            distance_m: Signal::NotAvailable,
            direction: MachineDirection::NotAvailable,
        }
    }
}

impl GroundBasedSpeedDist {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let spd = encode_u16_signal(self.speed_mps, 0.001);
        data[0] = (spd & 0xFF) as u8;
        data[1] = ((spd >> 8) & 0xFF) as u8;
        let dist = encode_u32_signal(self.distance_m, 0.001);
        data[2] = (dist & 0xFF) as u8;
        data[3] = ((dist >> 8) & 0xFF) as u8;
        data[4] = ((dist >> 16) & 0xFF) as u8;
        data[5] = ((dist >> 24) & 0xFF) as u8;
        // Byte 8 bits 3-8 are unspecified and must be transmitted as ones
        // (ISO 11783-7 §5.5.9.1); emitting zeros here made every frame this
        // stack sent non-conformant.
        data[7] = 0xFC | (self.direction.as_u8() & 0x03);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        // Unspecified bits are "don't care" on receive (§5.4). Demanding they
        // be zero rejected every conformant transmitter: the committed
        // captures carry byte 8 = 0xFD/0xFC and byte 7 = 0x00, all of which
        // this decoder used to drop.
        if data.len() != 8 {
            return None;
        }
        let spd = (data[0] as u16) | ((data[1] as u16) << 8);
        let dist = (data[2] as u32)
            | ((data[3] as u32) << 8)
            | ((data[4] as u32) << 16)
            | ((data[5] as u32) << 24);
        let speed_mps = u16_signal(spd, 0.001)?;
        let distance_m = u32_signal(dist, 0.001)?;
        Some(Self {
            speed_mps,
            distance_m,
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
    pub speed_mps: Signal<f64>,
    /// `0.001 m` per bit (total accumulated).
    pub distance_m: Signal<f64>,
    pub direction: MachineDirection,
    pub source: SpeedSource,
    /// 3 bits: 0 = not limited, …, 7 = N/A.
    pub limit_status: u8,
    pub exit_code: u8,
}

impl Default for MachineSelectedSpeedFull {
    fn default() -> Self {
        Self {
            speed_mps: Signal::NotAvailable,
            distance_m: Signal::NotAvailable,
            direction: MachineDirection::NotAvailable,
            source: SpeedSource::WheelBased,
            limit_status: 0x07,
            // 6-bit field: 0x3F is its "not available" value.
            exit_code: 0x3F,
        }
    }
}

impl MachineSelectedSpeedFull {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let spd = encode_u16_signal(self.speed_mps, 0.001);
        data[0] = (spd & 0xFF) as u8;
        data[1] = ((spd >> 8) & 0xFF) as u8;
        let dist = encode_u32_signal(self.distance_m, 0.001);
        data[2] = (dist & 0xFF) as u8;
        data[3] = ((dist >> 8) & 0xFF) as u8;
        data[4] = ((dist >> 16) & 0xFF) as u8;
        data[5] = ((dist >> 24) & 0xFF) as u8;
        // Byte 8: direction bits 1-2, source bits 3-5 (3 bits), limit status
        // bits 6-8 (3 bits). Exit reason is the low 6 bits of byte 7.
        data[6] = 0xC0 | (self.exit_code & 0x3F);
        data[7] = (self.direction.as_u8() & 0x03)
            | ((self.source.as_u8() & 0x07) << 2)
            | ((self.limit_status & 0x07) << 5);
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
        let speed_mps = u16_signal(spd, 0.001)?;
        let distance_m = u32_signal(dist, 0.001)?;
        Some(Self {
            speed_mps,
            distance_m,
            direction: MachineDirection::try_from_u8(data[7] & 0x03)?,
            source: SpeedSource::try_from_u8((data[7] >> 2) & 0x07)?,
            limit_status: (data[7] >> 5) & 0x07,
            exit_code: data[6] & 0x3F,
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
    /// SPN 1873/1876: 0.4 %/bit. This used to be the raw wire byte, so the
    /// not-available sentinel 0xFF read back as a position of 255.
    pub position_percent: Signal<f64>,
    /// 2 bits: 0 = not in work, 1 = in work, 2 = error, 3 = N/A.
    pub in_work_indication: u8,
    pub limit_status: LimitStatus,
    pub exit_code: ExitReasonCode,
    /// Draft force in N (Class 2 TECU; 10 N per bit, offset −320 000).
    ///
    /// ISO 11783-9 §4.4.2.1 *requires* a tractor with no draft sensor to
    /// broadcast this as not-available, so it cannot be a plain `f64`.
    pub draft_force_n: Signal<f64>,
    /// `true` = `PGN_REAR_HITCH` (0xF005), `false` = `PGN_FRONT_HITCH`.
    pub is_rear: bool,
}

impl Default for HitchStatus {
    fn default() -> Self {
        Self {
            position_percent: Signal::NotAvailable,
            in_work_indication: 0x03,
            limit_status: LimitStatus::NotAvailable,
            exit_code: ExitReasonCode::NotAvailable,
            // 0 N was a claim, not the absence of a draft sensor.
            draft_force_n: Signal::NotAvailable,
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
        data[0] = encode_u8_signal(self.position_percent, 0.4);
        data[1] = (self.in_work_indication & 0x03)
            | ((self.limit_status.as_u8() & 0x03) << 2)
            | ((self.exit_code.as_u8() & 0x07) << 4);
        let force_raw = match self.draft_force_n {
            Signal::Value(v) => offset_scaled_u16_bounded(v, 320_000.0, 10.0),
            Signal::Error => 0xFE00,
            Signal::NotAvailable => 0xFFFF,
        };
        data[2] = (force_raw & 0xFF) as u8;
        data[3] = ((force_raw >> 8) & 0xFF) as u8;
        data
    }

    /// Decode from a payload. Caller specifies whether the payload
    /// was received on the front or rear hitch PGN.
    #[must_use]
    pub fn decode(data: &[u8], is_rear: bool) -> Option<Self> {
        // Byte 1 bit 8 is *defined* (the hitch exit code is 3 bits wide), so a
        // set bit there really is malformed. The tail is not: undefined bytes
        // are "don't care" on receive (ISO 11783-7 §5.4).
        if data.len() != 8 || data[1] & 0x80 != 0 {
            return None;
        }
        let force_raw = (data[2] as u16) | ((data[3] as u16) << 8);
        // G4: neither an unfitted draft sensor nor a failed position sensor may
        // cost the limit status and exit code carried in the same frame.
        Some(Self {
            position_percent: u8_signal(data[0], 0.4)?,
            in_work_indication: data[1] & 0x03,
            limit_status: LimitStatus::try_from_u8((data[1] >> 2) & 0x03)?,
            exit_code: ExitReasonCode::try_from_hitch_u8(data[1] >> 4)?,
            draft_force_n: match u16_signal(force_raw, 10.0)? {
                Signal::Value(v) => Signal::Value(v - 320_000.0),
                other => other,
            },
            is_rear,
        })
    }
}

/// PTO status feedback (front or rear).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PtoStatus {
    /// Shaft speed; 0.125 rpm per bit.
    ///
    /// ISO 11783-9 §4.4.2.1 *requires* a tractor with no PTO fitted to
    /// broadcast this as not-available, so it cannot be a plain `f64`: the
    /// crate could neither decode such a frame nor transmit one.
    pub shaft_speed_rpm: Signal<f64>,
    /// 2 bits: 0 = disengaged, 1 = engaged, 2 = error, 3 = N/A.
    pub engagement: u8,
    pub limit_status: LimitStatus,
    /// Note: only the low 2 bits are used on the wire here (PTO layout, unlike
    /// the hitch which uses 3), so `ExitReasonCode::NotAvailable` (7) cannot be
    /// represented: it encodes as 3 and reads back as `Fault`. Callers that
    /// need "no exit reason reported" on a PTO have no way to say so.
    pub exit_code: ExitReasonCode,
    /// 2 bits: 0 = not active, 1 = active, 2 = error, 3 = N/A.
    pub economy_mode: u8,
    /// `true` = `PGN_REAR_PTO` (0xF003), `false` = `PGN_FRONT_PTO`.
    pub is_rear: bool,
}

impl Default for PtoStatus {
    fn default() -> Self {
        Self {
            shaft_speed_rpm: Signal::NotAvailable,
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
        let rpm = encode_u16_signal(self.shaft_speed_rpm, 0.125);
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
        // ISO 11783-7:2022 §5.4: "All undefined bits should be received as
        // 'don't care' (either masked out or ignored). This permits them to be
        // defined and used in the future without causing any incompatibilities."
        // Requiring the exact value machbus writes made them load-bearing on
        // receive, so a transmitter one revision ahead was rejected outright.
        if data.len() != 8 {
            return None;
        }
        let rpm = (data[0] as u16) | ((data[1] as u16) << 8);
        // G4: "no PTO fitted" is a reading, not a malformed frame. Dropping it
        // hid the engagement, economy mode, limit status and exit code carried
        // in the same byte.
        Some(Self {
            shaft_speed_rpm: u16_signal(rpm, 0.125)?,
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
            speed_mps: 5.5.into(),
            distance_m: 12_345.678.into(),
            direction: MachineDirection::Forward,
            max_power_time_min: 60,
            key_switch_state: 1,
            implement_start_stop_operations_state: 1,
            operator_direction_reversed_state: 0,
        };
        let decoded = WheelBasedSpeedDist::decode(&m.encode()).unwrap();
        assert!((decoded.speed_mps.unwrap_or(f64::NAN) - 5.5).abs() < 1e-3);
        assert!((decoded.distance_m.unwrap_or(f64::NAN) - 12_345.678).abs() < 1e-3);
        assert_eq!(decoded.direction, MachineDirection::Forward);
        assert_eq!(decoded.max_power_time_min, 60);
        assert_eq!(decoded.key_switch_state, 1);
        assert_eq!(decoded.implement_start_stop_operations_state, 1);
        assert_eq!(decoded.operator_direction_reversed_state, 0);
    }

    #[test]
    fn ground_speed_round_trip() {
        let m = GroundBasedSpeedDist {
            speed_mps: 3.0.into(),
            distance_m: 100.0.into(),
            direction: MachineDirection::Reverse,
        };
        let decoded = GroundBasedSpeedDist::decode(&m.encode()).unwrap();
        assert!((decoded.speed_mps.unwrap_or(f64::NAN) - 3.0).abs() < 1e-3);
        assert!((decoded.distance_m.unwrap_or(f64::NAN) - 100.0).abs() < 1e-3);
        assert_eq!(decoded.direction, MachineDirection::Reverse);
    }

    #[test]
    fn machine_selected_speed_full_round_trip() {
        let m = MachineSelectedSpeedFull {
            speed_mps: 2.5.into(),
            distance_m: 1000.0.into(),
            direction: MachineDirection::Forward,
            source: SpeedSource::GroundBased,
            limit_status: 1,
            // Exit reason is a 6-bit field; 0x42 would not survive the mask.
            exit_code: 0x02,
        };
        let decoded = MachineSelectedSpeedFull::decode(&m.encode()).unwrap();
        assert!((decoded.speed_mps.unwrap_or(f64::NAN) - 2.5).abs() < 1e-3);
        assert!((decoded.distance_m.unwrap_or(f64::NAN) - 1000.0).abs() < 1e-3);
        assert_eq!(decoded.direction, MachineDirection::Forward);
        assert_eq!(decoded.source, SpeedSource::GroundBased);
        assert_eq!(decoded.limit_status, 1);
        assert_eq!(decoded.exit_code, 0x02);
    }

    #[test]
    fn hitch_status_round_trip_and_pgn() {
        // Raw 200 at the SPN 1873 resolution of 0.4 %/bit is 80 %.
        let m = HitchStatus {
            position_percent: Signal::Value(80.0),
            in_work_indication: 1,
            limit_status: LimitStatus::OperatorLimited,
            exit_code: ExitReasonCode::OperatorCmd,
            draft_force_n: Signal::Value(-100_000.0),
            is_rear: true,
        };
        let bytes = m.encode();
        assert_eq!(bytes[0], 200);
        let decoded = HitchStatus::decode(&bytes, true).unwrap();
        assert!((decoded.position_percent.value().unwrap() - 80.0).abs() < 0.4);
        assert_eq!(decoded.in_work_indication, 1);
        assert_eq!(decoded.limit_status, LimitStatus::OperatorLimited);
        assert_eq!(decoded.exit_code, ExitReasonCode::OperatorCmd);
        assert!((decoded.draft_force_n.value().unwrap() - -100_000.0).abs() < 10.0);
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
            shaft_speed_rpm: Signal::Value(540.0),
            engagement: 1,
            limit_status: LimitStatus::SystemLimited,
            exit_code: ExitReasonCode::Fault,
            economy_mode: 0,
            is_rear: false,
        };
        let bytes = m.encode();
        let decoded = PtoStatus::decode(&bytes, false).unwrap();
        assert!((decoded.shaft_speed_rpm.value().unwrap() - 540.0).abs() < 0.125);
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
            speed_mps: f64::INFINITY.into(),
            distance_m: f64::INFINITY.into(),
            ..Default::default()
        }
        .encode();
        assert_eq!(&wheel_high[..6], &[0, 0, 0, 0, 0, 0]);

        let wheel_high = WheelBasedSpeedDist {
            speed_mps: 1.0e9.into(),
            distance_m: 1.0e12.into(),
            ..Default::default()
        }
        .encode();
        assert_eq!(&wheel_high[..6], &[0xFF, 0xFA, 0xFF, 0xFF, 0xFF, 0xFA]);

        let ground_low = GroundBasedSpeedDist {
            speed_mps: Signal::Value(-1.0),
            distance_m: f64::NAN.into(),
            ..Default::default()
        }
        .encode();
        assert_eq!(&ground_low[..6], &[0, 0, 0, 0, 0, 0]);

        let selected_high = MachineSelectedSpeedFull {
            speed_mps: 1.0e9.into(),
            distance_m: 1.0e12.into(),
            ..Default::default()
        }
        .encode();
        assert_eq!(&selected_high[..6], &[0xFF, 0xFA, 0xFF, 0xFF, 0xFF, 0xFA]);

        let pto_high = PtoStatus {
            shaft_speed_rpm: Signal::Value(1.0e9),
            ..Default::default()
        }
        .encode();
        assert_eq!(&pto_high[..2], &[0xFF, 0xFA]);

        let hitch_high = HitchStatus {
            draft_force_n: Signal::Value(1.0e12),
            ..Default::default()
        }
        .encode();
        assert_eq!(&hitch_high[2..4], &[0xFF, 0xFA]);

        let hitch_low = HitchStatus {
            draft_force_n: Signal::Value(f64::NEG_INFINITY),
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
        // Unspecified bits in the speed messages are "don't care" on receive
        // (ISO 11783-7 §5.4). This block used to assert the opposite — that a
        // frame whose reserved bits are set to 1, which is what a conformant
        // transmitter sends, must be rejected. See the dedicated tests below.

        // B5 — the hitch keeps its byte-1 bit 8 check: that bit is *defined*
        // (ISO 11783-7 hitch layout uses 3 bits for the exit code), so a set
        // bit there is a malformed frame rather than a future field.
        let mut hitch_bad_reserved = HitchStatus::default().encode();
        hitch_bad_reserved[1] |= 0x80;
        assert!(HitchStatus::decode(&hitch_bad_reserved, true).is_none());

        // Undefined tail bytes are "don't care" on receive (§5.4).
        let hitch = HitchStatus::default();
        let mut hitch_future_tail = hitch.encode();
        hitch_future_tail[4] = 0x00;
        hitch_future_tail[7] = 0x5A;
        assert_eq!(HitchStatus::decode(&hitch_future_tail, true), Some(hitch));

        // Compared field-by-field rather than as a whole struct: the PTO exit
        // code is only 2 bits on the wire, so `NotAvailable` (7) encodes as 3
        // and reads back as `Fault`. That asymmetry predates this change and is
        // a modelling gap in `ExitReasonCode`, not a padding question.
        let pto = PtoStatus {
            exit_code: ExitReasonCode::SystemCmd,
            ..PtoStatus::default()
        };
        let mut pto_future_tail = pto.encode();
        pto_future_tail[3] = 0x00;
        pto_future_tail[7] = 0x5A;
        assert_eq!(PtoStatus::decode(&pto_future_tail, true), Some(pto));
    }

    /// W1 — real Machine Selected Speed frames captured off a tractor's ISOBUS
    /// (`can_log2.txt`, TECU at SA 0xF0). Byte 8 = 0xE1 means direction
    /// forward, source wheel-based, limit status "not available". The decoder
    /// used to read the source as 2 bits and the limit status at bit 5 instead
    /// of bit 6, and rejected any frame with byte 8 bit 8 set — which is every
    /// frame carrying limit status 7. All 1592 in the capture were dropped, so
    /// nothing could observe the speed the tractor was closing its loop on.
    #[test]
    fn machine_selected_speed_decodes_real_captured_frames() {
        for (frame, label) in [
            (
                [0x00, 0x00, 0xF7, 0x4A, 0x00, 0x00, 0xFF, 0xE1],
                "moving, dir forward",
            ),
            (
                [0x00, 0x00, 0xF7, 0x4A, 0x00, 0x00, 0xFF, 0xE0],
                "stopped, dir reverse",
            ),
        ] {
            let decoded = MachineSelectedSpeedFull::decode(&frame)
                .unwrap_or_else(|| panic!("real captured frame must decode ({label})"));
            assert_eq!(decoded.source, SpeedSource::WheelBased, "{label}");
            assert_eq!(
                decoded.limit_status, 7,
                "{label}: limit status not available"
            );
        }

        assert_eq!(
            MachineSelectedSpeedFull::decode(&[0x00, 0x00, 0xF7, 0x4A, 0x00, 0x00, 0xFF, 0xE1])
                .unwrap()
                .direction,
            MachineDirection::Forward
        );
    }

    /// W7 — raw 4 is *simulated* speed. Decoded as a 2-bit field it aliased to
    /// `WheelBased`, so a bench signal was indistinguishable from real motion.
    #[test]
    fn speed_source_distinguishes_simulated_from_wheel_based() {
        assert_eq!(SpeedSource::try_from_u8(4), Some(SpeedSource::Simulated));
        assert_eq!(SpeedSource::from_u8(4), SpeedSource::Simulated);
        assert!(!SpeedSource::Simulated.is_trustworthy_for_control());
        assert!(!SpeedSource::NotAvailable.is_trustworthy_for_control());
        assert!(SpeedSource::WheelBased.is_trustworthy_for_control());

        let mut frame = [0x00, 0x00, 0xF7, 0x4A, 0x00, 0x00, 0xFF, 0xE1];
        frame[7] = (frame[7] & !0x1C) | (4 << 2);
        assert_eq!(
            MachineSelectedSpeedFull::decode(&frame).unwrap().source,
            SpeedSource::Simulated
        );
    }

    /// W2 — ISO 11783-7 §5.5.9.1 requires unspecified bits to be transmitted
    /// as ones, and §5.4 requires receivers to ignore them. This encoder wrote
    /// zeros and its decoder demanded zeros, so machbus and every conformant
    /// transmitter talked past each other.
    #[test]
    fn ground_based_speed_uses_ones_for_unspecified_bits() {
        let encoded = GroundBasedSpeedDist {
            speed_mps: 1.5.into(),
            distance_m: 100.0.into(),
            direction: MachineDirection::Forward,
        }
        .encode();
        assert_eq!(
            encoded[7] & 0xFC,
            0xFC,
            "byte 8 bits 3-8 are unspecified and must be ones"
        );

        // The capture's byte-8 values, which the old decoder rejected outright.
        for tail in [0xFDu8, 0xFC] {
            let mut frame = encoded;
            frame[7] = tail;
            assert!(
                GroundBasedSpeedDist::decode(&frame).is_some(),
                "captured byte 8 = {tail:#04X} must decode"
            );
        }
        // Zero-padded reserved bits are legal to receive too.
        let mut zero_padded = encoded;
        zero_padded[7] &= 0x03;
        assert!(GroundBasedSpeedDist::decode(&zero_padded).is_some());
    }
}

#[cfg(test)]
mod capture_tests {
    use super::*;

    /// W2 + W5 — the exact Ground-Based Speed & Distance frame the committed
    /// capture contains 1592 times (`can_log2.txt`, TECU at SA 0xF0). This
    /// machine has no ground-speed sensor, so it reports the speed as
    /// not-available while still broadcasting a valid PG. The decoder used to
    /// reject it twice over: byte 8's unspecified bits are ones, and the
    /// speed raw is 0xFFFF.
    #[test]
    fn ground_based_speed_decodes_the_captured_no_sensor_frame() {
        for (frame, label) in [
            (
                [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFD],
                "byte 8 = 0xFD",
            ),
            (
                [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFC],
                "byte 8 = 0xFC",
            ),
        ] {
            let decoded = GroundBasedSpeedDist::decode(&frame)
                .unwrap_or_else(|| panic!("captured frame must decode ({label})"));
            assert!(
                decoded.speed_mps.is_not_available(),
                "{label}: no ground-speed sensor fitted"
            );
            assert_eq!(
                decoded.distance_m.value(),
                Some(65.535),
                "{label}: the distance in the same frame is still valid data"
            );
        }

        assert_eq!(
            GroundBasedSpeedDist::decode(&[0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFD])
                .unwrap()
                .direction,
            MachineDirection::Forward
        );
    }

    /// A faulted sensor and an absent one must not look alike: that ambiguity
    /// is what made "no reading" indistinguishable from "bus unplugged".
    #[test]
    fn error_and_not_available_speeds_stay_distinguishable() {
        let mut faulted = [0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFD];
        faulted[0..2].copy_from_slice(&0xFE00_u16.to_le_bytes());
        let decoded = GroundBasedSpeedDist::decode(&faulted).unwrap();
        assert!(decoded.speed_mps.is_error());
        assert!(!decoded.speed_mps.is_not_available());
    }
}
