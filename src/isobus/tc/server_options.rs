//! ISO 11783-10 Task Controller server enumerations.
//!
//! Mirrors the C++ `machbus::isobus::tc::server_options.hpp`.

use crate::net::Priority;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ObjectPoolActivationError {
    #[default]
    NoErrors = 0x00,
    ThereAreErrorsInTheDDOP = 0x01,
    TaskControllerRanOutOfMemoryDuringActivation = 0x02,
    AnyOtherError = 0x04,
    DifferentDDOPExistsWithSameStructureLabel = 0x08,
}

impl ObjectPoolActivationError {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0x01 => Self::ThereAreErrorsInTheDDOP,
            0x02 => Self::TaskControllerRanOutOfMemoryDuringActivation,
            0x04 => Self::AnyOtherError,
            0x08 => Self::DifferentDDOPExistsWithSameStructureLabel,
            _ => Self::NoErrors,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::NoErrors),
            0x01 => Some(Self::ThereAreErrorsInTheDDOP),
            0x02 => Some(Self::TaskControllerRanOutOfMemoryDuringActivation),
            0x04 => Some(Self::AnyOtherError),
            0x08 => Some(Self::DifferentDDOPExistsWithSameStructureLabel),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ObjectPoolDeletionErrors {
    #[default]
    ObjectPoolIsReferencedByTaskData = 0,
    ServerCannotCheckForObjectPoolReferences = 1,
    ErrorDetailsNotAvailable = 0xFF,
}

impl ObjectPoolDeletionErrors {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::ServerCannotCheckForObjectPoolReferences,
            0xFF => Self::ErrorDetailsNotAvailable,
            _ => Self::ObjectPoolIsReferencedByTaskData,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::ObjectPoolIsReferencedByTaskData),
            1 => Some(Self::ServerCannotCheckForObjectPoolReferences),
            0xFF => Some(Self::ErrorDetailsNotAvailable),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ObjectPoolErrorCodes {
    #[default]
    NoErrors = 0x00,
    MethodOrAttributeNotSupported = 0x01,
    UnknownObjectReference = 0x02,
    AnyOtherError = 0x04,
    DDOPWasDeletedFromVolatileMemory = 0x08,
}

impl ObjectPoolErrorCodes {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0x01 => Self::MethodOrAttributeNotSupported,
            0x02 => Self::UnknownObjectReference,
            0x04 => Self::AnyOtherError,
            0x08 => Self::DDOPWasDeletedFromVolatileMemory,
            _ => Self::NoErrors,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::NoErrors),
            0x01 => Some(Self::MethodOrAttributeNotSupported),
            0x02 => Some(Self::UnknownObjectReference),
            0x04 => Some(Self::AnyOtherError),
            0x08 => Some(Self::DDOPWasDeletedFromVolatileMemory),
            _ => None,
        }
    }
}

/// Process-data command codes (low nibble of byte 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ProcessDataCommands {
    #[default]
    TechnicalCapabilities = 0x00,
    DeviceDescriptor = 0x01,
    RequestValue = 0x02,
    Value = 0x03,
    MeasurementTimeInterval = 0x04,
    MeasurementDistanceInterval = 0x05,
    MeasurementMinimumWithinThreshold = 0x06,
    MeasurementMaximumWithinThreshold = 0x07,
    MeasurementChangeThreshold = 0x08,
    PeerControlAssignment = 0x09,
    SetValueAndAcknowledge = 0x0A,
    Acknowledge = 0x0D,
    Status = 0x0E,
    ClientTask = 0x0F,
}

