//! ISO 11783-7 G-addendum guidance commands and status.
//!
//! Mirrors the C++ `machbus::isobus::implement::guidance.hpp`. Three
//! 100 ms messages share the curvature scaling rule (0.25 km⁻¹/bit,
//! offset −8032):
//!
//! - `PGN_GUIDANCE_CURVATURE_CMD` (0xFE46) — controller → steering system
//! - `PGN_GUIDANCE_MACHINE_INFO` (0xAC00) — agricultural guidance machine info
//! - `PGN_GUIDANCE_SYSTEM` (0xFE45) — controller status (readiness)
//!
//! The C++ `GuidanceCurvatureInterface` (IsoNet-coupled) is not
//! ported.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SteeringReadiness {
    NotReady = 0,
    MechanicalReady = 1,
    FullyReady = 2,
    Error = 3,
    #[default]
    NotAvailable = 7,
}

impl SteeringReadiness {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            0 => Self::NotReady,
            1 => Self::MechanicalReady,
            2 => Self::FullyReady,
            3 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotReady),
            1 => Some(Self::MechanicalReady),
            2 => Some(Self::FullyReady),
            3 => Some(Self::Error),
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
pub enum MechanicalLockout {
    NotActive = 0,
    Active = 1,
    Error = 2,
    #[default]
    NotAvailable = 3,
}

impl MechanicalLockout {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::NotActive,
            1 => Self::Active,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotActive),
            1 => Some(Self::Active),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum RequestResetCommandStatus {
    ResetNotRequired = 0,
    ResetRequired = 1,
    Error = 2,
    #[default]
    NotAvailable = 3,
}

impl RequestResetCommandStatus {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::ResetNotRequired,
            1 => Self::ResetRequired,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::ResetNotRequired),
            1 => Some(Self::ResetRequired),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum GenericSaeBs02SlotValue {
    DisabledOffPassive = 0,
    EnabledOnActive = 1,
    ErrorIndication = 2,
    #[default]
    NotAvailableTakeNoAction = 3,
}

impl GenericSaeBs02SlotValue {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::DisabledOffPassive,
            1 => Self::EnabledOnActive,
            2 => Self::ErrorIndication,
            _ => Self::NotAvailableTakeNoAction,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::DisabledOffPassive),
            1 => Some(Self::EnabledOnActive),
            2 => Some(Self::ErrorIndication),
            3 => Some(Self::NotAvailableTakeNoAction),
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
pub enum GuidanceLimitStatus {
    NotLimited = 0,
    OperatorLimitedControlled = 1,
    LimitedHigh = 2,
    LimitedLow = 3,
    Reserved1 = 4,
    Reserved2 = 5,
    NonRecoverableFault = 6,
    #[default]
    NotAvailable = 7,
}

impl GuidanceLimitStatus {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            0 => Self::NotLimited,
            1 => Self::OperatorLimitedControlled,
            2 => Self::LimitedHigh,
            3 => Self::LimitedLow,
            4 => Self::Reserved1,
            5 => Self::Reserved2,
            6 => Self::NonRecoverableFault,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NotLimited),
            1 => Some(Self::OperatorLimitedControlled),
            2 => Some(Self::LimitedHigh),
            3 => Some(Self::LimitedLow),
            6 => Some(Self::NonRecoverableFault),
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

/// Encode a curvature value to its 16-bit raw form
/// (0.25 km⁻¹ per bit, offset −8032).
/// Minimum commandable curvature (1/km) before the wire encoder clamps.
pub const CURVATURE_MIN_PER_KM: f64 = -8032.0;
/// Maximum commandable curvature (1/km) before the wire encoder clamps.
pub const CURVATURE_MAX_PER_KM: f64 = 8031.75;
const CURVATURE_OFFSET_PER_KM: f64 = 8032.0;
const CURVATURE_RESOLUTION_PER_KM: f64 = 0.25;
const CURVATURE_NOT_AVAILABLE_RAW: u16 = 0xFFFF;
const CURVATURE_MAX_RAW: u16 = 0xFAFF;

