use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};
use std::ptr;

use crate::isobus::fs::{FileClientConfig, FileServerConfig};
use crate::isobus::implement::tractor_commands::{HitchCommand, PtoCommand, ValveCommand};
use crate::isobus::tc::{DDOP, TCClientConfig, TCServerConfig};
use crate::isobus::tim::{TimAuthority, TimInterlocks, TimOptionSet};
use crate::isobus::vt::{ObjectID, ObjectPool, VTClientConfig, VTServerConfig, WorkingSet};
use crate::isobus::{Functionalities, GroupFunctionResponder};
use crate::j1939::diagnostic::{Dtc, Fmi};
use crate::j1939::{
    Eec1 as J1939Eec1, Etc1 as J1939Etc1, LanguageData, PowerRole, Request2Responder,
};
use crate::net::{
    Address, ClaimState as NetClaimState, Frame, Identifier, Message, NULL_ADDRESS, Name, Priority,
};
use crate::nmea::{GNSSPosition, NMEAConfig, NMEAInterface};
use crate::session::Session;
use crate::session::plugins::{
    Auxiliary, ControlFunctionalities, Diagnostics, DmMemory, FsClient, FsServer, Gnss,
    GroupFunction, Guidance, Heartbeat, Implement, LanguageCommand, MaintainPower, NameManagement,
    Powertrain, Request2, ScClient, ScMaster, ShortcutButton, TcClient, TcServer, Tim, VtClient,
    VtServer,
};
use crate::session::sys::{
    DiagEvent, Event, FsEvent, FsServerEvent, GnssEvent, GuidanceEvent, Hitch, ImplementEvent, Pto,
    TcEvent, TcServerEvent, VtEvent, VtServerEvent,
};
use crate::time::Instant;

/// Monotonic version for the public C ABI shape.
///
/// Bump this when exported function signatures, `repr(C)` POD layouts, enum
/// discriminants, or ownership contracts change in a way C callers must audit.
/// `3` marks the rewrite onto the `session` facade.
pub const MACHBUS_C_ABI_VERSION: u32 = 3;

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn clear_last_error() {
    LAST_ERROR.with(|slot| *slot.borrow_mut() = None);
}

fn set_last_error(message: impl Into<String>) {
    let message = message.into().replace('\0', " ");
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = Some(
            CString::new(message).unwrap_or_else(|_| CString::new("machbus ffi error").unwrap()),
        );
    });
}

fn bool_result<T, E: std::fmt::Display>(r: Result<T, E>) -> bool {
    match r {
        Ok(_) => {
            clear_last_error();
            true
        }
        Err(e) => {
            set_last_error(e.to_string());
            false
        }
    }
}

// ─── Public POD types ─────────────────────────────────────────────────

/// Node configuration. Build a default with [`machbus_session_default_config`]
/// and override fields before passing to [`machbus_session_new`].
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusConfig {
    /// Raw 64-bit ISO 11783-5 NAME (use `Name::raw` from Rust).
    pub name_raw: u64,
    /// Preferred address before claim arbitration.
    pub preferred_address: u8,
    /// Plug a [`Diagnostics`] subsystem (DM1 raise/clear + peer decode).
    pub enable_diagnostics: bool,
    /// DM1 broadcast cadence; 0 = use default (1000 ms).
    pub diagnostics_interval_ms: u32,
    /// Plug a [`Gnss`] subsystem (NMEA 2000 navigation, all messages).
    pub enable_gnss: bool,
    /// Plug an [`Implement`] subsystem (hitch/PTO/valve command + decode).
    pub enable_implement: bool,
    /// Plug a [`VtClient`] subsystem with an empty object pool / working set.
    pub enable_vt_client: bool,
    /// Plug a [`TcClient`] subsystem with an empty DDOP.
    pub enable_tc_client: bool,
    /// Plug an [`Auxiliary`] subsystem (AUX-O/AUX-N function decode/broadcast).
    pub enable_auxiliary: bool,
    /// Plug a [`DmMemory`] subsystem (DM14/DM15/DM16 memory access + ECU id).
    pub enable_dm_memory: bool,
    /// Plug an [`FsClient`] subsystem (ISO 11783-13 file-server client).
    pub enable_fs_client: bool,
    /// Plug an [`FsServer`] subsystem (ISO 11783-13 file server).
    pub enable_fs_server: bool,
    /// Plug a [`ControlFunctionalities`] subsystem (ISO 11783-12 reporting).
    pub enable_control_functionalities: bool,
    /// Plug a [`GroupFunction`] subsystem (ISO group function responder).
    pub enable_group_function: bool,
    /// Plug a [`Heartbeat`] subsystem; `heartbeat_interval_ms` 0 = 100 ms.
    pub enable_heartbeat: bool,
    /// Heartbeat broadcast cadence; 0 = use default (100 ms).
    pub heartbeat_interval_ms: u32,
    /// Plug a [`LanguageCommand`] subsystem (ISO language/units command).
    pub enable_language_command: bool,
    /// Plug a [`MaintainPower`] subsystem; `maintain_power_role_tecu` selects role.
    pub enable_maintain_power: bool,
    /// MaintainPower role: `true` = TECU server, `false` = CF/client.
    pub maintain_power_role_tecu: bool,
    /// Plug a [`NameManagement`] subsystem (commanded-address / NAME mgmt).
    pub enable_name_management: bool,
    /// Plug a [`Powertrain`] subsystem (EEC1/ETC1/VIN broadcast + snapshot).
    pub enable_powertrain: bool,
    /// Plug a [`Request2`] subsystem (PGN request2 responder).
    pub enable_request2: bool,
    /// Plug a [`ScClient`] subsystem (sequence-control client).
    pub enable_sc_client: bool,
    /// Plug a [`ScMaster`] subsystem (sequence-control master).
    pub enable_sc_master: bool,
    /// Plug a [`ShortcutButton`] subsystem (ISO stop-all shortcut button).
    pub enable_shortcut_button: bool,
    /// Plug a [`TcServer`] subsystem (task-controller server, default config).
    pub enable_tc_server: bool,
    /// Plug a [`VtServer`] subsystem (virtual terminal server, default config).
    pub enable_vt_server: bool,
    /// Plug a [`Tim`] subsystem (tractor-implement management, no options).
    pub enable_tim: bool,
    /// Plug a [`Guidance`] subsystem (ISO 11783-7 curvature-based autosteer).
    pub enable_guidance: bool,
}

