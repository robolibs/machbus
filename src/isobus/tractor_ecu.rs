//! ISO 11783-9 Tractor ECU classification and power management types.
//!
//! Mirrors the data types from C++ `machbus::isobus::tractor_ecu.hpp`.
//! The `TractorECU` aggregator class (IsoNet-coupled, composes
//! `TimServer` + `WorkingSetManager` + `TractorFacilitiesInterface` +
//! `StateMachine<PowerState>`) is intentionally not ported. Users
//! compose the building blocks themselves.
//!
//! # `TecuClass` placement
//!
//! The canonical `TecuClass` (matching C++ `TECUClass`) lives in
//! [`crate::isobus::implement::tractor_facilities`]. This module
//! re-exports it so [`TecuClassification`]'s [`Display`] impl can
//! render `"Class 2NF"` etc.
//!
//! [`Display`]: core::fmt::Display

// ─── TecuClass ─────────────────────────────────────────────────────────

pub use crate::isobus::implement::tractor_facilities::TecuClass;

// ─── Power state ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum PowerState {
    #[default]
    PowerOff,
    /// Normal operation.
    IgnitionOn,
    /// Key off; up to 3 min of power available.
    ShutdownInitiated,
    /// Power down complete.
    FinalShutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SafeModeTrigger {
    #[default]
    None,
    PowerLoss,
    EcuPowerLoss,
    CanBusFail,
    TecuCommLoss,
    ManualTrigger,
}

/// How a command relates to the TECU safe-mode constraints (ISO 11783-9 §4.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TecuCommandKind {
    /// Engages motion / starts an actuator (hitch lower, PTO engage, drive).
    Engage,
    /// Disengages / stops / moves to a safe state.
    Disengage,
    /// Read-only status, no actuation.
    Query,
}

/// TECU safe-mode guard (ISO 11783-9 §4.7). Enforces the safety obligations as
/// repo-owned logic: no unexpected start (engage commands are blocked while in
/// safe mode), must allow stop (disengage always passes), loss-of-comms auto-
/// stop (enter on the relevant trigger), and operator override (explicit clear).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TecuSafeMode {
    active: bool,
    trigger: SafeModeTrigger,
}

impl TecuSafeMode {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            active: false,
            trigger: SafeModeTrigger::None,
        }
    }

    /// Enter safe mode, recording why.
    pub const fn enter(&mut self, trigger: SafeModeTrigger) {
        self.active = true;
        self.trigger = trigger;
    }

    /// Operator override / conditions clear: leave safe mode.
    pub const fn clear(&mut self) {
        self.active = false;
        self.trigger = SafeModeTrigger::None;
    }

    #[must_use]
    pub const fn is_active(&self) -> bool {
        self.active
    }

    #[must_use]
    pub const fn trigger(&self) -> SafeModeTrigger {
        self.trigger
    }

    /// Whether a command of `kind` may take effect now. In safe mode only
    /// disengage/stop and read-only queries are allowed; engage commands are
    /// refused (and the caller should NACK them).
    #[must_use]
    pub const fn allows(&self, kind: TecuCommandKind) -> bool {
        !self.active || matches!(kind, TecuCommandKind::Disengage | TecuCommandKind::Query)
    }
}

// ─── Classification ───────────────────────────────────────────────────

/// TECU classification: base class plus addendum flags.
///
/// Renders as `"Class 2NF"` etc. via the [`Display`] impl — addendum
/// order is `N`, `F`, `G`, `P`, `M` matching ISO 11783-9 §4.4.2.
///
/// [`Display`]: core::fmt::Display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TecuClassification {
    pub base_class: TecuClass,
    /// **N** — GPS / navigation position.
    pub navigation: bool,
    /// **G** — Guidance / steering (v2+).
    pub guidance: bool,
    /// **F** — Front hitch / PTO.
    pub front_mounted: bool,
    /// **P** — Powertrain control (v2+).
    pub powertrain: bool,
    /// **M** — Motion initiation (v2+).
    pub motion_init: bool,
    /// `1` or `2`.
    pub version: u8,
    /// `0` = primary, `1+` = secondary.
    pub instance: u8,
}

impl Default for TecuClassification {
    fn default() -> Self {
        Self {
            base_class: TecuClass::Class1,
            navigation: false,
            guidance: false,
            front_mounted: false,
            powertrain: false,
            motion_init: false,
            version: 1,
            instance: 0,
        }
    }
}

