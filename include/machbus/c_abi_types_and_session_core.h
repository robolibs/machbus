
#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

typedef enum {
  MACHBUS_CLAIM_STATE_NONE = 0,
  MACHBUS_CLAIM_STATE_WAIT_FOR_CLAIM = 1,
  MACHBUS_CLAIM_STATE_SEND_REQUEST = 2,
  MACHBUS_CLAIM_STATE_WAIT_FOR_CONTEST = 3,
  MACHBUS_CLAIM_STATE_SEND_CLAIM = 4,
  MACHBUS_CLAIM_STATE_CLAIMED = 5,
  MACHBUS_CLAIM_STATE_FAILED = 6,
} MachbusClaimState;

/**
 * Discriminant for [`MachbusEvent::kind`]. Mirrors the unified
 * [`Event`] surface. Subsystem events that have no
 * stable C payload yet collapse to [`MachbusEventKind::Other`].
 */
typedef enum {
  MACHBUS_EVENT_KIND_NONE = 0,
  MACHBUS_EVENT_KIND_ADDRESS_CLAIM_CLAIMED = 1,
  MACHBUS_EVENT_KIND_ADDRESS_CLAIM_LOST = 2,
  MACHBUS_EVENT_KIND_ADDRESS_CLAIM_DISCONNECTED = 3,
  MACHBUS_EVENT_KIND_BUS_ERROR = 4,
  MACHBUS_EVENT_KIND_BUS_DROPPED_FRAME = 5,
  MACHBUS_EVENT_KIND_DIAG_RAISED = 6,
  MACHBUS_EVENT_KIND_DIAG_CLEARED = 7,
  MACHBUS_EVENT_KIND_DIAG_DM1_RECEIVED = 8,
  MACHBUS_EVENT_KIND_GNSS_POSITION = 9,
  MACHBUS_EVENT_KIND_GNSS_COG = 10,
  MACHBUS_EVENT_KIND_GNSS_SOG = 11,
  MACHBUS_EVENT_KIND_GNSS_HEADING = 12,
  MACHBUS_EVENT_KIND_IMP_HITCH_COMMAND = 13,
  MACHBUS_EVENT_KIND_IMP_PTO_COMMAND = 14,
  MACHBUS_EVENT_KIND_IMP_AUX_VALVE_COMMAND = 15,
  MACHBUS_EVENT_KIND_CUSTOM = 16,
  MACHBUS_EVENT_KIND_GNSS_MAGNETIC_VARIATION = 17,
  MACHBUS_EVENT_KIND_GNSS_ATTITUDE = 18,
  MACHBUS_EVENT_KIND_GNSS_DOPS = 19,
  MACHBUS_EVENT_KIND_GNSS_SYSTEM_TIME = 20,
  MACHBUS_EVENT_KIND_VT_STATE_CHANGED = 21,
  MACHBUS_EVENT_KIND_VT_SOFT_KEY = 22,
  MACHBUS_EVENT_KIND_VT_BUTTON = 23,
  MACHBUS_EVENT_KIND_VT_NUMERIC_VALUE_CHANGED = 24,
  MACHBUS_EVENT_KIND_VT_STRING_VALUE_CHANGED = 25,
  MACHBUS_EVENT_KIND_VT_POOL_ERROR = 26,
  MACHBUS_EVENT_KIND_VT_LANGUAGE_CHANGED = 27,
  MACHBUS_EVENT_KIND_VT_ACTIVE_WORKING_SET = 28,
  MACHBUS_EVENT_KIND_TC_STATE_CHANGED = 29,
  MACHBUS_EVENT_KIND_FS_CONNECTED = 30,
  MACHBUS_EVENT_KIND_FS_DISCONNECTED = 31,
  MACHBUS_EVENT_KIND_FS_OPEN_RESPONSE = 32,
  MACHBUS_EVENT_KIND_FS_CLOSE_RESPONSE = 33,
  MACHBUS_EVENT_KIND_FS_READ_RESPONSE = 34,
  MACHBUS_EVENT_KIND_FS_WRITE_RESPONSE = 35,
  MACHBUS_EVENT_KIND_FS_SEEK_RESPONSE = 36,
  MACHBUS_EVENT_KIND_FS_CURRENT_DIRECTORY_RESPONSE = 37,
  MACHBUS_EVENT_KIND_FS_CHANGE_DIRECTORY_RESPONSE = 38,
  MACHBUS_EVENT_KIND_FS_ERROR = 39,
  MACHBUS_EVENT_KIND_VT_SERVER_STATE_CHANGED = 40,
  MACHBUS_EVENT_KIND_VT_SERVER_CLIENT_CONNECTED = 41,
  MACHBUS_EVENT_KIND_VT_SERVER_CLIENT_DISCONNECTED = 42,
  MACHBUS_EVENT_KIND_VT_SERVER_ACTIVE_WORKING_SET_CHANGED = 43,
  MACHBUS_EVENT_KIND_VT_SERVER_SOFT_KEY = 44,
  MACHBUS_EVENT_KIND_VT_SERVER_BUTTON = 45,
  MACHBUS_EVENT_KIND_VT_SERVER_NUMERIC_VALUE_CHANGED = 46,
  MACHBUS_EVENT_KIND_VT_SERVER_STRING_VALUE_CHANGED = 47,
  MACHBUS_EVENT_KIND_VT_SERVER_INPUT_OBJECT_SELECTED = 48,
  MACHBUS_EVENT_KIND_FS_SERVER_CLIENT_CONNECTED = 49,
  MACHBUS_EVENT_KIND_FS_SERVER_CLIENT_DISCONNECTED = 50,
  MACHBUS_EVENT_KIND_FS_SERVER_FILE_OPENED = 51,
  MACHBUS_EVENT_KIND_FS_SERVER_FILE_CLOSED = 52,
  MACHBUS_EVENT_KIND_TC_SERVER_STATE_CHANGED = 53,
  MACHBUS_EVENT_KIND_TC_SERVER_CLIENT_VERSION_RECEIVED = 54,
  MACHBUS_EVENT_KIND_VT_AUX_CAPABILITIES = 76,
  MACHBUS_EVENT_KIND_POWERTRAIN = 89,
  MACHBUS_EVENT_KIND_TC_SERVER_PEER_CONTROL_ASSIGNMENT = 75,
  MACHBUS_EVENT_KIND_GUIDANCE_MACHINE_INFO = 90,
  MACHBUS_EVENT_KIND_OTHER = 99,
} MachbusEventKind;

typedef enum {
  MACHBUS_HITCH_FRONT = 0,
  MACHBUS_HITCH_REAR = 1,
} MachbusHitch;

typedef enum {
  MACHBUS_HITCH_COMMAND_NO_ACTION = 0,
  MACHBUS_HITCH_COMMAND_LOWER = 1,
  MACHBUS_HITCH_COMMAND_RAISE = 2,
  MACHBUS_HITCH_COMMAND_POSITION = 3,
} MachbusHitchCommand;

typedef enum {
  MACHBUS_PTO_FRONT = 0,
  MACHBUS_PTO_REAR = 1,
} MachbusPto;

typedef enum {
  MACHBUS_PTO_COMMAND_NO_ACTION = 0,
  MACHBUS_PTO_COMMAND_ENGAGE = 1,
  MACHBUS_PTO_COMMAND_DISENGAGE = 2,
  MACHBUS_PTO_COMMAND_SET_SPEED = 3,
} MachbusPtoCommand;

typedef enum {
  MACHBUS_VALVE_COMMAND_NO_ACTION = 0,
  MACHBUS_VALVE_COMMAND_EXTEND = 1,
  MACHBUS_VALVE_COMMAND_RETRACT = 2,
  MACHBUS_VALVE_COMMAND_FLOAT = 3,
  MACHBUS_VALVE_COMMAND_BLOCK = 4,
} MachbusValveCommand;

