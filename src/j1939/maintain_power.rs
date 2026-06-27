//! Maintain Power — ISO 11783-9 §4.6.
//!
//! Mirrors the C++ `machbus::j1939::maintain_power.hpp`. Two parts:
//!
//! 1. [`MaintainPowerData`] — wire codec for implement work/park/ready/
//!    transport state and ECU/PWR maintain requests.
//! 2. [`PowerManager`] — pump-style state machine modelling the
//!    Running → ShutdownPending → Maintaining → PowerOff lifecycle.
//!    The C++ version embeds `IsoNet&` and sends directly; the Rust
//!    port returns broadcasts from [`PowerManager::update`] for the
//!    caller to route.

use alloc::{collections::BTreeSet, vec::Vec};

use crate::net::constants::{
    HEARTBEAT_INTERVAL_MS, POWER_MAINTAIN_REPEAT_MS, POWER_MAX_EXTENSION_MS, POWER_SHUTDOWN_MIN_MS,
};
use crate::net::event::Event;
use crate::net::message::Message;
use crate::net::pgn_defs::PGN_MAINTAIN_POWER;
use crate::net::types::Address;

// ─── Codec ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum KeySwitchState {
    Off = 0,
    NotOff = 1,
    Error = 2,
    #[default]
    NotAvailable = 3,
}

impl KeySwitchState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Off,
            1 => Self::NotOff,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            1 => Some(Self::NotOff),
            2 => Some(Self::Error),
            3 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum MaintainPowerRequest {
    #[default]
    NoRequest = 0,
    EcuRequest = 1,
    Error = 2,
    NotAvailable = 3,
}

impl MaintainPowerRequest {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            1 => Self::EcuRequest,
            2 => Self::Error,
            3 => Self::NotAvailable,
            _ => Self::NoRequest,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoRequest),
            1 => Some(Self::EcuRequest),
            2 => Some(Self::Error),
            3 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaintainPowerData {
    pub implement_in_work_state: MaintainPowerState,
    pub implement_park_state: MaintainPowerState,
    pub implement_ready_to_work_state: MaintainPowerState,
    pub implement_transport_state: MaintainPowerState,
    pub maintain_actuator_power: MaintainPowerRequirement,
    pub maintain_ecu_power: MaintainPowerRequirement,
    pub timestamp_us: u64,
}

