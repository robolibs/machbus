/// Hide a VT object by id. Requires the VT client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_hide(h: *mut MachbusSession, object_id: u16) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let vt = plugin_mut!(h, VtClient);
    bool_result(vt.hide(ObjectID(object_id)))
}

/// Set a VT object's numeric value. Requires the VT client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_set_value(
    h: *mut MachbusSession,
    object_id: u16,
    value: u32,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let vt = plugin_mut!(h, VtClient);
    bool_result(vt.set_value(ObjectID(object_id), value))
}

/// Set a VT object's string value (UTF-8, NUL-terminated). Requires the VT
/// client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_set_string(
    h: *mut MachbusSession,
    object_id: u16,
    value: *const c_char,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let Some(s) = read_c_str(value) else {
        set_last_error("invalid VT string value");
        return false;
    };
    let vt = plugin_mut!(h, VtClient);
    bool_result(vt.set_string(ObjectID(object_id), &s))
}

// ─── TC client ────────────────────────────────────────────────────────

/// Begin TC client connection / DDOP upload. Requires the TC client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_connect(h: *mut MachbusSession) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let tc = plugin_mut!(h, TcClient);
    bool_result(tc.connect())
}

/// Disconnect the TC client. Requires the TC client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_disconnect(h: *mut MachbusSession) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let tc = plugin_mut!(h, TcClient);
    bool_result(tc.disconnect())
}

/// TC client connection state code (see `tc_state_code`); 0 (Disconnected) if
/// the TC client subsystem is not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_state(h: *const MachbusSession) -> u32 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<TcClient>())
        .map(|tc| tc_state_code(tc.state()))
        .unwrap_or(0)
}

/// Whether the TC client is connected.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tc_is_connected(h: *const MachbusSession) -> bool {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<TcClient>())
        .map(TcClient::is_connected)
        .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════════════════════
// Standalone codec layer — inspect/decode/encode CAN, J1939 and NMEA
// without a session. Mirrors `machbus::net` / `machbus::j1939` /
// `machbus::nmea` 1:1. (Pattern slice: net::Identifier + j1939::Eec1.)
// ═══════════════════════════════════════════════════════════════════════

// ── net::Identifier ── decompose a raw 29-bit CAN identifier ──────────

/// J1939 message priority (0 = highest).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_identifier_priority(raw: u32) -> u8 {
    u8::from(Identifier::from_raw(raw).priority())
}

/// Parameter Group Number.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_identifier_pgn(raw: u32) -> u32 {
    Identifier::from_raw(raw).pgn()
}

/// Source address.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_identifier_source(raw: u32) -> u8 {
    Identifier::from_raw(raw).source()
}

/// Destination address (meaningful only for PDU1).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_identifier_destination(raw: u32) -> u8 {
    Identifier::from_raw(raw).destination()
}

/// True for PDU2 (broadcast) identifiers; false for PDU1 (peer-to-peer).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_identifier_is_pdu2(raw: u32) -> bool {
    Identifier::from_raw(raw).is_pdu2()
}

/// True if the identifier addresses the global (broadcast) destination.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_identifier_is_broadcast(raw: u32) -> bool {
    Identifier::from_raw(raw).is_broadcast()
}

// ── j1939::Eec1 ── engine speed / torque (PGN 61444) ──────────────────

/// `#[repr(C)]` mirror of [`machbus::j1939::Eec1`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEec1 {
    pub engine_torque_percent: f64,
    pub driver_demand_percent: f64,
    pub actual_engine_percent: f64,
    pub engine_speed_rpm: f64,
    pub starter_mode: u8,
    pub source_address: u8,
}

impl From<crate::j1939::Eec1> for MachbusEec1 {
    fn from(e: crate::j1939::Eec1) -> Self {
        Self {
            engine_torque_percent: e.engine_torque_percent,
            driver_demand_percent: e.driver_demand_percent,
            actual_engine_percent: e.actual_engine_percent,
            engine_speed_rpm: e.engine_speed_rpm,
            starter_mode: e.starter_mode,
            source_address: e.source_address,
        }
    }
}

impl From<MachbusEec1> for crate::j1939::Eec1 {
    fn from(e: MachbusEec1) -> Self {
        Self {
            engine_torque_percent: e.engine_torque_percent,
            driver_demand_percent: e.driver_demand_percent,
            actual_engine_percent: e.actual_engine_percent,
            engine_speed_rpm: e.engine_speed_rpm,
            starter_mode: e.starter_mode,
            source_address: e.source_address,
        }
    }
}

/// Decode an 8-byte EEC1 payload into `out`. Returns false (and sets the
/// last error) on a null/short/invalid payload.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_eec1_decode(
    data: *const u8,
    len: usize,
    out: *mut MachbusEec1,
) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    match crate::j1939::Eec1::decode(bytes) {
        Some(e) => {
            unsafe { *out = e.into() };
            clear_last_error();
            true
        }
        None => {
            set_last_error("EEC1 decode failed");
            false
        }
    }
}

/// Encode an EEC1 struct into the caller's 8-byte buffer `out`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_eec1_encode(input: *const MachbusEec1, out: *mut u8) -> bool {
    if input.is_null() || out.is_null() {
        set_last_error("null pointer");
        return false;
    }
    let eec1: crate::j1939::Eec1 = (unsafe { *input }).into();
    let bytes = eec1.encode();
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    clear_last_error();
    true
}

// ─── Codec helper macros ──────────────────────────────────────────────
//
// `pod_codec!` generates a fixed-buffer decode/encode pair for a `#[repr(C)]`
// POD that has `From<Rust> for Pod` and `From<Pod> for Rust` plus a Rust
// `decode(&[u8]) -> Option<_>` / `encode() -> [u8; N]`. The encode buffer must
// be at least `N` bytes wide.

macro_rules! pod_codec {
    (
        $pod:ty, $rust:path, $enc_len:expr,
        decode = $dec_fn:ident, encode = $enc_fn:ident,
        err = $err:expr
    ) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $dec_fn(data: *const u8, len: usize, out: *mut $pod) -> bool {
            if out.is_null() {
                set_last_error("null output pointer");
                return false;
            }
            let bytes = match read_bytes(data, len) {
                Ok(b) => b,
                Err(e) => {
                    set_last_error(e);
                    return false;
                }
            };
            match <$rust>::decode(bytes) {
                Some(v) => {
                    unsafe { *out = v.into() };
                    clear_last_error();
                    true
                }
                None => {
                    set_last_error($err);
                    false
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $enc_fn(input: *const $pod, out: *mut u8) -> bool {
            if input.is_null() || out.is_null() {
                set_last_error("null pointer");
                return false;
            }
            let v: $rust = (unsafe { *input }).into();
            let bytes = v.encode();
            unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, $enc_len) };
            clear_last_error();
            true
        }
    };
}

