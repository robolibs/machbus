//! ISO 11783-14 Sequence Control common types.
//!
//! Mirrors the C++ `machbus::isobus::sc::types.hpp`. Constants,
//! enums, sequence step / config structs.

use alloc::string::String;

// ─── ISO 11783-14 message codes (Annex F) ──────────────────────────────

/// Message code in byte 0 of `PGN_SC_MASTER_STATUS`.
pub const SC_MSG_CODE_MASTER: u8 = 0x95;
/// Message code in byte 0 of `PGN_SC_CLIENT_STATUS`.
pub const SC_MSG_CODE_CLIENT: u8 = 0x96;

// ─── ISO 11783-14 timeouts (Annex F) ───────────────────────────────────

/// Timeout while in Recording / PlayBack / Abort.
pub const SC_STATUS_TIMEOUT_ACTIVE_MS: u32 = 600;
/// Timeout while in Ready.
pub const SC_STATUS_TIMEOUT_READY_MS: u32 = 3000;
/// Minimum spacing between consecutive status messages.
pub const SC_STATUS_MIN_SPACING_MS: u32 = 100;
/// 5 Hz cadence during active states.
pub const SC_STATUS_ACTIVE_RATE_MS: u32 = 200;
/// Maximum Sequence Control step id that can be represented on the wire.
///
/// The SC status payload carries a selected sequence number in byte 3. Values
/// above `0x31` are reserved by the protocol, and `0xFF` is the Ready /
/// not-applicable sentinel.
pub const SC_MAX_SEQUENCE_STEP_ID: u16 = 0x31;
/// Sequence Control master/client status messages are classic 8-byte CAN
/// payloads. Prefix-decoding shorter or overlong reassembled buffers can hide
/// malformed tail data.
pub const SC_STATUS_PAYLOAD_LEN: usize = 8;
/// Sequence number / step-id sentinel for Ready or not-applicable states.
pub const SC_SEQUENCE_NUMBER_NOT_AVAILABLE: u8 = 0xFF;

// ─── Internal unified state ────────────────────────────────────────────

/// Library-internal unified SC state. Maps to ISO byte 2 + byte 4 via
/// the helpers in [`super::master`] / [`super::client`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SCState {
    #[default]
    Idle = 0,
    Ready = 1,
    Active = 2,
    Paused = 3,
    Complete = 4,
    Error = 5,
}

// ─── ISO 11783-14 byte 2 enums ─────────────────────────────────────────

/// Master state, F.2 byte 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SCMasterState {
    #[default]
    Inactive = 0,
    Active = 1,
    Initialization = 2,
}

impl SCMasterState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Active,
            2 => Self::Initialization,
            _ => Self::Inactive,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Inactive),
            1 => Some(Self::Active),
            2 => Some(Self::Initialization),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[must_use]
pub(crate) const fn sc_master_state_byte_is_valid(v: u8) -> bool {
    SCMasterState::try_from_u8(v).is_some()
}

/// Client state, F.3 byte 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SCClientState {
    #[default]
    Disabled = 0,
    Enabled = 1,
    Initialization = 2,
}

impl SCClientState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Enabled,
            2 => Self::Initialization,
            _ => Self::Disabled,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Disabled),
            1 => Some(Self::Enabled),
            2 => Some(Self::Initialization),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[must_use]
pub(crate) const fn sc_client_state_byte_is_valid(v: u8) -> bool {
    SCClientState::try_from_u8(v).is_some()
}

// ─── ISO 11783-14 byte 4: sequence state ───────────────────────────────

/// Sequence state, F.2 / F.3 byte 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SCSequenceState {
    #[default]
    Reserved = 0,
    Ready = 1,
    Recording = 2,
    RecordingCompletion = 3,
    PlayBack = 4,
    Abort = 5,
}

impl SCSequenceState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Ready,
            2 => Self::Recording,
            3 => Self::RecordingCompletion,
            4 => Self::PlayBack,
            5 => Self::Abort,
            _ => Self::Reserved,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Reserved),
            1 => Some(Self::Ready),
            2 => Some(Self::Recording),
            3 => Some(Self::RecordingCompletion),
            4 => Some(Self::PlayBack),
            5 => Some(Self::Abort),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[must_use]