impl Default for MaintainPowerData {
    fn default() -> Self {
        Self {
            implement_in_work_state: MaintainPowerState::NotAvailable,
            implement_park_state: MaintainPowerState::NotAvailable,
            implement_ready_to_work_state: MaintainPowerState::NotAvailable,
            implement_transport_state: MaintainPowerState::NotAvailable,
            maintain_actuator_power: MaintainPowerRequirement::DontCare,
            maintain_ecu_power: MaintainPowerRequirement::DontCare,
            timestamp_us: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum MaintainPowerState {
    Inactive = 0,
    Active = 1,
    Error = 2,
    #[default]
    NotAvailable = 3,
}

impl MaintainPowerState {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Inactive,
            1 => Self::Active,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Inactive),
            1 => Some(Self::Active),
            2 => Some(Self::Error),
            3 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum MaintainPowerRequirement {
    NoFurtherRequirement = 0,
    RequirementFor2SecondsMore = 1,
    Error = 2,
    #[default]
    DontCare = 3,
}

impl MaintainPowerRequirement {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::NoFurtherRequirement,
            1 => Self::RequirementFor2SecondsMore,
            2 => Self::Error,
            _ => Self::DontCare,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoFurtherRequirement),
            1 => Some(Self::RequirementFor2SecondsMore),
            2 => Some(Self::Error),
            3 => Some(Self::DontCare),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl MaintainPowerData {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = 0x0F
            | ((self.implement_in_work_state.as_u8() & 0x03) << 4)
            | ((self.implement_park_state.as_u8() & 0x03) << 6);
        data[1] = (self.implement_ready_to_work_state.as_u8() & 0x03)
            | ((self.implement_transport_state.as_u8() & 0x03) << 2)
            | ((self.maintain_actuator_power.as_u8() & 0x03) << 4)
            | ((self.maintain_ecu_power.as_u8() & 0x03) << 6);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[0] & 0x0F != 0x0F {
            return None;
        }
        if data[2..].iter().any(|&byte| byte != 0xFF) {
            return None;
        }
        Some(Self {
            implement_in_work_state: MaintainPowerState::try_from_u8((data[0] >> 4) & 0x03)?,
            implement_park_state: MaintainPowerState::try_from_u8(data[0] >> 6)?,
            implement_ready_to_work_state: MaintainPowerState::try_from_u8(data[1] & 0x03)?,
            implement_transport_state: MaintainPowerState::try_from_u8((data[1] >> 2) & 0x03)?,
            maintain_actuator_power: MaintainPowerRequirement::try_from_u8((data[1] >> 4) & 0x03)?,
            maintain_ecu_power: MaintainPowerRequirement::try_from_u8(data[1] >> 6)?,
            timestamp_us: 0,
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_MAINTAIN_POWER) {
            return None;
        }
        let mut d = Self::decode(&msg.data)?;
        d.timestamp_us = msg.timestamp_us;
        Some(d)
    }
}

// ─── PowerManager ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum PowerState {
    #[default]
    Running,
    ShutdownPending,
    Maintaining,
    PowerOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PowerRole {
    /// Power source — broadcasts key state, observes maintain
    /// requests, owns the shutdown sequence.
    Tecu,
    /// Consumer — sends maintain requests when it needs power, tracks
    /// the TECU's key-switch broadcasts.
    Cf,
}

/// State machine for the Running → ShutdownPending → Maintaining →
/// PowerOff lifecycle. Pump-style: [`Self::update`] returns the
/// [`MaintainPowerData`] messages the caller should broadcast.
pub struct PowerManager {
    role: PowerRole,
    state: PowerState,
    /// Time in non-Running state (ms).
    shutdown_timer_ms: u32,
    /// Time since the last maintain request *received* (TECU mode).
    maintain_timer_ms: u32,
    /// Periodic broadcast cadence (TECU mode, 100 ms).
    broadcast_timer_ms: u32,
    /// Periodic maintain-request cadence (CF mode, 1 s).
    request_timer_ms: u32,
    requesting_power: bool,
    /// TECU mode: source addresses that have requested maintained power during
    /// the current shutdown sequence.
    requesting_clients: BTreeSet<Address>,

    pub on_state_change: Event<PowerState>,
    pub on_power_off: Event<()>,
}

impl PowerManager {
    #[must_use]
    pub fn new(role: PowerRole) -> Self {
        Self {
            role,
            state: PowerState::Running,
            shutdown_timer_ms: 0,
            maintain_timer_ms: 0,
            broadcast_timer_ms: 0,
            request_timer_ms: 0,
            requesting_power: false,
            requesting_clients: BTreeSet::new(),
            on_state_change: Event::new(),
            on_power_off: Event::new(),
        }
    }

    /// TECU mode: source addresses that have requested maintained power during
    /// the current shutdown sequence (cleared on key-on).
    pub fn requesting_clients(&self) -> impl Iterator<Item = Address> + '_ {
        self.requesting_clients.iter().copied()
    }

    #[inline]
    #[must_use]
    pub fn state(&self) -> PowerState {
        self.state
    }

    #[inline]
    #[must_use]
    pub fn role(&self) -> PowerRole {
        self.role
    }

    /// TECU: observe a key-off event.
    pub fn key_off(&mut self) {
        if matches!(self.state, PowerState::Running) {
            self.transition(PowerState::ShutdownPending);
            self.shutdown_timer_ms = 0;
            self.maintain_timer_ms = 0;
            tracing::info!(target: "machbus.power", "key-off detected, shutdown pending");
        }
    }

    /// TECU: observe a key-on event (cancels any pending shutdown).
    pub fn key_on(&mut self) {
        if !matches!(self.state, PowerState::Running) {
            self.transition(PowerState::Running);
            self.shutdown_timer_ms = 0;
            self.requesting_clients.clear();
            tracing::info!(target: "machbus.power", "key-on, power restored");
        }
    }

    /// CF: ask for power extension. Sends a maintain-request on the
    /// next [`Self::update`] tick.
    pub fn request_power(&mut self, need_power: bool) {
        self.requesting_power = need_power;
        if need_power {
            // Trigger send on next update.
            self.request_timer_ms = POWER_MAINTAIN_REPEAT_MS;
        }
    }

    /// Drive timers and produce any broadcasts. The caller routes the
    /// returned [`MaintainPowerData`] through `IsoNet::send`.
    pub fn update(&mut self, elapsed_ms: u32) -> Vec<MaintainPowerData> {
        let mut out = Vec::new();
        match self.role {
            PowerRole::Tecu => self.update_tecu(elapsed_ms, &mut out),
            PowerRole::Cf => self.update_cf(elapsed_ms, &mut out),
        }
        out
    }

    /// Process an incoming PGN_MAINTAIN_POWER message.
    pub fn handle_message(&mut self, msg: &Message) {
        if !msg.has_usable_envelope_for_pgn(PGN_MAINTAIN_POWER) {
            return;
        }
        let Some(mpd) = MaintainPowerData::from_message(msg) else {
            return;
        };
        match self.role {
            PowerRole::Tecu => {
                if matches!(
                    mpd.maintain_ecu_power,
                    MaintainPowerRequirement::RequirementFor2SecondsMore
                ) || matches!(
                    mpd.maintain_actuator_power,
                    MaintainPowerRequirement::RequirementFor2SecondsMore
                ) {
                    self.maintain_timer_ms = 0;
                    self.requesting_clients.insert(msg.source);
                    tracing::trace!(
                        target: "machbus.power",
                        from = msg.source,
                        "maintain request received",
                    );
                }
            }
            PowerRole::Cf => {}
        }
    }

    // ─── Internal ──────────────────────────────────────────────────

    fn update_tecu(&mut self, elapsed_ms: u32, out: &mut Vec<MaintainPowerData>) {
        self.broadcast_timer_ms = self.broadcast_timer_ms.saturating_add(elapsed_ms);
        if self.broadcast_timer_ms >= HEARTBEAT_INTERVAL_MS {
            self.broadcast_timer_ms = 0;
            out.push(MaintainPowerData {
                implement_in_work_state: MaintainPowerState::Inactive,
                implement_park_state: MaintainPowerState::Inactive,
                implement_ready_to_work_state: MaintainPowerState::Inactive,
                implement_transport_state: MaintainPowerState::Inactive,
                maintain_ecu_power: MaintainPowerRequirement::NoFurtherRequirement,
                maintain_actuator_power: MaintainPowerRequirement::NoFurtherRequirement,
                timestamp_us: 0,
            });
        }

        match self.state {
            PowerState::ShutdownPending => {
                self.shutdown_timer_ms = self.shutdown_timer_ms.saturating_add(elapsed_ms);
                self.maintain_timer_ms = self.maintain_timer_ms.saturating_add(elapsed_ms);
                if self.shutdown_timer_ms >= POWER_SHUTDOWN_MIN_MS {
                    if self.maintain_timer_ms >= POWER_MAINTAIN_REPEAT_MS * 2 {
                        self.transition(PowerState::PowerOff);
                        self.on_power_off.emit(&());
                        tracing::info!(target: "machbus.power", "no maintain requests, power off");
                    } else {
                        self.transition(PowerState::Maintaining);
                        tracing::info!(
                            target: "machbus.power",
                            "maintain requests active, extending power",
                        );
                    }
                }
            }
            PowerState::Maintaining => {
                self.shutdown_timer_ms = self.shutdown_timer_ms.saturating_add(elapsed_ms);
                self.maintain_timer_ms = self.maintain_timer_ms.saturating_add(elapsed_ms);
                if self.shutdown_timer_ms >= POWER_MAX_EXTENSION_MS {
                    self.transition(PowerState::PowerOff);
                    self.on_power_off.emit(&());
                    tracing::warn!(target: "machbus.power", "max power extension reached, forcing off");
                } else if self.maintain_timer_ms >= POWER_MAINTAIN_REPEAT_MS * 2 {
                    self.transition(PowerState::PowerOff);
                    self.on_power_off.emit(&());
                    tracing::info!(
                        target: "machbus.power",
                        "maintain requests stopped, power off",
                    );
                }
            }
            _ => {}
        }
    }

    fn update_cf(&mut self, elapsed_ms: u32, out: &mut Vec<MaintainPowerData>) {
        if !self.requesting_power {
            return;
        }
        self.request_timer_ms = self.request_timer_ms.saturating_add(elapsed_ms);
        if self.request_timer_ms >= POWER_MAINTAIN_REPEAT_MS {
            self.request_timer_ms = 0;
            out.push(MaintainPowerData {
                implement_in_work_state: MaintainPowerState::Active,
                implement_park_state: MaintainPowerState::Active,
                implement_ready_to_work_state: MaintainPowerState::Active,
                implement_transport_state: MaintainPowerState::Active,
                maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
                maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
                timestamp_us: 0,
            });
        }
    }

    fn transition(&mut self, new_state: PowerState) {
        if self.state != new_state {
            self.state = new_state;
            self.on_state_change.emit(&new_state);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_MAINTAIN_POWER;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn maintain_request_msg() -> Message {
        let mpd = MaintainPowerData {
            maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
            timestamp_us: 0,
            ..Default::default()
        };
        Message::new(PGN_MAINTAIN_POWER, mpd.encode().to_vec(), 0x10)
    }

    #[test]
    fn maintain_power_data_round_trip() {
        let mpd = MaintainPowerData {
            implement_in_work_state: MaintainPowerState::Active,
            implement_park_state: MaintainPowerState::Active,
            implement_ready_to_work_state: MaintainPowerState::Active,
            implement_transport_state: MaintainPowerState::Active,
            maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
            maintain_ecu_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
            timestamp_us: 0,
        };
        let bytes = mpd.encode();
        let decoded = MaintainPowerData::decode(&bytes).unwrap();
        assert_eq!(decoded, mpd);
        assert_eq!(bytes, [0x5F, 0x55, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
    }

    #[test]
    fn maintain_power_data_rejects_short_and_overlong_payloads() {
        let bytes = MaintainPowerData::default().encode();
        assert!(MaintainPowerData::decode(&bytes[..7]).is_none());
        assert!(MaintainPowerData::decode(&[bytes.as_slice(), &[0x00]].concat()).is_none());
    }

    #[test]
    fn maintain_power_data_rejects_reserved_bits_and_tails() {
        let mut bytes = MaintainPowerData {
            maintain_ecu_power: MaintainPowerRequirement::NoFurtherRequirement,
            timestamp_us: 0,
            ..Default::default()
        }
        .encode();

        bytes[0] &= !0x01;
        assert!(MaintainPowerData::decode(&bytes).is_none());

        let mut bytes = MaintainPowerData::default().encode();
        bytes[2] = 0x00;
        assert!(MaintainPowerData::decode(&bytes).is_none());
    }

    #[test]
    fn power_manager_starts_running() {
        let pm = PowerManager::new(PowerRole::Tecu);
        assert_eq!(pm.state(), PowerState::Running);
    }

    #[test]
    fn tecu_key_off_starts_shutdown() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        let states = Rc::new(RefCell::new(Vec::new()));
        let s = states.clone();
        pm.on_state_change
            .subscribe(move |st| s.borrow_mut().push(*st));
        pm.key_off();
        assert_eq!(pm.state(), PowerState::ShutdownPending);
        assert_eq!(*states.borrow(), vec![PowerState::ShutdownPending]);
    }

    #[test]
    fn tecu_key_on_cancels_shutdown() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        pm.key_off();
        pm.key_on();
        assert_eq!(pm.state(), PowerState::Running);
    }

    #[test]
    fn tecu_broadcasts_at_100ms() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        let out1 = pm.update(50);
        assert!(out1.is_empty());
        let out2 = pm.update(60); // 110 ms total → broadcast
        assert_eq!(out2.len(), 1);
        assert_eq!(
            out2[0].maintain_ecu_power,
            MaintainPowerRequirement::NoFurtherRequirement
        );
    }

    #[test]
    fn tecu_powers_off_after_min_with_no_requests() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        let off_count = Rc::new(RefCell::new(0u32));
        let oc = off_count.clone();
        pm.on_power_off.subscribe(move |_| *oc.borrow_mut() += 1);

        pm.key_off();
        for _ in 0..20 {
            pm.update(100); // 2000 ms total → minimum key-off hold elapsed.
        }
        assert_eq!(pm.state(), PowerState::PowerOff);
        assert_eq!(*off_count.borrow(), 1);
    }

    #[test]
    fn tecu_enters_maintaining_when_requests_are_fresh_at_min_hold() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        let request = maintain_request_msg();

        pm.key_off();
        pm.update(1_000);
        pm.handle_message(&request);
        pm.update(1_000);

        assert_eq!(pm.state(), PowerState::Maintaining);
    }

    #[test]
    fn tecu_tracks_requesting_clients_and_clears_on_key_on() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        pm.key_off();
        pm.handle_message(&maintain_request_msg()); // source 0x10

        // A second requester via the actuator-power field.
        let mpd = MaintainPowerData {
            maintain_actuator_power: MaintainPowerRequirement::RequirementFor2SecondsMore,
            ..Default::default()
        };
        let msg2 = Message::new(PGN_MAINTAIN_POWER, mpd.encode().to_vec(), 0x22);
        pm.handle_message(&msg2);

        let clients: Vec<_> = pm.requesting_clients().collect();
        assert_eq!(clients, vec![0x10, 0x22]);

        // Key-on (power restored) starts a fresh shutdown cycle.
        pm.key_on();
        assert_eq!(pm.requesting_clients().count(), 0);
    }

