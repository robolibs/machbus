use machbus::geo::Wgs;
use machbus::isobus::fs::{
    CCMMessage, FSError, FSFunction, FSNack, FileAttributes, FileClient, FileClientConfig,
    FileServer as IsoFileServer, FileServerConfig as IsoFileServerConfig, FileServerProperties,
    FileServerPropertiesV2, FileServerStatus, OpenFlags, VolumeStateV2, VolumeStatus, encode_ccm,
};
use machbus::isobus::implement::{
    AuxValveCommandMsg, AuxValveFlowMsg, CurvatureCommand, CurvatureCommandStatus,
    DriveStrategyCmd, DriveStrategyMode, ExitReasonCode, GenericSaeBs02SlotValue,
    GroundBasedSpeedDist, GuidanceLimitStatus, GuidanceMachineInfo, GuidanceSystemCmd,
    GuidanceSystemStatus, HitchCommand, HitchCommandMsg, HitchPtoCombinedCmd, HitchRollPitchCmd,
    HitchStatus, LightState, LightingState, LimitStatus, MachineDirection,
    MachineSelectedSpeedFull, MachineSelectedSpeedMsg, MachineSpeedCommandMsg, MechanicalLockout,
    PtoCommand, PtoCommandMsg, PtoStatus, RequestResetCommandStatus, SpeedExitCode, SpeedSource,
    SteeringReadiness, TractorControlModeMsg, TractorFacilities, TractorFacilitiesRole,
    TractorMode, ValveCommand, ValveFailSafe, ValveLimitStatus, ValveState, WheelBasedSpeedDist,
    estimated_flow_pgn, measured_flow_pgn,
};
use machbus::isobus::sc::{
    SC_MAX_SEQUENCE_STEP_ID, SCClient, SCClientConfig, SCMaster, SCMasterConfig, SCState,
    SequenceStep,
};
use machbus::isobus::tc::{
    DDI, DDI_DATABASE, DDI_DATABASE_FINGERPRINT_FNV1A64, DDI_DATABASE_SIZE, DDI_DATABASE_VERSION,
    DDIDatabase, DDOP, DDOPHelpers, DeviceElement, DeviceElementType, DeviceObject,
    DeviceProcessData, DeviceProperty, DeviceValuePresentation, ElementNumber, GeoPoint, ObjectID,
    ObjectPoolActivationError, ObjectPoolDeletionErrors, ObjectPoolErrorCodes,
    PeerControlAssignment, PeerControlInterface, PrescriptionMap, PrescriptionZone,
    ProcessDataAcknowledgeErrorCodes, ProcessDataCommands, TCGEOInterface, TCObjectType,
    TCServerConfig, TCState, TaskControllerClient, TaskControllerServer, TriggerMethod, ddi,
    ddi_database_fingerprint, ddi_lookup, point_in_prescription_zone,
    prescription_rate_process_data_payload, prescription_rate_to_engineering, tc_cmd,
};
use machbus::isobus::tim::{
    AuxValveCommand, HitchState, MAX_AUX_VALVES, MAX_HITCH_POSITION, PtoState, TimAuthority,
    TimAuthorityState, TimCommand, TimInterlock, TimInterlocks, TimOption, TimOptionSet,
    TimValidationError,
};
use machbus::isobus::tractor_ecu::{
    PowerConfig, PowerState as TecuPowerState, TecuClass, TecuClassification, TecuConfig,
    TecuMaintainPowerRequest,
};
use machbus::isobus::vt::{
    AuxCapabilityDiscovery, AuxChannelCapability, GestureType, GraphicsContextV6,
    KeyActivationCode, ObjectID as VTObjectID, ObjectPool as VTObjectPool,
    ObjectType as VTObjectType, ServerWorkingSet, StoredPoolVersion, TouchGesture, UpdateOp,
    VT_STATUS_INTERVAL_MS, VTClient, VTClientConfig, VTClientStateTracker, VTServer,
    VTServerConfig, VTServerState, VTState, cmd,
};
use machbus::isobus::{
    AuxFunctionState, AuxFunctionType, AuxNFunction, AuxOFunction, BasicTractorEcuOptions,
    FILE_SERVER_BUSY_STATUS_INTERVAL_MS, FILE_SERVER_STATUS_INTERVAL_MS, FS_REQUEST_TIMEOUT_MS,
    FileAttribute, FileOperation, FileProperties, FileServerConfig as LegacyFileServerConfig,
    FileTransferError, Functionalities, Functionality, FunctionalityData, GroupFunctionError,
    GroupFunctionMsg, GroupFunctionType, MinimumControlFunctionOptions,
    TractorImplementManagementOptions,
};
use machbus::j1939::acknowledgment::{AckControl, Acknowledgment};
use machbus::j1939::heartbeat::hb_seq;
use machbus::j1939::pgn_request::{decode_request, encode_request};
use machbus::j1939::proprietary_b_pgn;
use machbus::j1939::shortcut_button;
use machbus::j1939::{
    Aftertreatment1, Aftertreatment2, AmbientConditions, AreaUnit, ComponentIdentification,
    CruiseControl, DashDisplay, DateFormat, DecimalSymbol, DiagProtocol, DiagnosticLamps,
    DiagnosticProtocolId, DistanceUnit, Dm3ClearPreviouslyActiveRequest, Dm4Message, Dm6Message,
    Dm7Command, Dm8TestResult, Dm9VehicleIdentificationRequest, Dm10VehicleIdentification,
    Dm11ClearActiveRequest, Dm12Message, Dm13Command, Dm13Signals, Dm13SuspendSignal, Dm14Command,
    Dm14PointerType, Dm14Request, Dm15Response, Dm15Status, Dm16Transfer, Dm20Response,
    Dm21Readiness, Dm22Control, Dm22Message, Dm23Message, Dm25Request, DmClearAllRequest,
    DmDtcList, Dtc, EcuIdentification, Eec1, Eec2, Eec3, EngineFluidLp, EngineHours, EngineTemp1,
    EngineTemp2, Etc1, Fmi, ForceUnit, FreezeFrame, FuelConsumption, FuelEconomy, HeartbeatRequest,
    HeartbeatSender, LampStatus, LanguageData, MaintainPowerData, MaintainPowerRequirement,
    MaintainPowerState, MassUnit, MonitorPerformanceRatio, OverrideControlMode,
    PGN_HEARTBEAT_REQUEST, PowerManager, PowerRole, PowerState, PressureUnit,
    ProductIdentification, ProprietaryMsg, Request2Msg, ShortcutButtonState,
    SoftwareIdentification, SpeedAndDistance, SpnSnapshot, TemperatureUnit, TimeDate, TimeFormat,
    TransferMsg, TransmissionOilTemp, Tsc1, UnitSystem, VehicleIdentification, VehiclePosition,
    Vep1, VolumeUnit,
};
use machbus::net::constants::{
    BROADCAST_ADDRESS, ETP_TIMEOUT_T1_MS, NULL_ADDRESS, TP_BAM_INTER_PACKET_MS,
    TP_MAX_PACKETS_PER_CTS, TP_TIMEOUT_T1_MS, TP_TIMEOUT_T3_MS,
};
use machbus::net::error::Error;
use machbus::net::etp::ExtendedTransportProtocol;
use machbus::net::fast_packet::FastPacketProtocol;
use machbus::net::pgn_defs::{
    PGN_ACKNOWLEDGMENT, PGN_ADDRESS_CLAIMED, PGN_ATTITUDE, PGN_AUX_INPUT_STATUS,
    PGN_AUX_INPUT_TYPE2, PGN_AUX_VALVE_0_7, PGN_AUX_VALVE_24_31, PGN_AUX_VALVE_CMD,
    PGN_AUX_VALVE_ESTIMATED_FLOW_BASE, PGN_AUX_VALVE_MEASURED_FLOW_BASE, PGN_BATTERY_STATUS,
    PGN_COMMANDED_ADDRESS, PGN_CONFIG_INFO, PGN_DM1, PGN_DM2, PGN_DM3, PGN_DM6, PGN_DM11, PGN_DM12,
    PGN_DM14, PGN_DM23, PGN_DRIVE_STRATEGY_CMD, PGN_ECU_TO_TC, PGN_ECU_TO_VT, PGN_ETP_CM,
    PGN_ETP_DT, PGN_FILE_CLIENT_TO_SERVER, PGN_FILE_SERVER_TO_CLIENT, PGN_FLUID_LEVEL,
    PGN_FRONT_HITCH, PGN_FRONT_HITCH_CMD, PGN_FRONT_HITCH_ROLL_PITCH_CMD, PGN_FRONT_PTO,
    PGN_FRONT_PTO_CMD, PGN_GNSS_COG_SOG_RAPID, PGN_GNSS_DOPS, PGN_GNSS_POSITION_DATA,
    PGN_GNSS_POSITION_RAPID, PGN_GNSS_SATELLITES_IN_VIEW, PGN_GROUND_BASED_SPEED_DIST,
    PGN_GUIDANCE_CURVATURE_CMD, PGN_GUIDANCE_MACHINE_INFO, PGN_GUIDANCE_SYSTEM,
    PGN_GUIDANCE_SYSTEM_CMD, PGN_HEADING_TRACK, PGN_HEARTBEAT,
    PGN_HEARTBEAT_N2K, PGN_HITCH_PTO_COMBINED_CMD, PGN_HUMIDITY, PGN_LANGUAGE_COMMAND,
    PGN_LIGHTING_DATA, PGN_MACHINE_SELECTED_SPEED, PGN_MACHINE_SELECTED_SPEED_CMD,
    PGN_MAGNETIC_VARIATION, PGN_MAINTAIN_POWER, PGN_NIU_NETWORK_MSG, PGN_OUTSIDE_ENVIRONMENTAL,
    PGN_PRESSURE, PGN_PRODUCT_INFO, PGN_PROPRIETARY_A, PGN_PROPRIETARY_A2, PGN_RATE_OF_TURN,
    PGN_REAR_HITCH, PGN_REAR_HITCH_CMD, PGN_REAR_HITCH_ROLL_PITCH_CMD, PGN_REAR_PTO,
    PGN_REAR_PTO_CMD, PGN_REQUEST, PGN_REQUEST2, PGN_REQUIRED_TRACTOR_FACILITIES, PGN_RUDDER,
    PGN_SC_CLIENT_STATUS, PGN_SC_MASTER_STATUS, PGN_SHORTCUT_BUTTON, PGN_SPEED_WATER,
    PGN_SYSTEM_TIME, PGN_TC_TO_ECU, PGN_TEMPERATURE, PGN_TIME_DATE, PGN_TP_CM, PGN_TP_DT,
    PGN_TRACTOR_CONTROL_MODE, PGN_TRACTOR_FACILITIES_RESPONSE, PGN_TRANSFER, PGN_VT_TO_ECU,
    PGN_WATER_DEPTH, PGN_WHEEL_BASED_SPEED_DIST, PGN_WIND_DATA, PGN_WORKING_SET_MASTER, PGN_XTE,
};
use machbus::net::tp::{TP_T_HOLD_MS, TpSessionState, TransportProtocol};
use machbus::net::{
    AddressClaimer, CanBusConfig, ErrorCode, FilterRule, ForwardPolicy, Frame, ISO_CAN_BITRATE,
    ISO_SAMPLE_POINT_MAX, ISO_SAMPLE_POINT_MIN, ISO_SAMPLE_POINT_NOMINAL, Identifier, InternalCf,
    Message, Name, NameManagementMsg, NameManager, NameMgmtMode, NameNackReason, Niu, NiuConfig,
    NiuFilterMode, NiuFunction, NiuNetworkMsg, Priority, Router, SessionState, Side,
    hash_to_version, parse_iop_data, validate, validate_can_bus_config,
};
use machbus::nmea::{
    BatteryStatusData, FluidLevelData, FluidType, GNSSDOPMode, GNSSFixType, GNSSPosition,
    HumidityData, HumiditySource, N2KConfigInfo, N2KHeartbeat, N2KManagement, N2KProductInfo,
    NMEAConfig, NMEAInterface, NmeaUtcDateTime, OutsideEnvironmentalData, PressureData,
    PressureSource, RudderData, RudderDirection, SerialGNSS, SpeedWaterData, SpeedWaterRefType,
    SystemTimeData, TemperatureData, TemperatureSource, TimeSource, WaterDepthData, WindData,
    WindReference, XTEData, XTEMode,
};
use proptest::prelude::*;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use wirebit::can::CanFrame;

