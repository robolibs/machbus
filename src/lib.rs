//! `machbus` — ISO 11783 (ISOBUS) + J1939 + NMEA2000 networking stack.
//!
//! Rust port of the C++ `machbus` library. See `PLAN.md` for
//! the current hardening plan.
//!
//! Subsystems:
//! - [`net`] — CAN frame layer, network management, address claiming, transport
//!   protocols (TP/ETP/FastPacket), utilities.
//! - `j1939` — *(phase 9)* J1939/SAE protocol services.
//! - `isobus` — *(phases 10–15)* ISO 11783 application layer (VT, TC, SC,
//!   implement messages, file server).
//! - `nmea` — *(phase 16)* NMEA2000 definitions, parsing/generation, GNSS.

#![cfg_attr(feature = "embedded", no_std)]
// Rust 1.96 added collapsible_match; these patterns pre-date it.
#![allow(clippy::collapsible_match)]

extern crate alloc;

#[cfg(feature = "default")]
pub mod ffi;
#[cfg(feature = "embedded")]
pub mod fixed;
pub mod geo;
pub mod isobus;
pub mod j1939;
pub mod net;
pub mod nmea;
#[cfg(feature = "default")]
pub mod python;
#[cfg(any(feature = "default", feature = "cli"))]
pub mod session;
#[cfg(feature = "embedded")]
#[path = "embedded_session.rs"]
pub mod session;
pub mod time;
pub mod vt_storage;

pub use time::Instant;