// `pod_codec_try_encode!` is the same but for Rust `encode() -> Result<[u8; N]>`.
macro_rules! pod_codec_try_encode {
    (
        $pod:ty, $rust:path, $enc_len:expr,
        decode = $dec_fn:ident, encode = $enc_fn:ident,
        err = $err:expr
    ) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $dec_fn(data: *const u8, len: usize, out: *mut $pod) -> bool {
            if out.is_null() {
                set_last_error("null output pointer");
                return false;
            }
            let bytes = match read_bytes(data, len) {
                Ok(b) => b,
                Err(e) => {
                    set_last_error(e);
                    return false;
                }
            };
            match <$rust>::decode(bytes) {
                Some(v) => {
                    unsafe { *out = v.into() };
                    clear_last_error();
                    true
                }
                None => {
                    set_last_error($err);
                    false
                }
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn $enc_fn(input: *const $pod, out: *mut u8) -> bool {
            if input.is_null() || out.is_null() {
                set_last_error("null pointer");
                return false;
            }
            let v: $rust = (unsafe { *input }).into();
            match v.encode() {
                Ok(bytes) => {
                    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, $enc_len) };
                    clear_last_error();
                    true
                }
                Err(e) => {
                    set_last_error(e.to_string());
                    false
                }
            }
        }
    };
}

// ══════════════════════════════════════════════════════════════════════
// j1939::engine — remaining fixed-size POD codecs
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::Eec2`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEec2 {
    pub accel_pedal_position: u8,
    pub engine_load_percent: f64,
    pub accel_pedal_low_idle: u8,
    pub accel_pedal_kickdown: u8,
    pub road_speed_limit: u8,
}

impl From<crate::j1939::Eec2> for MachbusEec2 {
    fn from(e: crate::j1939::Eec2) -> Self {
        Self {
            accel_pedal_position: e.accel_pedal_position,
            engine_load_percent: e.engine_load_percent,
            accel_pedal_low_idle: e.accel_pedal_low_idle,
            accel_pedal_kickdown: e.accel_pedal_kickdown,
            road_speed_limit: e.road_speed_limit,
        }
    }
}

impl From<MachbusEec2> for crate::j1939::Eec2 {
    fn from(e: MachbusEec2) -> Self {
        Self {
            accel_pedal_position: e.accel_pedal_position,
            engine_load_percent: e.engine_load_percent,
            accel_pedal_low_idle: e.accel_pedal_low_idle,
            accel_pedal_kickdown: e.accel_pedal_kickdown,
            road_speed_limit: e.road_speed_limit,
        }
    }
}

pod_codec!(
    MachbusEec2,
    crate::j1939::Eec2,
    8,
    decode = machbus_j1939_eec2_decode,
    encode = machbus_j1939_eec2_encode,
    err = "EEC2 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::Eec3`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEec3 {
    pub nominal_friction_percent: f64,
    pub desired_operating_speed_rpm: f64,
    pub operating_speed_asymmetry: u8,
}

impl From<crate::j1939::Eec3> for MachbusEec3 {
    fn from(e: crate::j1939::Eec3) -> Self {
        Self {
            nominal_friction_percent: e.nominal_friction_percent,
            desired_operating_speed_rpm: e.desired_operating_speed_rpm,
            operating_speed_asymmetry: e.operating_speed_asymmetry,
        }
    }
}

impl From<MachbusEec3> for crate::j1939::Eec3 {
    fn from(e: MachbusEec3) -> Self {
        Self {
            nominal_friction_percent: e.nominal_friction_percent,
            desired_operating_speed_rpm: e.desired_operating_speed_rpm,
            operating_speed_asymmetry: e.operating_speed_asymmetry,
        }
    }
}

pod_codec!(
    MachbusEec3,
    crate::j1939::Eec3,
    8,
    decode = machbus_j1939_eec3_decode,
    encode = machbus_j1939_eec3_encode,
    err = "EEC3 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::EngineTemp1`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEngineTemp1 {
    pub coolant_temp_c: f64,
    pub fuel_temp_c: f64,
    pub oil_temp_c: f64,
    pub turbo_oil_temp_c: f64,
    pub intercooler_temp_c: f64,
}

impl From<crate::j1939::EngineTemp1> for MachbusEngineTemp1 {
    fn from(e: crate::j1939::EngineTemp1) -> Self {
        Self {
            coolant_temp_c: e.coolant_temp_c,
            fuel_temp_c: e.fuel_temp_c,
            oil_temp_c: e.oil_temp_c,
            turbo_oil_temp_c: e.turbo_oil_temp_c,
            intercooler_temp_c: e.intercooler_temp_c,
        }
    }
}

impl From<MachbusEngineTemp1> for crate::j1939::EngineTemp1 {
    fn from(e: MachbusEngineTemp1) -> Self {
        Self {
            coolant_temp_c: e.coolant_temp_c,
            fuel_temp_c: e.fuel_temp_c,
            oil_temp_c: e.oil_temp_c,
            turbo_oil_temp_c: e.turbo_oil_temp_c,
            intercooler_temp_c: e.intercooler_temp_c,
        }
    }
}

pod_codec!(
    MachbusEngineTemp1,
    crate::j1939::EngineTemp1,
    8,
    decode = machbus_j1939_engine_temp1_decode,
    encode = machbus_j1939_engine_temp1_encode,
    err = "EngineTemp1 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::EngineTemp2`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEngineTemp2 {
    pub engine_oil_temp_c: f64,
    pub turbo_oil_temp_c: f64,
    pub engine_intercooler_temp_c: f64,
    pub turbo_1_temp_c: f64,
}

impl From<crate::j1939::EngineTemp2> for MachbusEngineTemp2 {
    fn from(e: crate::j1939::EngineTemp2) -> Self {
        Self {
            engine_oil_temp_c: e.engine_oil_temp_c,
            turbo_oil_temp_c: e.turbo_oil_temp_c,
            engine_intercooler_temp_c: e.engine_intercooler_temp_c,
            turbo_1_temp_c: e.turbo_1_temp_c,
        }
    }
}