const TIME_DATE_AGISOSTACK: &[u8; 8] = include_bytes!("../fixtures/j1939/time_date_agisostack.bin");
const J1939_TIME_DATE_HEX: &str = include_str!("../fixtures/j1939/time_date.hex");
const REQUEST_ADDRESS_CLAIM: &[u8; 3] = include_bytes!("../fixtures/j1939/request_address_claim.bin");
const J1939_ADDRESS_CLAIM_HEX: &str = include_str!("../fixtures/j1939/address_claim.hex");
const ACK_REQUEST_TO_0X42: &[u8; 8] = include_bytes!("../fixtures/j1939/ack_request_to_0x42.bin");
const J1939_REQUEST_ACK_HEX: &str = include_str!("../fixtures/j1939/request_ack.hex");
const DTC_MAX_SPN_CONDITION_OC127: &[u8; 4] =
    include_bytes!("../fixtures/j1939/dtc_max_spn_condition_oc127.bin");
const DM1_AMBER_SPN523312: &[u8; 8] = include_bytes!("../fixtures/j1939/dm1_amber_spn523312.bin");
const DM1_LAMP_ONLY_AMBER: &[u8; 8] = include_bytes!("../fixtures/j1939/dm1_lamp_only_amber.bin");
const DM2_PREVIOUS_TWO_DTCS: &[u8; 10] = include_bytes!("../fixtures/j1939/dm2_previous_two_dtcs.bin");
const DM3_CLEAR_PREVIOUS_REQUEST: &[u8; 8] =
    include_bytes!("../fixtures/j1939/dm3_clear_previous_request.bin");
const DM11_CLEAR_ACTIVE_REQUEST: &[u8; 8] =
    include_bytes!("../fixtures/j1939/dm11_clear_active_request.bin");
const J1939_DIAGNOSTIC_FIXED_CODECS_HEX: &str =
    include_str!("../fixtures/j1939/diagnostic_fixed_codecs.hex");
const J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX: &str =
    include_str!("../fixtures/j1939/diagnostic_variable_codecs.hex");
const J1939_DIAGNOSTIC_REQUEST_RESPONSE_HEX: &str =
    include_str!("../fixtures/j1939/diagnostic_request_response.hex");
const J1939_ENGINE_POWERTRAIN_CODECS_HEX: &str =
    include_str!("../fixtures/j1939/engine_powertrain_codecs.hex");
const J1939_HEARTBEAT_MAINTAIN_POWER_HEX: &str =
    include_str!("../fixtures/j1939/heartbeat_maintain_power.hex");
const J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX: &str =
    include_str!("../fixtures/j1939/language_shortcut_request2.hex");
const J1939_DM_MEMORY_HEX: &str = include_str!("../fixtures/j1939/dm_memory.hex");
const J1939_PROPRIETARY_CODECS_HEX: &str = include_str!("../fixtures/j1939/proprietary_codecs.hex");
const ISOBUS_FS_CODECS_HEX: &str = include_str!("../fixtures/isobus/fs_codecs.hex");
const ISOBUS_CAN_BUS_CONFIG: &str = include_str!("../fixtures/isobus/can_bus_config.txt");
const ISOBUS_CAN_FRAME_WRAPPER_HEX: &str = include_str!("../fixtures/isobus/can_frame_wrapper.hex");
const ISOBUS_NAME_MANAGEMENT_HEX: &str = include_str!("../fixtures/isobus/name_management.hex");
const ISOBUS_LEGACY_FILE_TRANSFER_HEX: &str =
    include_str!("../fixtures/isobus/legacy_file_transfer.hex");
const ISOBUS_AUX_GROUP_CODECS_HEX: &str = include_str!("../fixtures/isobus/aux_group_codecs.hex");
const ISOBUS_NIU_CONTROL_HEX: &str = include_str!("../fixtures/isobus/niu_control.hex");
const REAR_HITCH_RAISE: &[u8; 8] = include_bytes!("../fixtures/isobus/rear_hitch_raise.bin");
const ISOBUS_IMPLEMENT_CONTROLS_STATUS_HEX: &str =
    include_str!("../fixtures/isobus/implement_controls_status.hex");
const ISOBUS_SC_STATUS_HEX: &str = include_str!("../fixtures/isobus/sc_status.hex");
const ISOBUS_TC_DDI_DATABASE_SNAPSHOT: &str =
    include_str!("../fixtures/isobus/tc_ddi_database_snapshot.txt");
const ISOBUS_TC_DDOP_HEX: &str = include_str!("../fixtures/isobus/tc_ddop.hex");
const ISOBUS_TC_DDOP_HELPER_EXPECTATIONS: &str =
    include_str!("../fixtures/isobus/tc_ddop_helper_expectations.txt");
const ISOBUS_TRACTOR_ECU_SNAPSHOT: &str = include_str!("../fixtures/isobus/tractor_ecu_snapshot.txt");
const ISOBUS_VT_COMMANDS_HEX: &str = include_str!("../fixtures/isobus/vt_commands.hex");
const ISOBUS_VT_AUX_CAPS_HEX: &str = include_str!("../fixtures/isobus/vt_aux_caps.hex");
const ISOBUS_VT_OBJECT_POOL_HEX: &str = include_str!("../fixtures/isobus/vt_object_pool.hex");
const ISOBUS_VT_V6_HELPERS_HEX: &str = include_str!("../fixtures/isobus/vt_v6_helpers.hex");
const ISOBUS_VT_SERVER_WORKING_SET_HEX: &str =
    include_str!("../fixtures/isobus/vt_server_working_set.hex");
const ISOBUS_IOP_PARSER_HEX: &str = include_str!("../fixtures/isobus/iop_parser.hex");
const ISOBUS_TC_GEO_PRESCRIPTION: &str = include_str!("../fixtures/isobus/tc_geo_prescription.txt");
const ISOBUS_TC_OBJECT_POOL_HEX: &str = include_str!("../fixtures/isobus/tc_object_pool.hex");
const ISOBUS_TC_PEER_CONTROL_HEX: &str = include_str!("../fixtures/isobus/tc_peer_control.hex");
const ISOBUS_TC_PROCESS_DATA_HEX: &str = include_str!("../fixtures/isobus/tc_process_data.hex");
fn valid_tc_server_config() -> TCServerConfig {
    TCServerConfig::default().with_booms(1).with_sections(1)
}

const ISOBUS_TIM_CODECS_HEX: &str = include_str!("../fixtures/isobus/tim_codecs.hex");
const ISOBUS_CONTROL_FUNCTIONALITIES_HEX: &str =
    include_str!("../fixtures/isobus/control_functionalities.hex");
const FP_9B_FRAME0: &[u8; 8] = include_bytes!("../fixtures/nmea/fast_packet_9b_frame0.bin");
const FP_9B_FRAME1: &[u8; 8] = include_bytes!("../fixtures/nmea/fast_packet_9b_frame1.bin");
const FP_10B_FRAME0: &[u8; 8] = include_bytes!("../fixtures/nmea/fast_packet_10b_frame0.bin");
const FP_10B_FRAME1: &[u8; 8] = include_bytes!("../fixtures/nmea/fast_packet_10b_frame1.bin");
const FP_10B_SEQ1_FRAME0: &[u8; 8] =
    include_bytes!("../fixtures/nmea/fast_packet_10b_seq1_frame0.bin");
const FP_10B_SEQ1_FRAME1: &[u8; 8] =
    include_bytes!("../fixtures/nmea/fast_packet_10b_seq1_frame1.bin");
const FP_223B_FRAME0: &[u8; 8] = include_bytes!("../fixtures/nmea/fast_packet_223b_frame0.bin");
const FP_223B_FRAME31: &[u8; 8] = include_bytes!("../fixtures/nmea/fast_packet_223b_frame31.bin");
const NAME_MAGIC_RAW_LE: &[u8; 8] = include_bytes!("../fixtures/agisostack/name_magic_raw_le.bin");
const TIME_DATE_CANDUMP: &str = include_str!("../fixtures/traces/time_date_agisostack.candump");
const BRACKETED_TIME_DATE_CANDUMP: &str =
    include_str!("../fixtures/traces/bracketed_time_date.candump");
const STANDARD_ID_REJECTION_CANDUMP: &str =
    include_str!("../fixtures/traces/standard_id_rejection.candump");