/// `true` if `curvature_per_km` lies within the encodable range, i.e. building a
/// [`CurvatureCommand`] with it will not silently clamp the value. A non-finite
/// value is out of range (it encodes as not-available).
#[must_use]
pub fn curvature_within_range(curvature_per_km: f64) -> bool {
    curvature_per_km.is_finite()
        && (CURVATURE_MIN_PER_KM..=CURVATURE_MAX_PER_KM).contains(&curvature_per_km)
}

#[inline]
fn encode_curvature(curvature_per_km: f64) -> u16 {
    if !curvature_per_km.is_finite() {
        return CURVATURE_NOT_AVAILABLE_RAW;
    }
    let clamped = curvature_per_km.clamp(CURVATURE_MIN_PER_KM, CURVATURE_MAX_PER_KM);
    ((clamped + CURVATURE_OFFSET_PER_KM) / CURVATURE_RESOLUTION_PER_KM) as u16
}

#[inline]
fn decode_curvature(raw: u16) -> Option<f64> {
    if raw == CURVATURE_NOT_AVAILABLE_RAW || raw > CURVATURE_MAX_RAW {
        None
    } else {
        Some(raw as f64 * CURVATURE_RESOLUTION_PER_KM - CURVATURE_OFFSET_PER_KM)
    }
}

fn fixed8_with_ff_tail(data: &[u8], used: usize) -> bool {
    data.len() == 8 && data[used..].iter().all(|&byte| byte == 0xFF)
}

/// Curvature command (PGN 0xFE46) — guidance controller → steering
/// system. Curvature in 1/km.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CurvatureCommand {
    pub curvature: f64,
    /// Reserved for implementation use. Not standardized on the wire.
    pub curvature_rate: f64,
}

impl CurvatureCommand {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let raw = encode_curvature(self.curvature);
        data[0] = (raw & 0xFF) as u8;
        data[1] = ((raw >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_tail(data, 2) {
            return None;
        }
        let raw = (data[0] as u16) | ((data[1] as u16) << 8);
        Some(Self {
            curvature: decode_curvature(raw)?,
            curvature_rate: 0.0,
        })
    }
}

/// Agricultural guidance machine info (PGN 0xAC00) — steering ECU broadcast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuidanceMachineInfo {
    /// 1/km, 0.25/km per bit offset −8032.
    pub estimated_curvature: f64,
    /// SPN 5243, bits 0..1 of byte 2.
    pub lockout: MechanicalLockout,
    /// SPN 5242, bits 2..3 of byte 2.
    pub steering_system_readiness_state: GenericSaeBs02SlotValue,
    /// SPN 5241, bits 4..5 of byte 2.
    pub steering_input_position_status: GenericSaeBs02SlotValue,
    /// SPN 5240, bits 6..7 of byte 2.
    pub request_reset_status: RequestResetCommandStatus,
    /// SPN 5726, bits 5..7 of byte 3.
    pub guidance_limit_status: GuidanceLimitStatus,
    /// SPN 5725, bits 0..5 of byte 4.
    pub guidance_system_command_exit_reason_code: u8,
    /// SPN 9726, bits 6..7 of byte 4.
    pub remote_engage_switch_status: GenericSaeBs02SlotValue,
}

impl Default for GuidanceMachineInfo {
    fn default() -> Self {
        Self {
            estimated_curvature: 0.0,
            lockout: MechanicalLockout::NotAvailable,
            steering_system_readiness_state: GenericSaeBs02SlotValue::NotAvailableTakeNoAction,
            steering_input_position_status: GenericSaeBs02SlotValue::NotAvailableTakeNoAction,
            request_reset_status: RequestResetCommandStatus::NotAvailable,
            guidance_limit_status: GuidanceLimitStatus::NotAvailable,
            guidance_system_command_exit_reason_code: 0x3F,
            remote_engage_switch_status: GenericSaeBs02SlotValue::NotAvailableTakeNoAction,
        }
    }
}