impl From<MachbusEngineTemp2> for crate::j1939::EngineTemp2 {
    fn from(e: MachbusEngineTemp2) -> Self {
        Self {
            engine_oil_temp_c: e.engine_oil_temp_c,
            turbo_oil_temp_c: e.turbo_oil_temp_c,
            engine_intercooler_temp_c: e.engine_intercooler_temp_c,
            turbo_1_temp_c: e.turbo_1_temp_c,
        }
    }
}

pod_codec!(
    MachbusEngineTemp2,
    crate::j1939::EngineTemp2,
    8,
    decode = machbus_j1939_engine_temp2_decode,
    encode = machbus_j1939_engine_temp2_encode,
    err = "EngineTemp2 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::EngineFluidLp`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEngineFluidLp {
    pub oil_pressure_kpa: f64,
    pub coolant_pressure_kpa: f64,
    pub oil_level_percent: u8,
    pub coolant_level_percent: u8,
    pub fuel_delivery_pressure_kpa: f64,
    pub crankcase_pressure_kpa: f64,
}

impl From<crate::j1939::EngineFluidLp> for MachbusEngineFluidLp {
    fn from(e: crate::j1939::EngineFluidLp) -> Self {
        Self {
            oil_pressure_kpa: e.oil_pressure_kpa,
            coolant_pressure_kpa: e.coolant_pressure_kpa,
            oil_level_percent: e.oil_level_percent,
            coolant_level_percent: e.coolant_level_percent,
            fuel_delivery_pressure_kpa: e.fuel_delivery_pressure_kpa,
            crankcase_pressure_kpa: e.crankcase_pressure_kpa,
        }
    }
}

impl From<MachbusEngineFluidLp> for crate::j1939::EngineFluidLp {
    fn from(e: MachbusEngineFluidLp) -> Self {
        Self {
            oil_pressure_kpa: e.oil_pressure_kpa,
            coolant_pressure_kpa: e.coolant_pressure_kpa,
            oil_level_percent: e.oil_level_percent,
            coolant_level_percent: e.coolant_level_percent,
            fuel_delivery_pressure_kpa: e.fuel_delivery_pressure_kpa,
            crankcase_pressure_kpa: e.crankcase_pressure_kpa,
        }
    }
}

pod_codec!(
    MachbusEngineFluidLp,
    crate::j1939::EngineFluidLp,
    8,
    decode = machbus_j1939_engine_fluid_lp_decode,
    encode = machbus_j1939_engine_fluid_lp_encode,
    err = "EngineFluidLp decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::EngineHours`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEngineHours {
    pub total_hours: f64,
    pub total_revolutions: f64,
}

impl From<crate::j1939::EngineHours> for MachbusEngineHours {
    fn from(e: crate::j1939::EngineHours) -> Self {
        Self {
            total_hours: e.total_hours,
            total_revolutions: e.total_revolutions,
        }
    }
}

impl From<MachbusEngineHours> for crate::j1939::EngineHours {
    fn from(e: MachbusEngineHours) -> Self {
        Self {
            total_hours: e.total_hours,
            total_revolutions: e.total_revolutions,
        }
    }
}

pod_codec!(
    MachbusEngineHours,
    crate::j1939::EngineHours,
    8,
    decode = machbus_j1939_engine_hours_decode,
    encode = machbus_j1939_engine_hours_encode,
    err = "EngineHours decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::FuelEconomy`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusFuelEconomy {
    pub fuel_rate_lph: f64,
    pub instantaneous_lph: f64,
    pub throttle_position: f64,
}

impl From<crate::j1939::FuelEconomy> for MachbusFuelEconomy {
    fn from(e: crate::j1939::FuelEconomy) -> Self {
        Self {
            fuel_rate_lph: e.fuel_rate_lph,
            instantaneous_lph: e.instantaneous_lph,
            throttle_position: e.throttle_position,
        }
    }
}

impl From<MachbusFuelEconomy> for crate::j1939::FuelEconomy {
    fn from(e: MachbusFuelEconomy) -> Self {
        Self {
            fuel_rate_lph: e.fuel_rate_lph,
            instantaneous_lph: e.instantaneous_lph,
            throttle_position: e.throttle_position,
        }
    }
}

pod_codec!(
    MachbusFuelEconomy,
    crate::j1939::FuelEconomy,
    8,
    decode = machbus_j1939_fuel_economy_decode,
    encode = machbus_j1939_fuel_economy_encode,
    err = "FuelEconomy decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::Tsc1`]. `override_mode` is the
/// raw [`OverrideControlMode`] byte (0=NoOverride, 1=SpeedControl,
/// 2=TorqueControl, 3=SpeedTorqueLimit).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusTsc1 {
    pub override_mode: u8,
    pub requested_speed_rpm: f64,
    pub requested_torque_percent: f64,
}

impl From<crate::j1939::Tsc1> for MachbusTsc1 {
    fn from(e: crate::j1939::Tsc1) -> Self {
        Self {
            override_mode: e.override_mode.as_u8(),
            requested_speed_rpm: e.requested_speed_rpm,
            requested_torque_percent: e.requested_torque_percent,
        }
    }
}

impl From<MachbusTsc1> for crate::j1939::Tsc1 {
    fn from(e: MachbusTsc1) -> Self {
        Self {
            override_mode: crate::j1939::OverrideControlMode::from_u8(e.override_mode),
            requested_speed_rpm: e.requested_speed_rpm,
            requested_torque_percent: e.requested_torque_percent,
        }
    }
}

pod_codec!(
    MachbusTsc1,
    crate::j1939::Tsc1,
    8,
    decode = machbus_j1939_tsc1_decode,
    encode = machbus_j1939_tsc1_encode,
    err = "TSC1 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::Vep1`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusVep1 {
    pub battery_voltage_v: f64,
    pub alternator_current_a: f64,
    pub charging_system_voltage_v: f64,
    pub key_switch_voltage_v: f64,
}

impl From<crate::j1939::Vep1> for MachbusVep1 {
    fn from(e: crate::j1939::Vep1) -> Self {
        Self {
            battery_voltage_v: e.battery_voltage_v,
            alternator_current_a: e.alternator_current_a,
            charging_system_voltage_v: e.charging_system_voltage_v,
            key_switch_voltage_v: e.key_switch_voltage_v,
        }
    }
}

impl From<MachbusVep1> for crate::j1939::Vep1 {
    fn from(e: MachbusVep1) -> Self {
        Self {
            battery_voltage_v: e.battery_voltage_v,
            alternator_current_a: e.alternator_current_a,
            charging_system_voltage_v: e.charging_system_voltage_v,
            key_switch_voltage_v: e.key_switch_voltage_v,
        }
    }
}