const MALFORMED_CANDUMP: &str = include_str!("../fixtures/traces/malformed_candump.candump");
const TP_BAM_20B_PGN_EF00: &[u8; 8] = include_bytes!("../fixtures/j1939/tp_bam_20b_pgn_ef00.bin");
const TP_RTS_20B_PGN_EF00: &[u8; 8] = include_bytes!("../fixtures/j1939/tp_rts_20b_pgn_ef00.bin");
const TP_CTS_20B_PGN_EF00: &[u8; 8] = include_bytes!("../fixtures/j1939/tp_cts_20b_pgn_ef00.bin");
const TP_CTS_HOLD_PGN_EF00: &[u8; 8] = include_bytes!("../fixtures/j1939/tp_cts_hold_pgn_ef00.bin");
const TP_DT_SEQ1_20B_PAYLOAD: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_dt_seq1_20b_payload.bin");
const TP_DT_SEQ2_20B_PAYLOAD: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_dt_seq2_20b_payload.bin");
const TP_DT_SEQ3_20B_PAYLOAD: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_dt_seq3_20b_payload.bin");
const TP_EOMA_20B_PGN_EF00: &[u8; 8] = include_bytes!("../fixtures/j1939/tp_eoma_20b_pgn_ef00.bin");
const TP_RTS_MALFORMED_1786B_255PKTS_PGN_EF00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_rts_malformed_1786b_255pkts_pgn_ef00.bin");
const TP_ABORT_BAD_SEQUENCE_PGN_EF00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_abort_bad_sequence_pgn_ef00.bin");
const TP_ABORT_DUPLICATE_SEQUENCE_PGN_EF00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_abort_duplicate_sequence_pgn_ef00.bin");
const TP_ABORT_NO_RESOURCES_PGN_EF00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_abort_no_resources_pgn_ef00.bin");
const TP_ABORT_TIMEOUT_PGN_EF00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_abort_timeout_pgn_ef00.bin");
const TP_ABORT_UNEXPECTED_SIZE_PGN_EF00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/tp_abort_unexpected_size_pgn_ef00.bin");
const TP_MALFORMED_CM_DT_CORPUS_HEX: &str =
    include_str!("../fixtures/j1939/tp_malformed_cm_dt_corpus.hex");
const ETP_RTS_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_rts_2000b_pgn_ca00.bin");
const ETP_CTS_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_cts_2000b_pgn_ca00.bin");
const ETP_CTS_3_PACKETS_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_cts_3_packets_2000b_pgn_ca00.bin");
const ETP_CTS_NEXT4_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_cts_next4_2000b_pgn_ca00.bin");
const ETP_CTS_HOLD_PGN_CA00_HEX: &str = include_str!("../fixtures/j1939/etp_cts_hold_pgn_ca00.hex");
const ETP_DPO_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_dpo_2000b_pgn_ca00.bin");
const ETP_DPO_3_PACKETS_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_dpo_3_packets_2000b_pgn_ca00.bin");
const ETP_DT_SEQ1_2000B_PAYLOAD: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_dt_seq1_2000b_payload.bin");
const ETP_DT_SEQ3_2000B_PAYLOAD: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_dt_seq3_2000b_payload.bin");
const ETP_EOMA_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_eoma_2000b_pgn_ca00.bin");
const ETP_DPO_BAD_OFFSET_2000B_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_dpo_bad_offset_2000b_pgn_ca00.bin");
const ETP_RTS_TP_SIZED_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_rts_tp_sized_pgn_ca00.bin");
const ETP_RTS_OVER_MAX_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_rts_over_max_pgn_ca00.bin");
const ETP_ABORT_BAD_SEQUENCE_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_abort_bad_sequence_pgn_ca00.bin");
const ETP_ABORT_NO_RESOURCES_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_abort_no_resources_pgn_ca00.bin");
const ETP_ABORT_TIMEOUT_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_abort_timeout_pgn_ca00.bin");
const ETP_ABORT_UNEXPECTED_SIZE_PGN_CA00: &[u8; 8] =
    include_bytes!("../fixtures/j1939/etp_abort_unexpected_size_pgn_ca00.bin");
const ETP_MALFORMED_CM_DT_CORPUS_HEX: &str =
    include_str!("../fixtures/j1939/etp_malformed_cm_dt_corpus.hex");
const ETP_RTS_4096B_PGN_CA00_HEX: &str = include_str!("../fixtures/j1939/etp_rts_4096b_pgn_ca00.hex");
const ETP_CTS_4096B_PGN_CA00_HEX: &str = include_str!("../fixtures/j1939/etp_cts_4096b_pgn_ca00.hex");
const ETP_DPO_FIRST_4096B_PGN_CA00_HEX: &str =
    include_str!("../fixtures/j1939/etp_dpo_first_4096b_pgn_ca00.hex");
const ETP_DT_SEQ1_4096B_PAYLOAD_HEX: &str =
    include_str!("../fixtures/j1939/etp_dt_seq1_4096b_payload.hex");
const ETP_DPO_LAST_4096B_PGN_CA00_HEX: &str =
    include_str!("../fixtures/j1939/etp_dpo_last_4096b_pgn_ca00.hex");
const ETP_DT_SEQ10_LAST_4096B_PAYLOAD_HEX: &str =
    include_str!("../fixtures/j1939/etp_dt_seq10_last_4096b_payload.hex");
const ETP_EOMA_4096B_PGN_CA00_HEX: &str =
    include_str!("../fixtures/j1939/etp_eoma_4096b_pgn_ca00.hex");
const FP_10B_BAD_COUNTER: &[u8; 8] =
    include_bytes!("../fixtures/nmea/fast_packet_10b_bad_counter.bin");
const FP_8B_MALFORMED_FIRST: &[u8; 8] =
    include_bytes!("../fixtures/nmea/fast_packet_8b_malformed_first.bin");
const FP_224B_MALFORMED_FIRST: &[u8; 8] =
    include_bytes!("../fixtures/nmea/fast_packet_224b_malformed_first.bin");
const N2K_PRODUCT_INFO_MACHBUS_PAYLOAD: &[u8; 134] =
    include_bytes!("../fixtures/nmea/n2k_product_info_machbus_payload.bin");
const N2K_PRODUCT_INFO_MACHBUS_FRAME0: &[u8; 8] =
    include_bytes!("../fixtures/nmea/n2k_product_info_machbus_frame0.bin");
const N2K_PRODUCT_INFO_MACHBUS_FRAME19: &[u8; 8] =
    include_bytes!("../fixtures/nmea/n2k_product_info_machbus_frame19.bin");
const NMEA_GNSS_POSITION_RAPID_52N_5E_HEX: &str =
    include_str!("../fixtures/nmea/gnss_position_rapid_52n_5e.hex");
const NMEA_GNSS_COG_SOG_EAST_5_5MPS_HEX: &str =
    include_str!("../fixtures/nmea/gnss_cog_sog_east_5_5mps.hex");
const NMEA_HEADING_TRACK_1RAD_DEV_NEG0_1_VAR0_25_HEX: &str =
    include_str!("../fixtures/nmea/heading_track_1rad_dev_neg0_1_var0_25.hex");
const NMEA_MAGNETIC_VARIATION_HEX: &str = include_str!("../fixtures/nmea/magnetic_variation.hex");
const NMEA_SYSTEM_TIME_HEX: &str = include_str!("../fixtures/nmea/system_time.hex");
const NMEA_GNSS_POSITION_DATA_52N_5E_ALT12_345_RTK_HEX: &str =
    include_str!("../fixtures/nmea/gnss_position_data_52n_5e_alt12_345_rtk.hex");
const NMEA_GNSS_POSITION_DATA_FAST_PACKET_HEX: &str =
    include_str!("../fixtures/nmea/gnss_position_data_fast_packet.hex");
const NMEA_GNSS_SATS_IN_VIEW_FAST_PACKET_HEX: &str =
    include_str!("../fixtures/nmea/gnss_sats_in_view_fast_packet.hex");
const NMEA_GNSS_POSITION_DATA_REFERENCE_STATION_LENGTHS_HEX: &str =
    include_str!("../fixtures/nmea/gnss_position_data_reference_station_lengths.hex");
const NMEA_GNSS_DOPS_AUTO_3D_HDOP0_85_VDOP1_10_TDOP0_50_HEX: &str =
    include_str!("../fixtures/nmea/gnss_dops_auto_3d_hdop0_85_vdop1_10_tdop0_50.hex");
const NMEA_GNSS_DOPS_BAD_RESERVED_BITS_HEX: &str =
    include_str!("../fixtures/nmea/gnss_dops_bad_reserved_bits.hex");
const NMEA_GNSS_DOPS_BAD_RESERVED_MODE_HEX: &str =
    include_str!("../fixtures/nmea/gnss_dops_bad_reserved_mode.hex");
const NMEA_ATTITUDE_YAW1_PITCH_NEG0_1_ROLL0_25_HEX: &str =
    include_str!("../fixtures/nmea/attitude_yaw1_pitch_neg0_1_roll0_25.hex");
const N2K_CONFIG_INFO_BRIDGE_CABIN_ACMECO_HEX: &str =
    include_str!("../fixtures/nmea/n2k_config_info_bridge_cabin_acmeco.hex");
const N2K_CONFIG_INFO_MAX70_DESC1_HEX: &str =
    include_str!("../fixtures/nmea/n2k_config_info_max70_desc1.hex");
const N2K_CONFIG_INFO_OVERLONG_DESC1_HEX: &str =
    include_str!("../fixtures/nmea/n2k_config_info_overlong_desc1.hex");
const N2K_MANAGEMENT_MALFORMED_HEX: &str =
    include_str!("../fixtures/nmea/n2k_management_malformed.hex");
const N2K_CONFIG_INFO_FAST_PACKET_HEX: &str =
    include_str!("../fixtures/nmea/n2k_config_info_fast_packet.hex");
const N2K_HEARTBEAT_60000MS_SEQ5_CLASSES_AABB_HEX: &str =
    include_str!("../fixtures/nmea/n2k_heartbeat_60000ms_seq5_classes_aabb.hex");
const N2K_HEARTBEAT_BAD_RESERVED_BITS_HEX: &str =
    include_str!("../fixtures/nmea/n2k_heartbeat_bad_reserved_bits.hex");
const N2K_HEARTBEAT_BAD_TAIL_HEX: &str = include_str!("../fixtures/nmea/n2k_heartbeat_bad_tail.hex");
const NMEA_ENVIRONMENTAL_CODECS_HEX: &str = include_str!("../fixtures/nmea/environmental_codecs.hex");
const NMEA_NAVIGATION_POWER_CODECS_HEX: &str =
    include_str!("../fixtures/nmea/navigation_power_codecs.hex");
const NMEA0183_SERIAL_GNSS_MIXED_LOG: &str = include_str!("../fixtures/nmea/serial_gnss_mixed.nmea");
const NMEA0183_SERIAL_GNSS_GARBAGE_SOAK_LOG: &str =
    include_str!("../fixtures/nmea/serial_gnss_garbage_soak.nmea");

