//! ISO 11783-7 G-addendum guidance commands and status.
//!
//! `PGN_GUIDANCE_MACHINE_INFO` (0xAC00) is the steering ECU's 100 ms
//! broadcast. The commanded direction travels on `GuidanceSystemCmd`
//! (PGN 0xAD00, in `drive_strategy.rs`); both share the curvature scaling
//! rule (0.25 km⁻¹/bit, offset −8032).

use crate::isobus::implement::Signal;

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

/// `true` if `curvature_per_km` lies within the encodable range, i.e. encoding
/// it will not silently clamp the value. A non-finite value is out of range
/// (it encodes as not-available).
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

/// Band SPN 1817 per ISO 11783-7 §5.2.4 Table 1 (16-bit row).
#[inline]
fn decode_curvature(raw: u16) -> Signal<f64> {
    match raw {
        0..=CURVATURE_MAX_RAW => {
            Signal::Value(f64::from(raw) * CURVATURE_RESOLUTION_PER_KM - CURVATURE_OFFSET_PER_KM)
        }
        0xFE00..=0xFEFF => Signal::Error,
        // Includes the reserved band: nothing meaningful to report.
        _ => Signal::NotAvailable,
    }
}

/// A fixed 8-byte payload. The trailing undefined bytes are deliberately *not*
/// checked: ISO 11783-7 §5.4 makes undefined bits and bytes "don't care" on
/// receive so they can be assigned in a later revision without breaking
/// deployed receivers. `used` is kept for documentation of the defined width.
fn fixed8_with_ff_tail(data: &[u8], used: usize) -> bool {
    let _ = used;
    data.len() == 8
}