impl Default for MachbusConfig {
    fn default() -> Self {
        Self {
            name_raw: 0,
            preferred_address: 0x80,
            enable_diagnostics: false,
            diagnostics_interval_ms: 0,
            enable_gnss: false,
            enable_implement: false,
            enable_vt_client: false,
            enable_tc_client: false,
            enable_auxiliary: false,
            enable_dm_memory: false,
            enable_fs_client: false,
            enable_fs_server: false,
            enable_control_functionalities: false,
            enable_group_function: false,
            enable_heartbeat: false,
            heartbeat_interval_ms: 0,
            enable_language_command: false,
            enable_maintain_power: false,
            maintain_power_role_tecu: false,
            enable_name_management: false,
            enable_powertrain: false,
            enable_request2: false,
            enable_sc_client: false,
            enable_sc_master: false,
            enable_shortcut_button: false,
            enable_tc_server: false,
            enable_vt_server: false,
            enable_tim: false,
            enable_guidance: false,
        }
    }
}

/// Result of [`machbus_session_validate_can_bus_config`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MachbusCanBusValidation {
    pub bitrate_ok: bool,
    pub sample_point_ok: bool,
    pub bit_timing_ok: bool,
    pub physical_mode_ok: bool,
    pub overall_ok: bool,
}

#[allow(clippy::too_many_arguments)]
fn can_bus_config_from_abi(
    bitrate: u32,
    sample_point: f64,
    sjw: u8,
    prop_seg: u8,
    phase_seg1: u8,
    phase_seg2: u8,
    silent_mode: bool,
    loopback: bool,
) -> crate::net::CanBusConfig {
    crate::net::CanBusConfig {
        bitrate,
        sample_point,
        sjw,
        prop_seg,
        phase_seg1,
        phase_seg2,
        silent_mode,
        loopback,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusClaimState {
    None = 0,
    WaitForClaim = 1,
    SendRequest = 2,
    WaitForContest = 3,
    SendClaim = 4,
    Claimed = 5,
    Failed = 6,
}

impl From<NetClaimState> for MachbusClaimState {
    fn from(s: NetClaimState) -> Self {
        match s {
            NetClaimState::None => Self::None,
            NetClaimState::WaitForClaim => Self::WaitForClaim,
            NetClaimState::SendRequest => Self::SendRequest,
            NetClaimState::WaitForContest => Self::WaitForContest,
            NetClaimState::SendClaim => Self::SendClaim,
            NetClaimState::Claimed => Self::Claimed,
            NetClaimState::Failed => Self::Failed,
        }
    }
}

/// Discriminant for [`MachbusEvent::kind`]. Mirrors the unified
/// [`Event`] surface. Subsystem events that have no
/// stable C payload yet collapse to [`MachbusEventKind::Other`].
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusEventKind {
    None = 0,
    AddressClaimClaimed = 1,
    AddressClaimLost = 2,
    AddressClaimDisconnected = 3,
    BusError = 4,
    BusDroppedFrame = 5,
    DiagRaised = 6,
    DiagCleared = 7,
    DiagDm1Received = 8,
    GnssPosition = 9,
    GnssCog = 10,
    GnssSog = 11,
    GnssHeading = 12,
    ImpHitchCommand = 13,
    ImpPtoCommand = 14,
    ImpAuxValveCommand = 15,
    Custom = 16,
    GnssMagneticVariation = 17,
    GnssAttitude = 18,
    GnssDops = 19,
    GnssSystemTime = 20,
    VtStateChanged = 21,
    VtSoftKey = 22,
    VtButton = 23,
    VtNumericValueChanged = 24,
    VtStringValueChanged = 25,
    VtPoolError = 26,
    VtLanguageChanged = 27,
    VtActiveWorkingSet = 28,
    TcStateChanged = 29,
    FsConnected = 30,
    FsDisconnected = 31,
    FsOpenResponse = 32,
    FsCloseResponse = 33,
    FsReadResponse = 34,
    FsWriteResponse = 35,
    FsSeekResponse = 36,
    FsCurrentDirectoryResponse = 37,
    FsChangeDirectoryResponse = 38,
    FsError = 39,
    VtServerStateChanged = 40,
    VtServerClientConnected = 41,
    VtServerClientDisconnected = 42,
    VtServerActiveWorkingSetChanged = 43,
    VtServerSoftKey = 44,
    VtServerButton = 45,
    VtServerNumericValueChanged = 46,
    VtServerStringValueChanged = 47,
    VtServerInputObjectSelected = 48,
    FsServerClientConnected = 49,
    FsServerClientDisconnected = 50,
    FsServerFileOpened = 51,
    FsServerFileClosed = 52,
    TcServerStateChanged = 53,
    TcServerClientVersionReceived = 54,
    VtAuxCapabilities = 76,
    Powertrain = 89,
    TcServerPeerControlAssignment = 75,
    GuidanceMachineInfo = 90,
    Other = 99,
}

/// A flattened, C-friendly view of one [`Event`].
///
/// The payload fields are interpreted according to [`MachbusEvent::kind`]; see
/// `classify_event` for the field meaning of each kind.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MachbusEvent {
    pub kind: MachbusEventKind,
    /// Source address (for inbound events) or own address.
    pub source: u8,
    /// SPN for diag events, PGN for Custom, object id for VT events, or 0.
    pub spn_or_pgn: u32,
    /// FMI byte for diag events; reused as raw subcommand/status byte for
    /// subsystem events.
    pub fmi_or_sub: u8,
    /// First f64 payload (latitude, COG, ramp speed…). NaN if unused.
    pub d0: f64,
    /// Second f64 payload (longitude, etc.). NaN if unused.
    pub d1: f64,
    /// Auxiliary u32 (numeric value, target speed RPM, enum code, length, etc.).
    pub u0: u32,
}

impl MachbusEvent {
    fn empty(kind: MachbusEventKind) -> Self {
        Self {
            kind,
            source: 0,
            spn_or_pgn: 0,
            fmi_or_sub: 0,
            d0: f64::NAN,
            d1: f64::NAN,
            u0: 0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachbusGnssPosition {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude_m: f64,
    pub speed_mps: f64,
    pub heading_rad: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusHitch {
    Front = 0,
    Rear = 1,
}

impl From<MachbusHitch> for Hitch {
    fn from(h: MachbusHitch) -> Self {
        match h {
            MachbusHitch::Front => Hitch::Front,
            MachbusHitch::Rear => Hitch::Rear,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusPto {
    Front = 0,
    Rear = 1,
}

impl From<MachbusPto> for Pto {
    fn from(p: MachbusPto) -> Self {
        match p {
            MachbusPto::Front => Pto::Front,
            MachbusPto::Rear => Pto::Rear,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusHitchCommand {
    NoAction = 0,
    Lower = 1,
    Raise = 2,
    Position = 3,
}

impl From<MachbusHitchCommand> for HitchCommand {
    fn from(c: MachbusHitchCommand) -> Self {
        match c {
            MachbusHitchCommand::NoAction => HitchCommand::NoAction,
            MachbusHitchCommand::Lower => HitchCommand::Lower,
            MachbusHitchCommand::Raise => HitchCommand::Raise,
            MachbusHitchCommand::Position => HitchCommand::Position,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusPtoCommand {
    NoAction = 0,
    Engage = 1,
    Disengage = 2,
    SetSpeed = 3,
}

impl From<MachbusPtoCommand> for PtoCommand {
    fn from(c: MachbusPtoCommand) -> Self {
        match c {
            MachbusPtoCommand::NoAction => PtoCommand::NoAction,
            MachbusPtoCommand::Engage => PtoCommand::Engage,
            MachbusPtoCommand::Disengage => PtoCommand::Disengage,
            MachbusPtoCommand::SetSpeed => PtoCommand::SetSpeed,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MachbusValveCommand {
    NoAction = 0,
    Extend = 1,
    Retract = 2,
    Float = 3,
    Block = 4,
}

impl From<MachbusValveCommand> for ValveCommand {
    fn from(c: MachbusValveCommand) -> Self {
        match c {
            MachbusValveCommand::NoAction => ValveCommand::NoAction,
            MachbusValveCommand::Extend => ValveCommand::Extend,
            MachbusValveCommand::Retract => ValveCommand::Retract,
            MachbusValveCommand::Float => ValveCommand::Float,
            MachbusValveCommand::Block => ValveCommand::Block,
        }
    }
}

// ─── Opaque handle ────────────────────────────────────────────────────

/// Owned opaque node handle wrapping a sans-IO [`Session`].
///
/// Created by [`machbus_session_new`] and released exactly once by
/// [`machbus_session_free`]. The free function accepts `NULL`; passing the same
/// non-`NULL` pointer to `machbus_session_free` more than once is invalid. Set
/// caller variables to `NULL` after freeing to avoid double-free bugs.
pub struct MachbusSession {
    session: Session,
    /// Monotonic virtual clock advanced by [`machbus_session_tick`].
    now: Instant,
}

fn build_session(cfg: MachbusConfig) -> Result<MachbusSession, crate::net::error::Error> {
    build_session_with_content(cfg, None, None)
}

/// Build a session, optionally threading prebuilt VT pool / working set and TC
/// DDOP content into the VT/TC client plugins instead of empty defaults.
///
/// When `vt_content` is `Some`, a VT client carrying it is plugged regardless
/// of `cfg.enable_vt_client`; otherwise `cfg.enable_vt_client` plugs an empty
/// VT client. The same rule applies to `ddop` / `cfg.enable_tc_client`.
fn build_session_with_content(
    cfg: MachbusConfig,
    vt_content: Option<(ObjectPool, WorkingSet)>,
    ddop: Option<DDOP>,
) -> Result<MachbusSession, crate::net::error::Error> {
    let name = Name::from_raw(cfg.name_raw);
    let mut builder = Session::builder(name, cfg.preferred_address);

    if cfg.enable_diagnostics {
        let interval = if cfg.diagnostics_interval_ms == 0 {
            1000
        } else {
            cfg.diagnostics_interval_ms
        };
        builder = builder.plug(Diagnostics::every(interval));
    }
    if cfg.enable_gnss {
        builder = builder.plug(Gnss::new(NMEAConfig::default().with_all(true)));
    }
    if cfg.enable_implement {
        builder = builder.plug(Implement::new());
    }
    if let Some((pool, ws)) = vt_content {
        builder = builder.plug(VtClient::new(VTClientConfig::default(), pool, ws));
    } else if cfg.enable_vt_client {
        builder = builder.plug(VtClient::new(
            VTClientConfig::default(),
            ObjectPool::default(),
            WorkingSet::default(),
        ));
    }
    if let Some(ddop) = ddop {
        builder = builder.plug(TcClient::new(TCClientConfig::default(), ddop));
    } else if cfg.enable_tc_client {
        builder = builder.plug(TcClient::new(TCClientConfig::default(), DDOP::default()));
    }
    if cfg.enable_auxiliary {
        builder = builder.plug(Auxiliary::new());
    }
    if cfg.enable_dm_memory {
        builder = builder.plug(DmMemory::new(None));
    }
    if cfg.enable_fs_client {
        builder = builder.plug(FsClient::new(FileClientConfig::default()));
    }
    if cfg.enable_fs_server {
        builder = builder.plug(FsServer::new(FileServerConfig::default()));
    }
    if cfg.enable_control_functionalities {
        builder = builder.plug(ControlFunctionalities::new(Functionalities::new()));
    }
    if cfg.enable_group_function {
        builder = builder.plug(GroupFunction::new(GroupFunctionResponder::new()));
    }
    if cfg.enable_heartbeat {
        let interval = if cfg.heartbeat_interval_ms == 0 {
            100
        } else {
            cfg.heartbeat_interval_ms
        };
        builder = builder.plug(Heartbeat::every(interval));
    }
    if cfg.enable_language_command {
        builder = builder.plug(LanguageCommand::new(LanguageData::default()));
    }
    if cfg.enable_maintain_power {
        let role = if cfg.maintain_power_role_tecu {
            PowerRole::Tecu
        } else {
            PowerRole::Cf
        };
        builder = builder.plug(MaintainPower::new(role));
    }
    if cfg.enable_name_management {
        builder = builder.plug(NameManagement::new());
    }
    if cfg.enable_powertrain {
        builder = builder.plug(Powertrain::new());
    }
    if cfg.enable_request2 {
        builder = builder.plug(Request2::new(Request2Responder::new()));
    }
    if cfg.enable_sc_client {
        builder = builder.plug(ScClient::new(crate::isobus::SCClientConfig::default()));
    }
    if cfg.enable_sc_master {
        builder = builder.plug(ScMaster::new(crate::isobus::SCMasterConfig::default()));
    }
    if cfg.enable_shortcut_button {
        builder = builder.plug(ShortcutButton::new());
    }
    if cfg.enable_tc_server {
        builder = builder.plug(TcServer::new(TCServerConfig::default())?);
    }
    if cfg.enable_vt_server {
        builder = builder.plug(VtServer::new(VTServerConfig::default())?);
    }
    if cfg.enable_tim {
        builder = builder.plug(Tim::new(TimAuthority::new(TimOptionSet::empty())));
    }
    if cfg.enable_guidance {
        builder = builder.plug(Guidance::new());
    }

    let session = builder.build()?;
    Ok(MachbusSession {
        session,
        now: Instant::ZERO,
    })
}

fn handle_mut<'a>(p: *mut MachbusSession) -> Result<&'a mut MachbusSession, &'static str> {
    if p.is_null() {
        return Err("null machbus session handle");
    }
    // SAFETY: validated non-null; caller owns the box.
    Ok(unsafe { &mut *p })
}

fn handle_ref<'a>(p: *const MachbusSession) -> Result<&'a MachbusSession, &'static str> {
    if p.is_null() {
        return Err("null machbus session handle");
    }
    // SAFETY: validated non-null; caller owns the box.
    Ok(unsafe { &*p })
}

/// Borrow a plugged subsystem mutably or set the last error and return.
macro_rules! plugin_mut {
    ($h:expr, $ty:ty) => {{
        match $h.session.get_mut::<$ty>() {
            Some(p) => p,
            None => {
                set_last_error(concat!(stringify!($ty), " subsystem not plugged"));
                return false;
            }
        }
    }};
}

fn read_c_str(p: *const c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    // SAFETY: caller provides a valid NUL-terminated C string or null.
    unsafe { CStr::from_ptr(p) }
        .to_str()
        .ok()
        .map(|s| s.to_string())
}

fn read_bytes<'a>(p: *const u8, len: usize) -> Result<&'a [u8], &'static str> {
    if len == 0 {
        return Ok(&[]);
    }
    if p.is_null() {
        return Err("null data pointer");
    }
    // SAFETY: caller supplied a non-null pointer valid for `len` bytes.
    Ok(unsafe { std::slice::from_raw_parts(p, len) })
}

// ─── Last-error accessor ──────────────────────────────────────────────

/// The last error message set by a failing call on this thread, or `NULL`.
/// The returned pointer is valid until the next ABI call on the same thread.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_last_error() -> *const c_char {
    LAST_ERROR.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|m| m.as_ptr())
            .unwrap_or(ptr::null())
    })
}

/// Current C ABI version. See [`MACHBUS_C_ABI_VERSION`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_abi_version() -> u32 {
    MACHBUS_C_ABI_VERSION
}

// ─── CAN-config validation helpers ────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_validate_can_bus_config(
    bitrate: u32,
    sample_point: f64,
    sjw: u8,
    prop_seg: u8,
    phase_seg1: u8,
    phase_seg2: u8,
    silent_mode: bool,
    loopback: bool,
) -> MachbusCanBusValidation {
    let config = can_bus_config_from_abi(
        bitrate,
        sample_point,
        sjw,
        prop_seg,
        phase_seg1,
        phase_seg2,
        silent_mode,
        loopback,
    );
    let validation = crate::net::validate_can_bus_config(&config);
    MachbusCanBusValidation {
        bitrate_ok: validation.bitrate_ok,
        sample_point_ok: validation.sample_point_ok,
        bit_timing_ok: validation.bit_timing_ok,
        physical_mode_ok: validation.physical_mode_ok,
        overall_ok: validation.overall_ok,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_enforce_iso_can_config(
    bitrate: u32,
    sample_point: f64,
    sjw: u8,
    prop_seg: u8,
    phase_seg1: u8,
    phase_seg2: u8,
    silent_mode: bool,
    loopback: bool,
) -> bool {
    let config = can_bus_config_from_abi(
        bitrate,
        sample_point,
        sjw,
        prop_seg,
        phase_seg1,
        phase_seg2,
        silent_mode,
        loopback,
    );
    bool_result(crate::net::enforce_iso_can_config(&config))
}

// ─── Lifecycle ────────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_default_config() -> MachbusConfig {
    MachbusConfig::default()
}

/// Create a node from `cfg` (or defaults if `cfg` is `NULL`). Returns `NULL` on
/// failure; inspect [`machbus_session_last_error`]. Free with
/// [`machbus_session_free`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_new(cfg: *const MachbusConfig) -> *mut MachbusSession {
    let cfg = if cfg.is_null() {
        MachbusConfig::default()
    } else {
        // SAFETY: non-null per the check.
        unsafe { *cfg }
    };
    match build_session(cfg) {
        Ok(h) => {
            clear_last_error();
            Box::into_raw(Box::new(h))
        }
        Err(e) => {
            set_last_error(e.to_string());
            ptr::null_mut()
        }
    }
}

/// Free a node. Accepts `NULL`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_free(h: *mut MachbusSession) {
    if h.is_null() {
        return;
    }
    // SAFETY: pointer originated from Box::into_raw in machbus_session_new.
    unsafe { drop(Box::from_raw(h)) };
}

/// Begin address claiming. Drive it forward with [`machbus_session_tick`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_start_address_claim(h: *mut MachbusSession) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    bool_result(h.session.start())
}

/// Advance the virtual clock by `dt_ms` and run cadences/timers. Outbound
/// frames produced are then available via [`machbus_session_poll_transmit`].
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_tick(h: *mut MachbusSession, dt_ms: u32) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    h.now = h.now.add_millis(u64::from(dt_ms));
    h.session.tick(h.now);
    clear_last_error();
    true
}

// ─── IO bridge ────────────────────────────────────────────────────────

/// Feed one received CAN frame (extended 29-bit `raw_id`, up to 8 data bytes).
/// `port` identifies the receiving bus.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_feed(
    h: *mut MachbusSession,
    port: u8,
    raw_id: u32,
    data: *const u8,
    len: usize,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    if len > 8 {
        set_last_error("CAN frame length exceeds 8 bytes");
        return false;
    }
    let mut buf = [0xFFu8; 8];
    buf[..len].copy_from_slice(bytes);
    let frame = Frame::new(Identifier::from_raw(raw_id), buf, len as u8);
    h.session.feed(port, &frame, h.now);
    clear_last_error();
    true
}

/// Drain the next outbound frame. On success writes `out_port`, `out_raw_id`,
/// up to 8 bytes into `out_data`, and the byte count into `out_len`, then
/// returns `true`. Returns `false` (without setting an error) when the transmit
/// queue is drained. Call repeatedly until it returns `false`.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_poll_transmit(
    h: *mut MachbusSession,
    out_port: *mut u8,
    out_raw_id: *mut u32,
    out_data: *mut u8,
    out_len: *mut usize,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let Some((port, frame)) = h.session.poll_transmit() else {
        clear_last_error();
        return false;
    };
    let payload = frame.payload();
    if !out_port.is_null() {
        // SAFETY: caller-provided writable pointer.
        unsafe { *out_port = port };
    }
    if !out_raw_id.is_null() {
        // SAFETY: caller-provided writable pointer.
        unsafe { *out_raw_id = frame.id.raw };
    }
    if !out_data.is_null() {
        // SAFETY: out_data points to a buffer of at least 8 bytes per contract.
        let dst = unsafe { std::slice::from_raw_parts_mut(out_data, 8) };
        dst[..payload.len()].copy_from_slice(payload);
    }
    if !out_len.is_null() {
        // SAFETY: caller-provided writable pointer.
        unsafe { *out_len = payload.len() };
    }
    clear_last_error();
    true
}

/// Raw escape hatch: queue an application message from the local control
/// function. `priority` is the 3-bit J1939 priority (0 = highest, 6 = default).
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_send_raw(
    h: *mut MachbusSession,
    pgn: u32,
    data: *const u8,
    len: usize,
    dst: u8,
    priority: u8,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let bytes = match read_bytes(data, len) {
        Ok(b) => b,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let prio = Priority::from_u8(priority);
    bool_result(h.session.send_raw(pgn, bytes, dst as Address, prio))
}

// ─── Introspection ────────────────────────────────────────────────────

#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_address(h: *const MachbusSession) -> u8 {
    handle_ref(h)
        .map(|h| h.session.address())
        .unwrap_or(NULL_ADDRESS)
}

#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_claim_state(h: *const MachbusSession) -> MachbusClaimState {
    handle_ref(h)
        .map(|h| h.session.claim_state().into())
        .unwrap_or(MachbusClaimState::None)
}

#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_is_claimed(h: *const MachbusSession) -> bool {
    handle_ref(h)
        .map(|h| h.session.is_claimed())
        .unwrap_or(false)
}

// ─── Event polling ────────────────────────────────────────────────────

fn vt_state_code(state: crate::isobus::vt::VTState) -> u32 {
    match state {
        crate::isobus::vt::VTState::Disconnected => 0,
        crate::isobus::vt::VTState::WaitForVTStatus => 1,
        crate::isobus::vt::VTState::SendWorkingSetMaster => 2,
        crate::isobus::vt::VTState::SendGetMemory => 3,
        crate::isobus::vt::VTState::WaitForMemory => 4,
        crate::isobus::vt::VTState::UploadPool => 5,
        crate::isobus::vt::VTState::WaitForPoolStore => 6,
        crate::isobus::vt::VTState::WaitForEndOfPool => 7,
        crate::isobus::vt::VTState::ReloadPool => 8,
        crate::isobus::vt::VTState::Connected => 9,
    }
}

fn vt_server_state_code(state: crate::isobus::vt::VTServerState) -> u32 {
    match state {
        crate::isobus::vt::VTServerState::Disconnected => 0,
        crate::isobus::vt::VTServerState::WaitForClientStatus => 1,
        crate::isobus::vt::VTServerState::SendWorkingSetMaster => 2,
        crate::isobus::vt::VTServerState::WaitForPoolUpload => 3,
        crate::isobus::vt::VTServerState::Connected => 4,
    }
}

fn tc_state_code(state: crate::isobus::tc::TCState) -> u32 {
    match state {
        crate::isobus::tc::TCState::Disconnected => 0,
        crate::isobus::tc::TCState::WaitForStartup => 1,
        crate::isobus::tc::TCState::WaitForServerStatus => 2,
        crate::isobus::tc::TCState::SendWorkingSetMaster => 3,
        crate::isobus::tc::TCState::RequestVersion => 4,
        crate::isobus::tc::TCState::WaitForVersion => 5,
        crate::isobus::tc::TCState::ProcessDDOP => 6,
        crate::isobus::tc::TCState::TransferDDOP => 7,
        crate::isobus::tc::TCState::WaitForPoolResponse => 8,
        crate::isobus::tc::TCState::ActivatePool => 9,
        crate::isobus::tc::TCState::WaitForActivation => 10,
        crate::isobus::tc::TCState::Connected => 11,
        crate::isobus::tc::TCState::DeactivatePool => 12,
        crate::isobus::tc::TCState::WaitForDeactivation => 13,
        crate::isobus::tc::TCState::DeletePool => 14,
        crate::isobus::tc::TCState::WaitForDeletePool => 15,
        crate::isobus::tc::TCState::RequestStructureLabel => 16,
        crate::isobus::tc::TCState::WaitForStructureLabel => 17,
        crate::isobus::tc::TCState::RequestLocalizationLabel => 18,
        crate::isobus::tc::TCState::WaitForLocalizationLabel => 19,
    }
}

fn language_code_u16(lang: crate::isobus::vt::LanguageCode) -> u16 {
    u16::from_le_bytes(lang.code)
}

fn classify_fs_result<T>(
    result: std::result::Result<T, crate::isobus::fs::FSError>,
    out: &mut MachbusEvent,
    ok: impl FnOnce(T) -> u32,
) {
    match result {
        Ok(value) => {
            out.u0 = ok(value);
        }
        Err(error) => {
            out.source = error.as_u8();
        }
    }
}

/// Flatten one [`Event`] into an [`MachbusEvent`]. Subsystem events that have no
/// stable C payload yet collapse to [`MachbusEventKind::Other`].
fn classify_event(ev: Event, out: &mut MachbusEvent) {
    *out = MachbusEvent::empty(MachbusEventKind::Other);
    match ev {
        Event::AddressClaim(c) => match c {
            crate::session::sys::ClaimEvent::Claimed { address } => {
                out.kind = MachbusEventKind::AddressClaimClaimed;
                out.source = address;
            }
            crate::session::sys::ClaimEvent::Lost { previous_address } => {
                out.kind = MachbusEventKind::AddressClaimLost;
                out.source = previous_address;
            }
            crate::session::sys::ClaimEvent::Disconnected => {
                out.kind = MachbusEventKind::AddressClaimDisconnected;
            }
        },
        Event::Bus(b) => match b {
            crate::session::sys::BusEvent::Error { port } => {
                out.kind = MachbusEventKind::BusError;
                out.source = port;
            }
            crate::session::sys::BusEvent::DroppedFrame { port } => {
                out.kind = MachbusEventKind::BusDroppedFrame;
                out.source = port;
            }
            crate::session::sys::BusEvent::ConfinementChanged { port, .. } => {
                out.kind = MachbusEventKind::BusError;
                out.source = port;
            }
        },
        Event::Diag(d) => match d {
            DiagEvent::Raised(dtc) => {
                out.kind = MachbusEventKind::DiagRaised;
                out.spn_or_pgn = dtc.spn;
                out.fmi_or_sub = dtc.fmi.as_u8();
            }
            DiagEvent::Cleared(dtc) => {
                out.kind = MachbusEventKind::DiagCleared;
                out.spn_or_pgn = dtc.spn;
                out.fmi_or_sub = dtc.fmi.as_u8();
            }
            DiagEvent::Dm1Received { source, active, .. } => {
                out.kind = MachbusEventKind::DiagDm1Received;
                out.source = source;
                out.u0 = active.len() as u32;
            }
            // The remaining diagnostic-memory events have no stable C payload
            // yet; expose them through the Rust event API.
            _ => {}
        },
        Event::Gnss(g) => match g {
            GnssEvent::Position(p) => {
                out.kind = MachbusEventKind::GnssPosition;
                out.d0 = p.wgs.latitude;
                out.d1 = p.wgs.longitude;
            }
            GnssEvent::Cog(c) => {
                out.kind = MachbusEventKind::GnssCog;
                out.d0 = c;
            }
            GnssEvent::Sog(s) => {
                out.kind = MachbusEventKind::GnssSog;
                out.d0 = s;
            }
            GnssEvent::Heading(h) => {
                out.kind = MachbusEventKind::GnssHeading;
                out.d0 = h;
            }
            GnssEvent::MagneticVariation(v) => {
                out.kind = MachbusEventKind::GnssMagneticVariation;
                out.d0 = v;
            }
            GnssEvent::Attitude {
                yaw,
                pitch,
                roll: _,
            } => {
                out.kind = MachbusEventKind::GnssAttitude;
                out.d0 = yaw;
                out.d1 = pitch;
            }
            GnssEvent::Dops(d) => {
                out.kind = MachbusEventKind::GnssDops;
                out.source = d.sid;
                out.fmi_or_sub = d.actual_mode.as_u8();
                out.d0 = d.hdop;
                out.d1 = d.vdop;
                out.u0 = (d.tdop * 1000.0) as u32;
            }
            GnssEvent::SystemTime(t) => {
                out.kind = MachbusEventKind::GnssSystemTime;
                out.source = t.sid;
                out.fmi_or_sub = t.source.as_u8();
                out.spn_or_pgn = t.days_since_epoch as u32;
                out.d0 = t.seconds_since_midnight;
            }
        },
        Event::Vt(v) => match v {
            VtEvent::StateChanged(state) => {
                out.kind = MachbusEventKind::VtStateChanged;
                out.u0 = vt_state_code(state);
            }
            VtEvent::SoftKey { id, code } => {
                out.kind = MachbusEventKind::VtSoftKey;
                out.spn_or_pgn = id.0 as u32;
                out.fmi_or_sub = code.as_u8();
            }
            VtEvent::Button { id, code } => {
                out.kind = MachbusEventKind::VtButton;
                out.spn_or_pgn = id.0 as u32;
                out.fmi_or_sub = code.as_u8();
            }
            VtEvent::NumericValueChanged { id, value } => {
                out.kind = MachbusEventKind::VtNumericValueChanged;
                out.spn_or_pgn = id.0 as u32;
                out.u0 = value;
            }
            VtEvent::StringValueChanged { id, value } => {
                out.kind = MachbusEventKind::VtStringValueChanged;
                out.spn_or_pgn = id.0 as u32;
                out.u0 = value.len() as u32;
            }
            VtEvent::PoolError(code) => {
                out.kind = MachbusEventKind::VtPoolError;
                out.fmi_or_sub = code;
            }
            VtEvent::LanguageChanged { from, to } => {
                out.kind = MachbusEventKind::VtLanguageChanged;
                out.spn_or_pgn = language_code_u16(from) as u32;
                out.u0 = language_code_u16(to) as u32;
            }
            VtEvent::ActiveWorkingSet(active) => {
                out.kind = MachbusEventKind::VtActiveWorkingSet;
                out.u0 = u32::from(active);
            }
            VtEvent::AuxCapabilities {
                source,
                capabilities,
            } => {
                out.kind = MachbusEventKind::VtAuxCapabilities;
                out.source = source;
                out.fmi_or_sub = capabilities.vt_version;
                out.u0 = capabilities.channels.len() as u32;
                if let Some(first) = capabilities.channels.first() {
                    out.spn_or_pgn = u32::from(first.channel_id) | (u32::from(first.aux_type) << 8);
                }
            }
        },
        Event::Tc(t) => match t {
            TcEvent::StateChanged(state) => {
                out.kind = MachbusEventKind::TcStateChanged;
                out.u0 = tc_state_code(state);
            }
        },
        Event::Fs(f) => match f {
            FsEvent::Connected => {
                out.kind = MachbusEventKind::FsConnected;
            }
            FsEvent::Disconnected => {
                out.kind = MachbusEventKind::FsDisconnected;
            }
            FsEvent::OpenResponse { tan, result } => {
                out.kind = MachbusEventKind::FsOpenResponse;
                out.fmi_or_sub = tan;
                classify_fs_result(result, out, u32::from);
            }
            FsEvent::CloseResponse { tan, result } => {
                out.kind = MachbusEventKind::FsCloseResponse;
                out.fmi_or_sub = tan;
                classify_fs_result(result, out, u32::from);
            }
            FsEvent::ReadResponse { tan, result } => {
                out.kind = MachbusEventKind::FsReadResponse;
                out.fmi_or_sub = tan;
                classify_fs_result(result, out, |payload| payload.len() as u32);
            }
            FsEvent::WriteResponse { tan, result } => {
                out.kind = MachbusEventKind::FsWriteResponse;
                out.fmi_or_sub = tan;
                classify_fs_result(result, out, u32::from);
            }
            FsEvent::SeekResponse { tan, result } => {
                out.kind = MachbusEventKind::FsSeekResponse;
                out.fmi_or_sub = tan;
                classify_fs_result(result, out, |()| 0);
            }
            FsEvent::CurrentDirectoryResponse { tan, result } => {
                out.kind = MachbusEventKind::FsCurrentDirectoryResponse;
                out.fmi_or_sub = tan;
                classify_fs_result(result, out, |path| path.len() as u32);
            }
            FsEvent::ChangeDirectoryResponse { tan, result } => {
                out.kind = MachbusEventKind::FsChangeDirectoryResponse;
                out.fmi_or_sub = tan;
                classify_fs_result(result, out, |path| path.len() as u32);
            }
            FsEvent::Error(error) => {
                out.kind = MachbusEventKind::FsError;
                out.source = error.as_u8();
            }
            // Remaining FS responses (properties/status/move/delete/attrs/
            // volume/date-time) have no stable C payload yet.
            _ => {}
        },
        Event::Imp(i) => match i {
            ImplementEvent::HitchCommand { hitch, msg } => {
                out.kind = MachbusEventKind::ImpHitchCommand;
                out.source = match hitch {
                    Hitch::Front => 0,
                    Hitch::Rear => 1,
                };
                out.fmi_or_sub = msg.command.as_u8();
                out.u0 = msg.target_position as u32;
            }
            ImplementEvent::PtoCommand { pto, msg } => {
                out.kind = MachbusEventKind::ImpPtoCommand;
                out.source = match pto {
                    Pto::Front => 0,
                    Pto::Rear => 1,
                };
                out.fmi_or_sub = msg.command.as_u8();
                out.u0 = msg.target_speed_rpm as u32;
            }
            ImplementEvent::AuxValveCommand(m) => {
                out.kind = MachbusEventKind::ImpAuxValveCommand;
                out.source = m.valve_index;
                out.fmi_or_sub = m.command.as_u8();
                out.u0 = m.flow_rate as u32;
            }
            // Status-side implement events stay Other until the C ABI grows
            // stable float payload fields.
            _ => {}
        },
        Event::VtServer(v) => match v {
            VtServerEvent::StateChanged(state) => {
                out.kind = MachbusEventKind::VtServerStateChanged;
                out.u0 = vt_server_state_code(state);
            }
            VtServerEvent::ClientConnected(address) => {
                out.kind = MachbusEventKind::VtServerClientConnected;
                out.source = address;
            }
            VtServerEvent::ClientDisconnected(address) => {
                out.kind = MachbusEventKind::VtServerClientDisconnected;
                out.source = address;
            }
            VtServerEvent::ActiveWorkingSetChanged { from, to } => {
                out.kind = MachbusEventKind::VtServerActiveWorkingSetChanged;
                out.source = from;
                out.fmi_or_sub = to;
            }
            VtServerEvent::SoftKey { id, key_number } => {
                out.kind = MachbusEventKind::VtServerSoftKey;
                out.spn_or_pgn = id.0 as u32;
                out.source = key_number;
            }
            VtServerEvent::Button { id, key_number } => {
                out.kind = MachbusEventKind::VtServerButton;
                out.spn_or_pgn = id.0 as u32;
                out.source = key_number;
            }
            VtServerEvent::NumericValueChanged { id, value } => {
                out.kind = MachbusEventKind::VtServerNumericValueChanged;
                out.spn_or_pgn = id.0 as u32;
                out.u0 = value;
            }
            VtServerEvent::StringValueChanged { id, value } => {
                out.kind = MachbusEventKind::VtServerStringValueChanged;
                out.spn_or_pgn = id.0 as u32;
                out.u0 = value.len() as u32;
            }
            VtServerEvent::InputObjectSelected {
                id,
                selected,
                edit_active,
            } => {
                out.kind = MachbusEventKind::VtServerInputObjectSelected;
                out.spn_or_pgn = id.0 as u32;
                out.fmi_or_sub = u8::from(selected) | (u8::from(edit_active) << 1);
            }
        },
        Event::FsServer(f) => match f {
            FsServerEvent::ClientConnected(address) => {
                out.kind = MachbusEventKind::FsServerClientConnected;
                out.source = address;
            }
            FsServerEvent::ClientDisconnected(address) => {
                out.kind = MachbusEventKind::FsServerClientDisconnected;
                out.source = address;
            }
            FsServerEvent::FileOpened { client, path } => {
                out.kind = MachbusEventKind::FsServerFileOpened;
                out.source = client;
                out.u0 = path.len() as u32;
            }
            FsServerEvent::FileClosed { client, handle } => {
                out.kind = MachbusEventKind::FsServerFileClosed;
                out.source = client;
                out.u0 = handle as u32;
            }
        },
        Event::TcServer(t) => match t {
            TcServerEvent::StateChanged(state) => {
                out.kind = MachbusEventKind::TcServerStateChanged;
                out.u0 = state as u32;
            }
            TcServerEvent::ClientVersionReceived { address, version } => {
                out.kind = MachbusEventKind::TcServerClientVersionReceived;
                out.source = address;
                out.u0 = version as u32;
            }
            TcServerEvent::PeerControlAssignment {
                source,
                destination,
                source_element,
                source_ddi,
                destination_element,
                destination_ddi,
            } => {
                out.kind = MachbusEventKind::TcServerPeerControlAssignment;
                out.source = source;
                out.fmi_or_sub = destination;
                out.spn_or_pgn = (u32::from(source_element.0) << 16) | u32::from(source_ddi.0);
                out.u0 = (u32::from(destination_element.0) << 16) | u32::from(destination_ddi.0);
            }
        },
        Event::Powertrain(ev) => {
            out.kind = MachbusEventKind::Powertrain;
            match ev {
                crate::session::sys::PowertrainEvent::Eec1 { source, .. } => {
                    out.source = source;
                    out.spn_or_pgn = crate::net::pgn_defs::PGN_EEC1;
                }
                crate::session::sys::PowertrainEvent::Etc1 { source, .. } => {
                    out.source = source;
                    out.spn_or_pgn = crate::net::pgn_defs::PGN_ETC1;
                }
                crate::session::sys::PowertrainEvent::VehicleIdentification { source, data } => {
                    out.source = source;
                    out.spn_or_pgn = crate::net::pgn_defs::PGN_VEHICLE_ID;
                    out.u0 = data.vin.len() as u32;
                }
                _ => {}
            }
        }
        Event::Guidance(g) => match g {
            GuidanceEvent::MachineInfo {
                source,
                estimated_curvature,
                steering_ready,
                limit_status,
            } => {
                out.kind = MachbusEventKind::GuidanceMachineInfo;
                out.source = source;
                // d0 = estimated curvature (1/km); fmi_or_sub = steering-ready
                // flag (1 = ready); u0 = raw guidance limit status byte.
                out.d0 = estimated_curvature;
                out.fmi_or_sub = u8::from(steering_ready);
                out.u0 = u32::from(limit_status);
            }
        },
        Event::Custom { pgn, source, data } => {
            out.kind = MachbusEventKind::Custom;
            out.spn_or_pgn = pgn;
            out.source = source;
            out.fmi_or_sub = data.first().copied().unwrap_or(0xFF);
            out.u0 = data.len() as u32;
        }
        // Subsystems without a stable C payload (auxiliary, TIM, sequence
        // control, shortcut button, maintain-power, heartbeat, language
        // command, DM memory) surface as Other; use the Rust event API for
        // their full detail.
        _ => {}
    }
}

/// Drain the next application event into `out`. Returns `true` and fills `out`
/// when an event was available; returns `false` and writes a `None`-kind event
/// (without setting an error) when the queue is drained.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_poll_event(
    h: *mut MachbusSession,
    out: *mut MachbusEvent,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    if out.is_null() {
        set_last_error("null event-out pointer");
        return false;
    }
    let Some(ev) = h.session.poll_event() else {
        // SAFETY: caller-provided non-null target.
        unsafe { (*out) = MachbusEvent::empty(MachbusEventKind::None) };
        clear_last_error();
        return false;
    };
    // SAFETY: out validated above; classification fills the struct.
    unsafe { classify_event(ev, &mut *out) };
    clear_last_error();
    true
}