pub(crate) const fn sc_sequence_state_byte_is_valid(v: u8) -> bool {
    SCSequenceState::try_from_u8(v).is_some()
}

#[must_use]
pub(crate) const fn sc_status_sequence_number_is_valid(
    sequence_state: SCSequenceState,
    sequence_number: u8,
) -> bool {
    match sequence_state {
        SCSequenceState::Ready => sequence_number == SC_SEQUENCE_NUMBER_NOT_AVAILABLE,
        SCSequenceState::PlayBack | SCSequenceState::Abort => {
            (sequence_number as u16) <= SC_MAX_SEQUENCE_STEP_ID
        }
        _ => true,
    }
}

#[must_use]
pub(crate) const fn sc_status_sequence_state_is_supported(sequence_state: SCSequenceState) -> bool {
    match sequence_state {
        SCSequenceState::Ready | SCSequenceState::PlayBack | SCSequenceState::Abort => true,
        SCSequenceState::Reserved
        | SCSequenceState::Recording
        | SCSequenceState::RecordingCompletion => false,
    }
}

#[must_use]
pub(crate) const fn sc_inactive_status_sequence_fields_are_valid(
    sequence_state: SCSequenceState,
    sequence_number: u8,
) -> bool {
    matches!(sequence_state, SCSequenceState::Reserved)
        && sequence_number == SC_SEQUENCE_NUMBER_NOT_AVAILABLE
}

#[must_use]
pub(crate) const fn sc_master_busy_flags_are_valid(flags: u8) -> bool {
    flags & !0x03 == 0
}

#[must_use]
pub(crate) fn sc_status_reserved_tail_is_valid(data: &[u8]) -> bool {
    data.len() == SC_STATUS_PAYLOAD_LEN && data[5..].iter().all(|&byte| byte == 0xFF)
}

// ─── ISO 11783-14 byte 5 of SCClientStatus ─────────────────────────────

/// Client function error state, F.3 byte 5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SCClientFuncError {
    #[default]
    NoErrors = 0,
    /// Error state didn't change since last report.
    NoChange = 1,
    /// Error state changed since last report.
    Changed = 2,
    /// Operator confirmation required.
    NeedsConfirm = 3,
}

impl SCClientFuncError {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::NoChange,
            2 => Self::Changed,
            3 => Self::NeedsConfirm,
            _ => Self::NoErrors,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoErrors),
            1 => Some(Self::NoChange),
            2 => Some(Self::Changed),
            3 => Some(Self::NeedsConfirm),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[must_use]
pub(crate) const fn sc_client_func_error_byte_is_valid(v: u8) -> bool {
    SCClientFuncError::try_from_u8(v).is_some()
}

// ─── Library-side commands ─────────────────────────────────────────────

/// Sequence control commands the master may issue (library-internal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SCCommand {
    #[default]
    Start = 0,
    Pause = 1,
    Resume = 2,
    Abort = 3,
}

// ─── Sequence step ─────────────────────────────────────────────────────

/// One step in a sequence-control plan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SequenceStep {
    /// Wire-visible step id. Must be unique per master sequence and fit in
    /// `0..=SC_MAX_SEQUENCE_STEP_ID`.
    pub step_id: u16,
    pub description: String,
    pub duration_ms: u32,
    pub completed: bool,
}

// ─── Configurations ────────────────────────────────────────────────────

/// Configuration for [`super::master::SCMaster`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SCMasterConfig {
    pub ready_timeout_ms: u32,
    pub active_timeout_ms: u32,
    pub status_interval_ms: u32,
    /// Number of unique SC clients that must acknowledge Ready and PlayBack
    /// before the master treats the sequence/step as active.
    ///
    /// `0` is treated as `1` by [`SCMasterConfig::required_client_count`] so
    /// literal configs cannot accidentally create a sequence that starts with
    /// no client participation.
    pub required_client_count: u8,
}