#[test]
fn fixture_agisostack_name_magic_raw_matches_builder_layout() {
    let raw = u64::from_le_bytes(*NAME_MAGIC_RAW_LE);
    let from_fields = Name::default()
        .with_self_configurable(true)
        .with_industry_group(1)
        .with_device_class(2)
        .with_function_code(3)
        .with_identity_number(4)
        .with_ecu_instance(5)
        .with_function_instance(6)
        .with_device_class_instance(7)
        .with_manufacturer_code(8);

    assert_eq!(raw, 10_881_826_125_818_888_196);
    assert_eq!(from_fields.raw, raw);
}

#[test]
fn fixture_isobus_can_physical_config_values_are_stable() {
    let fixture_u32 = |name| {
        parse_named_text_value(ISOBUS_CAN_BUS_CONFIG, name)
            .parse::<u32>()
            .unwrap()
    };
    let fixture_f64 = |name| {
        parse_named_text_value(ISOBUS_CAN_BUS_CONFIG, name)
            .parse::<f64>()
            .unwrap()
    };

    assert_eq!(ISO_CAN_BITRATE, fixture_u32("iso_bitrate"));
    assert!((ISO_SAMPLE_POINT_NOMINAL - fixture_f64("sample_point_nominal")).abs() < f64::EPSILON);
    assert!((ISO_SAMPLE_POINT_MIN - fixture_f64("sample_point_min")).abs() < f64::EPSILON);
    assert!((ISO_SAMPLE_POINT_MAX - fixture_f64("sample_point_max")).abs() < f64::EPSILON);

    let default_validation = validate_can_bus_config(&CanBusConfig::default());
    assert!(default_validation.overall_ok);
    assert!(default_validation.error_message.is_empty());

    let lower =
        validate_can_bus_config(&CanBusConfig::default().sample_point(ISO_SAMPLE_POINT_MIN));
    let upper =
        validate_can_bus_config(&CanBusConfig::default().sample_point(ISO_SAMPLE_POINT_MAX));
    assert!(lower.overall_ok);
    assert!(upper.overall_ok);

    let bad_bitrate = validate_can_bus_config(&CanBusConfig::default().bitrate(500_000));
    assert!(!bad_bitrate.overall_ok);
    assert_eq!(
        bad_bitrate.error_message,
        parse_named_text_value(ISOBUS_CAN_BUS_CONFIG, "wrong_bitrate_error")
    );

    let bad_sample_point =
        validate_can_bus_config(&CanBusConfig::default().sample_point(ISO_SAMPLE_POINT_MIN - 0.01));
    assert!(!bad_sample_point.overall_ok);
    assert_eq!(
        bad_sample_point.error_message,
        parse_named_text_value(ISOBUS_CAN_BUS_CONFIG, "bad_sample_point_error")
    );
}

#[test]
fn fixture_isobus_can_frame_wrapper_bytes_are_stable() {
    let hex_u32 = |name| {
        u32::from_str_radix(
            parse_named_text_value(ISOBUS_CAN_FRAME_WRAPPER_HEX, name),
            16,
        )
        .unwrap()
    };
    let fixture_len = |name| {
        parse_named_text_value(ISOBUS_CAN_FRAME_WRAPPER_HEX, name)
            .parse::<u8>()
            .unwrap()
    };

    let request = Frame::from_message(Priority::Default, PGN_REQUEST, 0x10, 0x42, &[1, 2, 3]);
    assert_eq!(request.id.raw, hex_u32("request_pdu1_raw_id"));
    assert_eq!(
        request.data,
        parse_named_hex_frame(ISOBUS_CAN_FRAME_WRAPPER_HEX, "request_pdu1_payload")
    );
    assert_eq!(request.length, fixture_len("request_pdu1_length"));
    assert_eq!(request.pgn(), PGN_REQUEST);
    assert_eq!(request.destination(), 0x42);

    let round_trip = Frame::from_can_frame(&request.to_can_frame()).unwrap();
    assert_eq!(round_trip.id, request.id);
    assert_eq!(round_trip.payload(), request.payload());

    let dm1 = Frame::from_message(Priority::Default, PGN_DM1, 0x80, 0x42, &[0xAA]);
    assert_eq!(dm1.id.raw, hex_u32("dm1_pdu2_raw_id"));
    assert_eq!(
        dm1.data,
        parse_named_hex_frame(ISOBUS_CAN_FRAME_WRAPPER_HEX, "dm1_pdu2_payload")
    );
    assert_eq!(dm1.length, fixture_len("dm1_pdu2_length"));
    assert_eq!(dm1.destination(), BROADCAST_ADDRESS);
    assert!(dm1.is_broadcast());
}

#[test]
fn fixture_j1939_time_date_decodes_and_reencodes() {
    let msg = Message::new(PGN_TIME_DATE, TIME_DATE_AGISOSTACK.to_vec(), 0x47);
    let td = TimeDate::decode(&msg).expect("fixture must decode");

    assert_eq!(td.year, Some(2023));
    assert_eq!(td.month, Some(8));
    assert_eq!(td.day, Some(7));
    assert_eq!(td.hours, Some(22));
    assert_eq!(td.minutes, Some(49));
    assert_eq!(td.seconds, Some(41));
    assert_eq!(td.utc_offset_hours, Some(-5));
    assert_eq!(td.utc_offset_min, Some(0));
    assert_eq!(td.encode(), *TIME_DATE_AGISOSTACK);
    assert_eq!(
        parse_named_hex_frame(J1939_TIME_DATE_HEX, "time_date_agisostack"),
        *TIME_DATE_AGISOSTACK
    );

    let default_payload = parse_named_hex_frame(J1939_TIME_DATE_HEX, "time_date_default");
    let default_msg = Message::new(PGN_TIME_DATE, default_payload.to_vec(), 0x80);
    let default_td = TimeDate::decode(&default_msg).expect("default fixture must decode");
    assert_eq!(default_td.encode(), default_payload);
    assert!(default_td.seconds.is_none());
    assert!(default_td.year.is_none());

    let first_day = parse_named_hex_frame(J1939_TIME_DATE_HEX, "time_date_first_day_1985_utc0");
    assert_eq!(
        TimeDate {
            seconds: Some(0),
            minutes: Some(0),
            hours: Some(0),
            day: Some(1),
            month: Some(1),
            year: Some(1985),
            utc_offset_min: Some(0),
            utc_offset_hours: Some(0),
            timestamp_us: 0,
        }
        .encode(),
        first_day
    );
    let decoded = TimeDate::decode(&Message::new(PGN_TIME_DATE, first_day.to_vec(), 0x80))
        .expect("first day fixture must decode");
    assert_eq!(decoded.seconds, Some(0));
    assert_eq!(decoded.minutes, Some(0));
    assert_eq!(decoded.hours, Some(0));
    assert_eq!(decoded.day, Some(1));
    assert_eq!(decoded.month, Some(1));
    assert_eq!(decoded.year, Some(1985));
    assert_eq!(decoded.utc_offset_min, Some(0));
    assert_eq!(decoded.utc_offset_hours, Some(0));

    let upper = parse_named_hex_frame(J1939_TIME_DATE_HEX, "time_date_upper_non_na");
    assert_eq!(
        TimeDate {
            seconds: Some(63),
            minutes: Some(254),
            hours: Some(254),
            day: Some(63),
            month: Some(254),
            year: Some(2239),
            utc_offset_min: Some(125),
            utc_offset_hours: Some(125),
            timestamp_us: 0,
        }
        .encode(),
        upper
    );
    let decoded = TimeDate::decode(&Message::new(PGN_TIME_DATE, upper.to_vec(), 0x80))
        .expect("upper non-NA fixture must decode");
    assert_eq!(decoded.seconds, Some(63));
    assert_eq!(decoded.minutes, Some(254));
    assert_eq!(decoded.hours, Some(254));
    assert_eq!(decoded.day, Some(63));
    assert_eq!(decoded.month, Some(254));
    assert_eq!(decoded.year, Some(2239));
    assert_eq!(decoded.utc_offset_min, Some(125));
    assert_eq!(decoded.utc_offset_hours, Some(125));

    let clamped = parse_named_hex_frame(J1939_TIME_DATE_HEX, "time_date_clamped_high");
    assert_eq!(
        TimeDate {
            seconds: Some(250),
            minutes: Some(255),
            hours: Some(255),
            day: Some(250),
            month: Some(255),
            year: Some(3000),
            utc_offset_min: Some(500),
            utc_offset_hours: Some(127),
            timestamp_us: 0,
        }
        .encode(),
        clamped
    );
    assert!(
        clamped[..6].iter().all(|&byte| byte != 0xFF),
        "Some values must not encode as not-available"
    );

    for name in ["time_date_short", "time_date_overlong"] {
        assert_eq!(
            TimeDate::decode(&Message::new(
                PGN_TIME_DATE,
                parse_named_hex_bytes(J1939_TIME_DATE_HEX, name),
                0x80
            )),
            None,
            "{name}"
        );
    }
}

#[test]
fn fixture_candump_trace_payload_matches_time_date_bytes() {
    let payload_hex = TIME_DATE_CANDUMP
        .trim()
        .split_once('#')
        .expect("candump line must contain #")
        .1;
    let parsed = parse_hex_bytes(payload_hex);
    assert_eq!(parsed, TIME_DATE_AGISOSTACK);
}

#[test]
fn fixture_bracketed_candump_trace_payload_matches_time_date_bytes() {
    let (raw_id, data, extended) = parse_candump_fixture_line(BRACKETED_TIME_DATE_CANDUMP.trim())
        .expect("bracketed candump line must parse");
    assert_eq!(raw_id, 0x18FEE647);
    assert!(extended);
    assert_eq!(data, TIME_DATE_AGISOSTACK);
}

#[test]
fn fixture_malformed_candump_lines_are_ignored_before_driver_conversion() {
    for line in MALFORMED_CANDUMP.lines().filter(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with('#')
    }) {
        assert!(parse_candump_fixture_line(line).is_none(), "{line}");
    }
}