// ─── Diagnostics ──────────────────────────────────────────────────────

/// Raise a diagnostic trouble code (broadcast on the next DM1 cadence).
/// Requires the diagnostics subsystem to be enabled.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_diag_raise(
    h: *mut MachbusSession,
    spn: u32,
    fmi: u8,
    occurrence_count: u8,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let diag = plugin_mut!(h, Diagnostics);
    diag.raise(Dtc {
        spn,
        fmi: Fmi::from_u8(fmi),
        occurrence_count,
    });
    clear_last_error();
    true
}

/// Clear all active diagnostic trouble codes.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_diag_clear(h: *mut MachbusSession) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let diag = plugin_mut!(h, Diagnostics);
    diag.clear();
    clear_last_error();
    true
}

/// Number of currently active local diagnostic trouble codes, or 0 if the
/// diagnostics subsystem is not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_diag_active_count(h: *const MachbusSession) -> usize {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Diagnostics>())
        .map(|d| d.active().len())
        .unwrap_or(0)
}

// ─── GNSS ─────────────────────────────────────────────────────────────

/// Broadcast a GNSS position. Requires the GNSS subsystem to be enabled.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_gnss_broadcast_position(
    h: *mut MachbusSession,
    pos: *const MachbusGnssPosition,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    if pos.is_null() {
        set_last_error("null GNSS position pointer");
        return false;
    }
    // SAFETY: non-null per the check.
    let p = unsafe { *pos };
    let gnss = plugin_mut!(h, Gnss);
    let mut gp = GNSSPosition::default();
    gp.wgs.latitude = p.latitude;
    gp.wgs.longitude = p.longitude;
    gp.wgs.altitude = p.altitude_m;
    gp.altitude_m = Some(p.altitude_m);
    gp.speed_mps = Some(p.speed_mps);
    gp.heading_rad = Some(p.heading_rad);
    gnss.broadcast_position(&gp);
    clear_last_error();
    true
}