/**
 * Process-data trigger methods. Bitmask, OR multiple together.
 */
enum TriggerMethod
#ifdef __cplusplus
  : uint8_t
#endif // __cplusplus
 {
  TRIGGER_METHOD_TIME_INTERVAL = 1,
  TRIGGER_METHOD_DISTANCE_INTERVAL = 2,
  TRIGGER_METHOD_THRESHOLD_LIMITS = 4,
  TRIGGER_METHOD_ON_CHANGE = 8,
  TRIGGER_METHOD_TOTAL = 16,
};
#ifndef __cplusplus
typedef uint8_t TriggerMethod;
#endif // __cplusplus

/**
 * Server option flags (bitfield in byte 1 of `Technical Capabilities`).
 *
 * The C++ exposes these as a `u8`-OR'able enum; Rust uses an explicit
 * `ServerOptionFlags` `u8` newtype with `with_*` builders so callers
 * don't reach for raw bit math.
 */
enum ServerOptions
#ifdef __cplusplus
  : uint8_t
#endif // __cplusplus
 {
  SERVER_OPTIONS_SUPPORTS_DOCUMENTATION = 1,
  SERVER_OPTIONS_SUPPORTS_TCGEO_WITHOUT_POSITION_BASED_CONTROL = 2,
  SERVER_OPTIONS_SUPPORTS_TCGEO_WITH_POSITION_BASED_CONTROL = 4,
  SERVER_OPTIONS_SUPPORTS_PEER_CONTROL_ASSIGNMENT = 8,
  SERVER_OPTIONS_SUPPORTS_IMPLEMENT_SECTION_CONTROL = 16,
};
#ifndef __cplusplus
typedef uint8_t ServerOptions;
#endif // __cplusplus

/**
 * Owned, opaque decoded [`machbus::j1939::ComponentIdentification`].
 */
typedef struct MachbusComponentIdentification MachbusComponentIdentification;

/**
 * Owned, opaque ISO 11783-10 Device Descriptor Object Pool (DDOP). Build with
 * [`machbus_ddop_new`], populate with the `machbus_ddop_add_*` adders, and
 * release with [`machbus_ddop_free`] (or transfer to a session). Each adder
 * returns the assigned ObjectID (>= 0) or -1 on error.
 */
typedef struct MachbusDdop MachbusDdop;

/**
 * Owned, opaque decoded [`machbus::j1939::Dm16Transfer`].
 */
typedef struct MachbusDm16Transfer MachbusDm16Transfer;

/**
 * Owned, opaque decoded [`machbus::j1939::Dm20Response`].
 */
typedef struct MachbusDm20Response MachbusDm20Response;

/**
 * Owned, opaque decoded [`machbus::j1939::Dm4Message`].
 */
typedef struct MachbusDm4Message MachbusDm4Message;

/**
 * Owned, opaque decoded [`machbus::j1939::DmDtcList`] (also DM6/DM12/DM23).
 */
typedef struct MachbusDmDtcList MachbusDmDtcList;

/**
 * Owned, opaque decoded [`machbus::j1939::EcuIdentification`].
 */
typedef struct MachbusEcuIdentification MachbusEcuIdentification;

/**
 * Owned, opaque decoded [`machbus::j1939::FreezeFrame`].
 */
typedef struct MachbusFreezeFrame MachbusFreezeFrame;

/**
 * Opaque standalone NMEA decoder. Create with [`machbus_nmea_new`], feed frames
 * with [`machbus_nmea_feed`], drain results with the `machbus_nmea_poll_*`
 * functions, and release with [`machbus_nmea_free`]. Single-threaded: keep the
 * handle pinned to the thread that created it.
 */
typedef struct MachbusNmea MachbusNmea;

/**
 * Owned, opaque decoded [`machbus::j1939::ProductIdentification`].
 */
typedef struct MachbusProductIdentification MachbusProductIdentification;

/**
 * Owned, opaque decoded [`machbus::j1939::Request2Msg`].
 */
typedef struct MachbusRequest2Msg MachbusRequest2Msg;

/**
 * Owned opaque node handle wrapping a sans-IO [`Session`].
 *
 * Created by [`machbus_session_new`] and released exactly once by
 * [`machbus_session_free`]. The free function accepts `NULL`; passing the same
 * non-`NULL` pointer to `machbus_session_free` more than once is invalid. Set
 * caller variables to `NULL` after freeing to avoid double-free bugs.
 */
typedef struct MachbusSession MachbusSession;

/**
 * Owned, opaque decoded [`machbus::j1939::SoftwareIdentification`].
 */
typedef struct MachbusSoftwareIdentification MachbusSoftwareIdentification;

/**
 * Owned, opaque decoded [`machbus::j1939::TransferMsg`].
 */
typedef struct MachbusTransferMsg MachbusTransferMsg;

/**
 * Owned, opaque ISO 11783-6 VT object pool. Build with
 * [`machbus_vt_pool_new`] or [`machbus_vt_pool_from_iop`], populate with the
 * typed `machbus_vt_pool_add_*` adders, and release with
 * [`machbus_vt_pool_free`] (or transfer to a session).
 */
typedef struct MachbusVtPool MachbusVtPool;

/**
 * A monotonic timestamp in microseconds since a driver-chosen origin.
 *
 * This is intentionally a plain `u64` micros newtype with no dependency on
 * `std::time` or any embedded time crate, so the core compiles unchanged on
 * hosted and bare-metal targets. Convert at the driver boundary with
 * [`Instant::from_micros`] / [`Instant::as_micros`] (and the provided `From`
 * impls).
 */
typedef struct Instant Instant;

/**
 * What the crate claims for the powertrain area.
 */
typedef struct PowertrainClaim PowertrainClaim;

/**
 * Result of [`machbus_session_validate_can_bus_config`].
 */
typedef struct {
  bool bitrate_ok;
  bool sample_point_ok;
  bool bit_timing_ok;
  bool physical_mode_ok;
  bool overall_ok;
} MachbusCanBusValidation;

/**
 * Node configuration. Build a default with [`machbus_session_default_config`]
 * and override fields before passing to [`machbus_session_new`].
 */