impl ProcessDataCommands {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x0F {
            0x01 => Self::DeviceDescriptor,
            0x02 => Self::RequestValue,
            0x03 => Self::Value,
            0x04 => Self::MeasurementTimeInterval,
            0x05 => Self::MeasurementDistanceInterval,
            0x06 => Self::MeasurementMinimumWithinThreshold,
            0x07 => Self::MeasurementMaximumWithinThreshold,
            0x08 => Self::MeasurementChangeThreshold,
            0x09 => Self::PeerControlAssignment,
            0x0A => Self::SetValueAndAcknowledge,
            0x0D => Self::Acknowledge,
            0x0E => Self::Status,
            0x0F => Self::ClientTask,
            _ => Self::TechnicalCapabilities,
        }
    }

    /// The CAN priority this command's Process Data message is sent at.
    ///
    /// ISO 11783-10 B.2 splits the Process Data PG's default priority three
    /// ways: "3 for messages with Command values 3₁₆, A₁₆, E₁₆, or F₁₆",
    /// "4 for messages with Command value D₁₆", and "5 for messages with
    /// Command values 0₁₆, 1₁₆, 2₁₆, or 4₁₆ through 9₁₆".
    ///
    /// "The differentiation of the default priority of the Process Data message
    /// into 3 different levels depending on the Command value has been
    /// introduced in ISO 11783-10 version 4. Prior to version 4, the default
    /// priority was 3 for all Process Data messages… giving higher priority to
    /// control and connection maintenance messages versus request and
    /// acknowledgement messages."
    ///
    /// Every TC message went out at the J1939 general default of 6 instead, so
    /// the status and client-task heartbeats — the ones a peer times out on —
    /// lost arbitration to ordinary request and measurement traffic.
    #[must_use]
    pub const fn priority(self) -> Priority {
        match self {
            Self::Value | Self::SetValueAndAcknowledge | Self::Status | Self::ClientTask => {
                Priority::Normal
            }
            Self::Acknowledge => Priority::BelowNormal,
            Self::TechnicalCapabilities
            | Self::DeviceDescriptor
            | Self::RequestValue
            | Self::MeasurementTimeInterval
            | Self::MeasurementDistanceInterval
            | Self::MeasurementMinimumWithinThreshold
            | Self::MeasurementMaximumWithinThreshold
            | Self::MeasurementChangeThreshold
            | Self::PeerControlAssignment => Priority::Low,
        }
    }

    /// The priority for a raw Process Data payload, read from its command
    /// nibble. Reserved commands (B₁₆, C₁₆) fall back to the request/ack tier.
    #[must_use]
    pub const fn priority_for_payload(data: &[u8]) -> Priority {
        match data {
            [first, ..] => match Self::try_from_u8(*first) {
                Some(command) => command.priority(),
                None => Priority::Low,
            },
            [] => Priority::Low,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v & 0x0F {
            0x00 => Some(Self::TechnicalCapabilities),
            0x01 => Some(Self::DeviceDescriptor),
            0x02 => Some(Self::RequestValue),
            0x03 => Some(Self::Value),
            0x04 => Some(Self::MeasurementTimeInterval),
            0x05 => Some(Self::MeasurementDistanceInterval),
            0x06 => Some(Self::MeasurementMinimumWithinThreshold),
            0x07 => Some(Self::MeasurementMaximumWithinThreshold),
            0x08 => Some(Self::MeasurementChangeThreshold),
            0x09 => Some(Self::PeerControlAssignment),
            0x0A => Some(Self::SetValueAndAcknowledge),
            0x0D => Some(Self::Acknowledge),
            0x0E => Some(Self::Status),
            0x0F => Some(Self::ClientTask),
            _ => None,
        }
    }
}

/// Server option flags (bitfield in byte 1 of `Technical Capabilities`).
///
/// The C++ exposes these as a `u8`-OR'able enum; Rust uses an explicit
/// `ServerOptionFlags` `u8` newtype with `with_*` builders so callers
/// don't reach for raw bit math.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ServerOptions {
    #[default]
    SupportsDocumentation = 0x01,
    SupportsTCGEOWithoutPositionBasedControl = 0x02,
    SupportsTCGEOWithPositionBasedControl = 0x04,
    SupportsPeerControlAssignment = 0x08,
    SupportsImplementSectionControl = 0x10,
}

impl ServerOptions {
    #[inline]
    #[must_use]
    pub const fn bit(self) -> u8 {
        self as u8
    }
}

impl core::ops::BitOr for ServerOptions {
    type Output = u8;
    fn bitor(self, rhs: Self) -> u8 {
        self as u8 | rhs as u8
    }
}

/// Known option bits in the TC technical-capabilities/version-response option
/// byte. Values outside this mask are reserved and should be rejected before
/// state is updated from a peer capability frame.
pub const TC_SERVER_OPTIONS_KNOWN_MASK: u8 = ServerOptions::SupportsDocumentation as u8
    | ServerOptions::SupportsTCGEOWithoutPositionBasedControl as u8
    | ServerOptions::SupportsTCGEOWithPositionBasedControl as u8
    | ServerOptions::SupportsPeerControlAssignment as u8
    | ServerOptions::SupportsImplementSectionControl as u8;