pod_codec!(
    MachbusVep1,
    crate::j1939::Vep1,
    8,
    decode = machbus_j1939_vep1_decode,
    encode = machbus_j1939_vep1_encode,
    err = "VEP1 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::AmbientConditions`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusAmbientConditions {
    pub barometric_pressure_kpa: f64,
    pub ambient_air_temp_c: f64,
    pub intake_air_temp_c: f64,
    pub road_surface_temp_c: f64,
}

impl From<crate::j1939::AmbientConditions> for MachbusAmbientConditions {
    fn from(e: crate::j1939::AmbientConditions) -> Self {
        Self {
            barometric_pressure_kpa: e.barometric_pressure_kpa,
            ambient_air_temp_c: e.ambient_air_temp_c,
            intake_air_temp_c: e.intake_air_temp_c,
            road_surface_temp_c: e.road_surface_temp_c,
        }
    }
}

impl From<MachbusAmbientConditions> for crate::j1939::AmbientConditions {
    fn from(e: MachbusAmbientConditions) -> Self {
        Self {
            barometric_pressure_kpa: e.barometric_pressure_kpa,
            ambient_air_temp_c: e.ambient_air_temp_c,
            intake_air_temp_c: e.intake_air_temp_c,
            road_surface_temp_c: e.road_surface_temp_c,
        }
    }
}

pod_codec!(
    MachbusAmbientConditions,
    crate::j1939::AmbientConditions,
    8,
    decode = machbus_j1939_ambient_conditions_decode,
    encode = machbus_j1939_ambient_conditions_encode,
    err = "AmbientConditions decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::DashDisplay`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDashDisplay {
    pub fuel_level_percent: u8,
    pub washer_fluid_level: u8,
    pub fuel_filter_diff_kpa: f64,
    pub oil_filter_diff_kpa: f64,
    pub cargo_ambient_temp_c: f64,
}

impl From<crate::j1939::DashDisplay> for MachbusDashDisplay {
    fn from(e: crate::j1939::DashDisplay) -> Self {
        Self {
            fuel_level_percent: e.fuel_level_percent,
            washer_fluid_level: e.washer_fluid_level,
            fuel_filter_diff_kpa: e.fuel_filter_diff_kpa,
            oil_filter_diff_kpa: e.oil_filter_diff_kpa,
            cargo_ambient_temp_c: e.cargo_ambient_temp_c,
        }
    }
}

impl From<MachbusDashDisplay> for crate::j1939::DashDisplay {
    fn from(e: MachbusDashDisplay) -> Self {
        Self {
            fuel_level_percent: e.fuel_level_percent,
            washer_fluid_level: e.washer_fluid_level,
            fuel_filter_diff_kpa: e.fuel_filter_diff_kpa,
            oil_filter_diff_kpa: e.oil_filter_diff_kpa,
            cargo_ambient_temp_c: e.cargo_ambient_temp_c,
        }
    }
}

pod_codec!(
    MachbusDashDisplay,
    crate::j1939::DashDisplay,
    8,
    decode = machbus_j1939_dash_display_decode,
    encode = machbus_j1939_dash_display_encode,
    err = "DashDisplay decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::VehiclePosition`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusVehiclePosition {
    pub latitude_deg: f64,
    pub longitude_deg: f64,
}

impl From<crate::j1939::VehiclePosition> for MachbusVehiclePosition {
    fn from(e: crate::j1939::VehiclePosition) -> Self {
        Self {
            latitude_deg: e.latitude_deg,
            longitude_deg: e.longitude_deg,
        }
    }
}

impl From<MachbusVehiclePosition> for crate::j1939::VehiclePosition {
    fn from(e: MachbusVehiclePosition) -> Self {
        Self {
            latitude_deg: e.latitude_deg,
            longitude_deg: e.longitude_deg,
        }
    }
}

pod_codec!(
    MachbusVehiclePosition,
    crate::j1939::VehiclePosition,
    8,
    decode = machbus_j1939_vehicle_position_decode,
    encode = machbus_j1939_vehicle_position_encode,
    err = "VehiclePosition decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::FuelConsumption`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusFuelConsumption {
    pub trip_fuel_l: f64,
    pub total_fuel_l: f64,
}

impl From<crate::j1939::FuelConsumption> for MachbusFuelConsumption {
    fn from(e: crate::j1939::FuelConsumption) -> Self {
        Self {
            trip_fuel_l: e.trip_fuel_l,
            total_fuel_l: e.total_fuel_l,
        }
    }
}

impl From<MachbusFuelConsumption> for crate::j1939::FuelConsumption {
    fn from(e: MachbusFuelConsumption) -> Self {
        Self {
            trip_fuel_l: e.trip_fuel_l,
            total_fuel_l: e.total_fuel_l,
        }
    }
}

pod_codec!(
    MachbusFuelConsumption,
    crate::j1939::FuelConsumption,
    8,
    decode = machbus_j1939_fuel_consumption_decode,
    encode = machbus_j1939_fuel_consumption_encode,
    err = "FuelConsumption decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::Aftertreatment1`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusAftertreatment1 {
    pub def_tank_level: f64,
    pub intake_nox_ppm: f64,
    pub outlet_nox_ppm: f64,
    pub intake_nox_reading_status: u8,
    pub outlet_nox_reading_status: u8,
}

impl From<crate::j1939::Aftertreatment1> for MachbusAftertreatment1 {
    fn from(e: crate::j1939::Aftertreatment1) -> Self {
        Self {
            def_tank_level: e.def_tank_level,
            intake_nox_ppm: e.intake_nox_ppm,
            outlet_nox_ppm: e.outlet_nox_ppm,
            intake_nox_reading_status: e.intake_nox_reading_status,
            outlet_nox_reading_status: e.outlet_nox_reading_status,
        }
    }
}

impl From<MachbusAftertreatment1> for crate::j1939::Aftertreatment1 {
    fn from(e: MachbusAftertreatment1) -> Self {
        Self {
            def_tank_level: e.def_tank_level,
            intake_nox_ppm: e.intake_nox_ppm,
            outlet_nox_ppm: e.outlet_nox_ppm,
            intake_nox_reading_status: e.intake_nox_reading_status,
            outlet_nox_reading_status: e.outlet_nox_reading_status,
        }
    }
}