#[cfg(feature = "embedded")]
pub mod embedded {
    //! `no_std + alloc` surface while the full protocol/session core is migrated.
    //!
    //! The low-level CAN/J1939 primitives are available now. The hosted
    //! orchestration/session modules remain behind `std` until their allocation,
    //! storage, transport, and geo dependencies have been split per `PLAN.md`.
    #[cfg(feature = "embedded")]
    pub use crate::fixed::{
        FixedBytes, FixedCapacityError, FixedFrameQueue, FixedMessage, FixedQueue, FixedSlots,
    };
    pub use crate::isobus::{
        AuxFunctionState, AuxFunctionType, AuxNFunction, AuxNOptions, AuxOFunction, AuxOOptions,
        BasicTractorEcuOptions, FileAttribute, FileClientState, FileOperation, FileProperties,
        FileServerConfig, FileServerState, FileTransferError, Functionalities, Functionality,
        FunctionalityData, GroupFunctionError, GroupFunctionMsg, GroupFunctionResponder,
        GroupFunctionSupport, GroupFunctionType, GuidanceData, MinimumControlFunctionOptions,
        OpenFileState, TaskControllerGeoServerOptions, TractorImplementManagementOptions,
        VolumeInfo,
    };
    pub use crate::isobus::{
        AuxValve, AuxValveCommand, HitchState, MAX_AUX_VALVES, MAX_HITCH_POSITION, PtoState,
        TIM_OPTION_BYTES, TIM_OPTION_DEFINED_MASK, TIM_UPDATE_INTERVAL_MS, TimArbitration,
        TimAuthority, TimAuthorityArbiter, TimAuthorityState, TimCommand, TimInterlock,
        TimInterlocks, TimOption, TimOptionSet, TimValidationError,
    };
    pub use crate::isobus::{
        AuxValveCommandMsg, AuxValveFlowMsg, CURVATURE_MAX_PER_KM, CURVATURE_MIN_PER_KM,
        CurvatureCommand, CurvatureCommandStatus, DriveStrategyCmd, DriveStrategyMode,
        ExitReasonCode, FacilityGroup, FamilyLevel, GenericSaeBs02SlotValue, GroundBasedSpeedDist,
        GuidanceLimitStatus, GuidanceMachineInfo, GuidanceSystemCmd, GuidanceSystemStatus,
        HitchCommand, HitchCommandMsg, HitchPtoCombinedCmd, HitchRollPitchCmd, HitchStatus,
        IMPLEMENT_FAMILIES, ImplementFamilyInfo, ImplementMessageFamily, LightState,
        LightingController, LightingState, LimitStatus, MachineDirection, MachineSelectedSpeedFull,
        MachineSelectedSpeedMsg, MachineSpeedCommandMsg, MechanicalLockout, PtoCommand,
        PtoCommandMsg, PtoStatus, RequestResetCommandStatus, RequiredFacilitiesAggregator,
        SpeedExitCode, SpeedSource, SteeringReadiness, TECU_FACILITY_MATRIX, TecuClass,
        TractorControlModeMsg, TractorFacilities, TractorFacilitiesRole, TractorMode, ValveCommand,
        ValveFailSafe, ValveLimitStatus, ValveState, WheelBasedSpeedDist, curvature_within_range,
        estimated_flow_pgn, facilities_in, family_info, measured_flow_pgn, wheel_slip_percent,
    };
    pub use crate::isobus::{
        SC_MAX_SEQUENCE_STEP_ID, SC_MSG_CODE_CLIENT, SC_MSG_CODE_MASTER, SC_STATUS_ACTIVE_RATE_MS,
        SC_STATUS_MIN_SPACING_MS, SC_STATUS_TIMEOUT_ACTIVE_MS, SC_STATUS_TIMEOUT_READY_MS,
        SC_TAN_MAX, SC_TAN_MIN, SC_TAN_NOT_AVAILABLE, SC_TAN_REPEAT_MS, SCClient, SCClientConfig,
        SCClientFuncError, SCClientState, SCCommand, SCD_LABEL_NONE, SCMaster, SCMasterConfig,
        SCMasterState, SCSequenceState, SCState, ScdAction, ScdLabel, SequenceRecorder,
        SequenceStep, SequenceTanTracker, scd_action,
    };
    pub use crate::net::{
        Address, AddressClaimer, AddressTable, AddressTranslation, AddressTranslationDb,
        BROADCAST_ADDRESS, BusPowerSupply, BusPowerViolation, CAN_DATA_LENGTH, CanBusConfig,
        CanBusValidation, CanTransport, CaptureLog, CaptureRecorder, CapturedFrame, CfState,
        CfType, ClaimState, ControlFunction, DATALINK_FEATURES, DataLinkFeature,
        DataLinkFeatureRow, DataSpan, DegradedAction, DlStatus, ECU_PWR_MIN_CURRENT_A, Error,
        ErrorCode, Event, ExtendedTransportProtocol, FAST_PACKET_MAX_DATA, FastPacketProtocol,
        FaultConfinementAction, FaultConfinementMonitor, FilterRule, FilterRuleSnapshot,
        ForwardPolicy, FreshnessRequirement, INVALID_TOKEN, ISO_CAN_BITRATE, ISO_SAMPLE_POINT_MAX,
        ISO_SAMPLE_POINT_MIN, ISO_SAMPLE_POINT_NOMINAL, Identifier, InternalCf, InternalCfHandle,
        IsoNet, ListenerToken, MANAGEMENT_ROLES, MAX_ADDRESS, ManagementBehavior, ManagementRole,
        ManagementSupport, Message, NIU_PROFILES, NOMINAL_SUPPLY_VOLTAGE_V, NULL_ADDRESS, Name,
        NameFilter, NameFilterField, NetworkConfig, NetworkStatistics, Niu, NiuConfig,
        NiuFilterMode, NiuFunction, NiuNetworkMsg, NiuPolicySnapshot, NiuProfile, NiuProfileStatus,
        NiuProfileSupport, NiuState, PGN_TABLE, PHYSICAL_FEATURES, PWR_MIN_CURRENT_A, PartnerCf,
        PartnerCfHandle, PeriodicTask, Pgn, PgnInfo, PhysStatus, PhysicalFeature,
        PhysicalFeatureRow, Priority, ProcessingFlags, RawIopObject, Result, Router,
        RouterPolicySnapshot, SafeState, SafetyConfig, SafetyPolicy, Scheduler, SessionState, Side,
        StateMachine, Timeout, Timer, TpSessionState, TpTimerSession, TransportAbortEvent,
        TransportAbortReason, TransportDirection, TransportProtocol, TransportSession,
        TransportStats, bus_power_is_adequate, datalink_feature, enforce_iso_can_config,
        fault_confinement_action, format_candump_line, hash_to_version, is_implemented,
        niu_profile, parse_candump_line, parse_iop_data, pgn_is_pdu2, pgn_is_valid, pgn_lookup,
        pgn_normalize, pgn_pdu_format, physical_feature, role_for, validate_bus_power,
        validate_can_bus_config,
    };
    pub use crate::nmea::{
        GNSSBatch, GNSSPosition, N2KConfigInfo, N2KHeartbeat, N2KManagement, N2KManagementConfig,
        N2KOutbound, N2KProductInfo, NMEA2000_INTERFACE_PGNS, NMEA2000_MANAGEMENT_PGNS,
        NMEA2000_SELECTED_PGNS, NMEAConfig, NMEAInterface, NmeaUtcDateTime, PendingRequest,
        SerialGNSS, SerialGNSSConfig,
    };
    #[cfg(feature = "embedded")]
    pub use crate::session::FixedEvent as FixedSessionEvent;
    pub use crate::session::{Driver, Event as SessionEvent, Session, SessionBuilder, Transport};
    pub use crate::time::Instant;
    pub use crate::vt_storage::{StoredPoolVersion, is_valid_classic_label};
}

#[cfg(any(feature = "default", feature = "cli"))]
pub mod prelude {
    //! Common re-exports for downstream code.
    pub use crate::net::{
        Address, BROADCAST_ADDRESS, DataSpan, Error, ErrorCode, Frame, Identifier, MAX_ADDRESS,
        Message, NULL_ADDRESS, Name, Pgn, Priority, Result,
    };
    pub use crate::time::Instant;
}

#[cfg(feature = "embedded")]
pub mod prelude {
    //! Common re-exports for embedded / `no_std + alloc` downstream code.
    pub use crate::embedded::*;
}
