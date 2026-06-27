//! ISO 11783-10 Task Controller server enumerations.
//!
//! Mirrors the C++ `machbus::isobus::tc::server_options.hpp`.

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