pod_codec!(
    MachbusAftertreatment1,
    crate::j1939::Aftertreatment1,
    8,
    decode = machbus_j1939_aftertreatment1_decode,
    encode = machbus_j1939_aftertreatment1_encode,
    err = "Aftertreatment1 decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::Aftertreatment2`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusAftertreatment2 {
    pub dpf_differential_pressure_kpa: f64,
    pub def_concentration: f64,
    pub dpf_soot_load_percent: f64,
    pub dpf_active_regeneration_status: u8,
    pub dpf_passive_regeneration_status: u8,
}

impl From<crate::j1939::Aftertreatment2> for MachbusAftertreatment2 {
    fn from(e: crate::j1939::Aftertreatment2) -> Self {
        Self {
            dpf_differential_pressure_kpa: e.dpf_differential_pressure_kpa,
            def_concentration: e.def_concentration,
            dpf_soot_load_percent: e.dpf_soot_load_percent,
            dpf_active_regeneration_status: e.dpf_active_regeneration_status,
            dpf_passive_regeneration_status: e.dpf_passive_regeneration_status,
        }
    }
}

impl From<MachbusAftertreatment2> for crate::j1939::Aftertreatment2 {
    fn from(e: MachbusAftertreatment2) -> Self {
        Self {
            dpf_differential_pressure_kpa: e.dpf_differential_pressure_kpa,
            def_concentration: e.def_concentration,
            dpf_soot_load_percent: e.dpf_soot_load_percent,
            dpf_active_regeneration_status: e.dpf_active_regeneration_status,
            dpf_passive_regeneration_status: e.dpf_passive_regeneration_status,
        }
    }
}

pod_codec!(
    MachbusAftertreatment2,
    crate::j1939::Aftertreatment2,
    8,
    decode = machbus_j1939_aftertreatment2_decode,
    encode = machbus_j1939_aftertreatment2_encode,
    err = "Aftertreatment2 decode failed"
);

// ── ComponentIdentification (`*`-delimited strings, variable length) ──
//
// String fields cannot live in a `#[repr(C)]` struct. Decode returns an
// opaque owned handle; copy out each field with the `*_field_into` accessors
// (each returns the byte length needed, NUL terminator excluded). Encode
// takes the same field strings and writes the variable-length payload.

/// Owned, opaque decoded [`machbus::j1939::ComponentIdentification`].
pub struct MachbusComponentIdentification(crate::j1939::ComponentIdentification);

fn copy_str_out(s: &str, out: *mut c_char, cap: usize) -> usize {
    let bytes = s.as_bytes();
    if !out.is_null() && cap > 0 {
        let n = core::cmp::min(bytes.len(), cap.saturating_sub(1));
        // SAFETY: out is valid for `cap` bytes per the contract.
        let dst = unsafe { std::slice::from_raw_parts_mut(out as *mut u8, cap) };
        dst[..n].copy_from_slice(&bytes[..n]);
        dst[n] = 0;
    }
    bytes.len()
}

/// Decode a ComponentIdentification payload. Returns an owned handle (free
/// with [`machbus_j1939_component_identification_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_component_identification_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusComponentIdentification {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::ComponentIdentification::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusComponentIdentification(v)))
        }
        None => {
            set_last_error("ComponentIdentification decode failed");
            ptr::null_mut()
        }
    }
}

/// Free a ComponentIdentification handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_component_identification_free(
    h: *mut MachbusComponentIdentification,
) {
    if h.is_null() {
        return;
    }
    // SAFETY: pointer originated from Box::into_raw in the decode call.
    unsafe { drop(Box::from_raw(h)) };
}

macro_rules! str_field_accessor {
    ($fn:ident, $handle:ty, $field:ident) => {
        /// Copy the field as a NUL-terminated UTF-8 string into `out` (`cap`
        /// bytes). Returns the full byte length (excluding NUL); if it equals
        /// or exceeds `cap`, the value was truncated.
        #[unsafe(no_mangle)]
        pub extern "C" fn $fn(h: *const $handle, out: *mut c_char, cap: usize) -> usize {
            let Some(h) = (unsafe { h.as_ref() }) else {
                set_last_error("null handle");
                return 0;
            };
            copy_str_out(&h.0.$field, out, cap)
        }
    };
}

str_field_accessor!(
    machbus_j1939_component_identification_make_into,
    MachbusComponentIdentification,
    make
);
str_field_accessor!(
    machbus_j1939_component_identification_model_into,
    MachbusComponentIdentification,
    model
);
str_field_accessor!(
    machbus_j1939_component_identification_serial_number_into,
    MachbusComponentIdentification,
    serial_number
);
str_field_accessor!(
    machbus_j1939_component_identification_unit_number_into,
    MachbusComponentIdentification,
    unit_number
);

/// Encode a ComponentIdentification from four NUL-terminated field strings into
/// `out` (cap bytes). Returns the full encoded length; if it exceeds `cap`
/// nothing is copied. Returns 0 on a null field pointer.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_component_identification_encode(
    make: *const c_char,
    model: *const c_char,
    serial_number: *const c_char,
    unit_number: *const c_char,
    out: *mut u8,
    cap: usize,
) -> usize {
    let (Some(make), Some(model), Some(serial_number), Some(unit_number)) = (
        read_c_str(make),
        read_c_str(model),
        read_c_str(serial_number),
        read_c_str(unit_number),
    ) else {
        set_last_error("null/invalid field string");
        return 0;
    };
    let v = crate::j1939::ComponentIdentification {
        make,
        model,
        serial_number,
        unit_number,
    };
    let bytes = v.encode();
    if !out.is_null() && bytes.len() <= cap {
        // SAFETY: out valid for cap bytes and bytes fits.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    }
    clear_last_error();
    bytes.len()
}

/// Decode a VehicleIdentification (`*`-terminated VIN) into `out` (cap bytes).
/// Returns the VIN byte length (excluding NUL), or 0 on decode failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_vehicle_identification_decode(
    data: *const u8,
    len: usize,
    out: *mut c_char,
    cap: usize,
) -> usize {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return 0;
        }
    };
    match crate::j1939::VehicleIdentification::decode(bytes) {
        Some(v) => {
            clear_last_error();
            copy_str_out(&v.vin, out, cap)
        }
        None => {
            set_last_error("VehicleIdentification decode failed");
            0
        }
    }
}

/// Encode a VehicleIdentification from a NUL-terminated VIN into `out` (cap
/// bytes). Returns the full encoded length; if it exceeds `cap` nothing is
/// copied.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_vehicle_identification_encode(
    vin: *const c_char,
    out: *mut u8,
    cap: usize,
) -> usize {
    let Some(vin) = read_c_str(vin) else {
        set_last_error("null/invalid VIN string");
        return 0;
    };
    let bytes = crate::j1939::VehicleIdentification { vin }.encode();
    if !out.is_null() && bytes.len() <= cap {
        // SAFETY: out valid for cap bytes and bytes fits.
        unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len()) };
    }
    clear_last_error();
    bytes.len()
}

