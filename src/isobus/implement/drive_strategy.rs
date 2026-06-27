//! ISO 11783-7 §11 drive strategy + auxiliary command codecs.
//!
//! Mirrors the C++ `machbus::isobus::implement::drive_strategy.hpp`.
//! Five PGNs landed here:
//!
//! - `PGN_DRIVE_STRATEGY_CMD` (0xFCCE) — implement → tractor
//! - `PGN_GUIDANCE_SYSTEM_CMD` (0xAD00) — Class xG / external guidance
//! - `PGN_HITCH_PTO_COMBINED_CMD` (0xFE42)
//! - `PGN_FRONT_HITCH_ROLL_PITCH_CMD` (0xF100)
//! - `PGN_REAR_HITCH_ROLL_PITCH_CMD` (0xF102)
//!
//! The C++ `DriveStrategyInterface` is intentionally not ported.

use crate::net::pgn_defs::{PGN_FRONT_HITCH_ROLL_PITCH_CMD, PGN_REAR_HITCH_ROLL_PITCH_CMD};
use crate::net::types::Pgn;

/// `PGN_DRIVE_STRATEGY_CMD` mode byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DriveStrategyMode {
    #[default]
    NoAction = 0,
    MaxPower = 1,
    MaxEconomy = 2,
    MaxSpeed = 3,
    Reserved = 0xFF,
}

impl DriveStrategyMode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::NoAction,
            1 => Self::MaxPower,
            2 => Self::MaxEconomy,
            3 => Self::MaxSpeed,
            _ => Self::Reserved,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoAction),
            1 => Some(Self::MaxPower),
            2 => Some(Self::MaxEconomy),
            3 => Some(Self::MaxSpeed),
            0xFF => Some(Self::Reserved),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

fn fixed8_with_ff_tail(data: &[u8], used: usize) -> bool {
    data.len() == 8 && data[used..].iter().all(|&byte| byte == 0xFF)
}

const GUIDANCE_SYSTEM_CURVATURE_MIN_PER_KM: f64 = -8032.0;
const GUIDANCE_SYSTEM_CURVATURE_MAX_PER_KM: f64 = 8031.75;
const GUIDANCE_SYSTEM_CURVATURE_OFFSET_PER_KM: f64 = 8032.0;
const GUIDANCE_SYSTEM_CURVATURE_RESOLUTION_PER_KM: f64 = 0.25;
const GUIDANCE_SYSTEM_CURVATURE_MAX_RAW: u16 = 0xFAFF;

fn encode_guidance_system_curvature(curvature_per_km: f64) -> u16 {
    if curvature_per_km.is_nan() {
        return 0;
    }
    if !curvature_per_km.is_finite() {
        return if curvature_per_km.is_sign_positive() {
            u16::MAX
        } else {
            0
        };
    }
    let clamped = curvature_per_km.clamp(
        GUIDANCE_SYSTEM_CURVATURE_MIN_PER_KM,
        GUIDANCE_SYSTEM_CURVATURE_MAX_PER_KM,
    );
    ((clamped + GUIDANCE_SYSTEM_CURVATURE_OFFSET_PER_KM)
        / GUIDANCE_SYSTEM_CURVATURE_RESOLUTION_PER_KM) as u16
}

/// Drive Strategy Command (PGN 0xFCCE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriveStrategyCmd {
    pub mode: DriveStrategyMode,
    /// `0.4 %` per bit (0–100 %); `0xFF` = N/A.
    pub target_speed_limit_percent: u8,
    /// `0.4 %` per bit (0–100 %); `0xFF` = N/A.
    pub target_engine_load_percent: u8,
}

impl Default for DriveStrategyCmd {
    fn default() -> Self {
        Self {
            mode: DriveStrategyMode::NoAction,
            target_speed_limit_percent: 0xFF,
            target_engine_load_percent: 0xFF,
        }
    }
}