    #[test]
    fn tecu_powers_off_when_maintain_requests_expire() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        let request = maintain_request_msg();

        pm.key_off();
        pm.update(1_000);
        pm.handle_message(&request);
        pm.update(1_000);
        assert_eq!(pm.state(), PowerState::Maintaining);

        pm.update(999);
        assert_eq!(pm.state(), PowerState::Maintaining);
        pm.update(1);
        assert_eq!(pm.state(), PowerState::PowerOff);
    }

    #[test]
    fn tecu_forces_power_off_at_max_extension_even_with_requests() {
        let mut pm = PowerManager::new(PowerRole::Tecu);
        let off_count = Rc::new(RefCell::new(0u32));
        let oc = off_count.clone();
        pm.on_power_off.subscribe(move |_| *oc.borrow_mut() += 1);
        let request = maintain_request_msg();

        pm.key_off();
        for _ in 0..(POWER_MAX_EXTENSION_MS / 1_000) {
            pm.handle_message(&request);
            pm.update(1_000);
            if matches!(pm.state(), PowerState::PowerOff) {
                break;
            }
        }

        assert_eq!(pm.state(), PowerState::PowerOff);
        assert_eq!(*off_count.borrow(), 1);
    }

    #[test]
    fn cf_request_power_emits_periodic_broadcast() {
        let mut pm = PowerManager::new(PowerRole::Cf);
        pm.request_power(true);
        let out = pm.update(0); // request_timer_ms primed to repeat interval
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].maintain_ecu_power,
            MaintainPowerRequirement::RequirementFor2SecondsMore
        );
    }

    #[test]
    fn cf_ignores_maintain_power_status_for_key_state() {
        let mut pm = PowerManager::new(PowerRole::Cf);
        let msg = maintain_request_msg();
        pm.handle_message(&msg);
        assert_eq!(pm.state(), PowerState::Running);
    }
}
