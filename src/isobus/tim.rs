//! ISO 11783-7 / ISO 11783-9 Tractor Implement Management (TIM).
//!
//! Mirrors the C++ `machbus::isobus::tim.hpp`. The C++ `TimServer` /
//! `TimClient` (IsoNet-coupled, request/respond plumbing) are
//! intentionally not ported — users compose the codecs below with
//! `IsoNet::register_pgn_callback` / `IsoNet::send` directly. PTO,
//! hitch, and aux-valve PGNs share simple bit-field layouts. A small
//! [`TimAuthority`] guard is provided so applications can require supported,
//! requested, and granted options plus local interlock clearance before
//! emitting command PGNs.

use crate::net::message::Message;
use crate::net::pgn_defs::{
    PGN_AUX_VALVE_0_7, PGN_AUX_VALVE_8_15, PGN_AUX_VALVE_16_23, PGN_AUX_VALVE_24_31,
    PGN_FRONT_HITCH, PGN_FRONT_PTO, PGN_REAR_HITCH, PGN_REAR_PTO,
};
use crate::net::types::Address;

/// Maximum aux-valve index in ISO 11783 (4 PGN groups × 8 valves).
pub const MAX_AUX_VALVES: u8 = 32;
/// Maximum hitch position value (`100.00%`, scaled in 0.01%/bit).
pub const MAX_HITCH_POSITION: u16 = 10_000;
/// Packed TIM option byte width.
pub const TIM_OPTION_BYTES: usize = 3;
/// Bit mask of defined TIM option bits in each option byte.
///
/// The current option set occupies bits 0..=21. The remaining high bits in the
/// third byte are reserved and must not be accepted as requested/granted
/// authority.
pub const TIM_OPTION_DEFINED_MASK: [u8; TIM_OPTION_BYTES] = [0xFF, 0xFF, 0x3F];
/// Recommended periodic update interval for TIM broadcasts.
pub const TIM_UPDATE_INTERVAL_MS: u32 = 100;

/// TIM Capability flags (ISO 11783-9 §6.4 / §6.6 — tractor facilities).
///
/// These bit indices correspond to the C++ `TimOption` enum values
/// and are typically packed into a `Required Tractor Facilities`
/// payload by upper-layer code. Kept here as named constants so
/// callers can OR them together without magic numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TimOption {
    FrontPtoDisengagementIsSupported = 0,
    FrontPtoEngagementCcwIsSupported = 1,
    FrontPtoEngagementCwIsSupported = 2,
    FrontPtoSpeedCcwIsSupported = 3,
    FrontPtoSpeedCwIsSupported = 4,
    RearPtoDisengagementIsSupported = 5,
    RearPtoEngagementCcwIsSupported = 6,
    RearPtoEngagementCwIsSupported = 7,
    RearPtoSpeedCcwIsSupported = 8,
    RearPtoSpeedCwIsSupported = 9,
    FrontHitchMotionIsSupported = 10,
    FrontHitchPositionIsSupported = 11,
    RearHitchMotionIsSupported = 12,
    RearHitchPositionIsSupported = 13,
    VehicleSpeedInForwardDirectionIsSupported = 14,
    VehicleSpeedInReverseDirectionIsSupported = 15,
    VehicleSpeedStartMotionIsSupported = 16,
    VehicleSpeedStopMotionIsSupported = 17,
    VehicleSpeedForwardSetByServerIsSupported = 18,
    VehicleSpeedReverseSetByServerIsSupported = 19,
    VehicleSpeedChangeDirectionIsSupported = 20,
    GuidanceCurvatureIsSupported = 21,
}

impl TimOption {
    #[inline]
    #[must_use]
    pub const fn bit(self) -> u8 {
        self as u8
    }
}

// ─── TIM option negotiation ───────────────────────────────────────────

/// Three-byte TIM option bitset used for capability negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimOptionSet {
    bytes: [u8; TIM_OPTION_BYTES],
}