// ══════════════════════════════════════════════════════════════════════
// j1939::diagnostic
// ══════════════════════════════════════════════════════════════════════

/// `#[repr(C)]` mirror of [`machbus::j1939::diagnostic::Fmi`] (J1939-73
/// Annex C). The discriminants match the wire FMI values exactly.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusFmi {
    AboveNormal = 0,
    BelowNormal = 1,
    Erratic = 2,
    VoltageHigh = 3,
    VoltageLow = 4,
    CurrentLow = 5,
    CurrentHigh = 6,
    MechanicalFail = 7,
    AbnormalFrequency = 8,
    AbnormalUpdate = 9,
    AbnormalRateChange = 10,
    RootCauseUnknown = 11,
    BadDevice = 12,
    OutOfCalibration = 13,
    SpecialInstructions = 14,
    AboveNormalLeast = 15,
    AboveNormalModerate = 16,
    BelowNormalLeast = 17,
    BelowNormalModerate = 18,
    ReceivedNetworkData = 19,
    DataDriftedHigh = 20,
    DataDriftedLow = 21,
    ConditionExists = 31,
}

impl From<Fmi> for MachbusFmi {
    fn from(f: Fmi) -> Self {
        // Discriminants are identical to the wire byte, so go via the byte.
        match f {
            Fmi::AboveNormal => Self::AboveNormal,
            Fmi::BelowNormal => Self::BelowNormal,
            Fmi::Erratic => Self::Erratic,
            Fmi::VoltageHigh => Self::VoltageHigh,
            Fmi::VoltageLow => Self::VoltageLow,
            Fmi::CurrentLow => Self::CurrentLow,
            Fmi::CurrentHigh => Self::CurrentHigh,
            Fmi::MechanicalFail => Self::MechanicalFail,
            Fmi::AbnormalFrequency => Self::AbnormalFrequency,
            Fmi::AbnormalUpdate => Self::AbnormalUpdate,
            Fmi::AbnormalRateChange => Self::AbnormalRateChange,
            Fmi::RootCauseUnknown => Self::RootCauseUnknown,
            Fmi::BadDevice => Self::BadDevice,
            Fmi::OutOfCalibration => Self::OutOfCalibration,
            Fmi::SpecialInstructions => Self::SpecialInstructions,
            Fmi::AboveNormalLeast => Self::AboveNormalLeast,
            Fmi::AboveNormalModerate => Self::AboveNormalModerate,
            Fmi::BelowNormalLeast => Self::BelowNormalLeast,
            Fmi::BelowNormalModerate => Self::BelowNormalModerate,
            Fmi::ReceivedNetworkData => Self::ReceivedNetworkData,
            Fmi::DataDriftedHigh => Self::DataDriftedHigh,
            Fmi::DataDriftedLow => Self::DataDriftedLow,
            Fmi::ConditionExists => Self::ConditionExists,
        }
    }
}

impl From<MachbusFmi> for Fmi {
    fn from(f: MachbusFmi) -> Self {
        Fmi::from_u8(f as u8)
    }
}

/// `#[repr(C)]` mirror of [`machbus::j1939::diagnostic::Dtc`]. `fmi` carries
/// the raw FMI wire value (see [`MachbusFmi`]).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDtc {
    pub spn: u32,
    pub fmi: u8,
    pub occurrence_count: u8,
}

impl From<Dtc> for MachbusDtc {
    fn from(d: Dtc) -> Self {
        Self {
            spn: d.spn,
            fmi: d.fmi.as_u8(),
            occurrence_count: d.occurrence_count,
        }
    }
}

impl From<MachbusDtc> for Dtc {
    fn from(d: MachbusDtc) -> Self {
        Self {
            spn: d.spn,
            fmi: Fmi::from_u8(d.fmi),
            occurrence_count: d.occurrence_count,
        }
    }
}

pod_codec!(
    MachbusDtc,
    crate::j1939::Dtc,
    4,
    decode = machbus_j1939_dtc_decode,
    encode = machbus_j1939_dtc_encode,
    err = "DTC decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::diagnostic::PreviouslyActiveDtc`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusPreviouslyActiveDtc {
    pub dtc: MachbusDtc,
    pub occurrence_count: u8,
}

impl From<crate::j1939::diagnostic::PreviouslyActiveDtc> for MachbusPreviouslyActiveDtc {
    fn from(d: crate::j1939::diagnostic::PreviouslyActiveDtc) -> Self {
        Self {
            dtc: d.dtc.into(),
            occurrence_count: d.occurrence_count,
        }
    }
}

impl From<MachbusPreviouslyActiveDtc> for crate::j1939::diagnostic::PreviouslyActiveDtc {
    fn from(d: MachbusPreviouslyActiveDtc) -> Self {
        Self {
            dtc: d.dtc.into(),
            occurrence_count: d.occurrence_count,
        }
    }
}

/// `#[repr(C)]` mirror of [`machbus::j1939::DiagnosticLamps`]. Each lamp /
/// flash field is the raw 2-bit wire value (`LampStatus`: 0=Off, 1=On,
/// 2=Error, 3=N/A; `LampFlash`: 0=Slow, 1=Fast, 2=Off, 3=N/A).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDiagnosticLamps {
    pub malfunction: u8,
    pub malfunction_flash: u8,
    pub red_stop: u8,
    pub red_stop_flash: u8,
    pub amber_warning: u8,
    pub amber_warning_flash: u8,
    pub engine_protect: u8,
    pub engine_protect_flash: u8,
}

impl From<crate::j1939::DiagnosticLamps> for MachbusDiagnosticLamps {
    fn from(l: crate::j1939::DiagnosticLamps) -> Self {
        Self {
            malfunction: l.malfunction.as_u8(),
            malfunction_flash: l.malfunction_flash.as_u8(),
            red_stop: l.red_stop.as_u8(),
            red_stop_flash: l.red_stop_flash.as_u8(),
            amber_warning: l.amber_warning.as_u8(),
            amber_warning_flash: l.amber_warning_flash.as_u8(),
            engine_protect: l.engine_protect.as_u8(),
            engine_protect_flash: l.engine_protect_flash.as_u8(),
        }
    }
}