impl DriveStrategyCmd {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.mode.as_u8();
        data[1] = self.target_speed_limit_percent;
        data[2] = self.target_engine_load_percent;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_tail(data, 3) {
            return None;
        }
        Some(Self {
            mode: DriveStrategyMode::try_from_u8(data[0])?,
            target_speed_limit_percent: data[1],
            target_engine_load_percent: data[2],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum CurvatureCommandStatus {
    #[default]
    NotIntendedToSteer = 0,
    IntendedToSteer = 1,
    ErrorIndication = 2,
    NotAvailable = 3,
}

impl CurvatureCommandStatus {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::NotIntendedToSteer,
            1 => Self::IntendedToSteer,
            2 => Self::ErrorIndication,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotIntendedToSteer),
            1 => Some(Self::IntendedToSteer),
            2 => Some(Self::ErrorIndication),
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

/// Agricultural Guidance System Command (PGN 0xAD00) — Class xG.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuidanceSystemCmd {
    /// 1/km, 0.25/km per bit, offset −8032.
    pub commanded_curvature: f64,
    /// Bits 0..1 of byte 2. Bits 2..7 are reserved and transmitted as one.
    pub status: CurvatureCommandStatus,
}

impl Default for GuidanceSystemCmd {
    fn default() -> Self {
        Self {
            commanded_curvature: 0.0,
            status: CurvatureCommandStatus::NotAvailable,
        }
    }
}

impl GuidanceSystemCmd {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let raw = encode_guidance_system_curvature(self.commanded_curvature);
        data[0] = (raw & 0xFF) as u8;
        data[1] = ((raw >> 8) & 0xFF) as u8;
        data[2] = 0xFC | self.status.as_u8();
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_tail(data, 3) {
            return None;
        }
        if data[2] & 0xFC != 0xFC {
            return None;
        }
        let raw = (data[0] as u16) | ((data[1] as u16) << 8);
        if raw > GUIDANCE_SYSTEM_CURVATURE_MAX_RAW {
            return None;
        }
        Some(Self {
            commanded_curvature: raw as f64 * 0.25 - 8032.0,
            status: CurvatureCommandStatus::try_from_u8(data[2] & 0x03)?,
        })
    }
}

/// Hitch + PTO combined command (PGN 0xFE42). Coordinates hitch
/// position and PTO engagement in a single message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitchPtoCombinedCmd {
    /// `0.0025 %` per bit (0–100 %); `0xFFFF` = N/A.
    pub hitch_position: u16,
    /// `0.125 rpm` per bit; `0xFFFF` = N/A.
    pub pto_speed_raw: u16,
    /// 2 bits: 0 = no action, 1 = lower, 2 = raise, 3 = N/A.
    pub hitch_cmd: u8,
    /// 2 bits: 0 = no action, 1 = engage, 2 = disengage, 3 = N/A.
    pub pto_cmd: u8,
}

impl Default for HitchPtoCombinedCmd {
    fn default() -> Self {
        Self {
            hitch_position: 0xFFFF,
            pto_speed_raw: 0xFFFF,
            hitch_cmd: 0x03,
            pto_cmd: 0x03,
        }
    }
}

impl HitchPtoCombinedCmd {
    #[must_use]
    pub fn pto_speed_rpm(&self) -> f64 {
        if self.pto_speed_raw == 0xFFFF {
            0.0
        } else {
            self.pto_speed_raw as f64 * 0.125
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.hitch_position & 0xFF) as u8;
        data[1] = ((self.hitch_position >> 8) & 0xFF) as u8;
        data[2] = (self.pto_speed_raw & 0xFF) as u8;
        data[3] = ((self.pto_speed_raw >> 8) & 0xFF) as u8;
        data[4] = (self.hitch_cmd & 0x03) | ((self.pto_cmd & 0x03) << 2);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_tail(data, 5) {
            return None;
        }
        if data[4] & 0xF0 != 0 {
            return None;
        }
        Some(Self {
            hitch_position: (data[0] as u16) | ((data[1] as u16) << 8),
            pto_speed_raw: (data[2] as u16) | ((data[3] as u16) << 8),
            hitch_cmd: data[4] & 0x03,
            pto_cmd: (data[4] >> 2) & 0x03,
        })
    }
}