typedef struct {
  /**
   * Raw 64-bit ISO 11783-5 NAME (use `Name::raw` from Rust).
   */
  uint64_t name_raw;
  /**
   * Preferred address before claim arbitration.
   */
  uint8_t preferred_address;
  /**
   * Plug a [`Diagnostics`] subsystem (DM1 raise/clear + peer decode).
   */
  bool enable_diagnostics;
  /**
   * DM1 broadcast cadence; 0 = use default (1000 ms).
   */
  uint32_t diagnostics_interval_ms;
  /**
   * Plug a [`Gnss`] subsystem (NMEA 2000 navigation, all messages).
   */
  bool enable_gnss;
  /**
   * Plug an [`Implement`] subsystem (hitch/PTO/valve command + decode).
   */
  bool enable_implement;
  /**
   * Plug a [`VtClient`] subsystem with an empty object pool / working set.
   */
  bool enable_vt_client;
  /**
   * Plug a [`TcClient`] subsystem with an empty DDOP.
   */
  bool enable_tc_client;
  /**
   * Plug an [`Auxiliary`] subsystem (AUX-O/AUX-N function decode/broadcast).
   */
  bool enable_auxiliary;
  /**
   * Plug a [`DmMemory`] subsystem (DM14/DM15/DM16 memory access + ECU id).
   */
  bool enable_dm_memory;
  /**
   * Plug an [`FsClient`] subsystem (ISO 11783-13 file-server client).
   */
  bool enable_fs_client;
  /**
   * Plug an [`FsServer`] subsystem (ISO 11783-13 file server).
   */
  bool enable_fs_server;
  /**
   * Plug a [`ControlFunctionalities`] subsystem (ISO 11783-12 reporting).
   */
  bool enable_control_functionalities;
  /**
   * Plug a [`GroupFunction`] subsystem (ISO group function responder).
   */
  bool enable_group_function;
  /**
   * Plug a [`Heartbeat`] subsystem; `heartbeat_interval_ms` 0 = 100 ms.
   */
  bool enable_heartbeat;
  /**
   * Heartbeat broadcast cadence; 0 = use default (100 ms).
   */
  uint32_t heartbeat_interval_ms;
  /**
   * Plug a [`LanguageCommand`] subsystem (ISO language/units command).
   */
  bool enable_language_command;
  /**
   * Plug a [`MaintainPower`] subsystem; `maintain_power_role_tecu` selects role.
   */
  bool enable_maintain_power;
  /**
   * MaintainPower role: `true` = TECU server, `false` = CF/client.
   */
  bool maintain_power_role_tecu;
  /**
   * Plug a [`NameManagement`] subsystem (commanded-address / NAME mgmt).
   */
  bool enable_name_management;
  /**
   * Plug a [`Powertrain`] subsystem (EEC1/ETC1/VIN broadcast + snapshot).
   */
  bool enable_powertrain;
  /**
   * Plug a [`Request2`] subsystem (PGN request2 responder).
   */
  bool enable_request2;
  /**
   * Plug a [`ScClient`] subsystem (sequence-control client).
   */
  bool enable_sc_client;
  /**
   * Plug a [`ScMaster`] subsystem (sequence-control master).
   */
  bool enable_sc_master;
  /**
   * Plug a [`ShortcutButton`] subsystem (ISO stop-all shortcut button).
   */
  bool enable_shortcut_button;
  /**
   * Plug a [`TcServer`] subsystem (task-controller server, default config).
   */
  bool enable_tc_server;
  /**
   * Plug a [`VtServer`] subsystem (virtual terminal server, default config).
   */
  bool enable_vt_server;
  /**
   * Plug a [`Tim`] subsystem (tractor-implement management, no options).
   */
  bool enable_tim;
  /**
   * Plug a [`Guidance`] subsystem (ISO 11783-7 curvature-based autosteer).
   */
  bool enable_guidance;
} MachbusConfig;

/**
 * A flattened, C-friendly view of one [`Event`].
 *
 * The payload fields are interpreted according to [`MachbusEvent::kind`]; see
 * `classify_event` for the field meaning of each kind.
 */
typedef struct {
  MachbusEventKind kind;
  /**
   * Source address (for inbound events) or own address.
   */
  uint8_t source;
  /**
   * SPN for diag events, PGN for Custom, object id for VT events, or 0.
   */
  uint32_t spn_or_pgn;
  /**
   * FMI byte for diag events; reused as raw subcommand/status byte for
   * subsystem events.
   */
  uint8_t fmi_or_sub;
  /**
   * First f64 payload (latitude, COG, ramp speed…). NaN if unused.
   */
  double d0;
  /**
   * Second f64 payload (longitude, etc.). NaN if unused.
   */
  double d1;
  /**
   * Auxiliary u32 (numeric value, target speed RPM, enum code, length, etc.).
   */
  uint32_t u0;
} MachbusEvent;

typedef struct {
  double latitude;
  double longitude;
  double altitude_m;
  double speed_mps;
  double heading_rad;
} MachbusGnssPosition;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Eec1`].
 */
typedef struct {
  double engine_torque_percent;
  double driver_demand_percent;
  double actual_engine_percent;
  double engine_speed_rpm;
  uint8_t starter_mode;
  uint8_t source_address;
} MachbusEec1;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Eec2`].
 */
typedef struct {
  uint8_t accel_pedal_position;
  double engine_load_percent;
  uint8_t accel_pedal_low_idle;
  uint8_t accel_pedal_kickdown;
  uint8_t road_speed_limit;
} MachbusEec2;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Eec3`].
 */
typedef struct {
  double nominal_friction_percent;
  double desired_operating_speed_rpm;
  uint8_t operating_speed_asymmetry;
} MachbusEec3;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::EngineTemp1`].
 */
typedef struct {
  double coolant_temp_c;
  double fuel_temp_c;
  double oil_temp_c;
  double turbo_oil_temp_c;
  double intercooler_temp_c;
} MachbusEngineTemp1;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::EngineTemp2`].
 */
typedef struct {
  double engine_oil_temp_c;
  double turbo_oil_temp_c;
  double engine_intercooler_temp_c;
  double turbo_1_temp_c;
} MachbusEngineTemp2;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::EngineFluidLp`].
 */
typedef struct {
  double oil_pressure_kpa;
  double coolant_pressure_kpa;
  uint8_t oil_level_percent;
  uint8_t coolant_level_percent;
  double fuel_delivery_pressure_kpa;
  double crankcase_pressure_kpa;
} MachbusEngineFluidLp;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::EngineHours`].
 */
typedef struct {
  double total_hours;
  double total_revolutions;
} MachbusEngineHours;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::FuelEconomy`].
 */
typedef struct {
  double fuel_rate_lph;
  double instantaneous_lph;
  double throttle_position;
} MachbusFuelEconomy;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Tsc1`]. `override_mode` is the
 * raw [`OverrideControlMode`] byte (0=NoOverride, 1=SpeedControl,
 * 2=TorqueControl, 3=SpeedTorqueLimit).
 */
typedef struct {
  uint8_t override_mode;
  double requested_speed_rpm;
  double requested_torque_percent;
} MachbusTsc1;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Vep1`].
 */
typedef struct {
  double battery_voltage_v;
  double alternator_current_a;
  double charging_system_voltage_v;
  double key_switch_voltage_v;
} MachbusVep1;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::AmbientConditions`].
 */
typedef struct {
  double barometric_pressure_kpa;
  double ambient_air_temp_c;
  double intake_air_temp_c;
  double road_surface_temp_c;
} MachbusAmbientConditions;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::DashDisplay`].
 */
typedef struct {
  uint8_t fuel_level_percent;
  uint8_t washer_fluid_level;
  double fuel_filter_diff_kpa;
  double oil_filter_diff_kpa;
  double cargo_ambient_temp_c;
} MachbusDashDisplay;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::VehiclePosition`].
 */
typedef struct {
  double latitude_deg;
  double longitude_deg;
} MachbusVehiclePosition;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::FuelConsumption`].
 */
typedef struct {
  double trip_fuel_l;
  double total_fuel_l;
} MachbusFuelConsumption;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Aftertreatment1`].
 */
typedef struct {
  double def_tank_level;
  double intake_nox_ppm;
  double outlet_nox_ppm;
  uint8_t intake_nox_reading_status;
  uint8_t outlet_nox_reading_status;
} MachbusAftertreatment1;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Aftertreatment2`].
 */
typedef struct {
  double dpf_differential_pressure_kpa;
  double def_concentration;
  double dpf_soot_load_percent;
  uint8_t dpf_active_regeneration_status;
  uint8_t dpf_passive_regeneration_status;
} MachbusAftertreatment2;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::diagnostic::Dtc`]. `fmi` carries
 * the raw FMI wire value (see [`MachbusFmi`]).
 */
typedef struct {
  uint32_t spn;
  uint8_t fmi;
  uint8_t occurrence_count;
} MachbusDtc;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::DiagnosticLamps`]. Each lamp /
 * flash field is the raw 2-bit wire value (`LampStatus`: 0=Off, 1=On,
 * 2=Error, 3=N/A; `LampFlash`: 0=Slow, 1=Fast, 2=Off, 3=N/A).
 */