impl From<MachbusDiagnosticLamps> for crate::j1939::DiagnosticLamps {
    fn from(l: MachbusDiagnosticLamps) -> Self {
        use crate::j1939::diagnostic::{LampFlash, LampStatus};
        Self {
            malfunction: LampStatus::from_u8(l.malfunction),
            malfunction_flash: LampFlash::from_u8(l.malfunction_flash),
            red_stop: LampStatus::from_u8(l.red_stop),
            red_stop_flash: LampFlash::from_u8(l.red_stop_flash),
            amber_warning: LampStatus::from_u8(l.amber_warning),
            amber_warning_flash: LampFlash::from_u8(l.amber_warning_flash),
            engine_protect: LampStatus::from_u8(l.engine_protect),
            engine_protect_flash: LampFlash::from_u8(l.engine_protect_flash),
        }
    }
}

pod_codec!(
    MachbusDiagnosticLamps,
    crate::j1939::DiagnosticLamps,
    2,
    decode = machbus_j1939_diagnostic_lamps_decode,
    encode = machbus_j1939_diagnostic_lamps_encode,
    err = "DiagnosticLamps decode failed"
);

/// `#[repr(C)]` mirror of [`machbus::j1939::DiagnosticProtocolId`] (DM5). The
/// `protocols` byte is the raw `DiagProtocol` bit field.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDiagnosticProtocolId {
    pub protocols: u8,
}

impl From<crate::j1939::DiagnosticProtocolId> for MachbusDiagnosticProtocolId {
    fn from(p: crate::j1939::DiagnosticProtocolId) -> Self {
        Self {
            protocols: p.protocols,
        }
    }
}

impl From<MachbusDiagnosticProtocolId> for crate::j1939::DiagnosticProtocolId {
    fn from(p: MachbusDiagnosticProtocolId) -> Self {
        Self {
            protocols: p.protocols,
        }
    }
}

pod_codec!(
    MachbusDiagnosticProtocolId,
    crate::j1939::DiagnosticProtocolId,
    8,
    decode = machbus_j1939_diagnostic_protocol_id_decode,
    encode = machbus_j1939_diagnostic_protocol_id_encode,
    err = "DiagnosticProtocolId decode failed"
);

// ── DmDtcList (lamps + Vec<Dtc>) — opaque owned handle ──

/// Owned, opaque decoded [`machbus::j1939::DmDtcList`] (also DM6/DM12/DM23).
pub struct MachbusDmDtcList(crate::j1939::DmDtcList);

/// Decode a DM1/DM2-style lamp+DTC list. Returns an owned handle (free with
/// [`machbus_dm_dtc_list_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm_dtc_list_decode(data: *const u8, len: usize) -> *mut MachbusDmDtcList {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::DmDtcList::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusDmDtcList(v)))
        }
        None => {
            set_last_error("DmDtcList decode failed");
            ptr::null_mut()
        }
    }
}

/// Copy the lamp block out of a DmDtcList handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm_dtc_list_lamps(
    h: *const MachbusDmDtcList,
    out: *mut MachbusDiagnosticLamps,
) -> bool {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return false;
    };
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    unsafe { *out = h.0.lamps.into() };
    clear_last_error();
    true
}

/// Number of DTCs in a DmDtcList handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm_dtc_list_count(h: *const MachbusDmDtcList) -> usize {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.dtcs.len(),
        None => 0,
    }
}

/// Copy the DTC at `idx` out of a DmDtcList handle. Returns false if `idx` is
/// out of range or pointers are null.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm_dtc_list_get(
    h: *const MachbusDmDtcList,
    idx: usize,
    out: *mut MachbusDtc,
) -> bool {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return false;
    };
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    match h.0.dtcs.get(idx) {
        Some(d) => {
            unsafe { *out = (*d).into() };
            clear_last_error();
            true
        }
        None => {
            set_last_error("DTC index out of range");
            false
        }
    }
}

/// Free a DmDtcList handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm_dtc_list_free(h: *mut MachbusDmDtcList) {
    if h.is_null() {
        return;
    }
    // SAFETY: pointer originated from Box::into_raw in the decode call.
    unsafe { drop(Box::from_raw(h)) };
}

// ── DmClearAllRequest (DM3/DM11) ──

/// Decode a DM3/DM11 clear-all request (the all-`0xFF` reserved payload).
/// Returns true on the canonical payload.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_dm_clear_all_request_decode(data: *const u8, len: usize) -> bool {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    if crate::j1939::DmClearAllRequest::decode(bytes).is_some() {
        clear_last_error();
        true
    } else {
        set_last_error("DmClearAllRequest decode failed");
        false
    }
}

/// Encode a DM3/DM11 clear-all request into the caller's 8-byte buffer.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_j1939_dm_clear_all_request_encode(out: *mut u8) -> bool {
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    let bytes = crate::j1939::DmClearAllRequest.encode();
    unsafe { core::ptr::copy_nonoverlapping(bytes.as_ptr(), out, 8) };
    clear_last_error();
    true
}

// ── Dm4Message (lamps split into 4 LampStatus + Vec<Dtc>) — opaque ──

/// Owned, opaque decoded [`machbus::j1939::Dm4Message`].
pub struct MachbusDm4Message(crate::j1939::Dm4Message);

/// `#[repr(C)]` lamp header of a DM4 message (each a raw `LampStatus` byte).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm4Lamps {
    pub mil_status: u8,
    pub red_stop_lamp: u8,
    pub amber_warning: u8,
    pub protect_lamp: u8,
}

/// Decode a DM4 message. Returns an owned handle (free with
/// [`machbus_dm4_message_free`]) or `NULL` on failure.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm4_message_decode(
    data: *const u8,
    len: usize,
) -> *mut MachbusDm4Message {
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return ptr::null_mut();
        }
    };
    match crate::j1939::Dm4Message::decode(bytes) {
        Some(v) => {
            clear_last_error();
            Box::into_raw(Box::new(MachbusDm4Message(v)))
        }
        None => {
            set_last_error("Dm4Message decode failed");
            ptr::null_mut()
        }
    }
}

/// Copy the DM4 lamp header out of a handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm4_message_lamps(
    h: *const MachbusDm4Message,
    out: *mut MachbusDm4Lamps,
) -> bool {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return false;
    };
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    unsafe {
        *out = MachbusDm4Lamps {
            mil_status: h.0.mil_status.as_u8(),
            red_stop_lamp: h.0.red_stop_lamp.as_u8(),
            amber_warning: h.0.amber_warning.as_u8(),
            protect_lamp: h.0.protect_lamp.as_u8(),
        };
    }
    clear_last_error();
    true
}

