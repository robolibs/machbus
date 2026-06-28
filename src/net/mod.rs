//! CAN frame layer, network management, address claiming, transport
//! protocols, and shared primitives.
//!
//! Phases 1–5 cover: types, constants, error, bitfield, data_span,
//! pgn_defs, pgn, identifier, frame, message, name, event, timer,
//! state_machine, bus_load, scheduler, policy, control_function,
//! internal_cf, partner_cf, address_claimer, working_set, session,
//! tp, etp, fast_packet. Subsequent phases add IsoNet, NIU,
//! name_manager, eth_can, iop_parser.

#[cfg(any(feature = "default", feature = "cli"))]
pub mod adapter;
pub mod address_claimer;
pub mod bitfield;
pub mod bus_load;
pub mod bus_power;
#[doc(hidden)]
pub mod can_adapter;
pub mod can_bus_config;
pub mod can_transport;
pub mod capture;
pub mod constants;
pub mod control_function;
pub mod data_span;
pub mod datalink_features;
pub mod error;
pub mod etp;
pub mod event;
pub mod fast_packet;
pub mod fault_confinement;
pub mod frame;
pub mod identifier;
pub mod internal_cf;
pub mod iop_parser;
pub mod management;
pub mod message;
pub mod name;
#[cfg(any(feature = "default", feature = "cli"))]
pub mod name_manager;
pub mod network_manager;
pub mod niu;
pub mod partner_cf;
pub mod pgn;
pub mod pgn_defs;
pub mod physical_features;
pub mod policy;
pub mod scheduler;
pub mod session;
pub mod state_machine;
pub mod timer;
#[cfg(any(feature = "default", feature = "cli"))]
pub mod topology;
pub mod tp;
pub mod types;
#[cfg(any(feature = "default", feature = "cli"))]
pub mod working_set;

#[cfg(any(feature = "default", feature = "cli"))]
pub use adapter::{
    AdapterCapabilities, AdapterReadiness, CapabilityCheck, REQUIRED_ADAPTER_BITRATE,
};
pub use address_claimer::AddressClaimer;
pub use bus_load::BusLoad;
pub use bus_power::{
    BusPowerSupply, BusPowerViolation, ECU_PWR_MIN_CURRENT_A, NOMINAL_SUPPLY_VOLTAGE_V,
    PWR_MIN_CURRENT_A, bus_power_is_adequate, validate_bus_power,
};
pub use can_bus_config::{
    CanBusConfig, CanBusValidation, ISO_CAN_BITRATE, ISO_SAMPLE_POINT_MAX, ISO_SAMPLE_POINT_MIN,
    ISO_SAMPLE_POINT_NOMINAL, enforce_iso_can_config, validate_can_bus_config,
};
pub use can_transport::CanTransport;
pub use capture::{
    CaptureLog, CaptureRecorder, CapturedFrame, format_candump_line, parse_candump_line,
};
pub use constants::{
    ADDRESS_CLAIM_RTXD_MAX_MS, ADDRESS_CLAIM_TIMEOUT_MS, BROADCAST_ADDRESS, CAN_DATA_LENGTH,
    ETP_MAX_DATA_LENGTH, ETP_TIMEOUT_T1_MS, FAST_PACKET_MAX_DATA, HEARTBEAT_INTERVAL_MS,
    MAX_ADDRESS, NULL_ADDRESS, POWER_MAINTAIN_REPEAT_MS, POWER_MAX_EXTENSION_MS,
    POWER_SHUTDOWN_MIN_MS, TP_BAM_INTER_PACKET_MS, TP_BYTES_PER_FRAME, TP_MAX_DATA_LENGTH,
    TP_MAX_PACKETS_PER_CTS, TP_TIMEOUT_T1_MS, TP_TIMEOUT_T2_MS, TP_TIMEOUT_T3_MS, TP_TIMEOUT_T4_MS,
    TP_TIMEOUT_TR_MS,
};
pub use control_function::{CfState, CfType, ControlFunction};
pub use data_span::DataSpan;
pub use datalink_features::{
    DATALINK_FEATURES, DataLinkFeature, DataLinkFeatureRow, DlStatus, datalink_feature,
};
pub use error::{Error, ErrorCode, Result};
pub use etp::ExtendedTransportProtocol;
#[cfg(feature = "embedded")]
pub use etp::{EtpCmdtTx, EtpRxFixed, EtpRxFixedOutcome};
pub use event::{Event, INVALID_TOKEN, ListenerToken};
pub use fast_packet::FastPacketProtocol;
pub use fault_confinement::{
    FaultConfinementAction, FaultConfinementMonitor, fault_confinement_action,
};
pub use frame::Frame;
pub use identifier::Identifier;
pub use internal_cf::{ClaimState, InternalCf};
#[cfg(any(feature = "default", feature = "cli"))]
pub use iop_parser::read_iop_file;
pub use iop_parser::{RawIopObject, hash_to_version, parse_iop_data, validate};
pub use management::{
    MANAGEMENT_ROLES, ManagementBehavior, ManagementRole, ManagementSupport, is_implemented,
    role_for,
};
pub use message::Message;
pub use name::Name;
#[cfg(any(feature = "default", feature = "cli"))]
pub use name_manager::{
    NameManagementMsg, NameManager, NameMgmtMode, NameMgmtReply, NameNackReason,
};
pub use network_manager::{
    InternalCfHandle, IsoNet, NetworkConfig, NetworkStatistics, PartnerCfHandle,
};
pub use niu::{
    AddressTable, AddressTranslation, AddressTranslationDb, FilterRule, FilterRuleSnapshot,
    ForwardPolicy, NIU_PROFILES, Niu, NiuConfig, NiuFilterMode, NiuFunction, NiuNetworkMsg,
    NiuPolicySnapshot, NiuProfile, NiuProfileStatus, NiuProfileSupport, NiuState, Router,
    RouterPolicySnapshot, Side, niu_profile,
};
pub use partner_cf::{NameFilter, NameFilterField, PartnerCf};
pub use pgn::{
    PGN_TABLE, PgnInfo, pgn_is_pdu2, pgn_is_valid, pgn_lookup, pgn_normalize, pgn_pdu_format,
};
pub use physical_features::{
    PHYSICAL_FEATURES, PhysStatus, PhysicalFeature, PhysicalFeatureRow, physical_feature,
};
pub use policy::{DegradedAction, FreshnessRequirement, SafeState, SafetyConfig, SafetyPolicy};
pub use scheduler::{PeriodicTask, ProcessingFlags, Scheduler};
pub use session::{
    SessionState, TransportAbortEvent, TransportAbortReason, TransportDirection, TransportSession,
    TransportStats,
};
pub use state_machine::StateMachine;
pub use timer::{Timeout, Timer};
#[cfg(any(feature = "default", feature = "cli"))]
pub use topology::{
    BusTopology, EcuType, MAX_BUS_LENGTH_M, MAX_ECUS_PER_SEGMENT, MAX_SIMPLE_STUB_LENGTH_M,
    MAX_TYPE_I_WEAK_PER_MACHINE, MIN_STUB_SPACING_M, TopologyViolation, topology_is_valid,
    validate_machine_ecu_types, validate_topology,
};
#[cfg(feature = "embedded")]
pub use tp::{TpCmdtTx, TpRxFixed, TpRxFixedOutcome};
pub use tp::{TpSessionState, TpTimerSession, TransportProtocol};
pub use types::{Address, Pgn, Priority};
#[cfg(any(feature = "default", feature = "cli"))]
pub use working_set::{MEMBER_MSG_INTERVAL_MS, WorkingSetManager};