#[test]
fn fixture_candump_trace_rejects_standard_ids_without_ext_promotion() {
    let mut accepted = Vec::new();
    let mut rejected_raw_ids = Vec::new();

    for line in STANDARD_ID_REJECTION_CANDUMP.lines() {
        let Some((raw_id, data, extended)) = parse_candump_fixture_line(line) else {
            continue;
        };
        let can = if extended {
            CanFrame::make_ext(raw_id, &data)
        } else {
            CanFrame::make_std(raw_id, &data)
        };
        if let Some(frame) = Frame::from_can_frame(&can) {
            accepted.push((raw_id, frame));
        } else {
            rejected_raw_ids.push(raw_id);
        }
    }

    assert_eq!(rejected_raw_ids, vec![0x123, 0x7FF]);
    assert_eq!(accepted.len(), 1);
    assert_eq!(accepted[0].0, 0x18FEE680);
    assert_eq!(accepted[0].1.pgn(), PGN_TIME_DATE);
    assert_eq!(accepted[0].1.source(), 0x80);
    assert_eq!(accepted[0].1.payload(), TIME_DATE_AGISOSTACK);
}

#[test]
fn fixture_j1939_request_and_ack_codecs_are_stable() {
    assert_eq!(
        decode_request(REQUEST_ADDRESS_CLAIM),
        Some(PGN_ADDRESS_CLAIMED)
    );
    assert_eq!(
        encode_request(PGN_ADDRESS_CLAIMED).unwrap(),
        *REQUEST_ADDRESS_CLAIM
    );

    assert_eq!(
        parse_named_hex_bytes(J1939_REQUEST_ACK_HEX, "request_address_claim_3"),
        REQUEST_ADDRESS_CLAIM
    );
    assert_eq!(
        decode_request(&parse_named_hex_bytes(
            J1939_REQUEST_ACK_HEX,
            "request_address_claim_ff_padded"
        )),
        Some(PGN_ADDRESS_CLAIMED)
    );
    for name in [
        "request_address_claim_len4",
        "request_address_claim_bad_padding",
        "request_address_claim_len9",
        "request_invalid_pgn_high_bits",
    ] {
        assert_eq!(
            decode_request(&parse_named_hex_bytes(J1939_REQUEST_ACK_HEX, name)),
            None,
            "{name}"
        );
    }

    let ack = Acknowledgment::decode(ACK_REQUEST_TO_0X42).expect("ACK fixture must decode");
    assert_eq!(ack.control, AckControl::PositiveAck);
    assert_eq!(ack.group_function, 0xFF);
    assert_eq!(ack.address, 0x42);
    assert_eq!(ack.acknowledged_pgn, PGN_REQUEST);
    assert_eq!(ack.encode().unwrap(), *ACK_REQUEST_TO_0X42);
    assert_eq!(
        parse_named_hex_frame(J1939_REQUEST_ACK_HEX, "ack_request_to_42"),
        *ACK_REQUEST_TO_0X42
    );
    for name in [
        "ack_request_short",
        "ack_request_overlong",
        "ack_request_reserved_control",
        "ack_request_bad_reserved_bytes",
        "ack_request_invalid_pgn_high_bits",
    ] {
        assert_eq!(
            Acknowledgment::decode(&parse_named_hex_bytes(J1939_REQUEST_ACK_HEX, name)),
            None,
            "{name}"
        );
    }
}