/// Broadcast course-over-ground / speed-over-ground (radians, m/s). Requires
/// the GNSS subsystem to be enabled.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_gnss_broadcast_cog_sog(
    h: *mut MachbusSession,
    cog_rad: f64,
    sog_mps: f64,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let gnss = plugin_mut!(h, Gnss);
    gnss.broadcast_cog_sog(cog_rad, sog_mps);
    clear_last_error();
    true
}

// ─── Implement ────────────────────────────────────────────────────────

/// Command a hitch (front/rear) raise/lower/no-action. For a target position,
/// use [`machbus_session_implement_command_hitch_position`]. Requires the
/// implement subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_implement_command_hitch(
    h: *mut MachbusSession,
    hitch: MachbusHitch,
    command: MachbusHitchCommand,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let imp = plugin_mut!(h, Implement);
    imp.command_hitch(hitch.into(), command.into());
    clear_last_error();
    true
}

/// Command a hitch to a target position (0..=1000 per mille) at `rate`.
/// Requires the implement subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_implement_command_hitch_position(
    h: *mut MachbusSession,
    hitch: MachbusHitch,
    target_position: u16,
    rate: u8,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let imp = plugin_mut!(h, Implement);
    imp.command_hitch_position(hitch.into(), target_position, rate);
    clear_last_error();
    true
}