typedef struct {
  uint8_t malfunction;
  uint8_t malfunction_flash;
  uint8_t red_stop;
  uint8_t red_stop_flash;
  uint8_t amber_warning;
  uint8_t amber_warning_flash;
  uint8_t engine_protect;
  uint8_t engine_protect_flash;
} MachbusDiagnosticLamps;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::DiagnosticProtocolId`] (DM5). The
 * `protocols` byte is the raw `DiagProtocol` bit field.
 */
typedef struct {
  uint8_t protocols;
} MachbusDiagnosticProtocolId;

/**
 * `#[repr(C)]` lamp header of a DM4 message (each a raw `LampStatus` byte).
 */
typedef struct {
  uint8_t mil_status;
  uint8_t red_stop_lamp;
  uint8_t amber_warning;
  uint8_t protect_lamp;
} MachbusDm4Lamps;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm7Command`].
 */
typedef struct {
  uint32_t spn;
  uint8_t test_id;
} MachbusDm7Command;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm8TestResult`].
 */
typedef struct {
  uint32_t spn;
  uint8_t test_id;
  uint8_t test_result;
  uint16_t test_value;
  uint16_t test_limit_min;
  uint16_t test_limit_max;
} MachbusDm8TestResult;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm13Signals`]. Network fields are
 * raw `Dm13Command` bytes (0=Suspend, 1=Resume, 3=DoNotCare); `suspend_signal`
 * is a raw `Dm13SuspendSignal` byte.
 */
typedef struct {
  uint8_t primary_vehicle_network;
  uint8_t sae_j1922_network;
  uint8_t sae_j1587_network;
  uint8_t current_data_link;
  uint8_t suspend_signal;
  uint16_t suspend_duration_s;
} MachbusDm13Signals;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm21Readiness`].
 */
typedef struct {
  uint16_t distance_with_mil_on_km;
  uint16_t distance_since_codes_cleared_km;
  uint16_t minutes_with_mil_on;
  uint16_t time_since_codes_cleared_min;
  uint8_t comprehensive_component;
  uint8_t fuel_system;
  uint8_t misfire;
} MachbusDm21Readiness;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm22Message`]. `control` is the
 * raw `Dm22Control` byte; `nack_reason` is the raw `Dm22NackReason` byte and is
 * only meaningful when `nack_reason_present` is true; `fmi` is the raw FMI.
 */
typedef struct {
  uint8_t control;
  uint8_t nack_reason;
  bool nack_reason_present;
  uint32_t spn;
  uint8_t fmi;
} MachbusDm22Message;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::MonitorPerformanceRatio`].
 */
typedef struct {
  uint32_t spn;
  uint16_t numerator;
  uint16_t denominator;
} MachbusMonitorPerformanceRatio;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::SpnSnapshot`].
 */
typedef struct {
  uint32_t spn;
  uint32_t value;
} MachbusSpnSnapshot;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm25Request`]. `fmi` is the raw FMI.
 */
typedef struct {
  uint32_t spn;
  uint8_t fmi;
  uint8_t frame_number;
} MachbusDm25Request;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm14Request`]. `command` is the raw
 * `Dm14Command` byte; `pointer_type` the raw `Dm14PointerType` byte.
 */
typedef struct {
  uint8_t command;
  uint8_t pointer_type;
  uint32_t address;
  uint16_t length;
  uint8_t key;
} MachbusDm14Request;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Dm15Response`]. `status` is the raw
 * `Dm15Status` byte.
 */
typedef struct {
  uint8_t status;
  uint16_t length;
  uint32_t address;
  uint8_t edcp_extension;
  uint8_t seed;
} MachbusDm15Response;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Acknowledgment`]. `control` is the
 * raw [`MachbusAckControl`] byte.
 */
typedef struct {
  uint8_t control;
  uint8_t group_function;
  uint32_t acknowledged_pgn;
  uint8_t address;
} MachbusAcknowledgment;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::LanguageData`]. The unit-system
 * fields are the raw enum bytes (all 0=Metric, with Imperial/US variants where
 * defined). `language_code` / `country_code` are the two ASCII bytes each.
 */
typedef struct {
  uint8_t language_code[2];
  uint8_t decimal;
  uint8_t time_format;
  uint8_t date_format;
  uint8_t distance;
  uint8_t area;
  uint8_t volume;
  uint8_t mass;
  uint8_t temperature;
  uint8_t pressure;
  uint8_t force;
  uint8_t country_code[2];
  uint8_t generic;
} MachbusLanguageData;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::MaintainPowerData`]. State fields
 * are raw `MaintainPowerState` bytes (0=Inactive, 1=Active, 2=Error, 3=N/A);
 * maintain fields are raw `MaintainPowerRequirement` bytes. `timestamp_us` is
 * repo-internal and not on the wire.
 */
typedef struct {
  uint8_t implement_in_work_state;
  uint8_t implement_park_state;
  uint8_t implement_ready_to_work_state;
  uint8_t implement_transport_state;
  uint8_t maintain_actuator_power;
  uint8_t maintain_ecu_power;
  uint64_t timestamp_us;
} MachbusMaintainPowerData;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::SpeedAndDistance`]. `*_present`
 * flags carry the `Option` discriminant; the value is ignored when false.
 * `timestamp_us` is repo-internal.
 */
typedef struct {
  double speed_mps;
  bool speed_mps_present;
  double distance_m;
  bool distance_m_present;
  uint64_t timestamp_us;
} MachbusSpeedAndDistance;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::Etc1`].
 */
typedef struct {
  int8_t current_gear;
  int8_t selected_gear;
  double output_shaft_speed_rpm;
  uint8_t shift_in_progress;
  uint8_t torque_converter_lockup;
} MachbusEtc1;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::TransmissionOilTemp`].
 */
typedef struct {
  double oil_temp_c;
} MachbusTransmissionOilTemp;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::CruiseControl`].
 */
typedef struct {
  double wheel_speed_kmh;
  uint8_t cc_active;
  uint8_t brake_switch;
  uint8_t clutch_switch;
  uint8_t park_brake;
  double cc_set_speed_kmh;
} MachbusCruiseControl;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::shortcut_button::ShortcutButtonMessage`].
 * `state` is the raw `ShortcutButtonState` byte (0=Stop, 1=Permit, 2=Error,
 * 3=N/A).
 */
typedef struct {
  uint8_t state;
  uint8_t transition_count;
} MachbusShortcutButtonMessage;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::TimeDate`]. Each `*_present` flag
 * carries the `Option` discriminant; the paired value is ignored when false.
 * `timestamp_us` is repo-internal.
 */
typedef struct {
  uint8_t seconds;
  bool seconds_present;
  uint8_t minutes;
  bool minutes_present;
  uint8_t hours;
  bool hours_present;
  uint8_t day;
  bool day_present;
  uint8_t month;
  bool month_present;
  uint16_t year;
  bool year_present;
  int16_t utc_offset_min;
  bool utc_offset_min_present;
  int8_t utc_offset_hours;
  bool utc_offset_hours_present;
  uint64_t timestamp_us;
} MachbusTimeDate;

/**
 * `#[repr(C)]` mirror of [`machbus::j1939::HeartbeatRequest`].
 */
typedef struct {
  /**
   * 18-bit requested PGN.
   */
  uint32_t requested_pgn;
  /**
   * Requested broadcast interval in milliseconds.
   */
  uint16_t interval_ms;
} MachbusHeartbeatRequest;

/**
 * `#[repr(C)]` mirror of [`machbus::isobus::AuxOFunction`] /
 * [`AuxNFunction`]. `kind`/`state` are the raw `AuxFunctionType` /
 * `AuxFunctionState` wire bytes (0=Type0/Off, 1=Type1/On, 2=Type2/Variable).
 */
typedef struct {
  uint8_t function_number;
  uint8_t kind;
  uint8_t state;
  uint16_t setpoint;
} MachbusAuxFunction;

/**
 * `#[repr(C)]` mirror of [`machbus::isobus::TimInterlocks`].
 */