impl GuidanceMachineInfo {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let raw = encode_curvature(self.estimated_curvature);
        data[0] = (raw & 0xFF) as u8;
        data[1] = ((raw >> 8) & 0xFF) as u8;
        data[2] = (self.lockout.as_u8() & 0x03)
            | ((self.steering_system_readiness_state.as_u8() & 0x03) << 2)
            | ((self.steering_input_position_status.as_u8() & 0x03) << 4)
            | ((self.request_reset_status.as_u8() & 0x03) << 6);
        data[3] = (self.guidance_limit_status.as_u8() & 0x07) << 5;
        data[4] = (self.guidance_system_command_exit_reason_code & 0x3F)
            | ((self.remote_engage_switch_status.as_u8() & 0x03) << 6);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_tail(data, 5) {
            return None;
        }
        if data[3] & 0x1F != 0 {
            return None;
        }
        let raw = (data[0] as u16) | ((data[1] as u16) << 8);
        Some(Self {
            estimated_curvature: decode_curvature(raw)?,
            lockout: MechanicalLockout::try_from_u8(data[2] & 0x03)?,
            steering_system_readiness_state: GenericSaeBs02SlotValue::try_from_u8(
                (data[2] >> 2) & 0x03,
            )?,
            steering_input_position_status: GenericSaeBs02SlotValue::try_from_u8(
                (data[2] >> 4) & 0x03,
            )?,
            request_reset_status: RequestResetCommandStatus::try_from_u8(data[2] >> 6)?,
            guidance_limit_status: GuidanceLimitStatus::try_from_u8(data[3] >> 5)?,
            guidance_system_command_exit_reason_code: data[4] & 0x3F,
            remote_engage_switch_status: GenericSaeBs02SlotValue::try_from_u8(data[4] >> 6)?,
        })
    }
}

/// System status (PGN 0xFE45) — guidance controller broadcast.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GuidanceSystemStatus {
    pub estimated_curvature: f64,
    pub readiness: SteeringReadiness,
    /// 2 bits: steering integrity (0 = lowest, 3 = N/A).
    pub integrity_level: u8,
}

