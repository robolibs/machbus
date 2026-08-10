//! AEF 023 TIM function identifiers and exit/reason codes (Annexes A.2.4, D).
//!
//! Neither existed in the crate. Function IDs are how a TIM message says which
//! of the machine's functions it is about, and the exit/reason code is how a
//! server explains why it will not accept — or has stopped accepting — remote
//! commands. Without them a client can see that automation stopped but not why,
//! which is the difference between "the operator took over" and "your command
//! was out of range".

/// A TIM function identifier (Table 8, A.2.4).
///
/// Auxiliary valves occupy `0x01..=0x20` individually, with `0x00` addressing
/// the whole set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TimFunctionId {
    /// Addresses every auxiliary valve at once.
    AuxValveSuperset,
    /// Auxiliary valve 1..=32.
    AuxValve(u8),
    FrontPto,
    RearPto,
    FrontHitch,
    RearHitch,
    VehicleSpeed,
    ExternalGuidance,
}

impl TimFunctionId {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0x00 => Some(Self::AuxValveSuperset),
            0x01..=0x20 => Some(Self::AuxValve(raw)),
            0x40 => Some(Self::FrontPto),
            0x41 => Some(Self::RearPto),
            0x42 => Some(Self::FrontHitch),
            0x43 => Some(Self::RearHitch),
            0x44 => Some(Self::VehicleSpeed),
            // 0x45 is reserved.
            0x46 => Some(Self::ExternalGuidance),
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::AuxValveSuperset => 0x00,
            Self::AuxValve(n) => n,
            Self::FrontPto => 0x40,
            Self::RearPto => 0x41,
            Self::FrontHitch => 0x42,
            Self::RearHitch => 0x43,
            Self::VehicleSpeed => 0x44,
            Self::ExternalGuidance => 0x46,
        }
    }

    /// The two functions an autonomous driving client needs: steering and
    /// speed. Both must be assigned before a machine can be driven remotely.
    #[must_use]
    pub const fn is_driving_function(self) -> bool {
        matches!(self, Self::ExternalGuidance | Self::VehicleSpeed)
    }
}

/// Why a TIM function will not accept, or has stopped accepting, remote
/// commands (D.7.2.2 and its per-function equivalents). A **5-bit** field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum TimExitReason {
    /// No reason — all clear.
    #[default]
    AllClear = 0x00,
    /// The required level of operator presence or awareness was not detected.
    OperatorPresenceNotDetected = 0x01,
    /// The implement released control of the function.
    ImplementReleasedControl = 0x02,
    /// The operator overrode the function.
    OperatorOverride = 0x03,
    /// An operator control is not in a valid position.
    OperatorControlNotInValidPosition = 0x04,
    /// The remote command stopped arriving within its timeout.
    RemoteCommandTimeout = 0x05,
    /// The commanded value was out of range or otherwise invalid.
    RemoteCommandOutOfRange = 0x06,
    /// The function is not calibrated.
    FunctionNotCalibrated = 0x07,
    /// An operator control has faulted.
    OperatorControlFault = 0x08,
    /// The function itself has faulted.
    FunctionFault = 0x09,
    /// The control unit is in diagnostic mode.
    ControlUnitInDiagnosticMode = 0x0F,
    /// Another guidance system has taken the machine.
    AlternateGuidanceSystemActive = 0x10,
    VehicleSpeedTooHigh = 0x11,
    VehicleSpeedTooLow = 0x12,
    /// The transmission gear does not allow remote commands (park, etc.).
    TransmissionGearDisallows = 0x13,
    HydraulicOilTemperatureTooLow = 0x14,
    HydraulicOilLevelTooLow = 0x15,
    ManufacturerSpecific = 0x1E,
    Error = 0x1F,
}