impl TimOptionSet {
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            bytes: [0; TIM_OPTION_BYTES],
        }
    }

    #[inline]
    #[must_use]
    pub const fn from_bytes(bytes: [u8; TIM_OPTION_BYTES]) -> Self {
        Self { bytes }
    }

    pub const fn try_from_bytes(bytes: [u8; TIM_OPTION_BYTES]) -> Result<Self, TimValidationError> {
        let set = Self { bytes };
        if set.has_reserved_bits() {
            return Err(TimValidationError::ReservedOptionBits { bytes });
        }
        Ok(set)
    }

    #[must_use]
    pub fn from_options(options: &[TimOption]) -> Self {
        let mut set = Self::empty();
        for &option in options {
            set.set(option, true);
        }
        set
    }

    #[inline]
    #[must_use]
    pub const fn as_bytes(self) -> [u8; TIM_OPTION_BYTES] {
        self.bytes
    }

    pub fn set(&mut self, option: TimOption, enabled: bool) {
        let (byte_idx, mask) = option_mask(option);
        if enabled {
            self.bytes[byte_idx] |= mask;
        } else {
            self.bytes[byte_idx] &= !mask;
        }
    }

    #[must_use]
    pub fn contains(&self, option: TimOption) -> bool {
        let (byte_idx, mask) = option_mask(option);
        (self.bytes[byte_idx] & mask) != 0
    }

    #[must_use]
    pub fn is_subset_of(&self, available: &Self) -> bool {
        self.bytes
            .iter()
            .zip(available.bytes.iter())
            .all(|(&required, &available)| required & !available == 0)
    }

    #[must_use]
    pub fn missing_from(&self, available: &Self) -> Self {
        let mut missing = Self::empty();
        for i in 0..TIM_OPTION_BYTES {
            missing.bytes[i] = self.bytes[i] & !available.bytes[i];
        }
        missing
    }

    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.bytes[0] == 0 && self.bytes[1] == 0 && self.bytes[2] == 0
    }

    #[must_use]
    pub const fn has_reserved_bits(&self) -> bool {
        (self.bytes[0] & !TIM_OPTION_DEFINED_MASK[0]) != 0
            || (self.bytes[1] & !TIM_OPTION_DEFINED_MASK[1]) != 0
            || (self.bytes[2] & !TIM_OPTION_DEFINED_MASK[2]) != 0
    }
}

/// TIM commands the local authority/interlock guard can reason about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimCommand {
    FrontPtoDisengage,
    FrontPtoEngageCcw,
    FrontPtoEngageCw,
    FrontPtoSpeedCcw,
    FrontPtoSpeedCw,
    RearPtoDisengage,
    RearPtoEngageCcw,
    RearPtoEngageCw,
    RearPtoSpeedCcw,
    RearPtoSpeedCw,
    FrontHitchMotion,
    FrontHitchPosition,
    RearHitchMotion,
    RearHitchPosition,
    VehicleSpeedForward,
    VehicleSpeedReverse,
    VehicleSpeedStartMotion,
    VehicleSpeedStopMotion,
    VehicleSpeedForwardSetByServer,
    VehicleSpeedReverseSetByServer,
    VehicleSpeedChangeDirection,
    GuidanceCurvature,
}

impl TimCommand {
    #[must_use]
    pub const fn required_option(self) -> TimOption {
        match self {
            Self::FrontPtoDisengage => TimOption::FrontPtoDisengagementIsSupported,
            Self::FrontPtoEngageCcw => TimOption::FrontPtoEngagementCcwIsSupported,
            Self::FrontPtoEngageCw => TimOption::FrontPtoEngagementCwIsSupported,
            Self::FrontPtoSpeedCcw => TimOption::FrontPtoSpeedCcwIsSupported,
            Self::FrontPtoSpeedCw => TimOption::FrontPtoSpeedCwIsSupported,
            Self::RearPtoDisengage => TimOption::RearPtoDisengagementIsSupported,
            Self::RearPtoEngageCcw => TimOption::RearPtoEngagementCcwIsSupported,
            Self::RearPtoEngageCw => TimOption::RearPtoEngagementCwIsSupported,
            Self::RearPtoSpeedCcw => TimOption::RearPtoSpeedCcwIsSupported,
            Self::RearPtoSpeedCw => TimOption::RearPtoSpeedCwIsSupported,
            Self::FrontHitchMotion => TimOption::FrontHitchMotionIsSupported,
            Self::FrontHitchPosition => TimOption::FrontHitchPositionIsSupported,
            Self::RearHitchMotion => TimOption::RearHitchMotionIsSupported,
            Self::RearHitchPosition => TimOption::RearHitchPositionIsSupported,
            Self::VehicleSpeedForward => TimOption::VehicleSpeedInForwardDirectionIsSupported,
            Self::VehicleSpeedReverse => TimOption::VehicleSpeedInReverseDirectionIsSupported,
            Self::VehicleSpeedStartMotion => TimOption::VehicleSpeedStartMotionIsSupported,
            Self::VehicleSpeedStopMotion => TimOption::VehicleSpeedStopMotionIsSupported,
            Self::VehicleSpeedForwardSetByServer => {
                TimOption::VehicleSpeedForwardSetByServerIsSupported
            }
            Self::VehicleSpeedReverseSetByServer => {
                TimOption::VehicleSpeedReverseSetByServerIsSupported
            }
            Self::VehicleSpeedChangeDirection => TimOption::VehicleSpeedChangeDirectionIsSupported,
            Self::GuidanceCurvature => TimOption::GuidanceCurvatureIsSupported,
        }
    }
}