/// Message families that only the primary (function-instance 0) TECU transmits;
/// higher-instance TECUs must not duplicate them (ISO 11783-9 §4.4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TecuExclusiveMessage {
    /// Power-management (maintain-power) broadcast.
    PowerManagement,
    /// Lighting command/data.
    Lighting,
    /// Language command.
    Language,
}

impl TecuClassification {
    /// `true` for the primary TECU (function instance 0), which owns the
    /// instance-exclusive message families.
    #[must_use]
    pub const fn is_primary(&self) -> bool {
        self.instance == 0
    }

    /// Whether this TECU instance may transmit an instance-exclusive message
    /// family. Only the primary (instance 0) may; secondaries must defer to it
    /// to avoid duplicate transmissions.
    #[must_use]
    pub const fn transmits_exclusive(&self, _message: TecuExclusiveMessage) -> bool {
        self.is_primary()
    }

    #[must_use]
    pub const fn is_valid(&self) -> bool {
        if self.version == 0 || self.version > 2 {
            return false;
        }
        if self.version == 1 && (self.guidance || self.powertrain || self.motion_init) {
            return false;
        }
        true
    }

    #[must_use]
    pub const fn allows_facilities(
        &self,
        facilities: &crate::isobus::implement::TractorFacilities,
    ) -> bool {
        if !self.is_valid() {
            return false;
        }
        if matches!(self.base_class, TecuClass::Class1)
            && (facilities.ground_based_distance
                || facilities.ground_based_direction
                || facilities.wheel_based_distance
                || facilities.wheel_based_direction
                || facilities.rear_draft
                || facilities.lighting
                || facilities.aux_valve_flow
                || facilities.rear_hitch_command
                || facilities.rear_pto_command
                || facilities.aux_valve_command)
        {
            return false;
        }
        if !matches!(self.base_class, TecuClass::Class3)
            && (facilities.rear_hitch_command
                || facilities.rear_pto_command
                || facilities.aux_valve_command
                || facilities.rear_hitch_limit_status
                || facilities.rear_hitch_exit_code
                || facilities.rear_pto_engagement_request
                || facilities.rear_pto_speed_limit_status
                || facilities.rear_pto_exit_code
                || facilities.aux_valve_limit_status
                || facilities.aux_valve_exit_code)
        {
            return false;
        }
        if !self.front_mounted
            && (facilities.front_hitch_position
                || facilities.front_hitch_in_work
                || facilities.front_pto_speed
                || facilities.front_pto_engagement
                || facilities.front_hitch_command
                || facilities.front_pto_command
                || facilities.front_hitch_limit_status
                || facilities.front_hitch_exit_code
                || facilities.front_pto_engagement_request
                || facilities.front_pto_speed_limit_status
                || facilities.front_pto_exit_code)
        {
            return false;
        }
        if !self.navigation && facilities.navigation {
            return false;
        }
        if !self.guidance && facilities.guidance {
            return false;
        }
        if !self.powertrain
            && (facilities.machine_selected_speed || facilities.machine_selected_speed_command)
        {
            return false;
        }
        if self.version < 2
            && (facilities.rear_hitch_limit_status
                || facilities.rear_hitch_exit_code
                || facilities.rear_pto_engagement_request
                || facilities.rear_pto_speed_limit_status
                || facilities.rear_pto_exit_code
                || facilities.aux_valve_limit_status
                || facilities.aux_valve_exit_code
                || facilities.front_hitch_limit_status
                || facilities.front_hitch_exit_code
                || facilities.front_pto_engagement_request
                || facilities.front_pto_speed_limit_status
                || facilities.front_pto_exit_code)
        {
            return false;
        }
        true
    }