/// Agricultural guidance machine info (PGN 0xAC00) — steering ECU broadcast.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuidanceMachineInfo {
    /// 1/km, 0.25/km per bit offset −8032.
    /// SPN 1817 estimated curvature.
    ///
    /// This used to be a plain `f64` and a not-available reading dropped the
    /// **whole** PG — so the lockout, readiness, limit status and exit reason
    /// went with it, and a steering ECU that simply was not steering read as a
    /// dead link (G4).
    ///
    /// It is a `Signal` rather than an `Option` because ISO 11783-7 §5.2.4
    /// keeps two outcomes apart that an `Option` collapses: the error band
    /// (`FE00..=FEFF`) is the ECU *declaring a sensor or sub-system fault*,
    /// while the not-available band (`FF00..=FFFF`) is an idle ECU that simply
    /// does not populate the parameter. An autonomy supervisor has to treat a
    /// faulted steering sensor differently from an idle one.
    pub estimated_curvature: Signal<f64>,
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
            estimated_curvature: Signal::NotAvailable,
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
        // Round-trip the distinction: a declared fault must not read back as
        // an idle ECU.
        let raw = match self.estimated_curvature {
            Signal::Value(v) => encode_curvature(v),
            Signal::Error => 0xFEFF,
            Signal::NotAvailable => CURVATURE_NOT_AVAILABLE_RAW,
        };
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
        // Byte 3 bits 0..4 are reserved. Conformant ECUs transmit reserved bits
        // as 1 (J1939 convention), so a real frame has byte 3 = 0xFF. Ignore
        // these bits rather than rejecting the frame — only bits 5..7 (guidance
        // limit status) carry meaning here.
        let raw = (data[0] as u16) | ((data[1] as u16) << 8);
        Some(Self {
            estimated_curvature: decode_curvature(raw),
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn decodes_real_captured_machine_info_with_reserved_bits_set() {
        // Real Agricultural Guidance Machine Info frames (PGN 0xAC00) captured
        // off a tractor's ISOBUS. Reserved bits in bytes 3..7 are transmitted
        // as 1 (byte 3 = 0xFF), per J1939 convention. The decoder must accept
        // these frames — rejecting on the reserved bits blinds the guidance
        // plugin to every real machine-info message.
        let frame = [0x64, 0x7D, 0x3C, 0xFF, 0xC0, 0xFF, 0xFF, 0xFF];
        let info =
            GuidanceMachineInfo::decode(&frame).expect("a real captured GMS frame must decode");
        assert!(info.estimated_curvature.value().is_some_and(|k| (k + 7.0).abs() < 0.25));
        assert_eq!(info.lockout, MechanicalLockout::NotActive);
        assert_eq!(
            info.guidance_limit_status,
            GuidanceLimitStatus::NotAvailable
        );
    }

    #[test]
    fn machine_info_round_trip() {
        let m = GuidanceMachineInfo {
            estimated_curvature: Signal::Value(-2.5),
            lockout: MechanicalLockout::Active,
            steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
            steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
            request_reset_status: RequestResetCommandStatus::ResetRequired,
            guidance_limit_status: GuidanceLimitStatus::LimitedLow,
            guidance_system_command_exit_reason_code: 27,
            remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
        };
        let decoded = GuidanceMachineInfo::decode(&m.encode()).unwrap();
        assert!(
            decoded
                .estimated_curvature
                .value()
                .is_some_and(|k| (k + 2.5).abs() < 0.25)
        );
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
    fn short_payload_returns_none() {
        assert!(GuidanceMachineInfo::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn overlong_payload_returns_none() {
        assert!(GuidanceMachineInfo::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn fixed_size_decoders_reject_bad_padding_and_reserved_controls() {
        let mut machine_bad_tail = GuidanceMachineInfo {
            estimated_curvature: Signal::Value(-2.5),
            lockout: MechanicalLockout::Active,
            steering_system_readiness_state: GenericSaeBs02SlotValue::EnabledOnActive,
            steering_input_position_status: GenericSaeBs02SlotValue::DisabledOffPassive,
            request_reset_status: RequestResetCommandStatus::ResetRequired,
            guidance_limit_status: GuidanceLimitStatus::LimitedLow,
            guidance_system_command_exit_reason_code: 27,
            remote_engage_switch_status: GenericSaeBs02SlotValue::EnabledOnActive,
        }
        .encode();
        // B6 / G3 — ISO 11783-7 §5.4: undefined trailing bytes are "don't care"
        // on receive. This used to assert rejection.
        machine_bad_tail[5] = 0x00;
        assert!(GuidanceMachineInfo::decode(&machine_bad_tail).is_some());

        // Reserved bits in byte 3 set to 1 (as conformant ECUs transmit them)
        // must be ignored, not rejected — the frame still decodes.
        let mut machine_reserved_set = machine_bad_tail;
        machine_reserved_set[5] = 0xFF;
        machine_reserved_set[3] |= 0x01;
        let decoded = GuidanceMachineInfo::decode(&machine_reserved_set)
            .expect("reserved bits set to 1 must not reject the frame");
        assert_eq!(
            decoded.guidance_limit_status,
            GuidanceLimitStatus::LimitedLow
        );
    }

    #[test]
    fn curvature_encoding_clamps_and_rejects_not_available_sentinel() {
        let high = GuidanceMachineInfo {
            estimated_curvature: Signal::Value(1.0e9),
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
            Signal::Value(CURVATURE_MAX_PER_KM)
        );
    }

    #[test]
    fn enums_round_trip() {
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

    /// H16/H63 — a steering system reporting its estimated curvature as
    /// not-available is a normal state when it is not steering. Propagating
    /// that with `?` dropped the **whole** PG, taking the mechanical lockout,
    /// readiness, limit status and exit reason with it — so a steering fault
    /// read as silence and the plugin saw a dead link instead of a report (G4).
    #[test]
    fn a_not_available_curvature_does_not_cost_the_rest_of_the_report() {
        let mut data = [0xFFu8; 8];
        data[0] = 0xFF;
        data[1] = 0xFF; // curvature = not available
        data[2] = 0b01_01_01_01;
        data[3] = 0xFF; // limit status in bits 5..7 = 7
        data[4] = 0x0A;

        let info = GuidanceMachineInfo::decode(&data)
            .expect("a not-available curvature must not drop the whole PG");
        assert_eq!(info.estimated_curvature, Signal::NotAvailable, "reported as absent");
        assert_eq!(info.lockout, MechanicalLockout::try_from_u8(0x01).unwrap());
        assert_eq!(info.guidance_system_command_exit_reason_code, 0x0A);

        // And it survives a round trip as absent rather than becoming a value.
        let reencoded = info.encode();
        assert_eq!(
            GuidanceMachineInfo::decode(&reencoded)
                .unwrap()
                .estimated_curvature,
            Signal::NotAvailable
        );

        // C6 — the error band is a *declared fault*, not the same thing as an
        // idle ECU that does not populate the parameter (§5.2.4). Collapsing
        // both into `None` made a failed steering sensor invisible.
        let mut faulted = data;
        faulted[0..2].copy_from_slice(&0xFEFFu16.to_le_bytes());
        let info = GuidanceMachineInfo::decode(&faulted).expect("still decodes");
        assert_eq!(info.estimated_curvature, Signal::Error);
        assert_eq!(
            GuidanceMachineInfo::decode(&info.encode())
                .unwrap()
                .estimated_curvature,
            Signal::Error,
            "a declared fault must survive a round trip as a fault"
        );
    }
}