impl Default for SCMasterConfig {
    fn default() -> Self {
        Self {
            ready_timeout_ms: SC_STATUS_TIMEOUT_READY_MS,
            active_timeout_ms: SC_STATUS_TIMEOUT_ACTIVE_MS,
            status_interval_ms: SC_STATUS_MIN_SPACING_MS,
            required_client_count: 1,
        }
    }
}

impl SCMasterConfig {
    #[must_use]
    pub const fn with_ready_timeout(mut self, ms: u32) -> Self {
        self.ready_timeout_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_active_timeout(mut self, ms: u32) -> Self {
        self.active_timeout_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_status_interval(mut self, ms: u32) -> Self {
        self.status_interval_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_required_client_count(mut self, count: u8) -> Self {
        self.required_client_count = if count == 0 { 1 } else { count };
        self
    }

    #[inline]
    #[must_use]
    pub const fn required_client_count(self) -> usize {
        if self.required_client_count == 0 {
            1
        } else {
            self.required_client_count as usize
        }
    }
}

/// Configuration for [`super::client::SCClient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SCClientConfig {
    pub min_status_spacing_ms: u32,
    pub busy_pause_timeout_ms: u32,
}

impl Default for SCClientConfig {
    fn default() -> Self {
        Self {
            min_status_spacing_ms: SC_STATUS_MIN_SPACING_MS,
            busy_pause_timeout_ms: SC_STATUS_TIMEOUT_ACTIVE_MS,
        }
    }
}

impl SCClientConfig {
    #[must_use]
    pub const fn with_min_spacing(mut self, ms: u32) -> Self {
        self.min_status_spacing_ms = ms;
        self
    }

    #[must_use]
    pub const fn with_busy_timeout(mut self, ms: u32) -> Self {
        self.busy_pause_timeout_ms = ms;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn master_state_round_trip() {
        for s in [
            SCMasterState::Inactive,
            SCMasterState::Active,
            SCMasterState::Initialization,
        ] {
            assert_eq!(SCMasterState::from_u8(s.as_u8()), s);
        }
        assert_eq!(SCMasterState::from_u8(0xFF), SCMasterState::Inactive);
    }

    #[test]
    fn client_state_round_trip() {
        for s in [
            SCClientState::Disabled,
            SCClientState::Enabled,
            SCClientState::Initialization,
        ] {
            assert_eq!(SCClientState::from_u8(s.as_u8()), s);
        }
    }

    #[test]
    fn sequence_state_round_trip() {
        for s in [
            SCSequenceState::Reserved,
            SCSequenceState::Ready,
            SCSequenceState::Recording,
            SCSequenceState::RecordingCompletion,
            SCSequenceState::PlayBack,
            SCSequenceState::Abort,
        ] {
            assert_eq!(SCSequenceState::from_u8(s.as_u8()), s);
        }
        assert_eq!(SCSequenceState::from_u8(0xFF), SCSequenceState::Reserved);
    }

    #[test]
    fn client_func_error_round_trip() {
        for e in [
            SCClientFuncError::NoErrors,
            SCClientFuncError::NoChange,
            SCClientFuncError::Changed,
            SCClientFuncError::NeedsConfirm,
        ] {
            assert_eq!(SCClientFuncError::from_u8(e.as_u8()), e);
        }
    }

    #[test]
    fn master_config_builder() {
        let c = SCMasterConfig::default()
            .with_ready_timeout(5000)
            .with_active_timeout(800)
            .with_status_interval(150)
            .with_required_client_count(2);
        assert_eq!(c.ready_timeout_ms, 5000);
        assert_eq!(c.active_timeout_ms, 800);
        assert_eq!(c.status_interval_ms, 150);
        assert_eq!(c.required_client_count(), 2);
        assert_eq!(
            SCMasterConfig {
                required_client_count: 0,
                ..SCMasterConfig::default()
            }
            .required_client_count(),
            1
        );
    }

    #[test]
    fn client_config_builder() {
        let c = SCClientConfig::default()
            .with_min_spacing(50)
            .with_busy_timeout(1000);
        assert_eq!(c.min_status_spacing_ms, 50);
        assert_eq!(c.busy_pause_timeout_ms, 1000);
    }
}