/// Local TIM authority state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TimAuthorityState {
    #[default]
    Idle,
    Requested,
    Granted,
    Denied,
    Revoked,
}

/// Safety/interlock reason that prevents TIM authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimInterlock {
    OperatorNotPresent,
    RoadTransportMode,
    ExternalStop,
    ImplementNotReady,
}

/// Local TIM interlock snapshot.
///
/// The defaults are the "all clear" state so tests and simple applications can
/// opt into individual blocking conditions explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TimInterlocks {
    pub operator_present: bool,
    pub road_transport_mode: bool,
    pub external_stop: bool,
    pub implement_ready: bool,
}

impl Default for TimInterlocks {
    fn default() -> Self {
        Self::all_clear()
    }
}

impl TimInterlocks {
    #[must_use]
    pub const fn all_clear() -> Self {
        Self {
            operator_present: true,
            road_transport_mode: false,
            external_stop: false,
            implement_ready: true,
        }
    }

    #[must_use]
    pub const fn with_operator_present(mut self, present: bool) -> Self {
        self.operator_present = present;
        self
    }

    #[must_use]
    pub const fn with_road_transport_mode(mut self, active: bool) -> Self {
        self.road_transport_mode = active;
        self
    }

    #[must_use]
    pub const fn with_external_stop(mut self, active: bool) -> Self {
        self.external_stop = active;
        self
    }

    #[must_use]
    pub const fn with_implement_ready(mut self, ready: bool) -> Self {
        self.implement_ready = ready;
        self
    }

    #[must_use]
    pub const fn blocking_reason(self) -> Option<TimInterlock> {
        if !self.operator_present {
            return Some(TimInterlock::OperatorNotPresent);
        }
        if self.road_transport_mode {
            return Some(TimInterlock::RoadTransportMode);
        }
        if self.external_stop {
            return Some(TimInterlock::ExternalStop);
        }
        if !self.implement_ready {
            return Some(TimInterlock::ImplementNotReady);
        }
        None
    }
}

/// Local TIM authority/interlock guard.
///
/// This is intentionally a pure state helper rather than an IsoNet-coupled TIM
/// client/server. It lets applications prove a command is supported, was part
/// of the granted authority request, and is still allowed by local interlocks
/// before emitting any command PGN.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimAuthority {
    available: TimOptionSet,
    requested: TimOptionSet,
    state: TimAuthorityState,
    interlocks: TimInterlocks,
    /// Operator consent gate (AEF automation operator-workflow layer).
    /// Defaults to `true` (consented) so the guard stays permissive unless
    /// an application opts into an explicit consent step.
    operator_consent: bool,
    /// Communication watchdog: if non-zero, a live grant is automatically
    /// revoked when this many milliseconds elapse without a keepalive (AEF
    /// loss-of-communication safe-stop). `0` disables the watchdog. The value
    /// is a repo-tunable safety timeout, not a wire field.
    comms_timeout_ms: u32,
    since_keepalive_ms: u32,
}

impl TimAuthority {
    #[must_use]
    pub const fn new(available: TimOptionSet) -> Self {
        Self {
            available,
            requested: TimOptionSet::empty(),
            state: TimAuthorityState::Idle,
            interlocks: TimInterlocks::all_clear(),
            operator_consent: true,
            comms_timeout_ms: 0,
            since_keepalive_ms: 0,
        }
    }

    /// Enable the communication watchdog with `timeout_ms` (0 disables it).
    #[must_use]
    pub const fn with_comms_timeout(mut self, timeout_ms: u32) -> Self {
        self.comms_timeout_ms = timeout_ms;
        self
    }

    /// Record that a control/status message was exchanged, resetting the
    /// communication watchdog.
    pub const fn keepalive(&mut self) {
        self.since_keepalive_ms = 0;
    }

    /// Advance the communication watchdog. Returns `true` if loss of
    /// communication just revoked a live grant (the caller should command the
    /// safe state). No-op when the watchdog is disabled or no grant is active.
    pub fn tick(&mut self, elapsed_ms: u32) -> bool {
        if self.comms_timeout_ms == 0 {
            return false;
        }
        self.since_keepalive_ms = self.since_keepalive_ms.saturating_add(elapsed_ms);
        if self.since_keepalive_ms >= self.comms_timeout_ms
            && self.state == TimAuthorityState::Granted
        {
            self.state = TimAuthorityState::Revoked;
            self.since_keepalive_ms = 0;
            return true;
        }
        false
    }

    #[must_use]
    pub const fn available_options(&self) -> TimOptionSet {
        self.available
    }

    #[must_use]
    pub const fn requested_options(&self) -> TimOptionSet {
        self.requested
    }