impl TimExitReason {
    #[must_use]
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw & 0x1F {
            0x00 => Some(Self::AllClear),
            0x01 => Some(Self::OperatorPresenceNotDetected),
            0x02 => Some(Self::ImplementReleasedControl),
            0x03 => Some(Self::OperatorOverride),
            0x04 => Some(Self::OperatorControlNotInValidPosition),
            0x05 => Some(Self::RemoteCommandTimeout),
            0x06 => Some(Self::RemoteCommandOutOfRange),
            0x07 => Some(Self::FunctionNotCalibrated),
            0x08 => Some(Self::OperatorControlFault),
            0x09 => Some(Self::FunctionFault),
            0x0F => Some(Self::ControlUnitInDiagnosticMode),
            0x10 => Some(Self::AlternateGuidanceSystemActive),
            0x11 => Some(Self::VehicleSpeedTooHigh),
            0x12 => Some(Self::VehicleSpeedTooLow),
            0x13 => Some(Self::TransmissionGearDisallows),
            0x14 => Some(Self::HydraulicOilTemperatureTooLow),
            0x15 => Some(Self::HydraulicOilLevelTooLow),
            0x1E => Some(Self::ManufacturerSpecific),
            0x1F => Some(Self::Error),
            // 0x0A..=0x0E and 0x16..=0x1D are reserved.
            _ => None,
        }
    }

    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// `true` when the operator, rather than a fault, ended the automation.
    /// These call for handing control back quietly; a fault does not.
    #[must_use]
    pub const fn is_operator_initiated(self) -> bool {
        matches!(
            self,
            Self::OperatorPresenceNotDetected
                | Self::OperatorOverride
                | Self::OperatorControlNotInValidPosition
        )
    }

    /// `true` when the client's own behaviour caused the exit, so retrying the
    /// same command unchanged will fail the same way.
    #[must_use]
    pub const fn is_client_fault(self) -> bool {
        matches!(
            self,
            Self::RemoteCommandTimeout | Self::RemoteCommandOutOfRange
        )
    }

    /// A short, stable identifier for logs and bindings.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AllClear => "all_clear",
            Self::OperatorPresenceNotDetected => "operator_presence_not_detected",
            Self::ImplementReleasedControl => "implement_released_control",
            Self::OperatorOverride => "operator_override",
            Self::OperatorControlNotInValidPosition => "operator_control_not_in_valid_position",
            Self::RemoteCommandTimeout => "remote_command_timeout",
            Self::RemoteCommandOutOfRange => "remote_command_out_of_range",
            Self::FunctionNotCalibrated => "function_not_calibrated",
            Self::OperatorControlFault => "operator_control_fault",
            Self::FunctionFault => "function_fault",
            Self::ControlUnitInDiagnosticMode => "control_unit_in_diagnostic_mode",
            Self::AlternateGuidanceSystemActive => "alternate_guidance_system_active",
            Self::VehicleSpeedTooHigh => "vehicle_speed_too_high",
            Self::VehicleSpeedTooLow => "vehicle_speed_too_low",
            Self::TransmissionGearDisallows => "transmission_gear_disallows",
            Self::HydraulicOilTemperatureTooLow => "hydraulic_oil_temperature_too_low",
            Self::HydraulicOilLevelTooLow => "hydraulic_oil_level_too_low",
            Self::ManufacturerSpecific => "manufacturer_specific",
            Self::Error => "error",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_ids_match_table_8() {
        assert_eq!(
            TimFunctionId::from_u8(0x00),
            Some(TimFunctionId::AuxValveSuperset)
        );
        assert_eq!(
            TimFunctionId::from_u8(0x01),
            Some(TimFunctionId::AuxValve(1))
        );
        assert_eq!(
            TimFunctionId::from_u8(0x20),
            Some(TimFunctionId::AuxValve(0x20))
        );
        assert_eq!(TimFunctionId::from_u8(0x40), Some(TimFunctionId::FrontPto));
        assert_eq!(
            TimFunctionId::from_u8(0x44),
            Some(TimFunctionId::VehicleSpeed)
        );
        assert_eq!(
            TimFunctionId::from_u8(0x46),
            Some(TimFunctionId::ExternalGuidance)
        );

        // Reserved bands stay reserved, including 0x45 between speed and guidance.
        for reserved in [0x21u8, 0x3F, 0x45, 0x47, 0xF3, 0xFF] {
            assert_eq!(TimFunctionId::from_u8(reserved), None, "{reserved:#04X}");
        }
    }

    #[test]
    fn function_ids_round_trip() {
        for raw in (0x00u8..=0x20).chain([0x40, 0x41, 0x42, 0x43, 0x44, 0x46]) {
            let id = TimFunctionId::from_u8(raw).expect("assigned");
            assert_eq!(id.as_u8(), raw);
        }
    }

    #[test]
    fn steering_and_speed_are_the_driving_pair() {
        assert!(TimFunctionId::ExternalGuidance.is_driving_function());
        assert!(TimFunctionId::VehicleSpeed.is_driving_function());
        assert!(!TimFunctionId::RearHitch.is_driving_function());
        assert!(!TimFunctionId::AuxValve(3).is_driving_function());
    }

    #[test]
    fn exit_reasons_cover_the_five_bit_table() {
        // The codes the audit called out as unnamed anywhere in the crate.
        assert_eq!(
            TimExitReason::from_u8(0x05),
            Some(TimExitReason::RemoteCommandTimeout)
        );
        assert_eq!(
            TimExitReason::from_u8(0x10),
            Some(TimExitReason::AlternateGuidanceSystemActive)
        );
        assert_eq!(
            TimExitReason::from_u8(0x13),
            Some(TimExitReason::TransmissionGearDisallows)
        );

        // Reserved bands.
        for reserved in [0x0Au8, 0x0E, 0x16, 0x1D] {
            assert_eq!(TimExitReason::from_u8(reserved), None, "{reserved:#04X}");
        }

        // It is 5 bits: anything above 0x1F is masked, not accepted verbatim.
        assert_eq!(
            TimExitReason::from_u8(0xE5),
            Some(TimExitReason::RemoteCommandTimeout)
        );
    }

    #[test]
    fn operator_exits_are_distinguishable_from_client_faults() {
        assert!(TimExitReason::OperatorOverride.is_operator_initiated());
        assert!(!TimExitReason::OperatorOverride.is_client_fault());

        // Retrying the same command after these will fail identically.
        assert!(TimExitReason::RemoteCommandTimeout.is_client_fault());
        assert!(TimExitReason::RemoteCommandOutOfRange.is_client_fault());
        assert!(!TimExitReason::RemoteCommandTimeout.is_operator_initiated());

        // A machine fault is neither.
        assert!(!TimExitReason::FunctionFault.is_operator_initiated());
        assert!(!TimExitReason::FunctionFault.is_client_fault());
    }
}