/// Hitch roll + pitch command (PGN 0xF100 front / 0xF102 rear).
/// Roll and pitch are unsigned 16-bit raw offsets at 0.0025 % / bit
/// with center at 50 %.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitchRollPitchCmd {
    pub roll_position: u16,
    pub pitch_position: u16,
    /// `true` if for the front hitch (PGN 0xF100), else rear (0xF102).
    pub is_front: bool,
}

impl Default for HitchRollPitchCmd {
    fn default() -> Self {
        Self {
            roll_position: 0xFFFF,
            pitch_position: 0xFFFF,
            is_front: false,
        }
    }
}

impl HitchRollPitchCmd {
    /// Returns the PGN this command should be transmitted on.
    #[must_use]
    pub const fn pgn(&self) -> Pgn {
        if self.is_front {
            PGN_FRONT_HITCH_ROLL_PITCH_CMD
        } else {
            PGN_REAR_HITCH_ROLL_PITCH_CMD
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.roll_position & 0xFF) as u8;
        data[1] = ((self.roll_position >> 8) & 0xFF) as u8;
        data[2] = (self.pitch_position & 0xFF) as u8;
        data[3] = ((self.pitch_position >> 8) & 0xFF) as u8;
        data
    }

    /// Decode from a payload. The caller specifies whether the
    /// payload was received on the front or rear PGN.
    #[must_use]
    pub fn decode(data: &[u8], is_front: bool) -> Option<Self> {
        if !fixed8_with_ff_tail(data, 4) {
            return None;
        }
        Some(Self {
            roll_position: (data[0] as u16) | ((data[1] as u16) << 8),
            pitch_position: (data[2] as u16) | ((data[3] as u16) << 8),
            is_front,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_strategy_round_trip() {
        let m = DriveStrategyCmd {
            mode: DriveStrategyMode::MaxEconomy,
            target_speed_limit_percent: 200,
            target_engine_load_percent: 150,
        };
        let decoded = DriveStrategyCmd::decode(&m.encode()).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn drive_strategy_mode_round_trip() {
        for m in [
            DriveStrategyMode::NoAction,
            DriveStrategyMode::MaxPower,
            DriveStrategyMode::MaxEconomy,
            DriveStrategyMode::MaxSpeed,
            DriveStrategyMode::Reserved,
        ] {
            assert_eq!(DriveStrategyMode::from_u8(m.as_u8()), m);
        }
    }

    #[test]
    fn guidance_system_cmd_round_trip() {
        let m = GuidanceSystemCmd {
            commanded_curvature: -1.5,
            status: CurvatureCommandStatus::IntendedToSteer,
        };
        let decoded = GuidanceSystemCmd::decode(&m.encode()).unwrap();
        assert!((decoded.commanded_curvature - -1.5).abs() < 0.25);
        assert_eq!(decoded.status, CurvatureCommandStatus::IntendedToSteer);
    }

    #[test]
    fn guidance_system_curvature_encode_clamps_instead_of_wrapping() {
        let low = GuidanceSystemCmd {
            commanded_curvature: -1_000_000.0,
            ..Default::default()
        }
        .encode();
        assert_eq!(&low[..2], &[0x00, 0x00]);

        let high = GuidanceSystemCmd {
            commanded_curvature: 1_000_000.0,
            ..Default::default()
        }
        .encode();
        assert_eq!(&high[..2], &[0xFF, 0xFA]);

        let nan = GuidanceSystemCmd {
            commanded_curvature: f64::NAN,
            ..Default::default()
        }
        .encode();
        assert_eq!(&nan[..2], &[0x00, 0x00]);
    }

    #[test]
    fn hitch_pto_combined_round_trip() {
        let m = HitchPtoCombinedCmd {
            hitch_position: 30_000,
            pto_speed_raw: 4320, // 540 rpm × 8 = 4320 raw counts
            hitch_cmd: 1,
            pto_cmd: 1,
        };
        let decoded = HitchPtoCombinedCmd::decode(&m.encode()).unwrap();
        assert_eq!(decoded, m);
        assert!((decoded.pto_speed_rpm() - 540.0).abs() < 0.125);
    }

    #[test]
    fn hitch_roll_pitch_pgn_routing() {
        let front = HitchRollPitchCmd {
            roll_position: 100,
            pitch_position: 200,
            is_front: true,
        };
        let rear = HitchRollPitchCmd {
            is_front: false,
            ..front
        };
        assert_eq!(front.pgn(), PGN_FRONT_HITCH_ROLL_PITCH_CMD);
        assert_eq!(rear.pgn(), PGN_REAR_HITCH_ROLL_PITCH_CMD);
    }

    #[test]
    fn hitch_roll_pitch_round_trip() {
        let m = HitchRollPitchCmd {
            roll_position: 12_345,
            pitch_position: 23_456,
            is_front: true,
        };
        let bytes = m.encode();
        let decoded = HitchRollPitchCmd::decode(&bytes, true).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn fixed_size_decoders_reject_bad_padding_and_reserved_controls() {
        let mut drive_bad_mode = DriveStrategyCmd {
            mode: DriveStrategyMode::MaxEconomy,
            target_speed_limit_percent: 200,
            target_engine_load_percent: 150,
        }
        .encode();
        drive_bad_mode[0] = 0x04;
        assert!(DriveStrategyCmd::decode(&drive_bad_mode).is_none());
        let mut drive_bad_tail = DriveStrategyCmd::default().encode();
        drive_bad_tail[3] = 0x00;
        assert!(DriveStrategyCmd::decode(&drive_bad_tail).is_none());

        let mut guidance_bad_tail = GuidanceSystemCmd {
            commanded_curvature: -1.5,
            status: CurvatureCommandStatus::IntendedToSteer,
        }
        .encode();
        guidance_bad_tail[3] = 0x00;
        assert!(GuidanceSystemCmd::decode(&guidance_bad_tail).is_none());

        let mut guidance_bad_reserved = GuidanceSystemCmd::default().encode();
        guidance_bad_reserved[2] = 0x03;
        assert!(GuidanceSystemCmd::decode(&guidance_bad_reserved).is_none());

        let mut combined_bad_control = HitchPtoCombinedCmd {
            hitch_position: 30_000,
            pto_speed_raw: 4320,
            hitch_cmd: 1,
            pto_cmd: 1,
        }
        .encode();
        combined_bad_control[4] |= 0x10;
        assert!(HitchPtoCombinedCmd::decode(&combined_bad_control).is_none());
        let mut combined_bad_tail = HitchPtoCombinedCmd::default().encode();
        combined_bad_tail[5] = 0x00;
        assert!(HitchPtoCombinedCmd::decode(&combined_bad_tail).is_none());

        let mut roll_pitch_bad_tail = HitchRollPitchCmd {
            roll_position: 12_345,
            pitch_position: 23_456,
            is_front: true,
        }
        .encode();
        roll_pitch_bad_tail[4] = 0x00;
        assert!(HitchRollPitchCmd::decode(&roll_pitch_bad_tail, true).is_none());
    }

    #[test]
    fn short_payloads_return_none() {
        assert!(DriveStrategyCmd::decode(&[0u8; 7]).is_none());
        assert!(GuidanceSystemCmd::decode(&[0u8; 7]).is_none());
        assert!(HitchPtoCombinedCmd::decode(&[0u8; 7]).is_none());
        assert!(HitchRollPitchCmd::decode(&[0u8; 7], false).is_none());
    }

    #[test]
    fn overlong_payloads_return_none() {
        assert!(DriveStrategyCmd::decode(&[0u8; 9]).is_none());
        assert!(GuidanceSystemCmd::decode(&[0u8; 9]).is_none());
        assert!(HitchPtoCombinedCmd::decode(&[0u8; 9]).is_none());
        assert!(HitchRollPitchCmd::decode(&[0u8; 9], false).is_none());
    }
}