/// Command a PTO (front/rear) engage/disengage/no-action. Requires the
/// implement subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_implement_command_pto(
    h: *mut MachbusSession,
    pto: MachbusPto,
    command: MachbusPtoCommand,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let imp = plugin_mut!(h, Implement);
    imp.command_pto(pto.into(), command.into());
    clear_last_error();
    true
}

/// Command a PTO target speed (RPM) with a ramp rate. Requires the implement
/// subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_implement_command_pto_speed(
    h: *mut MachbusSession,
    pto: MachbusPto,
    rpm: u16,
    ramp_rate: u8,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let imp = plugin_mut!(h, Implement);
    imp.command_pto_speed(pto.into(), rpm, ramp_rate);
    clear_last_error();
    true
}

/// Command an auxiliary valve by `valve_index` with a flow rate. Requires the
/// implement subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_implement_command_aux_valve(
    h: *mut MachbusSession,
    valve_index: u8,
    command: MachbusValveCommand,
    flow_rate: u16,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let imp = plugin_mut!(h, Implement);
    bool_result(imp.command_aux_valve(valve_index, command.into(), flow_rate))
}

// ─── Guidance (autosteer) ─────────────────────────────────────────────

/// Command the steering system to follow a path **curvature** in 1/km
/// (`0.0` = straight; sign follows the ISO 11783-7 wire convention). Broadcast
/// on the next tick as a Guidance System Command (PGN 0xAD00). Requires the
/// guidance subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_guidance_command_curvature(
    h: *mut MachbusSession,
    curvature_per_km: f64,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let g = plugin_mut!(h, Guidance);
    g.command_curvature(curvature_per_km);
    clear_last_error();
    true
}