typedef struct {
  bool operator_present;
  bool road_transport_mode;
  bool external_stop;
  bool implement_ready;
} MachbusTimInterlocks;

/**
 * Data Dictionary Identifier (ISO 11783-11).
 */
typedef uint16_t DDI;

#ifdef __cplusplus
extern "C" {
#endif // __cplusplus

/**
 * The last error message set by a failing call on this thread, or `NULL`.
 * The returned pointer is valid until the next ABI call on the same thread.
 */
const char *machbus_session_last_error(void);

/**
 * Current C ABI version. See [`MACHBUS_C_ABI_VERSION`].
 */
uint32_t machbus_session_abi_version(void);

MachbusCanBusValidation machbus_session_validate_can_bus_config(uint32_t bitrate,
                                                                double sample_point,
                                                                uint8_t sjw,
                                                                uint8_t prop_seg,
                                                                uint8_t phase_seg1,
                                                                uint8_t phase_seg2,
                                                                bool silent_mode,
                                                                bool loopback);

bool machbus_session_enforce_iso_can_config(uint32_t bitrate,
                                            double sample_point,
                                            uint8_t sjw,
                                            uint8_t prop_seg,
                                            uint8_t phase_seg1,
                                            uint8_t phase_seg2,
                                            bool silent_mode,
                                            bool loopback);

MachbusConfig machbus_session_default_config(void);

/**
 * Create a node from `cfg` (or defaults if `cfg` is `NULL`). Returns `NULL` on
 * failure; inspect [`machbus_session_last_error`]. Free with
 * [`machbus_session_free`].
 */
MachbusSession *machbus_session_new(const MachbusConfig *cfg);

/**
 * Free a node. Accepts `NULL`.
 */
void machbus_session_free(MachbusSession *h);

/**
 * Begin address claiming. Drive it forward with [`machbus_session_tick`].
 */
bool machbus_session_start_address_claim(MachbusSession *h);

/**
 * Advance the virtual clock by `dt_ms` and run cadences/timers. Outbound
 * frames produced are then available via [`machbus_session_poll_transmit`].
 */
bool machbus_session_tick(MachbusSession *h, uint32_t dt_ms);

/**
 * Feed one received CAN frame (extended 29-bit `raw_id`, up to 8 data bytes).
 * `port` identifies the receiving bus.
 */
bool machbus_session_feed(MachbusSession *h,
                          uint8_t port,
                          uint32_t raw_id,
                          const uint8_t *data,
                          uintptr_t len);

/**
 * Drain the next outbound frame. On success writes `out_port`, `out_raw_id`,
 * up to 8 bytes into `out_data`, and the byte count into `out_len`, then
 * returns `true`. Returns `false` (without setting an error) when the transmit
 * queue is drained. Call repeatedly until it returns `false`.
 */
bool machbus_session_poll_transmit(MachbusSession *h,
                                   uint8_t *out_port,
                                   uint32_t *out_raw_id,
                                   uint8_t *out_data,
                                   uintptr_t *out_len);

/**
 * Raw escape hatch: queue an application message from the local control
 * function. `priority` is the 3-bit J1939 priority (0 = highest, 6 = default).
 */
bool machbus_session_send_raw(MachbusSession *h,
                              uint32_t pgn,
                              const uint8_t *data,
                              uintptr_t len,
                              uint8_t dst,
                              uint8_t priority);

uint8_t machbus_session_address(const MachbusSession *h);

MachbusClaimState machbus_session_claim_state(const MachbusSession *h);

bool machbus_session_is_claimed(const MachbusSession *h);

/**
 * Drain the next application event into `out`. Returns `true` and fills `out`
 * when an event was available; returns `false` and writes a `None`-kind event
 * (without setting an error) when the queue is drained.
 */
bool machbus_session_poll_event(MachbusSession *h, MachbusEvent *out);

/**
 * Raise a diagnostic trouble code (broadcast on the next DM1 cadence).
 * Requires the diagnostics subsystem to be enabled.
 */
bool machbus_session_diag_raise(MachbusSession *h,
                                uint32_t spn,
                                uint8_t fmi,
                                uint8_t occurrence_count);

/**
 * Clear all active diagnostic trouble codes.
 */
bool machbus_session_diag_clear(MachbusSession *h);

/**
 * Number of currently active local diagnostic trouble codes, or 0 if the
 * diagnostics subsystem is not plugged.
 */
uintptr_t machbus_session_diag_active_count(const MachbusSession *h);

/**
 * Broadcast a GNSS position. Requires the GNSS subsystem to be enabled.
 */
bool machbus_session_gnss_broadcast_position(MachbusSession *h, const MachbusGnssPosition *pos);

/**
 * Broadcast course-over-ground / speed-over-ground (radians, m/s). Requires
 * the GNSS subsystem to be enabled.
 */
bool machbus_session_gnss_broadcast_cog_sog(MachbusSession *h, double cog_rad, double sog_mps);

/**
 * Command a hitch (front/rear) raise/lower/no-action. For a target position,
 * use [`machbus_session_implement_command_hitch_position`]. Requires the
 * implement subsystem.
 */
bool machbus_session_implement_command_hitch(MachbusSession *h,
                                             MachbusHitch hitch,
                                             MachbusHitchCommand command);

/**
 * Command a hitch to a target position (0..=1000 per mille) at `rate`.
 * Requires the implement subsystem.
 */
bool machbus_session_implement_command_hitch_position(MachbusSession *h,
                                                      MachbusHitch hitch,
                                                      uint16_t target_position,
                                                      uint8_t rate);

/**
 * Command a PTO (front/rear) engage/disengage/no-action. Requires the
 * implement subsystem.
 */
bool machbus_session_implement_command_pto(MachbusSession *h,
                                           MachbusPto pto,
                                           MachbusPtoCommand command);

/**
 * Command a PTO target speed (RPM) with a ramp rate. Requires the implement
 * subsystem.
 */
bool machbus_session_implement_command_pto_speed(MachbusSession *h,
                                                 MachbusPto pto,
                                                 uint16_t rpm,
                                                 uint8_t ramp_rate);

/**
 * Command an auxiliary valve by `valve_index` with a flow rate. Requires the
 * implement subsystem.
 */
bool machbus_session_implement_command_aux_valve(MachbusSession *h,
                                                 uint8_t valve_index,
                                                 MachbusValveCommand command,
                                                 uint16_t flow_rate);

/**
 * Command the steering system to follow a path **curvature** in 1/km
 * (`0.0` = straight; sign follows the ISO 11783-7 wire convention). Broadcast
 * on the next tick as a Guidance System Command (PGN 0xAD00). Requires the
 * guidance subsystem.
 */
bool machbus_session_guidance_command_curvature(MachbusSession *h, double curvature_per_km);

/**
 * Command a turn of the given **radius in metres** (curvature = 1000 / radius;
 * a zero or non-finite radius commands straight ahead). Requires the guidance
 * subsystem.
 */
bool machbus_session_guidance_command_radius(MachbusSession *h, double radius_m);

/**
 * Command with a robotics-style twist: linear velocity `linear_mps` (m/s,
 * forward positive) and angular/yaw velocity `angular_rad_s` (rad/s, left
 * positive). Sends both the steering curvature (`κ = ω / v`, PGN 0xAD00) and
 * the target speed (PGN 0xFD43). Requires the guidance subsystem.
 */
bool machbus_session_guidance_command_velocity(MachbusSession *h,
                                               double linear_mps,
                                               double angular_rad_s);

/**
 * Command straight-ahead (zero curvature). Requires the guidance subsystem.
 */
bool machbus_session_guidance_command_straight(MachbusSession *h);

/**
 * Write the steering system's last estimated curvature (1/km) into `out`.
 * Returns `false` (without setting an error) when no machine info has arrived
 * yet, or when the guidance subsystem is not plugged.
 */
bool machbus_session_guidance_estimated_curvature(const MachbusSession *h, double *out);

/**
 * Whether the steering system last reported it is ready/engaged to steer.
 * Returns `false` if no machine info has arrived or the subsystem is unplugged.
 */
bool machbus_session_guidance_is_steering_ready(const MachbusSession *h);

/**
 * Connect the VT client to a server address. Requires the VT client subsystem.
 */
bool machbus_session_vt_connect(MachbusSession *h, uint8_t server);

/**
 * Disconnect the VT client. Requires the VT client subsystem.
 */
bool machbus_session_vt_disconnect(MachbusSession *h);

/**
 * VT client connection state code (see `vt_state_code`); 0 (Disconnected) if
 * the VT client subsystem is not plugged.
 */
uint32_t machbus_session_vt_state(const MachbusSession *h);

/**
 * Whether the VT client is connected.
 */
bool machbus_session_vt_is_connected(const MachbusSession *h);

/**
 * Show a VT object by id. Requires the VT client subsystem.
 */
bool machbus_session_vt_show(MachbusSession *h, uint16_t object_id);

/**
 * Hide a VT object by id. Requires the VT client subsystem.
 */
bool machbus_session_vt_hide(MachbusSession *h, uint16_t object_id);

/**
 * Set a VT object's numeric value. Requires the VT client subsystem.
 */
bool machbus_session_vt_set_value(MachbusSession *h, uint16_t object_id, uint32_t value);

/**
 * Set a VT object's string value (UTF-8, NUL-terminated). Requires the VT
 * client subsystem.
 */
bool machbus_session_vt_set_string(MachbusSession *h, uint16_t object_id, const char *value);

/**
 * Begin TC client connection / DDOP upload. Requires the TC client subsystem.
 */
bool machbus_session_tc_connect(MachbusSession *h);

/**
 * Disconnect the TC client. Requires the TC client subsystem.
 */
bool machbus_session_tc_disconnect(MachbusSession *h);

/**
 * TC client connection state code (see `tc_state_code`); 0 (Disconnected) if
 * the TC client subsystem is not plugged.
 */
uint32_t machbus_session_tc_state(const MachbusSession *h);

/**
 * Whether the TC client is connected.
 */
bool machbus_session_tc_is_connected(const MachbusSession *h);

/**
 * J1939 message priority (0 = highest).
 */
uint8_t machbus_identifier_priority(uint32_t raw);

/**
 * Parameter Group Number.
 */
uint32_t machbus_identifier_pgn(uint32_t raw);

/**
 * Source address.
 */
uint8_t machbus_identifier_source(uint32_t raw);

/**
 * Destination address (meaningful only for PDU1).
 */
uint8_t machbus_identifier_destination(uint32_t raw);

/**
 * True for PDU2 (broadcast) identifiers; false for PDU1 (peer-to-peer).
 */
bool machbus_identifier_is_pdu2(uint32_t raw);

/**
 * True if the identifier addresses the global (broadcast) destination.
 */
bool machbus_identifier_is_broadcast(uint32_t raw);

/**
 * Decode an 8-byte EEC1 payload into `out`. Returns false (and sets the
 * last error) on a null/short/invalid payload.
 */
bool machbus_j1939_eec1_decode(const uint8_t *data, uintptr_t len, MachbusEec1 *out);

/**
 * Encode an EEC1 struct into the caller's 8-byte buffer `out`.
 */
bool machbus_j1939_eec1_encode(const MachbusEec1 *input, uint8_t *out);

bool machbus_j1939_eec2_decode(const uint8_t *data, uintptr_t len, MachbusEec2 *out);

bool machbus_j1939_eec2_encode(const MachbusEec2 *input, uint8_t *out);

bool machbus_j1939_eec3_decode(const uint8_t *data, uintptr_t len, MachbusEec3 *out);

bool machbus_j1939_eec3_encode(const MachbusEec3 *input, uint8_t *out);

bool machbus_j1939_engine_temp1_decode(const uint8_t *data, uintptr_t len, MachbusEngineTemp1 *out);

bool machbus_j1939_engine_temp1_encode(const MachbusEngineTemp1 *input, uint8_t *out);

bool machbus_j1939_engine_temp2_decode(const uint8_t *data, uintptr_t len, MachbusEngineTemp2 *out);

bool machbus_j1939_engine_temp2_encode(const MachbusEngineTemp2 *input, uint8_t *out);

bool machbus_j1939_engine_fluid_lp_decode(const uint8_t *data,
                                          uintptr_t len,
                                          MachbusEngineFluidLp *out);

bool machbus_j1939_engine_fluid_lp_encode(const MachbusEngineFluidLp *input, uint8_t *out);

bool machbus_j1939_engine_hours_decode(const uint8_t *data, uintptr_t len, MachbusEngineHours *out);

bool machbus_j1939_engine_hours_encode(const MachbusEngineHours *input, uint8_t *out);

bool machbus_j1939_fuel_economy_decode(const uint8_t *data, uintptr_t len, MachbusFuelEconomy *out);

bool machbus_j1939_fuel_economy_encode(const MachbusFuelEconomy *input, uint8_t *out);

bool machbus_j1939_tsc1_decode(const uint8_t *data, uintptr_t len, MachbusTsc1 *out);

bool machbus_j1939_tsc1_encode(const MachbusTsc1 *input, uint8_t *out);

bool machbus_j1939_vep1_decode(const uint8_t *data, uintptr_t len, MachbusVep1 *out);

bool machbus_j1939_vep1_encode(const MachbusVep1 *input, uint8_t *out);

bool machbus_j1939_ambient_conditions_decode(const uint8_t *data,
                                             uintptr_t len,
                                             MachbusAmbientConditions *out);

bool machbus_j1939_ambient_conditions_encode(const MachbusAmbientConditions *input, uint8_t *out);

bool machbus_j1939_dash_display_decode(const uint8_t *data, uintptr_t len, MachbusDashDisplay *out);

bool machbus_j1939_dash_display_encode(const MachbusDashDisplay *input, uint8_t *out);

bool machbus_j1939_vehicle_position_decode(const uint8_t *data,
                                           uintptr_t len,
                                           MachbusVehiclePosition *out);

bool machbus_j1939_vehicle_position_encode(const MachbusVehiclePosition *input, uint8_t *out);

bool machbus_j1939_fuel_consumption_decode(const uint8_t *data,
                                           uintptr_t len,
                                           MachbusFuelConsumption *out);

bool machbus_j1939_fuel_consumption_encode(const MachbusFuelConsumption *input, uint8_t *out);

bool machbus_j1939_aftertreatment1_decode(const uint8_t *data,
                                          uintptr_t len,
                                          MachbusAftertreatment1 *out);

bool machbus_j1939_aftertreatment1_encode(const MachbusAftertreatment1 *input, uint8_t *out);

bool machbus_j1939_aftertreatment2_decode(const uint8_t *data,
                                          uintptr_t len,
                                          MachbusAftertreatment2 *out);

bool machbus_j1939_aftertreatment2_encode(const MachbusAftertreatment2 *input, uint8_t *out);

/**
 * Decode a ComponentIdentification payload. Returns an owned handle (free
 * with [`machbus_j1939_component_identification_free`]) or `NULL` on failure.
 */
MachbusComponentIdentification *machbus_j1939_component_identification_decode(const uint8_t *data,
                                                                              uintptr_t len);

/**
 * Free a ComponentIdentification handle. Accepts `NULL`.
 */
void machbus_j1939_component_identification_free(MachbusComponentIdentification *h);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_j1939_component_identification_make_into(const MachbusComponentIdentification *h,
                                                           char *out,
                                                           uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_j1939_component_identification_model_into(const MachbusComponentIdentification *h,
                                                            char *out,
                                                            uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_j1939_component_identification_serial_number_into(const MachbusComponentIdentification *h,
                                                                    char *out,
                                                                    uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_j1939_component_identification_unit_number_into(const MachbusComponentIdentification *h,
                                                                  char *out,
                                                                  uintptr_t cap);

/**
 * Encode a ComponentIdentification from four NUL-terminated field strings into
 * `out` (cap bytes). Returns the full encoded length; if it exceeds `cap`
 * nothing is copied. Returns 0 on a null field pointer.
 */
uintptr_t machbus_j1939_component_identification_encode(const char *make,
                                                        const char *model,
                                                        const char *serial_number,
                                                        const char *unit_number,
                                                        uint8_t *out,
                                                        uintptr_t cap);

/**
 * Decode a VehicleIdentification (`*`-terminated VIN) into `out` (cap bytes).
 * Returns the VIN byte length (excluding NUL), or 0 on decode failure.
 */
uintptr_t machbus_j1939_vehicle_identification_decode(const uint8_t *data,
                                                      uintptr_t len,
                                                      char *out,
                                                      uintptr_t cap);

/**
 * Encode a VehicleIdentification from a NUL-terminated VIN into `out` (cap
 * bytes). Returns the full encoded length; if it exceeds `cap` nothing is
 * copied.
 */
uintptr_t machbus_j1939_vehicle_identification_encode(const char *vin, uint8_t *out, uintptr_t cap);

bool machbus_j1939_dtc_decode(const uint8_t *data, uintptr_t len, MachbusDtc *out);

bool machbus_j1939_dtc_encode(const MachbusDtc *input, uint8_t *out);

bool machbus_j1939_diagnostic_lamps_decode(const uint8_t *data,
                                           uintptr_t len,
                                           MachbusDiagnosticLamps *out);

bool machbus_j1939_diagnostic_lamps_encode(const MachbusDiagnosticLamps *input, uint8_t *out);

bool machbus_j1939_diagnostic_protocol_id_decode(const uint8_t *data,
                                                 uintptr_t len,
                                                 MachbusDiagnosticProtocolId *out);

bool machbus_j1939_diagnostic_protocol_id_encode(const MachbusDiagnosticProtocolId *input,
                                                 uint8_t *out);

/**
 * Decode a DM1/DM2-style lamp+DTC list. Returns an owned handle (free with
 * [`machbus_dm_dtc_list_free`]) or `NULL` on failure.
 */
MachbusDmDtcList *machbus_dm_dtc_list_decode(const uint8_t *data, uintptr_t len);

/**
 * Copy the lamp block out of a DmDtcList handle.
 */
bool machbus_dm_dtc_list_lamps(const MachbusDmDtcList *h, MachbusDiagnosticLamps *out);

/**
 * Number of DTCs in a DmDtcList handle.
 */
uintptr_t machbus_dm_dtc_list_count(const MachbusDmDtcList *h);

/**
 * Copy the DTC at `idx` out of a DmDtcList handle. Returns false if `idx` is
 * out of range or pointers are null.
 */
bool machbus_dm_dtc_list_get(const MachbusDmDtcList *h, uintptr_t idx, MachbusDtc *out);

/**
 * Free a DmDtcList handle. Accepts `NULL`.
 */
void machbus_dm_dtc_list_free(MachbusDmDtcList *h);

/**
 * Decode a DM3/DM11 clear-all request (the all-`0xFF` reserved payload).
 * Returns true on the canonical payload.
 */
bool machbus_j1939_dm_clear_all_request_decode(const uint8_t *data, uintptr_t len);

/**
 * Encode a DM3/DM11 clear-all request into the caller's 8-byte buffer.
 */
bool machbus_j1939_dm_clear_all_request_encode(uint8_t *out);

/**
 * Decode a DM4 message. Returns an owned handle (free with
 * [`machbus_dm4_message_free`]) or `NULL` on failure.
 */
MachbusDm4Message *machbus_dm4_message_decode(const uint8_t *data, uintptr_t len);

/**
 * Copy the DM4 lamp header out of a handle.
 */
bool machbus_dm4_message_lamps(const MachbusDm4Message *h, MachbusDm4Lamps *out);

/**
 * Number of DTCs in a DM4 message handle.
 */
uintptr_t machbus_dm4_message_count(const MachbusDm4Message *h);

/**
 * Copy the DTC at `idx` out of a DM4 message handle.
 */
bool machbus_dm4_message_get(const MachbusDm4Message *h, uintptr_t idx, MachbusDtc *out);

/**
 * Free a DM4 message handle. Accepts `NULL`.
 */
void machbus_dm4_message_free(MachbusDm4Message *h);

bool machbus_j1939_dm7_command_decode(const uint8_t *data, uintptr_t len, MachbusDm7Command *out);

bool machbus_j1939_dm7_command_encode(const MachbusDm7Command *input, uint8_t *out);

bool machbus_j1939_dm8_test_result_decode(const uint8_t *data,
                                          uintptr_t len,
                                          MachbusDm8TestResult *out);

bool machbus_j1939_dm8_test_result_encode(const MachbusDm8TestResult *input, uint8_t *out);

bool machbus_j1939_dm13_signals_decode(const uint8_t *data, uintptr_t len, MachbusDm13Signals *out);

bool machbus_j1939_dm13_signals_encode(const MachbusDm13Signals *input, uint8_t *out);

bool machbus_j1939_dm21_readiness_decode(const uint8_t *data,
                                         uintptr_t len,
                                         MachbusDm21Readiness *out);

bool machbus_j1939_dm21_readiness_encode(const MachbusDm21Readiness *input, uint8_t *out);

bool machbus_j1939_dm22_message_decode(const uint8_t *data, uintptr_t len, MachbusDm22Message *out);

bool machbus_j1939_dm22_message_encode(const MachbusDm22Message *input, uint8_t *out);

/**
 * Decode a DM9 Vehicle Identification request (a PGN-request naming the VIN
 * PGN). Returns true on a valid DM9 request.
 */
bool machbus_j1939_dm9_request_decode(const uint8_t *data, uintptr_t len);

/**
 * Encode a DM9 Vehicle Identification request into the caller's 3-byte buffer.
 */
bool machbus_j1939_dm9_request_encode(uint8_t *out);

/**
 * Decode a DM10 Vehicle Identification (`*`-terminated VIN) into `out` (cap
 * bytes). Returns the VIN byte length (excluding NUL), or 0 on failure.
 */
uintptr_t machbus_j1939_dm10_vehicle_identification_decode(const uint8_t *data,
                                                           uintptr_t len,
                                                           char *out,
                                                           uintptr_t cap);

/**
 * Encode a DM10 Vehicle Identification from a NUL-terminated VIN into `out`
 * (cap bytes). Returns the full encoded length; if it exceeds `cap` nothing is
 * copied. Returns 0 on validation/null failure.
 */
uintptr_t machbus_j1939_dm10_vehicle_identification_encode(const char *vin,
                                                           uint8_t *out,
                                                           uintptr_t cap);

/**
 * Decode a ProductIdentification payload. Returns an owned handle (free with
 * [`machbus_j1939_product_identification_free`]) or `NULL` on failure.
 */
MachbusProductIdentification *machbus_j1939_product_identification_decode(const uint8_t *data,
                                                                          uintptr_t len);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_j1939_product_identification_make_into(const MachbusProductIdentification *h,
                                                         char *out,
                                                         uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_j1939_product_identification_model_into(const MachbusProductIdentification *h,
                                                          char *out,
                                                          uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_j1939_product_identification_serial_number_into(const MachbusProductIdentification *h,
                                                                  char *out,
                                                                  uintptr_t cap);

/**
 * Free a ProductIdentification handle. Accepts `NULL`.
 */
void machbus_j1939_product_identification_free(MachbusProductIdentification *h);

/**
 * Encode a ProductIdentification from three NUL-terminated field strings into
 * `out` (cap bytes). Returns the full encoded length; if it exceeds `cap`
 * nothing is copied. Returns 0 on validation/null failure.
 */
uintptr_t machbus_j1939_product_identification_encode(const char *make,
                                                      const char *model,
                                                      const char *serial_number,
                                                      uint8_t *out,
                                                      uintptr_t cap);

/**
 * Decode a SoftwareIdentification payload. Returns an owned handle (free with
 * [`machbus_j1939_software_identification_free`]) or `NULL` on failure.
 */
MachbusSoftwareIdentification *machbus_j1939_software_identification_decode(const uint8_t *data,
                                                                            uintptr_t len);

/**
 * Number of version strings in a SoftwareIdentification handle.
 */
uintptr_t machbus_j1939_software_identification_count(const MachbusSoftwareIdentification *h);

/**
 * Copy version `idx` as a NUL-terminated string into `out` (cap bytes).
 * Returns the byte length (excluding NUL), or 0 if `idx` is out of range.
 */
uintptr_t machbus_j1939_software_identification_get_into(const MachbusSoftwareIdentification *h,
                                                         uintptr_t idx,
                                                         char *out,
                                                         uintptr_t cap);

/**
 * Free a SoftwareIdentification handle. Accepts `NULL`.
 */
void machbus_j1939_software_identification_free(MachbusSoftwareIdentification *h);

bool machbus_j1939_monitor_performance_ratio_decode(const uint8_t *data,
                                                    uintptr_t len,
                                                    MachbusMonitorPerformanceRatio *out);

bool machbus_j1939_monitor_performance_ratio_encode(const MachbusMonitorPerformanceRatio *input,
                                                    uint8_t *out);

/**
 * Decode a DM20 response. Returns an owned handle (free with
 * [`machbus_dm20_response_free`]) or `NULL` on failure.
 */
MachbusDm20Response *machbus_dm20_response_decode(const uint8_t *data, uintptr_t len);

/**
 * DM20 ignition-cycle counter, or 0 if the handle is null.
 */
uint8_t machbus_dm20_response_ignition_cycles(const MachbusDm20Response *h);

/**
 * DM20 OBD monitoring-conditions counter, or 0 if the handle is null.
 */
uint8_t machbus_dm20_response_obd_conditions(const MachbusDm20Response *h);

/**
 * Number of performance ratios in a DM20 response handle.
 */
uintptr_t machbus_dm20_response_count(const MachbusDm20Response *h);

/**
 * Copy the performance ratio at `idx` out of a DM20 response handle.
 */
bool machbus_dm20_response_get(const MachbusDm20Response *h,
                               uintptr_t idx,
                               MachbusMonitorPerformanceRatio *out);

/**
 * Free a DM20 response handle. Accepts `NULL`.
 */
void machbus_dm20_response_free(MachbusDm20Response *h);

bool machbus_j1939_spn_snapshot_decode(const uint8_t *data, uintptr_t len, MachbusSpnSnapshot *out);

bool machbus_j1939_spn_snapshot_encode(const MachbusSpnSnapshot *input, uint8_t *out);

/**
 * Decode a FreezeFrame payload. Returns an owned handle (free with
 * [`machbus_freeze_frame_free`]) or `NULL` on failure.
 */
MachbusFreezeFrame *machbus_freeze_frame_decode(const uint8_t *data, uintptr_t len);

/**
 * Copy the FreezeFrame DTC out of a handle.
 */
bool machbus_freeze_frame_dtc(const MachbusFreezeFrame *h, MachbusDtc *out);

/**
 * FreezeFrame internal timestamp (ms), or 0 if the handle is null.
 */
uint32_t machbus_freeze_frame_timestamp_ms(const MachbusFreezeFrame *h);

/**
 * Number of SPN snapshots in a FreezeFrame handle.
 */
uintptr_t machbus_freeze_frame_count(const MachbusFreezeFrame *h);

/**
 * Copy the SPN snapshot at `idx` out of a FreezeFrame handle.
 */
bool machbus_freeze_frame_get(const MachbusFreezeFrame *h, uintptr_t idx, MachbusSpnSnapshot *out);

/**
 * Free a FreezeFrame handle. Accepts `NULL`.
 */
void machbus_freeze_frame_free(MachbusFreezeFrame *h);

bool machbus_j1939_dm25_request_decode(const uint8_t *data, uintptr_t len, MachbusDm25Request *out);

bool machbus_j1939_dm25_request_encode(const MachbusDm25Request *input, uint8_t *out);

bool machbus_j1939_dm14_request_decode(const uint8_t *data, uintptr_t len, MachbusDm14Request *out);

bool machbus_j1939_dm14_request_encode(const MachbusDm14Request *input, uint8_t *out);

bool machbus_j1939_dm15_response_decode(const uint8_t *data,
                                        uintptr_t len,
                                        MachbusDm15Response *out);

bool machbus_j1939_dm15_response_encode(const MachbusDm15Response *input, uint8_t *out);

/**
 * Decode a DM16 binary transfer payload. Returns an owned handle (free with
 * [`machbus_dm16_transfer_free`]) or `NULL` on failure.
 */
MachbusDm16Transfer *machbus_dm16_transfer_decode(const uint8_t *data, uintptr_t len);

/**
 * Declared `num_bytes` of a DM16 transfer handle, or 0 if null.
 */
uint8_t machbus_dm16_transfer_num_bytes(const MachbusDm16Transfer *h);

/**
 * Copy the DM16 data bytes into `out` (cap bytes). Returns the full data
 * length; if it exceeds `cap` nothing is copied.
 */
uintptr_t machbus_dm16_transfer_data_into(const MachbusDm16Transfer *h,
                                          uint8_t *out,
                                          uintptr_t cap);

/**
 * Free a DM16 transfer handle. Accepts `NULL`.
 */
void machbus_dm16_transfer_free(MachbusDm16Transfer *h);

/**
 * Encode a DM16 single-frame transfer from `data` (`len` ≤ 7 bytes) into the
 * caller's 8-byte buffer `out`. Returns false on length/null errors.
 */
bool machbus_dm16_transfer_encode(const uint8_t *data, uintptr_t len, uint8_t *out);

/**
 * Decode an ECU Identification payload. Returns an owned handle (free with
 * [`machbus_ecu_identification_free`]) or `NULL` on failure.
 */
MachbusEcuIdentification *machbus_ecu_identification_decode(const uint8_t *data, uintptr_t len);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_ecu_identification_part_number_into(const MachbusEcuIdentification *h,
                                                      char *out,
                                                      uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_ecu_identification_serial_number_into(const MachbusEcuIdentification *h,
                                                        char *out,
                                                        uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_ecu_identification_location_into(const MachbusEcuIdentification *h,
                                                   char *out,
                                                   uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_ecu_identification_type_into(const MachbusEcuIdentification *h,
                                               char *out,
                                               uintptr_t cap);

/**
 * Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
 * bytes). Returns the full byte length (excluding NUL); if it equals
 * or exceeds `cap`, the value was truncated.
 */
uintptr_t machbus_ecu_identification_manufacturer_into(const MachbusEcuIdentification *h,
                                                       char *out,
                                                       uintptr_t cap);

/**
 * Whether the ECU Identification has the optional ISO 11783 hardware-id field.
 */
bool machbus_ecu_identification_has_hardware_id(const MachbusEcuIdentification *h);

/**
 * Copy the optional hardware-id field as a NUL-terminated string into `out`.
 * Returns the byte length (excluding NUL); 0 if absent.
 */
uintptr_t machbus_ecu_identification_hardware_id_into(const MachbusEcuIdentification *h,
                                                      char *out,
                                                      uintptr_t cap);

/**
 * Free an ECU Identification handle. Accepts `NULL`.
 */
void machbus_ecu_identification_free(MachbusEcuIdentification *h);