    #[must_use]
    pub const fn state(&self) -> TimAuthorityState {
        self.state
    }

    #[must_use]
    pub const fn interlocks(&self) -> TimInterlocks {
        self.interlocks
    }

    pub fn request(&mut self, requested: TimOptionSet) -> Result<(), TimValidationError> {
        if self.available.has_reserved_bits() {
            return Err(TimValidationError::ReservedOptionBits {
                bytes: self.available.as_bytes(),
            });
        }
        if requested.has_reserved_bits() {
            return Err(TimValidationError::ReservedOptionBits {
                bytes: requested.as_bytes(),
            });
        }
        if requested.is_empty() {
            return Err(TimValidationError::EmptyOptionRequest);
        }
        if !requested.is_subset_of(&self.available) {
            return Err(TimValidationError::UnsupportedOptions {
                requested,
                available: self.available,
            });
        }
        self.requested = requested;
        self.state = TimAuthorityState::Requested;
        Ok(())
    }

    pub fn grant(&mut self) -> Result<(), TimValidationError> {
        if self.state != TimAuthorityState::Requested {
            return Err(TimValidationError::AuthorityNotRequested { state: self.state });
        }
        if let Some(interlock) = self.interlocks.blocking_reason() {
            return Err(TimValidationError::InterlockActive { interlock });
        }
        if !self.operator_consent {
            return Err(TimValidationError::OperatorConsentRequired);
        }
        self.state = TimAuthorityState::Granted;
        self.since_keepalive_ms = 0;
        Ok(())
    }

    /// `true` if the operator has consented to automation.
    #[must_use]
    pub const fn operator_consent(&self) -> bool {
        self.operator_consent
    }

    /// Set the operator-consent gate. Withdrawing consent while authority
    /// is Granted revokes it (a granted automation cannot keep commanding
    /// once the operator withdraws consent), mirroring interlock loss.
    pub fn set_operator_consent(&mut self, consented: bool) {
        self.operator_consent = consented;
        if !consented && self.state == TimAuthorityState::Granted {
            self.state = TimAuthorityState::Revoked;
        }
    }

    pub const fn deny(&mut self) {
        self.state = TimAuthorityState::Denied;
    }

    pub const fn revoke(&mut self) {
        self.state = TimAuthorityState::Revoked;
    }

    pub fn set_interlocks(&mut self, interlocks: TimInterlocks) {
        self.interlocks = interlocks;
        if self.state == TimAuthorityState::Granted && interlocks.blocking_reason().is_some() {
            self.state = TimAuthorityState::Revoked;
        }
    }

    pub fn ensure_option(&self, option: TimOption) -> Result<(), TimValidationError> {
        if !self.available.contains(option) {
            return Err(TimValidationError::UnsupportedOption { option });
        }
        if !self.requested.contains(option) {
            return Err(TimValidationError::OptionNotRequested { option });
        }
        if let Some(interlock) = self.interlocks.blocking_reason() {
            return Err(TimValidationError::InterlockActive { interlock });
        }
        if !self.operator_consent {
            return Err(TimValidationError::OperatorConsentRequired);
        }
        if self.state != TimAuthorityState::Granted {
            return Err(TimValidationError::AuthorityNotGranted { state: self.state });
        }
        Ok(())
    }