/// Command a turn of the given **radius in metres** (curvature = 1000 / radius;
/// a zero or non-finite radius commands straight ahead). Requires the guidance
/// subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_guidance_command_radius(
    h: *mut MachbusSession,
    radius_m: f64,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let g = plugin_mut!(h, Guidance);
    g.command_radius(radius_m);
    clear_last_error();
    true
}

/// Command with a robotics-style twist: linear velocity `linear_mps` (m/s,
/// forward positive) and angular/yaw velocity `angular_rad_s` (rad/s, left
/// positive). Sends both the steering curvature (`κ = ω / v`, PGN 0xAD00) and
/// the target speed (PGN 0xFD43). Requires the guidance subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_guidance_command_velocity(
    h: *mut MachbusSession,
    linear_mps: f64,
    angular_rad_s: f64,
) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let g = plugin_mut!(h, Guidance);
    g.command_velocity(linear_mps, angular_rad_s);
    clear_last_error();
    true
}

/// Command straight-ahead (zero curvature). Requires the guidance subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_guidance_command_straight(h: *mut MachbusSession) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let g = plugin_mut!(h, Guidance);
    g.command_straight();
    clear_last_error();
    true
}

/// Write the steering system's last estimated curvature (1/km) into `out`.
/// Returns `false` (without setting an error) when no machine info has arrived
/// yet, or when the guidance subsystem is not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_guidance_estimated_curvature(
    h: *const MachbusSession,
    out: *mut f64,
) -> bool {
    let Some(curvature) = handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Guidance>())
        .and_then(|g| g.estimated_curvature())
    else {
        return false;
    };
    if !out.is_null() {
        // SAFETY: caller-provided writable pointer.
        unsafe { *out = curvature };
    }
    true
}