#[inline]
#[must_use]
pub const fn tc_options_byte_is_valid(options: u8) -> bool {
    options & !TC_SERVER_OPTIONS_KNOWN_MASK == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ProcessDataAcknowledgeErrorCodes {
    #[default]
    NoError = 0x00,
    ElementNotSupportedByThisDevice = 0x01,
    ValueIsOutsideValidRange = 0x02,
    NoProcessingResourcesAvailable = 0x03,
    DDEXValueNotSupported = 0x04,
}

impl ProcessDataAcknowledgeErrorCodes {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0x01 => Self::ElementNotSupportedByThisDevice,
            0x02 => Self::ValueIsOutsideValidRange,
            0x03 => Self::NoProcessingResourcesAvailable,
            0x04 => Self::DDEXValueNotSupported,
            _ => Self::NoError,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::NoError),
            0x01 => Some(Self::ElementNotSupportedByThisDevice),
            0x02 => Some(Self::ValueIsOutsideValidRange),
            0x03 => Some(Self::NoProcessingResourcesAvailable),
            0x04 => Some(Self::DDEXValueNotSupported),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TCServerState {
    #[default]
    Disconnected = 0,
    WaitForClients = 1,
    Active = 2,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ISO 11783-10 B.2 — "3 for messages with Command values 3₁₆, A₁₆, E₁₆, or
    /// F₁₆", "4 for messages with Command value D₁₆", "5 for messages with
    /// Command values 0₁₆, 1₁₆, 2₁₆, or 4₁₆ through 9₁₆".
    ///
    /// Every TC message previously went out at the J1939 general default of 6,
    /// so the status and client-task heartbeats a peer times out on lost
    /// arbitration to ordinary request and measurement traffic.
    #[test]
    fn process_data_priority_follows_the_command_value() {
        for command in [
            ProcessDataCommands::Value,
            ProcessDataCommands::SetValueAndAcknowledge,
            ProcessDataCommands::Status,
            ProcessDataCommands::ClientTask,
        ] {
            assert_eq!(command.priority(), Priority::Normal, "{command:?} is 3");
        }

        assert_eq!(
            ProcessDataCommands::Acknowledge.priority(),
            Priority::BelowNormal,
            "PDACK is 4"
        );

        for command in [
            ProcessDataCommands::TechnicalCapabilities,
            ProcessDataCommands::DeviceDescriptor,
            ProcessDataCommands::RequestValue,
            ProcessDataCommands::MeasurementTimeInterval,
            ProcessDataCommands::MeasurementDistanceInterval,
            ProcessDataCommands::MeasurementMinimumWithinThreshold,
            ProcessDataCommands::MeasurementMaximumWithinThreshold,
            ProcessDataCommands::MeasurementChangeThreshold,
            ProcessDataCommands::PeerControlAssignment,
        ] {
            assert_eq!(command.priority(), Priority::Low, "{command:?} is 5");
        }

        // Read off a payload's command nibble, which is how the plugins pick.
        // The TC Status message (0xFE) must outrank a request value (0x2n).
        assert_eq!(
            ProcessDataCommands::priority_for_payload(&[0xFE, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0xFF]),
            Priority::Normal
        );
        assert_eq!(
            ProcessDataCommands::priority_for_payload(&[0x12, 0x00, 0x01, 0x00, 0, 0, 0, 0]),
            Priority::Low
        );
        // B₁₆ and C₁₆ are Reserved, and an empty payload has no command at all.
        assert_eq!(
            ProcessDataCommands::priority_for_payload(&[0x0B]),
            Priority::Low
        );
        assert_eq!(ProcessDataCommands::priority_for_payload(&[]), Priority::Low);
    }

    #[test]
    fn process_data_commands_round_trip() {
        for c in [
            ProcessDataCommands::TechnicalCapabilities,
            ProcessDataCommands::DeviceDescriptor,
            ProcessDataCommands::RequestValue,
            ProcessDataCommands::Value,
            ProcessDataCommands::MeasurementTimeInterval,
            ProcessDataCommands::MeasurementDistanceInterval,
            ProcessDataCommands::MeasurementMinimumWithinThreshold,
            ProcessDataCommands::MeasurementMaximumWithinThreshold,
            ProcessDataCommands::MeasurementChangeThreshold,
            ProcessDataCommands::PeerControlAssignment,
            ProcessDataCommands::SetValueAndAcknowledge,
            ProcessDataCommands::Acknowledge,
            ProcessDataCommands::Status,
            ProcessDataCommands::ClientTask,
        ] {
            assert_eq!(ProcessDataCommands::from_u8(c.as_u8()), c);
            assert_eq!(ProcessDataCommands::try_from_u8(c.as_u8()), Some(c));
        }
        assert_eq!(
            ProcessDataCommands::from_u8(0x0B),
            ProcessDataCommands::TechnicalCapabilities
        );
        assert_eq!(ProcessDataCommands::try_from_u8(0x0B), None);
        assert_eq!(ProcessDataCommands::try_from_u8(0x0C), None);
        assert_eq!(ProcessDataCommands::try_from_u8(0xBC), None);
    }

    #[test]
    fn server_options_or_yields_bitfield() {
        let bits =
            ServerOptions::SupportsDocumentation | ServerOptions::SupportsImplementSectionControl;
        assert_eq!(bits, 0x11);
    }

    #[test]
    fn pool_activation_error_round_trip() {
        for e in [
            ObjectPoolActivationError::NoErrors,
            ObjectPoolActivationError::ThereAreErrorsInTheDDOP,
            ObjectPoolActivationError::TaskControllerRanOutOfMemoryDuringActivation,
            ObjectPoolActivationError::AnyOtherError,
            ObjectPoolActivationError::DifferentDDOPExistsWithSameStructureLabel,
        ] {
            assert_eq!(ObjectPoolActivationError::from_u8(e.as_u8()), e);
        }
    }
}