    pub fn ensure_command(&self, command: TimCommand) -> Result<(), TimValidationError> {
        self.ensure_option(command.required_option())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimValidationError {
    AuxValveIndexOutOfRange {
        index: u8,
    },
    HitchPositionOutOfRange {
        position: u16,
    },
    UnsupportedOption {
        option: TimOption,
    },
    UnsupportedOptions {
        requested: TimOptionSet,
        available: TimOptionSet,
    },
    ReservedOptionBits {
        bytes: [u8; TIM_OPTION_BYTES],
    },
    EmptyOptionRequest,
    OptionNotRequested {
        option: TimOption,
    },
    AuthorityNotRequested {
        state: TimAuthorityState,
    },
    AuthorityNotGranted {
        state: TimAuthorityState,
    },
    InterlockActive {
        interlock: TimInterlock,
    },
    /// The operator has not consented to automation (operator-workflow gate).
    OperatorConsentRequired,
}

#[inline]
const fn option_mask(option: TimOption) -> (usize, u8) {
    let bit = option.bit();
    ((bit / 8) as usize, 1u8 << (bit % 8))
}

fn fixed_payload_with_ff_tail(data: &[u8], used: usize) -> bool {
    data.len() == 8 && data[used..].iter().all(|&byte| byte == 0xFF)
}

fn aux_valve_pgn_matches_index(pgn: u32, index: u8) -> bool {
    match pgn {
        PGN_AUX_VALVE_0_7 => index < 8,
        PGN_AUX_VALVE_8_15 => (8..16).contains(&index),
        PGN_AUX_VALVE_16_23 => (16..24).contains(&index),
        PGN_AUX_VALVE_24_31 => (24..32).contains(&index),
        _ => false,
    }
}

fn decode_bool_byte(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

// ─── PTO ───────────────────────────────────────────────────────────────

/// PTO state (front or rear). Same wire format used by both
/// `PGN_FRONT_PTO` (0xFE54) and `PGN_REAR_PTO` (0xF003).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PtoState {
    pub engaged: bool,
    pub cw_direction: bool,
    /// Shaft speed in RPM.
    pub speed: u16,
}

impl PtoState {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = u8::from(self.engaged);
        data[1] = u8::from(self.cw_direction);
        data[2] = (self.speed & 0xFF) as u8;
        data[3] = ((self.speed >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if msg.pgn != PGN_FRONT_PTO && msg.pgn != PGN_REAR_PTO {
            return None;
        }
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return None;
        }
        if !fixed_payload_with_ff_tail(&msg.data, 4) {
            return None;
        }
        let engaged = decode_bool_byte(msg.data[0])?;
        let cw_direction = decode_bool_byte(msg.data[1])?;
        Some(Self {
            engaged,
            cw_direction,
            speed: (msg.data[2] as u16) | ((msg.data[3] as u16) << 8),
        })
    }
}

// ─── Hitch ─────────────────────────────────────────────────────────────

/// Hitch state (front or rear). Position is `0..=10000` for
/// `0.00%–100.00%` (0.01% per bit). Same wire format used by both
/// `PGN_FRONT_HITCH` (0xFE08) and `PGN_REAR_HITCH` (0xF005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitchState {
    pub motion_enabled: bool,
    pub position: u16,
}

impl HitchState {
    pub const fn validate(&self) -> Result<(), TimValidationError> {
        if self.position > MAX_HITCH_POSITION {
            return Err(TimValidationError::HitchPositionOutOfRange {
                position: self.position,
            });
        }
        Ok(())
    }

    pub fn try_encode(&self) -> Result<[u8; 8], TimValidationError> {
        self.validate()?;
        Ok(self.encode())
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = u8::from(self.motion_enabled);
        data[1] = (self.position & 0xFF) as u8;
        data[2] = ((self.position >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if msg.pgn != PGN_FRONT_HITCH && msg.pgn != PGN_REAR_HITCH {
            return None;
        }
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return None;
        }
        if !fixed_payload_with_ff_tail(&msg.data, 3) {
            return None;
        }
        let motion_enabled = decode_bool_byte(msg.data[0])?;
        let position = (msg.data[1] as u16) | ((msg.data[2] as u16) << 8);
        if position > MAX_HITCH_POSITION {
            return None;
        }
        Some(Self {
            motion_enabled,
            position,
        })
    }
}

// ─── Aux valve ─────────────────────────────────────────────────────────

/// Single aux-valve descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuxValve {
    pub state_supported: bool,
    pub flow_supported: bool,
    pub state: bool,
    /// Flow as percent or L/min, scaled per implementation.
    pub flow: u16,
}

/// Single aux-valve command (TIM client → server). The C++ uses
/// `PGN_AUX_VALVE_0_7` for the basic command path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuxValveCommand {
    pub index: u8,
    pub state: bool,
    pub flow: u16,
}

impl AuxValveCommand {
    pub const fn validate(&self) -> Result<(), TimValidationError> {
        if self.index >= MAX_AUX_VALVES {
            return Err(TimValidationError::AuxValveIndexOutOfRange { index: self.index });
        }
        Ok(())
    }

    pub fn try_encode(&self) -> Result<[u8; 8], TimValidationError> {
        self.validate()?;
        Ok(self.encode())
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.index;
        data[1] = u8::from(self.state);
        data[2] = (self.flow & 0xFF) as u8;
        data[3] = ((self.flow >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return None;
        }
        if !fixed_payload_with_ff_tail(&msg.data, 4) {
            return None;
        }
        if msg.data[0] >= MAX_AUX_VALVES {
            return None;
        }
        if !aux_valve_pgn_matches_index(msg.pgn, msg.data[0]) {
            return None;
        }
        let state = decode_bool_byte(msg.data[1])?;
        Some(Self {
            index: msg.data[0],
            state,
            flow: (msg.data[2] as u16) | ((msg.data[3] as u16) << 8),
        })
    }
}

/// Outcome of a TIM authority request when arbitrated between clients.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimArbitration {
    /// The requesting client now holds authority.
    Granted,
    /// Authority is held by another client (its address is returned).
    Denied { held_by: Address },
}