    /// Build the maximum tractor-facility advertisement for this local TECU
    /// class/addendum profile.
    ///
    /// Invalid class/version combinations return `None` so callers do not
    /// accidentally turn a rejected local profile into a wire claim.
    #[must_use]
    pub fn advertisable_facilities(&self) -> Option<crate::isobus::implement::TractorFacilities> {
        if !self.is_valid() {
            return None;
        }

        let mut facilities =
            crate::isobus::implement::TractorFacilities::default().with_class1_all();
        if matches!(self.base_class, TecuClass::Class2 | TecuClass::Class3) {
            facilities = facilities.with_class2_all();
        }
        if matches!(self.base_class, TecuClass::Class3) {
            facilities = facilities.with_class3_all();
        }
        if self.front_mounted {
            facilities.front_hitch_position = true;
            facilities.front_hitch_in_work = true;
            facilities.front_pto_speed = true;
            facilities.front_pto_engagement = true;
            if matches!(self.base_class, TecuClass::Class3) {
                facilities.front_hitch_command = true;
                facilities.front_pto_command = true;
            }
        }
        if self.navigation {
            facilities.navigation = true;
        }
        if self.guidance {
            facilities.guidance = true;
        }
        if self.powertrain {
            facilities.machine_selected_speed = true;
            facilities.machine_selected_speed_command = true;
        }
        if self.version >= 2 && matches!(self.base_class, TecuClass::Class3) {
            facilities = facilities.with_class3_v2_all();
            if self.front_mounted {
                facilities = facilities.with_front_v2_all();
            }
        }

        debug_assert!(self.allows_facilities(&facilities));
        Some(facilities)
    }
}

impl core::fmt::Display for TecuClassification {
    /// `"Class <n>[N][F][G][P][M]"` — order matches ISO 11783-9.
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Class {}", self.base_class.as_u8())?;
        if self.navigation {
            f.write_str("N")?;
        }
        if self.front_mounted {
            f.write_str("F")?;
        }
        if self.guidance {
            f.write_str("G")?;
        }
        if self.powertrain {
            f.write_str("P")?;
        }
        if self.motion_init {
            f.write_str("M")?;
        }
        Ok(())
    }
}

// ─── Power config ─────────────────────────────────────────────────────

/// Power-management timing and current limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PowerConfig {
    /// Maximum time after key-off before forced shutdown (default 3 min).
    pub shutdown_max_time_ms: u32,
    /// Minimum hold for a maintain-power request (default 2 s).
    pub maintain_timeout_ms: u32,
    /// ECU_PWR minimum current (A); ISO 11783-9 default = 15 A.
    pub ecu_pwr_current_amps: u8,
    /// PWR minimum current (A); default = 50 A.
    pub pwr_current_amps: u8,
}

impl Default for PowerConfig {
    fn default() -> Self {
        Self {
            shutdown_max_time_ms: 180_000,
            maintain_timeout_ms: 2_000,
            ecu_pwr_current_amps: 15,
            pwr_current_amps: 50,
        }
    }
}

impl PowerConfig {
    #[must_use]
    pub const fn shutdown_time(mut self, ms: u32) -> Self {
        self.shutdown_max_time_ms = ms;
        self
    }

    #[must_use]
    pub const fn maintain_timeout(mut self, ms: u32) -> Self {
        self.maintain_timeout_ms = ms;
        self
    }

    #[must_use]
    pub const fn ecu_power(mut self, amps: u8) -> Self {
        self.ecu_pwr_current_amps = amps;
        self
    }

    #[must_use]
    pub const fn power(mut self, amps: u8) -> Self {
        self.pwr_current_amps = amps;
        self
    }

    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.shutdown_max_time_ms > 0
            && self.maintain_timeout_ms > 0
            && self.maintain_timeout_ms <= self.shutdown_max_time_ms
            && self.ecu_pwr_current_amps > 0
            && self.pwr_current_amps > 0
    }
}

// ─── TECU config ──────────────────────────────────────────────────────

/// Top-level config for the (skipped) C++ `TractorECU` class.
/// Useful as a configuration record consumers can apply to their own
/// orchestration code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TecuConfig {
    pub classification: TecuClassification,
    pub power: PowerConfig,
    pub facilities_broadcast_interval_ms: u32,
    pub status_broadcast_interval_ms: u32,
    pub enable_gateway: bool,
}

impl Default for TecuConfig {
    fn default() -> Self {
        Self {
            classification: TecuClassification::default(),
            power: PowerConfig::default(),
            facilities_broadcast_interval_ms: 2_000,
            status_broadcast_interval_ms: 100,
            enable_gateway: false,
        }
    }
}

impl TecuConfig {
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.classification.is_valid()
            && self.power.is_valid()
            && self.facilities_broadcast_interval_ms > 0
            && self.status_broadcast_interval_ms > 0
    }
}

// ─── Maintain-power request (TECU side) ──────────────────────────────