impl GuidanceSystemStatus {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let raw = encode_curvature(self.estimated_curvature);
        data[0] = (raw & 0xFF) as u8;
        data[1] = ((raw >> 8) & 0xFF) as u8;
        data[2] = (self.readiness.as_u8() & 0x07) | ((self.integrity_level & 0x03) << 4);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_tail(data, 3) {
            return None;
        }
        if data[2] & 0xC8 != 0 {
            return None;
        }
        let raw = (data[0] as u16) | ((data[1] as u16) << 8);
        Some(Self {
            estimated_curvature: decode_curvature(raw)?,
            readiness: SteeringReadiness::try_from_u8(data[2] & 0x07)?,
            integrity_level: (data[2] >> 4) & 0x03,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curvature_command_round_trip() {
        let m = CurvatureCommand {
            curvature: 0.5, // 1/km
            curvature_rate: 0.0,
        };
        let decoded = CurvatureCommand::decode(&m.encode()).unwrap();
        assert!((decoded.curvature - 0.5).abs() < 0.25);
    }

    #[test]
    fn curvature_range_check_flags_out_of_range_and_nonfinite() {
        assert!(curvature_within_range(0.0));
        assert!(curvature_within_range(CURVATURE_MIN_PER_KM));
        assert!(curvature_within_range(CURVATURE_MAX_PER_KM));
        // Beyond the encodable range ⇒ would be silently clamped.
        assert!(!curvature_within_range(CURVATURE_MAX_PER_KM + 1.0));
        assert!(!curvature_within_range(CURVATURE_MIN_PER_KM - 1.0));
        assert!(!curvature_within_range(f64::NAN));
        assert!(!curvature_within_range(f64::INFINITY));
    }

    #[test]
    fn machine_info_round_trip() {
        let m = GuidanceMachineInfo {
            estimated_curvature: -2.5,
            lockout: MechanicalLockout::Active,
            steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
            steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
            request_reset_status: RequestResetCommandStatus::ResetRequired,
            guidance_limit_status: GuidanceLimitStatus::LimitedLow,
            guidance_system_command_exit_reason_code: 27,
            remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
        };
        let decoded = GuidanceMachineInfo::decode(&m.encode()).unwrap();
        assert!((decoded.estimated_curvature - -2.5).abs() < 0.25);
        assert_eq!(decoded.lockout, MechanicalLockout::Active);
        assert_eq!(
            decoded.steering_system_readiness_state,
            GenericSaeBs02SlotValue::EnabledOnActive
        );
        assert_eq!(
            decoded.request_reset_status,
            RequestResetCommandStatus::ResetRequired
        );
        assert_eq!(
            decoded.guidance_limit_status,
            GuidanceLimitStatus::LimitedLow
        );
        assert_eq!(decoded.guidance_system_command_exit_reason_code, 27);
    }

    #[test]
    fn system_status_round_trip() {
        let m = GuidanceSystemStatus {
            estimated_curvature: 1.0,
            readiness: SteeringReadiness::FullyReady,
            integrity_level: 2,
        };
        let decoded = GuidanceSystemStatus::decode(&m.encode()).unwrap();
        assert_eq!(decoded.readiness, SteeringReadiness::FullyReady);
        assert_eq!(decoded.integrity_level, 2);
    }

    #[test]
    fn short_payload_returns_none() {
        assert!(CurvatureCommand::decode(&[0u8; 7]).is_none());
        assert!(GuidanceMachineInfo::decode(&[0u8; 7]).is_none());
        assert!(GuidanceSystemStatus::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn overlong_payload_returns_none() {
        assert!(CurvatureCommand::decode(&[0u8; 9]).is_none());
        assert!(GuidanceMachineInfo::decode(&[0u8; 9]).is_none());
        assert!(GuidanceSystemStatus::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn fixed_size_decoders_reject_bad_padding_and_reserved_controls() {
        let mut curvature_bad_tail = CurvatureCommand {
            curvature: 0.5,
            curvature_rate: 0.0,
        }
        .encode();
        curvature_bad_tail[2] = 0x00;
        assert!(CurvatureCommand::decode(&curvature_bad_tail).is_none());

        let mut machine_bad_tail = GuidanceMachineInfo {
            estimated_curvature: -2.5,
            lockout: MechanicalLockout::Active,
            steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
            steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
            request_reset_status: RequestResetCommandStatus::ResetRequired,
            guidance_limit_status: GuidanceLimitStatus::LimitedLow,
            guidance_system_command_exit_reason_code: 27,
            remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
        }
        .encode();
        machine_bad_tail[5] = 0x00;
        assert!(GuidanceMachineInfo::decode(&machine_bad_tail).is_none());

        let mut machine_bad_reserved = machine_bad_tail;
        machine_bad_reserved[5] = 0xFF;
        machine_bad_reserved[3] |= 0x01;
        assert!(GuidanceMachineInfo::decode(&machine_bad_reserved).is_none());

        let mut status_bad_tail = GuidanceSystemStatus {
            estimated_curvature: 1.0,
            readiness: SteeringReadiness::FullyReady,
            integrity_level: 2,
        }
        .encode();
        status_bad_tail[3] = 0x00;
        assert!(GuidanceSystemStatus::decode(&status_bad_tail).is_none());

        let mut status_reserved_readiness = status_bad_tail;
        status_reserved_readiness[3] = 0xFF;
        status_reserved_readiness[2] = (2 << 4) | 4;
        assert!(GuidanceSystemStatus::decode(&status_reserved_readiness).is_none());

        let mut status_reserved_bits = status_reserved_readiness;
        status_reserved_bits[2] = (2 << 4) | 2 | 0x08;
        assert!(GuidanceSystemStatus::decode(&status_reserved_bits).is_none());
    }

    #[test]
    fn curvature_encoding_clamps_and_rejects_not_available_sentinel() {
        let low = CurvatureCommand {
            curvature: f64::NEG_INFINITY,
            curvature_rate: 0.0,
        }
        .encode();
        assert_eq!(&low[..2], &CURVATURE_NOT_AVAILABLE_RAW.to_le_bytes());
        assert!(CurvatureCommand::decode(&low).is_none());

        let high = GuidanceMachineInfo {
            estimated_curvature: 1.0e9,
            lockout: MechanicalLockout::NotActive,
            steering_system_readiness_state: GenericSaeBs02SlotValue::DisabledOffPassive,
            steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
            request_reset_status: RequestResetCommandStatus::ResetNotRequired,
            guidance_limit_status: GuidanceLimitStatus::NotLimited,
            guidance_system_command_exit_reason_code: 0,
            remote_engage_switch_status: GenericSaeBs02SlotValue::DisabledOffPassive,
        }
        .encode();
        assert_eq!(
            GuidanceMachineInfo::decode(&high)
                .unwrap()
                .estimated_curvature,
            CURVATURE_MAX_PER_KM
        );

        let low = GuidanceSystemStatus {
            estimated_curvature: -1.0e9,
            readiness: SteeringReadiness::FullyReady,
            integrity_level: 0,
        }
        .encode();
        assert_eq!(
            GuidanceSystemStatus::decode(&low)
                .unwrap()
                .estimated_curvature,
            CURVATURE_MIN_PER_KM
        );
    }

    #[test]
    fn enums_round_trip() {
        for r in [
            SteeringReadiness::NotReady,
            SteeringReadiness::MechanicalReady,
            SteeringReadiness::FullyReady,
            SteeringReadiness::Error,
            SteeringReadiness::NotAvailable,
        ] {
            assert_eq!(SteeringReadiness::from_u8(r.as_u8()), r);
        }
        for m in [
            MechanicalLockout::NotActive,
            MechanicalLockout::Active,
            MechanicalLockout::Error,
            MechanicalLockout::NotAvailable,
        ] {
            assert_eq!(MechanicalLockout::from_u8(m.as_u8()), m);
        }
        for v in [
            GenericSaeBs02SlotValue::DisabledOffPassive,
            GenericSaeBs02SlotValue::EnabledOnActive,
            GenericSaeBs02SlotValue::ErrorIndication,
            GenericSaeBs02SlotValue::NotAvailableTakeNoAction,
        ] {
            assert_eq!(GenericSaeBs02SlotValue::from_u8(v.as_u8()), v);
        }
        for r in [
            RequestResetCommandStatus::ResetNotRequired,
            RequestResetCommandStatus::ResetRequired,
            RequestResetCommandStatus::Error,
            RequestResetCommandStatus::NotAvailable,
        ] {
            assert_eq!(RequestResetCommandStatus::from_u8(r.as_u8()), r);
        }
        for l in [
            GuidanceLimitStatus::NotLimited,
            GuidanceLimitStatus::OperatorLimitedControlled,
            GuidanceLimitStatus::LimitedHigh,
            GuidanceLimitStatus::LimitedLow,
            GuidanceLimitStatus::NonRecoverableFault,
            GuidanceLimitStatus::NotAvailable,
        ] {
            assert_eq!(GuidanceLimitStatus::from_u8(l.as_u8()), l);
            assert_eq!(GuidanceLimitStatus::try_from_u8(l.as_u8()), Some(l));
        }
        for reserved in [
            GuidanceLimitStatus::Reserved1,
            GuidanceLimitStatus::Reserved2,
        ] {
            assert_eq!(GuidanceLimitStatus::from_u8(reserved.as_u8()), reserved);
            assert_eq!(GuidanceLimitStatus::try_from_u8(reserved.as_u8()), None);
        }
    }
}