#[test]
fn fixture_j1939_language_shortcut_request2_and_transfer_codecs_are_stable() {
    let default_language = parse_named_hex_frame(
        J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
        "language_default_en_metric",
    );
    let default_language_msg = Message::new(PGN_LANGUAGE_COMMAND, default_language.to_vec(), 0x80);
    assert_eq!(LanguageData::default().encode(), default_language);
    assert_eq!(
        LanguageData::decode(&default_language_msg),
        Some(LanguageData::default())
    );

    let mixed_language = parse_named_hex_frame(
        J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
        "language_de_mixed_units",
    );
    let mixed_language_data = LanguageData {
        language_code: [b'd', b'e'],
        decimal: DecimalSymbol::Comma,
        time_format: TimeFormat::TwelveHour,
        date_format: DateFormat::YyyyMmDd,
        distance: DistanceUnit::Imperial,
        area: AreaUnit::Us,
        volume: VolumeUnit::Imperial,
        mass: MassUnit::Us,
        temperature: TemperatureUnit::Imperial,
        pressure: PressureUnit::Imperial,
        force: ForceUnit::Imperial,
        country_code: [b'D', b'E'],
        generic: UnitSystem::Us,
    };
    assert_eq!(mixed_language_data.encode(), mixed_language);
    assert_eq!(
        LanguageData::decode(&Message::new(
            PGN_LANGUAGE_COMMAND,
            mixed_language.to_vec(),
            0x80,
        )),
        Some(mixed_language_data)
    );
    assert!(
        LanguageData::decode(&Message::new(
            PGN_LANGUAGE_COMMAND,
            default_language[..7].to_vec(),
            0x80,
        ))
        .is_none()
    );
    assert!(
        LanguageData::decode(&Message::new(
            PGN_LANGUAGE_COMMAND,
            [default_language.as_slice(), &[0x00]].concat(),
            0x80,
        ))
        .is_none()
    );
    for malformed in [
        "language_bad_reserved_bits",
        "language_bad_reserved_unit3",
        "language_bad_reserved_tail",
    ] {
        let payload = parse_named_hex_frame(J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX, malformed);
        assert!(
            LanguageData::decode(&Message::new(PGN_LANGUAGE_COMMAND, payload.to_vec(), 0x80,))
                .is_none(),
            "{malformed} must be rejected"
        );
    }

    let shortcut_stop =
        parse_named_hex_frame(J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX, "shortcut_stop");
    assert_eq!(
        shortcut_button::encode(ShortcutButtonState::StopImplementOperations),
        shortcut_stop
    );
    assert_eq!(
        shortcut_button::decode(&Message::new(
            PGN_SHORTCUT_BUTTON,
            shortcut_stop.to_vec(),
            0x80,
        )),
        Some(ShortcutButtonState::StopImplementOperations)
    );
    let shortcut_permit = parse_named_hex_frame(
        J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
        "shortcut_permit_counter9",
    );
    assert_eq!(
        shortcut_button::encode_with_transition_count(
            ShortcutButtonState::PermitAllImplementsToOperate,
            9,
        ),
        shortcut_permit
    );
    assert_eq!(
        shortcut_button::decode_message(&Message::new(
            PGN_SHORTCUT_BUTTON,
            shortcut_permit.to_vec(),
            0x80,
        )),
        Some(shortcut_button::ShortcutButtonMessage {
            state: ShortcutButtonState::PermitAllImplementsToOperate,
            transition_count: 9,
        })
    );
    let shortcut_not_available = parse_named_hex_frame(
        J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
        "shortcut_not_available",
    );
    assert_eq!(
        shortcut_button::decode(&Message::new(
            PGN_SHORTCUT_BUTTON,
            shortcut_not_available.to_vec(),
            0x80,
        )),
        Some(ShortcutButtonState::NotAvailable)
    );
    assert!(
        shortcut_button::decode(&Message::new(
            PGN_SHORTCUT_BUTTON,
            shortcut_stop[..7].to_vec(),
            0x80,
        ))
        .is_none()
    );
    assert!(
        shortcut_button::decode(&Message::new(
            PGN_SHORTCUT_BUTTON,
            [shortcut_stop.as_slice(), &[0x00]].concat(),
            0x80,
        ))
        .is_none()
    );
    for malformed in ["shortcut_bad_reserved_bits", "shortcut_bad_reserved_tail"] {
        let payload = parse_named_hex_frame(J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX, malformed);
        assert!(
            shortcut_button::decode(&Message::new(PGN_SHORTCUT_BUTTON, payload.to_vec(), 0x80,))
                .is_none(),
            "{malformed} must be rejected"
        );
    }

    let request2_transfer = parse_named_hex_frame(
        J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
        "request2_cafe_ext_transfer",
    );
    let request2_transfer_msg = Request2Msg {
        requested_pgn: 0xCAFE,
        extended_id: vec![0x01, 0x02, 0x03],
        use_transfer: true,
    };
    assert_eq!(request2_transfer_msg.encode().unwrap(), request2_transfer);
    assert_eq!(
        Request2Msg::from_message(&Message::new(
            PGN_REQUEST2,
            request2_transfer.to_vec(),
            0x80,
        )),
        Some(request2_transfer_msg)
    );

    let request2_direct = parse_named_hex_frame(
        J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
        "request2_request_noext_direct",
    );
    let request2_direct_msg = Request2Msg {
        requested_pgn: PGN_REQUEST,
        extended_id: vec![],
        use_transfer: false,
    };
    assert_eq!(request2_direct_msg.encode().unwrap(), request2_direct);
    assert_eq!(
        Request2Msg::decode(&request2_direct),
        Some(request2_direct_msg)
    );
    assert!(Request2Msg::decode(&request2_direct[..7]).is_none());
    assert!(Request2Msg::decode(&[request2_direct.as_slice(), &[0x00]].concat()).is_none());
    for malformed in [
        "request2_bad_reserved_control",
        "request2_bad_extended_id_hole",
        "request2_bad_reserved_tail",
        "request2_bad_pgn_high_bits",
    ] {
        assert!(
            Request2Msg::decode(&parse_named_hex_frame(
                J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }

    let transfer_payload = parse_named_hex_bytes(
        J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
        "transfer_time_date_payload",
    );
    let transfer_msg = TransferMsg {
        original_pgn: PGN_TIME_DATE,
        data: TIME_DATE_AGISOSTACK.to_vec(),
    };
    assert_eq!(transfer_msg.encode().unwrap(), transfer_payload);
    assert_eq!(TransferMsg::decode(&transfer_payload), Some(transfer_msg));
    assert!(TransferMsg::decode(&transfer_payload[..2]).is_none());
    assert!(
        TransferMsg::decode(&parse_named_hex_bytes(
            J1939_LANGUAGE_SHORTCUT_REQUEST2_HEX,
            "transfer_bad_original_pgn_high_bits",
        ))
        .is_none()
    );
}

#[test]
fn fixture_j1939_dm_memory_codecs_are_stable() {
    let dm14 = parse_named_hex_frame(J1939_DM_MEMORY_HEX, "dm14_write_virtual");
    let dm14_msg = Dm14Request {
        command: Dm14Command::Write,
        pointer_type: Dm14PointerType::DirectVirtual,
        address: 0x12_3456,
        length: 0xCAFE,
        key: 0xAB,
    };
    assert_eq!(dm14_msg.encode().unwrap(), dm14);
    assert_eq!(
        Dm14Request::from_message(&Message::new(PGN_DM14, dm14.to_vec(), 0x80)),
        Some(dm14_msg)
    );
    assert!(Dm14Request::decode(&dm14[..7]).is_none());
    assert!(Dm14Request::decode(&[dm14.as_slice(), &[0x00]].concat()).is_none());
    for malformed in ["dm14_bad_reserved_control", "dm14_bad_reserved_command"] {
        assert!(
            Dm14Request::decode(&parse_named_hex_frame(J1939_DM_MEMORY_HEX, malformed)).is_none(),
            "{malformed} should be rejected"
        );
    }

    let dm15 = parse_named_hex_frame(J1939_DM_MEMORY_HEX, "dm15_completed");
    let dm15_msg = Dm15Response {
        status: Dm15Status::Completed,
        length: 0xBEEF,
        address: 0x98_7654,
        edcp_extension: 0x42,
        seed: 0x99,
    };
    assert_eq!(dm15_msg.encode().unwrap(), dm15);
    assert_eq!(Dm15Response::decode(&dm15), Some(dm15_msg));
    assert!(Dm15Response::decode(&dm15[..7]).is_none());
    assert!(Dm15Response::decode(&[dm15.as_slice(), &[0x00]].concat()).is_none());
    for malformed in [
        "dm15_bad_reserved_status_bit",
        "dm15_bad_reserved_status_value",
    ] {
        assert!(
            Dm15Response::decode(&parse_named_hex_frame(J1939_DM_MEMORY_HEX, malformed)).is_none(),
            "{malformed} should be rejected"
        );
    }

    let dm16 = parse_named_hex_frame(J1939_DM_MEMORY_HEX, "dm16_five_bytes");
    let dm16_msg = Dm16Transfer {
        num_bytes: 5,
        data: vec![1, 2, 3, 4, 5],
    };
    assert_eq!(dm16_msg.encode().unwrap(), dm16);
    assert_eq!(Dm16Transfer::decode(&dm16), Some(dm16_msg));
    for malformed in [
        "dm16_bad_tail",
        "dm16_bad_classic_count",
        "dm16_bad_declared_length",
    ] {
        assert!(
            Dm16Transfer::decode(&parse_named_hex_bytes(J1939_DM_MEMORY_HEX, malformed)).is_none(),
            "{malformed} should be rejected"
        );
    }
    assert_eq!(
        Dm16Transfer::decode(&parse_named_hex_bytes(
            J1939_DM_MEMORY_HEX,
            "dm16_exact_large"
        )),
        Some(Dm16Transfer {
            num_bytes: 8,
            data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        })
    );

    let ecu_id = parse_named_hex_bytes(J1939_DM_MEMORY_HEX, "ecu_identification");
    let ecu_id_msg = EcuIdentification {
        ecu_part_number: "ABC123".into(),
        ecu_serial_number: "SN-001".into(),
        ecu_location: "MainCab".into(),
        ecu_type: "TECU".into(),
        ecu_manufacturer: "Acme".into(),
        ecu_hardware_id: None,
    };
    assert_eq!(ecu_id_msg.encode().unwrap(), ecu_id);
    assert_eq!(EcuIdentification::decode(&ecu_id), Some(ecu_id_msg));

    let agisostack_iso_ecu_id =
        parse_named_hex_bytes(J1939_DM_MEMORY_HEX, "ecu_identification_agisostack_iso");
    let agisostack_ecu_id_msg = EcuIdentification {
        ecu_part_number: "1234".into(),
        ecu_serial_number: "9876".into(),
        ecu_location: "The Internet".into(),
        ecu_type: "AgISOStack".into(),
        ecu_manufacturer: "None".into(),
        ecu_hardware_id: Some("Some Hardware ID".into()),
    };
    assert_eq!(
        agisostack_ecu_id_msg.encode_iso11783().unwrap(),
        agisostack_iso_ecu_id
    );
    assert_eq!(
        EcuIdentification::decode(&agisostack_iso_ecu_id),
        Some(agisostack_ecu_id_msg.clone())
    );
    assert_eq!(
        agisostack_ecu_id_msg.encode_j1939().unwrap(),
        parse_named_hex_bytes(J1939_DM_MEMORY_HEX, "ecu_identification_agisostack_j1939")
    );
    for malformed in [
        "ecu_identification_missing_field",
        "ecu_identification_trailing_data",
    ] {
        assert!(
            EcuIdentification::decode(&parse_named_hex_bytes(J1939_DM_MEMORY_HEX, malformed))
                .is_none(),
            "{malformed} should be rejected"
        );
    }
    assert_eq!(
        EcuIdentification::decode(&parse_named_hex_bytes(
            J1939_DM_MEMORY_HEX,
            "ecu_identification_extended_latin1"
        )),
        Some(EcuIdentification {
            ecu_part_number: "ABC123".into(),
            ecu_serial_number: "SN-001".into(),
            ecu_location: "MainCab".into(),
            ecu_type: "TECU".into(),
            ecu_manufacturer: "Acmeÿ".into(),
            ecu_hardware_id: None,
        })
    );
}

#[test]
fn fixture_j1939_address_claim_frame_bytes_are_stable() {
    let name = Name::from_raw(u64::from_le_bytes(*NAME_MAGIC_RAW_LE));
    let mut cf = InternalCf::new(name, 0, 0x80);
    let mut claimer = AddressClaimer::new(0);
    let frames = claimer.start(&mut cf);

    assert_eq!(frames.len(), 2);

    let request_raw_id = parse_hex_u64(parse_named_text_value(
        J1939_ADDRESS_CLAIM_HEX,
        "request_address_claim_raw_id",
    )) as u32;
    let request_payload =
        parse_named_hex_frame(J1939_ADDRESS_CLAIM_HEX, "request_address_claim_payload");
    assert_eq!(frames[0].id.raw, request_raw_id);
    assert_eq!(frames[0].pgn(), PGN_REQUEST);
    assert_eq!(frames[0].source(), 0xFE);
    assert_eq!(frames[0].destination(), BROADCAST_ADDRESS);
    assert_eq!(frames[0].data, request_payload);
    assert_eq!(decode_request(&request_payload), Some(PGN_ADDRESS_CLAIMED));
    for malformed in [
        "malformed_request_address_claim_bad_padding",
        "malformed_request_address_claim_overlong",
    ] {
        assert!(
            decode_request(&parse_named_hex_bytes(J1939_ADDRESS_CLAIM_HEX, malformed)).is_none(),
            "{malformed} must be rejected"
        );
    }

    let claim_raw_id = parse_hex_u64(parse_named_text_value(
        J1939_ADDRESS_CLAIM_HEX,
        "claim_magic_addr80_raw_id",
    )) as u32;
    let claim_payload =
        parse_named_hex_frame(J1939_ADDRESS_CLAIM_HEX, "claim_magic_addr80_payload");
    assert_eq!(frames[1].id.raw, claim_raw_id);
    assert_eq!(frames[1].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(frames[1].source(), 0x80);
    assert_eq!(frames[1].destination(), BROADCAST_ADDRESS);
    assert_eq!(frames[1].data, claim_payload);

    let cannot_claim = claimer.handle_duplicate_name(&mut cf);
    assert_eq!(cannot_claim.len(), 1);
    let cannot_raw_id = parse_hex_u64(parse_named_text_value(
        J1939_ADDRESS_CLAIM_HEX,
        "cannot_claim_magic_raw_id",
    )) as u32;
    let cannot_payload =
        parse_named_hex_frame(J1939_ADDRESS_CLAIM_HEX, "cannot_claim_magic_payload");
    assert_eq!(cannot_claim[0].id.raw, cannot_raw_id);
    assert_eq!(cannot_claim[0].pgn(), PGN_ADDRESS_CLAIMED);
    assert_eq!(cannot_claim[0].source(), 0xFE);
    assert_eq!(cannot_claim[0].destination(), BROADCAST_ADDRESS);
    assert_eq!(cannot_claim[0].data, cannot_payload);

    let mut overlong_name = name.to_bytes().to_vec();
    overlong_name.push(0xFF);
    assert!(
        Name::from_bytes(&overlong_name).is_none(),
        "NAME decode must reject prefix-compatible overlong payloads"
    );

    let mut name_manager = NameManager::new();
    let commanded_address =
        parse_named_hex_bytes(J1939_ADDRESS_CLAIM_HEX, "commanded_address_magic_to_42");
    assert_eq!(
        name_manager.handle_commanded_address(
            &Message::new(PGN_COMMANDED_ADDRESS, commanded_address, 0x10),
            name,
        ),
        Some(0x42)
    );
    assert!(
        name_manager
            .handle_commanded_address(
                &Message::new(
                    PGN_COMMANDED_ADDRESS,
                    parse_named_hex_bytes(
                        J1939_ADDRESS_CLAIM_HEX,
                        "malformed_commanded_address_overlong",
                    ),
                    0x10,
                ),
                name,
            )
            .is_none(),
        "Commanded Address must reject prefix-compatible overlong payloads"
    );
}

#[test]
fn fixture_isobus_name_management_codecs_are_stable() {
    let name = Name::from_raw(u64::from_le_bytes(*NAME_MAGIC_RAW_LE));
    let request_current_response =
        NameManagementMsg::for_name(NameMgmtMode::RequestCurrentResponse, name);
    let request_current_response_bytes =
        parse_named_hex_bytes(ISOBUS_NAME_MANAGEMENT_HEX, "request_current_response_magic");
    assert_eq!(
        request_current_response.encode(),
        request_current_response_bytes
    );
    assert_eq!(
        NameManagementMsg::decode(&request_current_response_bytes),
        Some(request_current_response)
    );

    let mut nack = NameManagementMsg::for_name(NameMgmtMode::NegativeAcknowledge, name);
    nack.nack_reason = NameNackReason::Conflict;
    let nack_bytes = parse_named_hex_bytes(ISOBUS_NAME_MANAGEMENT_HEX, "nack_conflict_magic");
    assert_eq!(nack.encode(), nack_bytes);
    assert_eq!(NameManagementMsg::decode(&nack_bytes), Some(nack));

    for malformed in [
        "malformed_short16",
        "malformed_overlong18",
        "malformed_unknown_mode",
        "malformed_bad_padding",
        "malformed_bad_nack_reason",
        "malformed_bad_nack_padding",
    ] {
        assert!(
            NameManagementMsg::decode(&parse_named_hex_bytes(
                ISOBUS_NAME_MANAGEMENT_HEX,
                malformed,
            ))
            .is_none(),
            "{malformed} must be rejected"
        );
    }
}

#[test]
fn fixture_j1939_proprietary_single_frame_codecs_are_stable() {
    let proprietary_a =
        parse_named_hex_frame(J1939_PROPRIETARY_CODECS_HEX, "proprietary_a_payload");
    let frame_a = Frame::from_message(
        Priority::Default,
        PGN_PROPRIETARY_A,
        0x80,
        0x42,
        &proprietary_a,
    );
    assert_eq!(
        frame_a.id.raw,
        parse_hex_u64(parse_named_text_value(
            J1939_PROPRIETARY_CODECS_HEX,
            "proprietary_a_dest42_src80_raw_id",
        )) as u32
    );
    assert_eq!(frame_a.pgn(), PGN_PROPRIETARY_A);
    assert_eq!(frame_a.source(), 0x80);
    assert_eq!(frame_a.destination(), 0x42);
    assert_eq!(frame_a.data, proprietary_a);
    let msg_a = Message {
        pgn: frame_a.pgn(),
        source: frame_a.source(),
        destination: frame_a.destination(),
        priority: frame_a.priority(),
        timestamp_us: frame_a.timestamp_us,
        data: frame_a.payload().to_vec(),
    };
    let decoded_a = ProprietaryMsg::from_message(&msg_a).expect("proprietary A decodes");
    assert!(decoded_a.is_proprietary_a());
    assert!(!decoded_a.is_proprietary_b());
    assert_eq!(decoded_a.destination, 0x42);

    let proprietary_a2 =
        parse_named_hex_frame(J1939_PROPRIETARY_CODECS_HEX, "proprietary_a2_payload");
    let frame_a2 = Frame::from_message(
        Priority::Default,
        PGN_PROPRIETARY_A2,
        0x80,
        0x42,
        &proprietary_a2,
    );
    assert_eq!(
        frame_a2.id.raw,
        parse_hex_u64(parse_named_text_value(
            J1939_PROPRIETARY_CODECS_HEX,
            "proprietary_a2_dest42_src80_raw_id",
        )) as u32
    );
    let msg_a2 = Message {
        pgn: frame_a2.pgn(),
        source: frame_a2.source(),
        destination: frame_a2.destination(),
        priority: frame_a2.priority(),
        timestamp_us: frame_a2.timestamp_us,
        data: frame_a2.payload().to_vec(),
    };
    assert!(
        ProprietaryMsg::from_message(&msg_a2)
            .expect("proprietary A2 decodes")
            .is_proprietary_a2()
    );

    let proprietary_b =
        parse_named_hex_frame(J1939_PROPRIETARY_CODECS_HEX, "proprietary_b_ge42_payload");
    let frame_b = Frame::from_message(
        Priority::Default,
        proprietary_b_pgn(0x42),
        0x80,
        0x55,
        &proprietary_b,
    );
    assert_eq!(
        frame_b.id.raw,
        parse_hex_u64(parse_named_text_value(
            J1939_PROPRIETARY_CODECS_HEX,
            "proprietary_b_ge42_src80_raw_id",
        )) as u32
    );
    assert_eq!(frame_b.pgn(), proprietary_b_pgn(0x42));
    assert_eq!(frame_b.destination(), BROADCAST_ADDRESS);
    assert_eq!(frame_b.data, proprietary_b);
    let msg_b = Message {
        pgn: frame_b.pgn(),
        source: frame_b.source(),
        destination: frame_b.destination(),
        priority: frame_b.priority(),
        timestamp_us: frame_b.timestamp_us,
        data: frame_b.payload().to_vec(),
    };
    let decoded_b = ProprietaryMsg::from_message(&msg_b).expect("proprietary B decodes");
    assert!(decoded_b.is_proprietary_b());
    assert_eq!(decoded_b.group_extension(), 0x42);
}

#[test]
fn fixture_isobus_legacy_file_transfer_enums_are_stable() {
    let operation_codes = parse_named_hex_bytes(ISOBUS_LEGACY_FILE_TRANSFER_HEX, "operation_codes");
    let expected_ops = [
        FileOperation::Read,
        FileOperation::Write,
        FileOperation::Delete,
        FileOperation::List,
        FileOperation::GetAttributes,
        FileOperation::SetAttributes,
        FileOperation::OpenFile,
        FileOperation::CloseFile,
        FileOperation::ReadData,
        FileOperation::WriteData,
        FileOperation::SeekFile,
        FileOperation::GetCurrentDir,
        FileOperation::ChangeCurrentDir,
        FileOperation::MakeDir,
        FileOperation::RemoveDir,
        FileOperation::MoveFile,
        FileOperation::CopyFile,
        FileOperation::GetFileSize,
        FileOperation::GetFreeSpace,
        FileOperation::GetVolumeInfo,
        FileOperation::GetServerStatus,
    ];
    assert_eq!(operation_codes.len(), expected_ops.len());
    for (&raw, &op) in operation_codes.iter().zip(expected_ops.iter()) {
        assert_eq!(raw, op.as_u8());
        assert_eq!(FileOperation::from_u8(raw), Some(op));
    }
    for reserved in [0x00, 0x07, 0x0F, 0x15, 0x24, 0x32, 0x42, 0x51, 0xFF] {
        assert_eq!(FileOperation::from_u8(reserved), None);
    }

    let error_codes = parse_named_hex_bytes(ISOBUS_LEGACY_FILE_TRANSFER_HEX, "error_codes");
    let expected_errors = [
        FileTransferError::NoError,
        FileTransferError::FileNotFound,
        FileTransferError::AccessDenied,
        FileTransferError::DiskFull,
        FileTransferError::InvalidFilename,
        FileTransferError::ServerBusy,
        FileTransferError::InvalidHandle,
        FileTransferError::EndOfFile,
        FileTransferError::VolumeNotMounted,
        FileTransferError::IoError,
        FileTransferError::InvalidSeekPosition,
        FileTransferError::InvalidParameter,
        FileTransferError::FileAlreadyOpen,
        FileTransferError::DirectoryNotEmpty,
        FileTransferError::Unknown,
    ];
    assert_eq!(error_codes.len(), expected_errors.len());
    for (&raw, &err) in error_codes.iter().zip(expected_errors.iter()) {
        assert_eq!(FileTransferError::from_u8(raw), err);
        assert_eq!(err.as_u8(), raw);
    }
    assert_eq!(FileTransferError::from_u8(0xAA), FileTransferError::Unknown);

    let attr_mask = parse_named_hex_bytes(
        ISOBUS_LEGACY_FILE_TRANSFER_HEX,
        "attribute_mask_readonly_hidden_dir_archive",
    )[0];
    assert_eq!(
        attr_mask,
        FileAttribute::ReadOnly.as_u8()
            | FileAttribute::Hidden.as_u8()
            | FileAttribute::Directory.as_u8()
            | FileAttribute::Archive.as_u8()
    );
    let props = FileProperties {
        name: "LOGS".into(),
        attributes: attr_mask,
        ..Default::default()
    };
    assert!(props.is_read_only());
    assert!(props.is_hidden());
    assert!(props.is_directory());

    let default_config = parse_named_hex_bytes(ISOBUS_LEGACY_FILE_TRANSFER_HEX, "default_config");
    let cfg = LegacyFileServerConfig::default();
    assert_eq!(default_config[0], 0x00);
    assert_eq!(default_config[1], cfg.max_open_files);
    assert_eq!(cfg.status_interval_ms, FILE_SERVER_STATUS_INTERVAL_MS);
    assert_eq!(FILE_SERVER_BUSY_STATUS_INTERVAL_MS, 200);
    assert_eq!(FS_REQUEST_TIMEOUT_MS, 6_000);
}

#[test]
fn fixture_isobus_aux_and_group_function_codecs_are_stable() {
    let aux_o = parse_named_hex_frame(ISOBUS_AUX_GROUP_CODECS_HEX, "aux_o_type1_variable_5000");
    let aux_o_msg = AuxOFunction {
        function_number: 7,
        r#type: AuxFunctionType::Type1,
        state: AuxFunctionState::Variable,
        setpoint: 5000,
    };
    assert_eq!(aux_o_msg.encode(), aux_o);
    assert_eq!(
        AuxOFunction::decode(&Message::new(PGN_AUX_INPUT_STATUS, aux_o.to_vec(), 0x80)),
        Some(aux_o_msg)
    );
    assert!(
        AuxOFunction::decode(&Message::new(
            PGN_AUX_INPUT_STATUS,
            aux_o[..7].to_vec(),
            0x80,
        ))
        .is_none()
    );
    assert!(
        AuxOFunction::decode(&Message::new(
            PGN_AUX_INPUT_STATUS,
            [aux_o.as_slice(), &[0x00]].concat(),
            0x80,
        ))
        .is_none()
    );
    for name in [
        "aux_o_bad_reserved_type",
        "aux_o_bad_reserved_state",
        "aux_o_bad_reserved_tail",
    ] {
        assert!(
            AuxOFunction::decode(&Message::new(
                PGN_AUX_INPUT_STATUS,
                parse_named_hex_bytes(ISOBUS_AUX_GROUP_CODECS_HEX, name),
                0x80,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }

    let aux_n = parse_named_hex_frame(ISOBUS_AUX_GROUP_CODECS_HEX, "aux_n_type2_variable_cafe");
    let aux_n_msg = AuxNFunction {
        function_number: 12,
        r#type: AuxFunctionType::Type2,
        state: AuxFunctionState::Variable,
        setpoint: 0xCAFE,
    };
    assert_eq!(aux_n_msg.encode(), aux_n);
    assert_eq!(
        AuxNFunction::decode(&Message::new(PGN_AUX_INPUT_TYPE2, aux_n.to_vec(), 0x80)),
        Some(aux_n_msg)
    );
    assert!(
        AuxNFunction::decode(&Message::new(
            PGN_AUX_INPUT_TYPE2,
            aux_n[..7].to_vec(),
            0x80,
        ))
        .is_none()
    );
    assert!(
        AuxNFunction::decode(&Message::new(
            PGN_AUX_INPUT_TYPE2,
            [aux_n.as_slice(), &[0x00]].concat(),
            0x80,
        ))
        .is_none()
    );
    for name in [
        "aux_n_bad_reserved_type",
        "aux_n_bad_reserved_state",
        "aux_n_bad_reserved_tail",
    ] {
        assert!(
            AuxNFunction::decode(&Message::new(
                PGN_AUX_INPUT_TYPE2,
                parse_named_hex_bytes(ISOBUS_AUX_GROUP_CODECS_HEX, name),
                0x80,
            ))
            .is_none(),
            "{name} must be rejected"
        );
    }

    let group_command =
        parse_named_hex_frame(ISOBUS_AUX_GROUP_CODECS_HEX, "group_command_cafe_params");
    let group_command_msg = GroupFunctionMsg {
        function_type: GroupFunctionType::Command,
        target_pgn: 0xCAFE,
        parameters: vec![1, 2, 3],
    };
    assert_eq!(group_command_msg.encode().unwrap(), group_command);
    let group_message = Message::new(PGN_ACKNOWLEDGMENT, group_command.to_vec(), 0x80);
    assert_eq!(
        GroupFunctionMsg::decode(&group_message.data),
        Some(group_command_msg)
    );
    assert!(
        GroupFunctionMsg::decode(&parse_named_hex_frame(
            ISOBUS_AUX_GROUP_CODECS_HEX,
            "group_reserved_function"
        ))
        .is_none()
    );
    assert!(
        GroupFunctionMsg::decode(&parse_named_hex_frame(
            ISOBUS_AUX_GROUP_CODECS_HEX,
            "group_bad_parameter_padding"
        ))
        .is_none()
    );

    let group_request = parse_named_hex_frame(
        ISOBUS_AUX_GROUP_CODECS_HEX,
        "group_request_request_no_params",
    );
    let group_request_msg = GroupFunctionMsg {
        function_type: GroupFunctionType::Request,
        target_pgn: PGN_REQUEST,
        parameters: vec![],
    };
    assert_eq!(group_request_msg.encode().unwrap(), group_request);
    assert_eq!(
        GroupFunctionMsg::decode(&group_request),
        Some(group_request_msg)
    );

    let group_request_cafe =
        parse_named_hex_frame(ISOBUS_AUX_GROUP_CODECS_HEX, "group_request_cafe_no_params");
    assert_eq!(
        GroupFunctionMsg::decode(&group_request_cafe),
        Some(GroupFunctionMsg {
            function_type: GroupFunctionType::Request,
            target_pgn: 0xCAFE,
            parameters: vec![],
        })
    );
    let group_request_beef =
        parse_named_hex_frame(ISOBUS_AUX_GROUP_CODECS_HEX, "group_request_beef_no_params");
    assert_eq!(
        GroupFunctionMsg::decode(&group_request_beef),
        Some(GroupFunctionMsg {
            function_type: GroupFunctionType::Request,
            target_pgn: 0xBEEF,
            parameters: vec![],
        })
    );

    let group_ack =
        parse_named_hex_frame(ISOBUS_AUX_GROUP_CODECS_HEX, "group_ack_abcd_four_params");
    let group_ack_msg = GroupFunctionMsg {
        function_type: GroupFunctionType::Acknowledge,
        target_pgn: 0xABCD,
        parameters: vec![1, 2, 3, 4],
    };
    assert_eq!(group_ack_msg.encode().unwrap(), group_ack);
    assert_eq!(GroupFunctionMsg::decode(&group_ack), Some(group_ack_msg));
    assert!(GroupFunctionMsg::decode(&group_ack[..7]).is_none());
    assert!(GroupFunctionMsg::decode(&[group_ack.as_slice(), &[0x00]].concat()).is_none());

    for (fixture, target_pgn, error) in [
        (
            "group_ack_cafe_no_error",
            0xCAFE,
            GroupFunctionError::NoError,
        ),
        (
            "group_ack_cafe_unsupported_function",
            0xCAFE,
            GroupFunctionError::UnsupportedFunction,
        ),
        (
            "group_ack_beef_unsupported_pgn",
            0xBEEF,
            GroupFunctionError::UnsupportedPgn,
        ),
    ] {
        let bytes = parse_named_hex_frame(ISOBUS_AUX_GROUP_CODECS_HEX, fixture);
        let expected = GroupFunctionMsg::acknowledge(target_pgn, error);
        assert_eq!(expected.encode().unwrap(), bytes, "{fixture}");
        assert_eq!(
            GroupFunctionMsg::decode(&bytes),
            Some(expected),
            "{fixture}"
        );
    }
}

#[test]
fn fixture_j1939_dtc_and_dm1_bytes_are_stable() {
    let max_dtc = Dtc::decode(DTC_MAX_SPN_CONDITION_OC127).expect("DTC fixture decodes");
    assert_eq!(max_dtc.spn, 0x7_FFFF);
    assert_eq!(max_dtc.fmi, Fmi::ConditionExists);
    assert_eq!(max_dtc.occurrence_count, 0x7F);
    assert_eq!(max_dtc.encode(), *DTC_MAX_SPN_CONDITION_OC127);
    assert!(
        Dtc::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dtc_reserved_occurrence_bit",
        ))
        .is_none(),
        "DTC occurrence-count reserved high bit must be rejected"
    );

    let expected_dm1 = DmDtcList {
        lamps: DiagnosticLamps {
            amber_warning: LampStatus::On,
            ..Default::default()
        },
        dtcs: vec![Dtc {
            spn: 523_312,
            fmi: Fmi::AboveNormal,
            occurrence_count: 0,
        }],
    };
    let dm1 = DmDtcList::decode(DM1_AMBER_SPN523312).expect("DM1 fixture decodes");
    assert_eq!(dm1, expected_dm1);
    assert_eq!(dm1.encode(), *DM1_AMBER_SPN523312);
    assert!(
        DmDtcList::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dm_dtc_list_bad_classic_tail",
        ))
        .is_none(),
        "classic DM DTC-list frames must preserve 0xFF reserved tail bytes"
    );
}

#[test]
fn fixture_j1939_lamp_only_dm1_filters_zero_dtc_placeholder() {
    let dm1 = DmDtcList::decode(DM1_LAMP_ONLY_AMBER).expect("lamp-only DM1 decodes");

    assert_eq!(dm1.lamps.amber_warning, LampStatus::On);
    assert!(dm1.dtcs.is_empty());
    assert_eq!(dm1.encode(), *DM1_LAMP_ONLY_AMBER);
}

#[test]
fn fixture_j1939_dm2_previous_dtc_payload_is_stable() {
    let dm2 = DmDtcList::decode(DM2_PREVIOUS_TWO_DTCS).expect("DM2 fixture decodes");

    assert_eq!(dm2.lamps.malfunction, LampStatus::On);
    assert_eq!(
        dm2.dtcs,
        vec![
            Dtc {
                spn: 100,
                fmi: Fmi::AboveNormal,
                occurrence_count: 1,
            },
            Dtc {
                spn: 200,
                fmi: Fmi::VoltageHigh,
                occurrence_count: 5,
            },
        ]
    );
    assert_eq!(dm2.encode(), *DM2_PREVIOUS_TWO_DTCS);
    assert!(
        DM2_PREVIOUS_TWO_DTCS.len() > 8,
        "two DTCs plus lamps require transport segmentation"
    );
}

#[test]
fn fixture_j1939_dm6_dm12_dm23_alias_payloads_are_stable() {
    let dm6_bytes = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dm6_pending_spn4660_current_low",
    );
    let dm6: Dm6Message = Dm6Message::decode(&dm6_bytes).expect("DM6 fixture decodes");
    assert_eq!(dm6.lamps.amber_warning, LampStatus::On);
    assert_eq!(
        dm6.dtcs,
        vec![Dtc {
            spn: 0x1234,
            fmi: Fmi::CurrentLow,
            occurrence_count: 2,
        }]
    );
    assert_eq!(dm6.encode(), dm6_bytes);

    let dm12_bytes = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dm12_emissions_active_spn86_voltage_low",
    );
    let dm12: Dm12Message = Dm12Message::decode(&dm12_bytes).expect("DM12 fixture decodes");
    assert_eq!(dm12.lamps.malfunction, LampStatus::On);
    assert_eq!(
        dm12.dtcs,
        vec![Dtc {
            spn: 0x56,
            fmi: Fmi::VoltageLow,
            occurrence_count: 1,
        }]
    );
    assert_eq!(dm12.encode(), dm12_bytes);

    let dm23_bytes = parse_named_hex_bytes(
        J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
        "dm23_previous_mil_off_spn1929_voltage_high",
    );
    let dm23: Dm23Message = Dm23Message::decode(&dm23_bytes).expect("DM23 fixture decodes");
    assert_eq!(dm23.lamps.red_stop, LampStatus::On);
    assert_eq!(
        dm23.dtcs,
        vec![Dtc {
            spn: 0x789,
            fmi: Fmi::VoltageHigh,
            occurrence_count: 4,
        }]
    );
    assert_eq!(dm23.encode(), dm23_bytes);

    for (pgn, payload) in [
        (PGN_DM6, dm6_bytes.as_slice()),
        (PGN_DM12, dm12_bytes.as_slice()),
        (PGN_DM23, dm23_bytes.as_slice()),
    ] {
        let frame = Frame::from_message(Priority::Default, pgn, 0x80, BROADCAST_ADDRESS, payload);
        assert_eq!(frame.pgn(), pgn);
        assert_eq!(frame.destination(), BROADCAST_ADDRESS);
        assert_eq!(frame.data, payload);
    }

    assert!(
        DmDtcList::decode(&parse_named_hex_bytes(
            J1939_DIAGNOSTIC_VARIABLE_CODECS_HEX,
            "dm_dtc_list_misaligned_transport_tail",
        ))
        .is_none(),
        "transport-length DM DTC lists must align to whole 4-byte DTC records"
    );
}

#[test]
fn fixture_j1939_dm3_and_dm11_clear_requests_use_reserved_ff_payloads() {
    let dm3: Dm3ClearPreviouslyActiveRequest =
        DmClearAllRequest::decode(DM3_CLEAR_PREVIOUS_REQUEST).expect("DM3 clear decodes");
    let dm11: Dm11ClearActiveRequest =
        DmClearAllRequest::decode(DM11_CLEAR_ACTIVE_REQUEST).expect("DM11 clear decodes");
    assert_eq!(dm3.encode(), *DM3_CLEAR_PREVIOUS_REQUEST);
    assert_eq!(dm11.encode(), *DM11_CLEAR_ACTIVE_REQUEST);

    let dm3_frame = Frame::from_message(
        Priority::Default,
        PGN_DM3,
        0x80,
        BROADCAST_ADDRESS,
        DM3_CLEAR_PREVIOUS_REQUEST,
    );
    assert_eq!(dm3_frame.pgn(), PGN_DM3);
    assert_eq!(dm3_frame.destination(), BROADCAST_ADDRESS);
    assert_eq!(dm3_frame.data, *DM3_CLEAR_PREVIOUS_REQUEST);

    let dm11_frame = Frame::from_message(
        Priority::Default,
        PGN_DM11,
        0x80,
        BROADCAST_ADDRESS,
        DM11_CLEAR_ACTIVE_REQUEST,
    );
    assert_eq!(dm11_frame.pgn(), PGN_DM11);
    assert_eq!(dm11_frame.destination(), BROADCAST_ADDRESS);
    assert_eq!(dm11_frame.data, *DM11_CLEAR_ACTIVE_REQUEST);
}