/// Tracks a single CF's request to keep tractor power on after key-off.
/// Distinct from `j1939::MaintainPowerRequest` (the wire-format enum) —
/// renamed `TecuMaintainPowerRequest` to avoid collision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TecuMaintainPowerRequest {
    pub requester: crate::net::types::Address,
    pub ecu_pwr: bool,
    pub pwr: bool,
    pub timestamp_ms: u32,
}

impl TecuMaintainPowerRequest {
    #[must_use]
    pub fn is_expired(&self, current_time_ms: u32, timeout_ms: u32) -> bool {
        current_time_ms.saturating_sub(self.timestamp_ms) > timeout_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tecu_instance_ownership_of_exclusive_messages() {
        let primary = TecuClassification {
            instance: 0,
            ..TecuClassification::default()
        };
        let secondary = TecuClassification {
            instance: 1,
            ..TecuClassification::default()
        };
        assert!(primary.is_primary());
        assert!(!secondary.is_primary());
        for msg in [
            TecuExclusiveMessage::PowerManagement,
            TecuExclusiveMessage::Lighting,
            TecuExclusiveMessage::Language,
        ] {
            assert!(primary.transmits_exclusive(msg), "primary owns {msg:?}");
            assert!(
                !secondary.transmits_exclusive(msg),
                "secondary must not duplicate {msg:?}"
            );
        }
    }

    #[test]
    fn tecu_safe_mode_enforces_no_start_must_allow_stop() {
        let mut sm = TecuSafeMode::new();
        // Inactive: everything is allowed.
        assert!(!sm.is_active());
        assert!(sm.allows(TecuCommandKind::Engage));

        // On loss of communication, enter safe mode.
        sm.enter(SafeModeTrigger::TecuCommLoss);
        assert!(sm.is_active());
        assert_eq!(sm.trigger(), SafeModeTrigger::TecuCommLoss);
        // No unexpected start: engage refused. Must allow stop: disengage + query pass.
        assert!(!sm.allows(TecuCommandKind::Engage));
        assert!(sm.allows(TecuCommandKind::Disengage));
        assert!(sm.allows(TecuCommandKind::Query));

        // Operator override clears it.
        sm.clear();
        assert!(!sm.is_active());
        assert!(sm.allows(TecuCommandKind::Engage));
        assert_eq!(sm.trigger(), SafeModeTrigger::None);
    }

    #[test]
    fn classification_renders_as_class_n_f() {
        let c = TecuClassification {
            base_class: TecuClass::Class2,
            navigation: true,
            front_mounted: true,
            ..Default::default()
        };
        assert_eq!(c.to_string(), "Class 2NF");
    }

    #[test]
    fn classification_renders_class_3_with_all_addenda() {
        let c = TecuClassification {
            base_class: TecuClass::Class3,
            navigation: true,
            front_mounted: true,
            guidance: true,
            powertrain: true,
            motion_init: true,
            ..Default::default()
        };
        assert_eq!(c.to_string(), "Class 3NFGPM");
    }

    #[test]
    fn classification_default_is_class1_no_addenda() {
        assert_eq!(TecuClassification::default().to_string(), "Class 1");
    }

    #[test]
    fn power_config_defaults_match_iso() {
        let p = PowerConfig::default();
        assert_eq!(p.shutdown_max_time_ms, 180_000);
        assert_eq!(p.maintain_timeout_ms, 2_000);
        assert_eq!(p.ecu_pwr_current_amps, 15);
        assert_eq!(p.pwr_current_amps, 50);
    }

    #[test]
    fn power_config_fluent_setters() {
        let p = PowerConfig::default()
            .shutdown_time(120_000)
            .maintain_timeout(1_500)
            .ecu_power(20)
            .power(75);
        assert_eq!(p.shutdown_max_time_ms, 120_000);
        assert_eq!(p.ecu_pwr_current_amps, 20);
    }

    #[test]
    fn maintain_power_request_expiry() {
        let req = TecuMaintainPowerRequest {
            requester: 0x10,
            ecu_pwr: true,
            pwr: false,
            timestamp_ms: 1_000,
        };
        // 1500 ms later, 2000 ms timeout → not expired.
        assert!(!req.is_expired(2_500, 2_000));
        // 3000 ms later → expired.
        assert!(req.is_expired(4_500, 2_000));
    }

    #[test]
    fn power_state_default_is_power_off() {
        assert_eq!(PowerState::default(), PowerState::PowerOff);
    }
}