/// Number of DTCs in a DM4 message handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm4_message_count(h: *const MachbusDm4Message) -> usize {
    match unsafe { h.as_ref() } {
        Some(h) => h.0.dtcs.len(),
        None => 0,
    }
}

/// Copy the DTC at `idx` out of a DM4 message handle.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm4_message_get(
    h: *const MachbusDm4Message,
    idx: usize,
    out: *mut MachbusDtc,
) -> bool {
    let Some(h) = (unsafe { h.as_ref() }) else {
        set_last_error("null handle");
        return false;
    };
    if out.is_null() {
        set_last_error("null output pointer");
        return false;
    }
    match h.0.dtcs.get(idx) {
        Some(d) => {
            unsafe { *out = (*d).into() };
            clear_last_error();
            true
        }
        None => {
            set_last_error("DTC index out of range");
            false
        }
    }
}

/// Free a DM4 message handle. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_dm4_message_free(h: *mut MachbusDm4Message) {
    if h.is_null() {
        return;
    }
    // SAFETY: pointer originated from Box::into_raw in the decode call.
    unsafe { drop(Box::from_raw(h)) };
}

// ── Dm7Command (8-byte fixed) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm7Command`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm7Command {
    pub spn: u32,
    pub test_id: u8,
}

impl From<crate::j1939::Dm7Command> for MachbusDm7Command {
    fn from(d: crate::j1939::Dm7Command) -> Self {
        Self {
            spn: d.spn,
            test_id: d.test_id,
        }
    }
}

impl From<MachbusDm7Command> for crate::j1939::Dm7Command {
    fn from(d: MachbusDm7Command) -> Self {
        Self {
            spn: d.spn,
            test_id: d.test_id,
        }
    }
}

pod_codec!(
    MachbusDm7Command,
    crate::j1939::Dm7Command,
    8,
    decode = machbus_j1939_dm7_command_decode,
    encode = machbus_j1939_dm7_command_encode,
    err = "Dm7Command decode failed"
);

// ── Dm8TestResult (11-byte Vec encode) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm8TestResult`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm8TestResult {
    pub spn: u32,
    pub test_id: u8,
    pub test_result: u8,
    pub test_value: u16,
    pub test_limit_min: u16,
    pub test_limit_max: u16,
}

impl From<crate::j1939::Dm8TestResult> for MachbusDm8TestResult {
    fn from(d: crate::j1939::Dm8TestResult) -> Self {
        Self {
            spn: d.spn,
            test_id: d.test_id,
            test_result: d.test_result,
            test_value: d.test_value,
            test_limit_min: d.test_limit_min,
            test_limit_max: d.test_limit_max,
        }
    }
}

impl From<MachbusDm8TestResult> for crate::j1939::Dm8TestResult {
    fn from(d: MachbusDm8TestResult) -> Self {
        Self {
            spn: d.spn,
            test_id: d.test_id,
            test_result: d.test_result,
            test_value: d.test_value,
            test_limit_min: d.test_limit_min,
            test_limit_max: d.test_limit_max,
        }
    }
}

pod_codec!(
    MachbusDm8TestResult,
    crate::j1939::Dm8TestResult,
    11,
    decode = machbus_j1939_dm8_test_result_decode,
    encode = machbus_j1939_dm8_test_result_encode,
    err = "Dm8TestResult decode failed"
);

// ── Dm13Signals (8-byte fixed) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm13Signals`]. Network fields are
/// raw `Dm13Command` bytes (0=Suspend, 1=Resume, 3=DoNotCare); `suspend_signal`
/// is a raw `Dm13SuspendSignal` byte.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm13Signals {
    pub primary_vehicle_network: u8,
    pub sae_j1922_network: u8,
    pub sae_j1587_network: u8,
    pub current_data_link: u8,
    pub suspend_signal: u8,
    pub suspend_duration_s: u16,
}

impl From<crate::j1939::Dm13Signals> for MachbusDm13Signals {
    fn from(d: crate::j1939::Dm13Signals) -> Self {
        Self {
            primary_vehicle_network: d.primary_vehicle_network.as_u8(),
            sae_j1922_network: d.sae_j1922_network.as_u8(),
            sae_j1587_network: d.sae_j1587_network.as_u8(),
            current_data_link: d.current_data_link.as_u8(),
            suspend_signal: d.suspend_signal.as_u8(),
            suspend_duration_s: d.suspend_duration_s,
        }
    }
}

impl From<MachbusDm13Signals> for crate::j1939::Dm13Signals {
    fn from(d: MachbusDm13Signals) -> Self {
        use crate::j1939::diagnostic::{Dm13Command, Dm13SuspendSignal};
        Self {
            primary_vehicle_network: Dm13Command::from_u8(d.primary_vehicle_network),
            sae_j1922_network: Dm13Command::from_u8(d.sae_j1922_network),
            sae_j1587_network: Dm13Command::from_u8(d.sae_j1587_network),
            current_data_link: Dm13Command::from_u8(d.current_data_link),
            suspend_signal: Dm13SuspendSignal::from_u8(d.suspend_signal)
                .unwrap_or(Dm13SuspendSignal::NotAvailable),
            suspend_duration_s: d.suspend_duration_s,
        }
    }
}

pod_codec!(
    MachbusDm13Signals,
    crate::j1939::Dm13Signals,
    8,
    decode = machbus_j1939_dm13_signals_decode,
    encode = machbus_j1939_dm13_signals_encode,
    err = "Dm13Signals decode failed"
);

// ── Dm21Readiness (11-byte Vec encode) ──

/// `#[repr(C)]` mirror of [`machbus::j1939::Dm21Readiness`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusDm21Readiness {
    pub distance_with_mil_on_km: u16,
    pub distance_since_codes_cleared_km: u16,
    pub minutes_with_mil_on: u16,
    pub time_since_codes_cleared_min: u16,
    pub comprehensive_component: u8,
    pub fuel_system: u8,
    pub misfire: u8,
}

impl From<crate::j1939::Dm21Readiness> for MachbusDm21Readiness {
    fn from(d: crate::j1939::Dm21Readiness) -> Self {
        Self {
            distance_with_mil_on_km: d.distance_with_mil_on_km,
            distance_since_codes_cleared_km: d.distance_since_codes_cleared_km,
            minutes_with_mil_on: d.minutes_with_mil_on,
            time_since_codes_cleared_min: d.time_since_codes_cleared_min,
            comprehensive_component: d.comprehensive_component,
            fuel_system: d.fuel_system,
            misfire: d.misfire,
        }
    }
}