/// Arbitrates exclusive TIM authority between competing clients on the server
/// side (AEF automation). Only one client may hold authority at a time; a
/// request from another client is denied until the holder releases it. A
/// repeated request from the current holder is idempotently granted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimAuthorityArbiter {
    holder: Option<Address>,
}

impl TimAuthorityArbiter {
    #[must_use]
    pub const fn new() -> Self {
        Self { holder: None }
    }

    /// The client currently holding authority, if any.
    #[must_use]
    pub const fn holder(&self) -> Option<Address> {
        self.holder
    }

    /// Request authority for `client`. Granted if free or already held by the
    /// same client; otherwise denied with the current holder.
    pub fn request(&mut self, client: Address) -> TimArbitration {
        match self.holder {
            None => {
                self.holder = Some(client);
                TimArbitration::Granted
            }
            Some(h) if h == client => TimArbitration::Granted,
            Some(held_by) => TimArbitration::Denied { held_by },
        }
    }

    /// Release authority if `client` holds it. Returns `true` if released.
    pub fn release(&mut self, client: Address) -> bool {
        if self.holder == Some(client) {
            self.holder = None;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::{
        PGN_AUX_VALVE_0_7, PGN_FRONT_HITCH, PGN_FRONT_PTO, PGN_REAR_HITCH, PGN_REAR_PTO,
    };

    #[test]
    fn pto_round_trip_front_and_rear() {
        let pto = PtoState {
            engaged: true,
            cw_direction: false,
            speed: 540,
        };
        let bytes = pto.encode();
        for pgn in [PGN_FRONT_PTO, PGN_REAR_PTO] {
            let msg = Message::new(pgn, bytes.to_vec(), 0x10);
            let d = PtoState::decode(&msg).unwrap();
            assert_eq!(d, pto);
        }
    }

    #[test]
    fn hitch_round_trip_front_and_rear() {
        let h = HitchState {
            motion_enabled: true,
            position: 7500,
        };
        let bytes = h.encode();
        for pgn in [PGN_FRONT_HITCH, PGN_REAR_HITCH] {
            let msg = Message::new(pgn, bytes.to_vec(), 0x10);
            assert_eq!(HitchState::decode(&msg).unwrap(), h);
        }
    }

    #[test]
    fn aux_valve_command_round_trip() {
        let c = AuxValveCommand {
            index: 3,
            state: true,
            flow: 5000,
        };
        let msg = Message::new(PGN_AUX_VALVE_0_7, c.encode().to_vec(), 0x10);
        assert_eq!(AuxValveCommand::decode(&msg).unwrap(), c);
    }

    #[test]
    fn aux_valve_command_rejects_out_of_range_index() {
        let invalid = AuxValveCommand {
            index: MAX_AUX_VALVES,
            state: true,
            flow: 1,
        };
        assert_eq!(
            invalid.validate(),
            Err(TimValidationError::AuxValveIndexOutOfRange {
                index: MAX_AUX_VALVES
            })
        );
        assert!(invalid.try_encode().is_err());
        assert!(
            AuxValveCommand::decode(&Message::new(
                PGN_AUX_VALVE_0_7,
                invalid.encode().to_vec(),
                0x10
            ))
            .is_none()
        );
    }

    #[test]
    fn hitch_rejects_out_of_range_position() {
        let invalid = HitchState {
            motion_enabled: true,
            position: MAX_HITCH_POSITION + 1,
        };
        assert_eq!(
            invalid.validate(),
            Err(TimValidationError::HitchPositionOutOfRange {
                position: MAX_HITCH_POSITION + 1
            })
        );
        assert!(invalid.try_encode().is_err());
        assert!(
            HitchState::decode(&Message::new(
                PGN_REAR_HITCH,
                invalid.encode().to_vec(),
                0x10
            ))
            .is_none()
        );
    }

    #[test]
    fn fixed_size_decoders_reject_malformed_lengths_and_padding() {
        let pto = PtoState {
            engaged: true,
            cw_direction: false,
            speed: 540,
        };
        let mut pto_bad_tail = pto.encode();
        pto_bad_tail[4] = 0;
        assert!(PtoState::decode(&Message::new(PGN_FRONT_PTO, vec![0u8; 3], 0)).is_none());
        assert!(
            PtoState::decode(&Message::new(PGN_FRONT_PTO, pto.encode().repeat(2), 0)).is_none()
        );
        assert!(PtoState::decode(&Message::new(PGN_FRONT_PTO, pto_bad_tail.to_vec(), 0)).is_none());

        let hitch = HitchState {
            motion_enabled: true,
            position: 7500,
        };
        let mut hitch_bad_tail = hitch.encode();
        hitch_bad_tail[3] = 0;
        assert!(HitchState::decode(&Message::new(PGN_REAR_HITCH, vec![0u8; 2], 0)).is_none());
        assert!(
            HitchState::decode(&Message::new(PGN_REAR_HITCH, hitch.encode().repeat(2), 0))
                .is_none()
        );
        assert!(
            HitchState::decode(&Message::new(PGN_REAR_HITCH, hitch_bad_tail.to_vec(), 0)).is_none()
        );

        let aux = AuxValveCommand {
            index: 3,
            state: true,
            flow: 5000,
        };
        let mut aux_bad_tail = aux.encode();
        aux_bad_tail[4] = 0;
        assert!(
            AuxValveCommand::decode(&Message::new(PGN_AUX_VALVE_0_7, vec![0u8; 3], 0)).is_none()
        );
        assert!(
            AuxValveCommand::decode(&Message::new(PGN_AUX_VALVE_0_7, aux.encode().repeat(2), 0))
                .is_none()
        );
        assert!(
            AuxValveCommand::decode(&Message::new(PGN_AUX_VALVE_0_7, aux_bad_tail.to_vec(), 0))
                .is_none()
        );
    }

    #[test]
    fn fixed_size_decoders_reject_reserved_boolean_bytes() {
        let mut pto_bad_engaged = PtoState {
            engaged: true,
            cw_direction: false,
            speed: 540,
        }
        .encode();
        pto_bad_engaged[0] = 2;
        assert!(
            PtoState::decode(&Message::new(PGN_FRONT_PTO, pto_bad_engaged.to_vec(), 0)).is_none()
        );

        let mut pto_bad_direction = PtoState {
            engaged: true,
            cw_direction: false,
            speed: 540,
        }
        .encode();
        pto_bad_direction[1] = 2;
        assert!(
            PtoState::decode(&Message::new(PGN_REAR_PTO, pto_bad_direction.to_vec(), 0)).is_none()
        );

        let mut hitch_bad_motion = HitchState {
            motion_enabled: true,
            position: 7500,
        }
        .encode();
        hitch_bad_motion[0] = 2;
        assert!(
            HitchState::decode(&Message::new(PGN_FRONT_HITCH, hitch_bad_motion.to_vec(), 0))
                .is_none()
        );

        let mut aux_bad_state = AuxValveCommand {
            index: 3,
            state: true,
            flow: 5000,
        }
        .encode();
        aux_bad_state[1] = 2;
        assert!(
            AuxValveCommand::decode(&Message::new(PGN_AUX_VALVE_0_7, aux_bad_state.to_vec(), 0))
                .is_none()
        );
    }

    #[test]
    fn tim_option_bits_are_unique() {
        let bits = [
            TimOption::FrontPtoDisengagementIsSupported.bit(),
            TimOption::RearHitchPositionIsSupported.bit(),
            TimOption::GuidanceCurvatureIsSupported.bit(),
        ];
        assert_eq!(bits, [0, 13, 21]);
    }

    #[test]
    fn tim_option_set_negotiates_required_subset() {
        let available = TimOptionSet::from_options(&[
            TimOption::FrontPtoDisengagementIsSupported,
            TimOption::RearHitchPositionIsSupported,
        ]);
        let ok = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let missing = TimOptionSet::from_options(&[TimOption::GuidanceCurvatureIsSupported]);

        assert!(ok.is_subset_of(&available));
        assert!(!missing.is_subset_of(&available));
        assert!(
            missing
                .missing_from(&available)
                .contains(TimOption::GuidanceCurvatureIsSupported)
        );
        assert_eq!(available.as_bytes(), [0x01, 0x20, 0x00]);
    }

    #[test]
    fn tim_authority_requires_supported_requested_and_granted_options() {
        let available = TimOptionSet::from_options(&[
            TimOption::RearHitchPositionIsSupported,
            TimOption::GuidanceCurvatureIsSupported,
        ]);
        let requested = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let mut authority = TimAuthority::new(available);

        assert_eq!(
            authority.ensure_command(TimCommand::RearHitchPosition),
            Err(TimValidationError::OptionNotRequested {
                option: TimOption::RearHitchPositionIsSupported
            })
        );
        authority.request(requested).unwrap();
        assert_eq!(
            authority.ensure_command(TimCommand::RearHitchPosition),
            Err(TimValidationError::AuthorityNotGranted {
                state: TimAuthorityState::Requested
            })
        );
        authority.grant().unwrap();
        assert!(
            authority
                .ensure_command(TimCommand::RearHitchPosition)
                .is_ok()
        );
        assert_eq!(
            authority.ensure_command(TimCommand::GuidanceCurvature),
            Err(TimValidationError::OptionNotRequested {
                option: TimOption::GuidanceCurvatureIsSupported
            })
        );
        assert_eq!(
            authority.ensure_command(TimCommand::RearPtoSpeedCw),
            Err(TimValidationError::UnsupportedOption {
                option: TimOption::RearPtoSpeedCwIsSupported
            })
        );
    }

    #[test]
    fn tim_authority_interlocks_block_and_revoke_grants() {
        let requested = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let mut authority = TimAuthority::new(requested);
        authority.set_interlocks(TimInterlocks::all_clear().with_operator_present(false));
        authority.request(requested).unwrap();
        assert_eq!(
            authority.grant(),
            Err(TimValidationError::InterlockActive {
                interlock: TimInterlock::OperatorNotPresent
            })
        );

        authority.set_interlocks(TimInterlocks::all_clear());
        authority.grant().unwrap();
        assert_eq!(authority.state(), TimAuthorityState::Granted);

        authority.set_interlocks(TimInterlocks::all_clear().with_road_transport_mode(true));
        assert_eq!(authority.state(), TimAuthorityState::Revoked);
        assert_eq!(
            authority.ensure_command(TimCommand::RearHitchPosition),
            Err(TimValidationError::InterlockActive {
                interlock: TimInterlock::RoadTransportMode
            })
        );

        authority.set_interlocks(TimInterlocks::all_clear());
        assert_eq!(
            authority.ensure_command(TimCommand::RearHitchPosition),
            Err(TimValidationError::AuthorityNotGranted {
                state: TimAuthorityState::Revoked
            })
        );
        authority.request(requested).unwrap();
        authority.grant().unwrap();
        assert!(
            authority
                .ensure_command(TimCommand::RearHitchPosition)
                .is_ok()
        );
    }

    #[test]
    fn tim_authority_operator_consent_gates_grant_and_commands() {
        let requested = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let mut authority = TimAuthority::new(requested);
        // Default is consented (permissive).
        assert!(authority.operator_consent());

        // Withhold consent: a request can be made but grant is blocked.
        authority.set_operator_consent(false);
        authority.request(requested).unwrap();
        assert_eq!(
            authority.grant(),
            Err(TimValidationError::OperatorConsentRequired)
        );

        // Consent given → grant succeeds and commands pass.
        authority.set_operator_consent(true);
        authority.grant().unwrap();
        assert!(
            authority
                .ensure_command(TimCommand::RearHitchPosition)
                .is_ok()
        );

        // Withdrawing consent while granted revokes authority and gates commands.
        authority.set_operator_consent(false);
        assert_eq!(authority.state(), TimAuthorityState::Revoked);
        assert_eq!(
            authority.ensure_command(TimCommand::RearHitchPosition),
            Err(TimValidationError::OperatorConsentRequired)
        );
    }

    #[test]
    fn tim_comms_watchdog_revokes_grant_on_loss_of_communication() {
        let available = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let requested = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let mut authority = TimAuthority::new(available).with_comms_timeout(500);
        authority.request(requested).unwrap();
        authority.grant().unwrap();

        // Within the timeout, with keepalives, the grant holds.
        assert!(!authority.tick(200));
        authority.keepalive();
        assert!(!authority.tick(400));
        assert_eq!(authority.state(), TimAuthorityState::Granted);

        // No keepalive past the timeout ⇒ loss-of-comms revoke + safe-stop signal.
        authority.keepalive();
        assert!(!authority.tick(300));
        assert!(authority.tick(300));
        assert_eq!(authority.state(), TimAuthorityState::Revoked);
        // Subsequent ticks do not re-fire.
        assert!(!authority.tick(1000));
    }

    #[test]
    fn tim_authority_arbiter_grants_one_client_at_a_time() {
        let mut arb = TimAuthorityArbiter::new();
        assert_eq!(arb.holder(), None);
        // First client gets authority.
        assert_eq!(arb.request(0x80), TimArbitration::Granted);
        assert_eq!(arb.holder(), Some(0x80));
        // Same client re-requesting is idempotently granted.
        assert_eq!(arb.request(0x80), TimArbitration::Granted);
        // A second client is denied while the first holds it.
        assert_eq!(arb.request(0x81), TimArbitration::Denied { held_by: 0x80 });
        // A non-holder cannot release; the holder can.
        assert!(!arb.release(0x81));
        assert!(arb.release(0x80));
        assert_eq!(arb.holder(), None);
        // Now the second client can take it.
        assert_eq!(arb.request(0x81), TimArbitration::Granted);
    }

    #[test]
    fn tim_comms_watchdog_disabled_by_default() {
        let available = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let requested = TimOptionSet::from_options(&[TimOption::RearHitchPositionIsSupported]);
        let mut authority = TimAuthority::new(available);
        authority.request(requested).unwrap();
        authority.grant().unwrap();
        // With no comms timeout configured, ticks never revoke.
        assert!(!authority.tick(100_000));
        assert_eq!(authority.state(), TimAuthorityState::Granted);
    }
}