/// Whether the steering system last reported it is ready/engaged to steer.
/// Returns `false` if no machine info has arrived or the subsystem is unplugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_guidance_is_steering_ready(h: *const MachbusSession) -> bool {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<Guidance>())
        .map(|g| g.is_steering_ready())
        .unwrap_or(false)
}

// ─── VT client ────────────────────────────────────────────────────────

/// Connect the VT client to a server address. Requires the VT client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_connect(h: *mut MachbusSession, server: u8) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let vt = plugin_mut!(h, VtClient);
    vt.connect_to(server as Address);
    clear_last_error();
    true
}

/// Disconnect the VT client. Requires the VT client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_disconnect(h: *mut MachbusSession) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let vt = plugin_mut!(h, VtClient);
    bool_result(vt.disconnect())
}

/// VT client connection state code (see `vt_state_code`); 0 (Disconnected) if
/// the VT client subsystem is not plugged.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_state(h: *const MachbusSession) -> u32 {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<VtClient>())
        .map(|vt| vt_state_code(vt.state()))
        .unwrap_or(0)
}

/// Whether the VT client is connected.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_is_connected(h: *const MachbusSession) -> bool {
    handle_ref(h)
        .ok()
        .and_then(|h| h.session.get::<VtClient>())
        .map(VtClient::is_connected)
        .unwrap_or(false)
}

/// Show a VT object by id. Requires the VT client subsystem.
#[unsafe(no_mangle)]
pub extern "C" fn machbus_session_vt_show(h: *mut MachbusSession, object_id: u16) -> bool {
    let h = match handle_mut(h) {
        Ok(h) => h,
        Err(e) => {
            set_last_error(e);
            return false;
        }
    };
    let vt = plugin_mut!(h, VtClient);
    bool_result(vt.show(ObjectID(object_id)))
}

