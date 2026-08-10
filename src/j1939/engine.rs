//! J1939 engine and powertrain status messages.
//!
//! Mirrors the C++ `machbus::j1939::engine.hpp`. 18 wire-format
//! structs, each with `encode` / `decode`. SPN scaling preserved
//! exactly — see ISO 11783-8 / J1939-71. The C++ `EngineInterface`
//! (IsoNet-coupled) is not ported; users register
//! `IsoNet::register_pgn_callback` and decode directly.

use alloc::{borrow::ToOwned, string::String, vec::Vec};

use crate::isobus::implement::Signal;
use crate::net::message::Message;
use crate::net::pgn_defs::{
    PGN_AMBIENT_CONDITIONS, PGN_AT1, PGN_AT2, PGN_COMPONENT_ID, PGN_DASH_DISPLAY, PGN_EEC1,
    PGN_EEC2, PGN_EEC3, PGN_EFLP, PGN_ENGINE_HOURS, PGN_ET1, PGN_ET2, PGN_FUEL_CONSUMPTION,
    PGN_FUEL_ECONOMY, PGN_TSC1, PGN_VEHICLE_ID, PGN_VEHICLE_POSITION, PGN_VEP1,
};

#[inline]
fn exact8_with_ff_tail(data: &[u8], tail_start: usize) -> bool {
    data.len() == 8 && data[tail_start..].iter().all(|&byte| byte == 0xFF)
}

fn decode_exact_star_fields(raw: &[u8], expected_fields: usize) -> Option<Vec<String>> {
    let mut fields = Vec::with_capacity(expected_fields);
    let mut start = 0usize;
    for (idx, &byte) in raw.iter().enumerate() {
        if byte == b'*' {
            if fields.len() == expected_fields {
                return None;
            }
            let field = core::str::from_utf8(&raw[start..idx]).ok()?.to_owned();
            fields.push(field);
            start = idx + 1;
        } else if !(0x20..=0x7E).contains(&byte) {
            return None;
        }
    }
    if start != raw.len() || fields.len() != expected_fields {
        return None;
    }
    Some(fields)
}

fn scaled_u8_non_na(value: f64, scale: f64) -> u8 {
    if !value.is_finite() {
        return 0;
    }
    let raw = value / scale;
    if raw <= 0.0 {
        0
    } else if raw >= 250.0 {
        250
    } else {
        raw as u8
    }
}

fn offset_scaled_u8_non_na(value: f64, offset: f64, scale: f64) -> u8 {
    scaled_u8_non_na(value + offset, scale)
}

/// Encode a [`Signal`] into a one-byte SLOT, round-tripping the error and
/// not-available indicators rather than flattening them to a number.
fn encode_u8_signal(signal: Signal<f64>, offset: f64) -> u8 {
    match signal {
        Signal::Value(v) => offset_scaled_u8_non_na(v, offset, 1.0),
        Signal::Error => 0xFE,
        Signal::NotAvailable => 0xFF,
    }
}

/// Two-byte equivalent of [`encode_u8_signal`].
fn encode_u16_signal(signal: Signal<f64>, scale: f64) -> u16 {
    match signal {
        Signal::Value(v) => scaled_u16_non_na(v, scale),
        // Both map onto the same not-available raw: see `u16_signal` for why
        // the error band is not carved out of the two-byte range here.
        Signal::Error | Signal::NotAvailable => 0xFFFF,
    }
}

/// Four-byte equivalent of [`u8_signal`].
#[inline]
fn u32_signal(raw: u32, resolution: f64) -> Signal<f64> {
    if raw >= u32::MAX - 1 {
        Signal::NotAvailable
    } else {
        Signal::Value(f64::from(raw) * resolution)
    }
}

/// Four-byte equivalent of [`encode_u16_signal`].
fn encode_u32_signal(signal: Signal<f64>, scale: f64) -> u32 {
    match signal {
        Signal::Value(v) => scaled_u32_non_na(v, scale),
        Signal::Error | Signal::NotAvailable => u32::MAX,
    }
}

/// [`encode_u8_signal`] for a SLOT whose resolution is not 1 per bit.
fn encode_u8_signal_scaled(signal: Signal<f64>, scale: f64) -> u8 {
    match signal {
        Signal::Value(v) => scaled_u8_non_na(v, scale),
        Signal::Error => 0xFE,
        Signal::NotAvailable => 0xFF,
    }
}

/// [`u8_signal`] for a SLOT whose resolution is not 1 per bit.
#[inline]
fn u8_signal_scaled(raw: u8, resolution: f64) -> Signal<f64> {
    u8_signal(raw, resolution, 0.0)
}

/// [`encode_u16_signal`] for a SLOT that carries an offset.
fn encode_u16_signal_offset(signal: Signal<f64>, offset: f64, scale: f64) -> u16 {
    match signal {
        Signal::Value(v) => offset_scaled_u16_non_na(v, offset, scale),
        Signal::Error | Signal::NotAvailable => 0xFFFF,
    }
}

fn scaled_u16_non_na(value: f64, scale: f64) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    let raw = value / scale;
    if raw <= 0.0 {
        0
    } else if raw >= f64::from(u16::MAX - 1) {
        u16::MAX - 2
    } else {
        raw as u16
    }
}

fn offset_scaled_u16_non_na(value: f64, offset: f64, scale: f64) -> u16 {
    scaled_u16_non_na(value + offset, scale)
}

fn scaled_u32_non_na(value: f64, scale: f64) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let raw = value / scale;
    if raw <= 0.0 {
        0
    } else if raw >= f64::from(u32::MAX - 1) {
        u32::MAX - 2
    } else {
        (raw + 0.5) as u32
    }
}

fn offset_scaled_u32_non_na(value: f64, offset: f64, scale: f64) -> u32 {
    scaled_u32_non_na(value + offset, scale)
}

#[inline]
fn u8_scaled_raw_is_defined(raw: u8) -> bool {
    raw <= 250
}

/// Decode a one-byte J1939 SLOT into a [`Signal`], keeping "error" and "not
/// available" distinguishable from a measurement.
///
/// J1939-71 reserves `0xFE` for the error indicator and `0xFF` for
/// not-available, with `0xFB..=0xFD` reserved. Treating any of them as a
/// whole-PG decode failure — as this module used to — makes a faulted sensor
/// indistinguishable from an unplugged bus and discards every *other*,
/// perfectly valid parameter in the same frame (G4).
#[inline]
fn u8_signal(raw: u8, resolution: f64, offset: f64) -> Signal<f64> {
    match raw {
        0xFF => Signal::NotAvailable,
        0xFE => Signal::Error,
        r if r <= 250 => Signal::Value(f64::from(r) * resolution + offset),
        // 0xFB..=0xFD are reserved; nothing meaningful to report.
        _ => Signal::NotAvailable,
    }
}

/// Two-byte equivalent of [`u8_signal`].
///
/// The not-available boundary is the one this crate already used
/// (`u16_data_is_available`: `raw < 0xFFFE`) rather than the full J1939-71 SLOT
/// banding. Narrowing the valid range would need the J1939/71 parameter tables,
/// which are a blocked item (6H) — so this adds the missing *representation* of
/// "absent" without redefining which raws count as a measurement.
#[inline]
fn u16_signal(raw: u16, resolution: f64, offset: f64) -> Signal<f64> {
    if raw >= u16::MAX - 1 {
        Signal::NotAvailable
    } else {
        Signal::Value(f64::from(raw) * resolution + offset)
    }
}

#[inline]
fn u16_data_is_available(raw: u16) -> bool {
    raw < u16::MAX - 1
}

#[inline]
fn u32_data_is_available(raw: u32) -> bool {
    raw < u32::MAX - 1
}

#[inline]
fn u8_percent_raw_is_defined(raw: u8) -> bool {
    raw <= 250 || raw == 0xFE
}

#[inline]
fn u8_percent_raw_is_defined_or_not_available(raw: u8) -> bool {
    raw <= 250 || raw >= 0xFE
}

#[inline]
fn u8_status_raw_is_defined(raw: u8) -> bool {
    raw <= 0x03 || raw == 0xFE
}

// ─── EEC1 — Electronic Engine Controller 1 (PGN 0x0F004) ──────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Eec1 {
    /// SPN 513: −125 to 125 %, offset −125.
    pub engine_torque_percent: Signal<f64>,
    /// SPN 512: −125 to 125 %, offset −125.
    pub driver_demand_percent: Signal<f64>,
    /// SPN 514: −125 to 125 %, offset −125.
    pub actual_engine_percent: Signal<f64>,
    /// SPN 190: 0.125 rpm/bit, 2 bytes.
    pub engine_speed_rpm: Signal<f64>,
    /// SPN 1675: 4 bits.
    pub starter_mode: u8,
    /// SPN 899: source of engine speed.
    pub source_address: u8,
}

impl Eec1 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = encode_u8_signal(self.engine_torque_percent, 125.0);
        data[1] = encode_u8_signal(self.driver_demand_percent, 125.0);
        data[2] = encode_u8_signal(self.actual_engine_percent, 125.0);
        let rpm = encode_u16_signal(self.engine_speed_rpm, 0.125);
        data[3] = (rpm & 0xFF) as u8;
        data[4] = ((rpm >> 8) & 0xFF) as u8;
        data[5] = self.source_address;
        data[6] = self.starter_mode & 0x0F;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 7) || data[6] & 0xF0 != 0 {
            return None;
        }
        // G4: one absent sub-signal must not cost the rest of the PG.
        let rpm = (data[3] as u16) | ((data[4] as u16) << 8);
        Some(Self {
            engine_torque_percent: u8_signal(data[0], 1.0, -125.0),
            driver_demand_percent: u8_signal(data[1], 1.0, -125.0),
            actual_engine_percent: u8_signal(data[2], 1.0, -125.0),
            engine_speed_rpm: u16_signal(rpm, 0.125, 0.0),
            source_address: data[5],
            starter_mode: data[6] & 0x0F,
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_EEC1) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── EEC2 (PGN 0x0F003) ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Eec2 {
    /// SPN 91: 0.4 %/bit. This used to be the raw wire byte; it is now the
    /// pedal position itself, with absence made explicit.
    pub accel_pedal_position: Signal<f64>,
    /// SPN 92: 1 %/bit.
    pub engine_load_percent: Signal<f64>,
    /// SPN 558: 2 bits.
    pub accel_pedal_low_idle: u8,
    /// SPN 559: 2 bits.
    pub accel_pedal_kickdown: u8,
    /// SPN 1437: 1 km/h per bit.
    pub road_speed_limit: Signal<f64>,
}

impl Default for Eec2 {
    /// The two-bit control statuses have their own not-available code (0b11);
    /// leaving them at 0 would claim the pedal is out of low idle and kickdown.
    fn default() -> Self {
        Self {
            accel_pedal_position: Signal::NotAvailable,
            engine_load_percent: Signal::NotAvailable,
            accel_pedal_low_idle: 0x03,
            accel_pedal_kickdown: 0x03,
            road_speed_limit: Signal::NotAvailable,
        }
    }
}

impl Eec2 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.accel_pedal_low_idle & 0x03) | ((self.accel_pedal_kickdown & 0x03) << 2);
        data[1] = encode_u8_signal_scaled(self.accel_pedal_position, 0.4);
        data[2] = encode_u8_signal(self.engine_load_percent, 0.0);
        data[3] = encode_u8_signal(self.road_speed_limit, 0.0);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 4) {
            return None;
        }
        if data[0] & 0xF0 != 0 {
            return None;
        }
        Some(Self {
            accel_pedal_low_idle: data[0] & 0x03,
            accel_pedal_kickdown: (data[0] >> 2) & 0x03,
            accel_pedal_position: u8_signal_scaled(data[1], 0.4),
            engine_load_percent: u8_signal(data[2], 1.0, 0.0),
            road_speed_limit: u8_signal(data[3], 1.0, 0.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_EEC2) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── EEC3 (PGN 0x0FEC0) ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Eec3 {
    /// SPN 514: 1 %/bit, offset −125.
    pub nominal_friction_percent: Signal<f64>,
    /// SPN 515: 0.125 rpm/bit, 2 bytes.
    pub desired_operating_speed_rpm: Signal<f64>,
    /// SPN 519: 1 %/bit.
    pub operating_speed_asymmetry: Signal<f64>,
}

impl Eec3 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = encode_u8_signal(self.nominal_friction_percent, 125.0);
        let spd = encode_u16_signal(self.desired_operating_speed_rpm, 0.125);
        data[1] = (spd & 0xFF) as u8;
        data[2] = ((spd >> 8) & 0xFF) as u8;
        data[3] = encode_u8_signal(self.operating_speed_asymmetry, 0.0);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 4) {
            return None;
        }
        let spd = (data[1] as u16) | ((data[2] as u16) << 8);
        Some(Self {
            nominal_friction_percent: u8_signal(data[0], 1.0, -125.0),
            desired_operating_speed_rpm: u16_signal(spd, 0.125, 0.0),
            operating_speed_asymmetry: u8_signal(data[3], 1.0, 0.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_EEC3) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── EngineTemp1 (ET1, PGN 0x0FEEE) ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineTemp1 {
    pub coolant_temp_c: Signal<f64>,     // SPN 110: 1 °C/bit, offset −40
    pub fuel_temp_c: Signal<f64>,        // SPN 174: 1 °C/bit, offset −40
    pub oil_temp_c: Signal<f64>,         // SPN 175: 0.03125 °C/bit, offset −273, 2 bytes
    pub turbo_oil_temp_c: Signal<f64>,   // SPN 176: 0.03125 °C/bit, offset −273, 2 bytes
    pub intercooler_temp_c: Signal<f64>, // SPN 52: 1 °C/bit, offset −40
}

impl EngineTemp1 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = encode_u8_signal(self.coolant_temp_c, 40.0);
        data[1] = encode_u8_signal(self.fuel_temp_c, 40.0);
        let oil = encode_u16_signal_offset(self.oil_temp_c, 273.0, 0.03125);
        data[2] = (oil & 0xFF) as u8;
        data[3] = ((oil >> 8) & 0xFF) as u8;
        let turbo = encode_u16_signal_offset(self.turbo_oil_temp_c, 273.0, 0.03125);
        data[4] = (turbo & 0xFF) as u8;
        data[5] = ((turbo >> 8) & 0xFF) as u8;
        data[6] = encode_u8_signal(self.intercooler_temp_c, 40.0);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 7) {
            return None;
        }
        let oil = (data[2] as u16) | ((data[3] as u16) << 8);
        let turbo = (data[4] as u16) | ((data[5] as u16) << 8);
        // G4: an engine that does not instrument, say, turbo oil temperature
        // still reports a perfectly good coolant temperature in the same frame.
        Some(Self {
            coolant_temp_c: u8_signal(data[0], 1.0, -40.0),
            fuel_temp_c: u8_signal(data[1], 1.0, -40.0),
            oil_temp_c: u16_signal(oil, 0.03125, -273.0),
            turbo_oil_temp_c: u16_signal(turbo, 0.03125, -273.0),
            intercooler_temp_c: u8_signal(data[6], 1.0, -40.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_ET1) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── EngineTemp2 (ET2, PGN 0x0FEED) ───────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineTemp2 {
    pub engine_oil_temp_c: Signal<f64>,
    pub turbo_oil_temp_c: Signal<f64>,
    pub engine_intercooler_temp_c: Signal<f64>,
    pub turbo_1_temp_c: Signal<f64>,
}

impl EngineTemp2 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let oil = encode_u16_signal_offset(self.engine_oil_temp_c, 273.0, 0.03125);
        data[0] = (oil & 0xFF) as u8;
        data[1] = ((oil >> 8) & 0xFF) as u8;
        let turbo_oil = encode_u16_signal_offset(self.turbo_oil_temp_c, 273.0, 0.03125);
        data[2] = (turbo_oil & 0xFF) as u8;
        data[3] = ((turbo_oil >> 8) & 0xFF) as u8;
        data[4] = encode_u8_signal(self.engine_intercooler_temp_c, 40.0);
        let turbo1 = encode_u16_signal_offset(self.turbo_1_temp_c, 273.0, 0.03125);
        data[5] = (turbo1 & 0xFF) as u8;
        data[6] = ((turbo1 >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 7) {
            return None;
        }
        let oil = (data[0] as u16) | ((data[1] as u16) << 8);
        let turbo_oil = (data[2] as u16) | ((data[3] as u16) << 8);
        let turbo1 = (data[5] as u16) | ((data[6] as u16) << 8);
        Some(Self {
            engine_oil_temp_c: u16_signal(oil, 0.03125, -273.0),
            turbo_oil_temp_c: u16_signal(turbo_oil, 0.03125, -273.0),
            engine_intercooler_temp_c: u8_signal(data[4], 1.0, -40.0),
            turbo_1_temp_c: u16_signal(turbo1, 0.03125, -273.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_ET2) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── EngineFluidLP (PGN 0x0FEEF) ──────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineFluidLp {
    pub oil_pressure_kpa: Signal<f64>,
    pub coolant_pressure_kpa: Signal<f64>,
    /// SPN 98: 0.4 %/bit. This used to be the raw wire byte; it is now the
    /// percentage the field name promises, with absence made explicit.
    pub oil_level_percent: Signal<f64>,
    /// SPN 111: 0.4 %/bit.
    pub coolant_level_percent: Signal<f64>,
    pub fuel_delivery_pressure_kpa: Signal<f64>,
    pub crankcase_pressure_kpa: Signal<f64>,
}

impl EngineFluidLp {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = encode_u8_signal_scaled(self.fuel_delivery_pressure_kpa, 4.0);
        data[1] = encode_u8_signal_scaled(self.oil_pressure_kpa, 4.0);
        data[2] = encode_u8_signal_scaled(self.coolant_pressure_kpa, 2.0);
        data[3] = encode_u8_signal_scaled(self.oil_level_percent, 0.4);
        data[4] = encode_u8_signal_scaled(self.coolant_level_percent, 0.4);
        let crank = encode_u16_signal_offset(self.crankcase_pressure_kpa, 250.0, 0.05);
        data[5] = (crank & 0xFF) as u8;
        data[6] = ((crank >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 7) {
            return None;
        }
        let crank = (data[5] as u16) | ((data[6] as u16) << 8);
        Some(Self {
            fuel_delivery_pressure_kpa: u8_signal_scaled(data[0], 4.0),
            oil_pressure_kpa: u8_signal_scaled(data[1], 4.0),
            coolant_pressure_kpa: u8_signal_scaled(data[2], 2.0),
            oil_level_percent: u8_signal_scaled(data[3], 0.4),
            coolant_level_percent: u8_signal_scaled(data[4], 0.4),
            crankcase_pressure_kpa: u16_signal(crank, 0.05, -250.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_EFLP) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── EngineHours (PGN 0x0FEE5) ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct EngineHours {
    /// SPN 247: 0.05 hr/bit, 4 bytes.
    pub total_hours: Signal<f64>,
    /// SPN 249: 1000 rev/bit, 4 bytes.
    pub total_revolutions: Signal<f64>,
}

impl EngineHours {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let hrs = encode_u32_signal(self.total_hours, 0.05);
        data[0..4].copy_from_slice(&hrs.to_le_bytes());
        let revs = encode_u32_signal(self.total_revolutions, 1000.0);
        data[4..8].copy_from_slice(&revs.to_le_bytes());
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let hrs = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let revs = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        Some(Self {
            total_hours: u32_signal(hrs, 0.05),
            total_revolutions: u32_signal(revs, 1000.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_ENGINE_HOURS) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── FuelEconomy (PGN 0x0FEF2) ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FuelEconomy {
    pub fuel_rate_lph: Signal<f64>,
    pub instantaneous_lph: Signal<f64>,
    pub throttle_position: Signal<f64>,
}

impl FuelEconomy {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let rate = encode_u16_signal(self.fuel_rate_lph, 0.05);
        data[0] = (rate & 0xFF) as u8;
        data[1] = ((rate >> 8) & 0xFF) as u8;
        let inst = encode_u16_signal(self.instantaneous_lph, 1.0 / 512.0);
        data[2] = (inst & 0xFF) as u8;
        data[3] = ((inst >> 8) & 0xFF) as u8;
        data[4] = encode_u8_signal_scaled(self.throttle_position, 0.4);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 5) {
            return None;
        }
        let rate = (data[0] as u16) | ((data[1] as u16) << 8);
        let inst = (data[2] as u16) | ((data[3] as u16) << 8);
        Some(Self {
            fuel_rate_lph: u16_signal(rate, 0.05, 0.0),
            instantaneous_lph: u16_signal(inst, 1.0 / 512.0, 0.0),
            throttle_position: u8_signal_scaled(data[4], 0.4),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_FUEL_ECONOMY) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── TSC1 — Torque/Speed Control 1 (PGN 0x0F006) ──────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum OverrideControlMode {
    #[default]
    NoOverride = 0,
    SpeedControl = 1,
    TorqueControl = 2,
    SpeedTorqueLimit = 3,
}

impl OverrideControlMode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            1 => Self::SpeedControl,
            2 => Self::TorqueControl,
            3 => Self::SpeedTorqueLimit,
            _ => Self::NoOverride,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoOverride),
            1 => Some(Self::SpeedControl),
            2 => Some(Self::TorqueControl),
            3 => Some(Self::SpeedTorqueLimit),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Tsc1 {
    pub override_mode: OverrideControlMode,
    /// Not-available is meaningful here: a request that governs torque only
    /// leaves the speed field at the sentinel, and vice versa.
    pub requested_speed_rpm: Signal<f64>,
    pub requested_torque_percent: Signal<f64>,
}

impl Tsc1 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.override_mode.as_u8() & 0x03;
        let spd = encode_u16_signal(self.requested_speed_rpm, 0.125);
        data[1] = (spd & 0xFF) as u8;
        data[2] = ((spd >> 8) & 0xFF) as u8;
        data[3] = encode_u8_signal(self.requested_torque_percent, 125.0);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 4) {
            return None;
        }
        if data[0] & !0x03 != 0 {
            return None;
        }
        let spd = (data[1] as u16) | ((data[2] as u16) << 8);
        Some(Self {
            override_mode: OverrideControlMode::try_from_u8(data[0])?,
            requested_speed_rpm: u16_signal(spd, 0.125, 0.0),
            requested_torque_percent: u8_signal(data[3], 1.0, -125.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_TSC1) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── VEP1 — Vehicle Electrical Power (PGN 0x0F009) ────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Vep1 {
    pub battery_voltage_v: Signal<f64>,
    pub alternator_current_a: Signal<f64>,
    pub charging_system_voltage_v: Signal<f64>,
    pub key_switch_voltage_v: Signal<f64>,
}

impl Vep1 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let bat = encode_u16_signal(self.battery_voltage_v, 0.05);
        data[0] = (bat & 0xFF) as u8;
        data[1] = ((bat >> 8) & 0xFF) as u8;
        let chrg = encode_u16_signal(self.charging_system_voltage_v, 0.05);
        data[2] = (chrg & 0xFF) as u8;
        data[3] = ((chrg >> 8) & 0xFF) as u8;
        let key = encode_u16_signal(self.key_switch_voltage_v, 0.05);
        data[4] = (key & 0xFF) as u8;
        data[5] = ((key >> 8) & 0xFF) as u8;
        data[6] = encode_u8_signal(self.alternator_current_a, 125.0);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 7) {
            return None;
        }
        let bat = (data[0] as u16) | ((data[1] as u16) << 8);
        let chrg = (data[2] as u16) | ((data[3] as u16) << 8);
        let key = (data[4] as u16) | ((data[5] as u16) << 8);
        Some(Self {
            battery_voltage_v: u16_signal(bat, 0.05, 0.0),
            charging_system_voltage_v: u16_signal(chrg, 0.05, 0.0),
            key_switch_voltage_v: u16_signal(key, 0.05, 0.0),
            alternator_current_a: u8_signal(data[6], 1.0, -125.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_VEP1) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── AmbientConditions (PGN 0x0FEF5) ──────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AmbientConditions {
    pub barometric_pressure_kpa: Signal<f64>,
    pub ambient_air_temp_c: Signal<f64>,
    pub intake_air_temp_c: Signal<f64>,
    pub road_surface_temp_c: Signal<f64>,
}

impl AmbientConditions {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = encode_u8_signal_scaled(self.barometric_pressure_kpa, 0.5);
        let amb = encode_u16_signal_offset(self.ambient_air_temp_c, 273.0, 0.03125);
        data[1] = (amb & 0xFF) as u8;
        data[2] = ((amb >> 8) & 0xFF) as u8;
        data[3] = encode_u8_signal(self.intake_air_temp_c, 40.0);
        let road = encode_u16_signal_offset(self.road_surface_temp_c, 273.0, 0.03125);
        data[4] = (road & 0xFF) as u8;
        data[5] = ((road >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 6) {
            return None;
        }
        let amb = (data[1] as u16) | ((data[2] as u16) << 8);
        let road = (data[4] as u16) | ((data[5] as u16) << 8);
        Some(Self {
            barometric_pressure_kpa: u8_signal_scaled(data[0], 0.5),
            ambient_air_temp_c: u16_signal(amb, 0.03125, -273.0),
            intake_air_temp_c: u8_signal(data[3], 1.0, -40.0),
            road_surface_temp_c: u16_signal(road, 0.03125, -273.0),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_AMBIENT_CONDITIONS) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── DashDisplay (PGN 0x0FEFC) ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DashDisplay {
    pub fuel_level_percent: u8,
    pub washer_fluid_level: u8,
    pub fuel_filter_diff_kpa: f64,
    pub oil_filter_diff_kpa: f64,
    pub cargo_ambient_temp_c: f64,
}

impl Default for DashDisplay {
    fn default() -> Self {
        Self {
            fuel_level_percent: 0xFF,
            washer_fluid_level: 0xFF,
            fuel_filter_diff_kpa: 0.0,
            oil_filter_diff_kpa: 0.0,
            cargo_ambient_temp_c: -40.0,
        }
    }
}

impl DashDisplay {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.washer_fluid_level;
        data[1] = self.fuel_level_percent;
        data[2] = scaled_u8_non_na(self.fuel_filter_diff_kpa, 2.0);
        data[3] = scaled_u8_non_na(self.oil_filter_diff_kpa, 0.5);
        let temp = offset_scaled_u16_non_na(self.cargo_ambient_temp_c, 273.0, 0.03125);
        data[4] = (temp & 0xFF) as u8;
        data[5] = ((temp >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 6) {
            return None;
        }
        let temp = (data[4] as u16) | ((data[5] as u16) << 8);
        if !u8_scaled_raw_is_defined(data[2])
            || !u8_scaled_raw_is_defined(data[3])
            || !u8_percent_raw_is_defined_or_not_available(data[0])
            || !u8_percent_raw_is_defined_or_not_available(data[1])
            || !u16_data_is_available(temp)
        {
            return None;
        }
        Some(Self {
            washer_fluid_level: data[0],
            fuel_level_percent: data[1],
            fuel_filter_diff_kpa: data[2] as f64 * 2.0,
            oil_filter_diff_kpa: data[3] as f64 * 0.5,
            cargo_ambient_temp_c: temp as f64 * 0.03125 - 273.0,
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_DASH_DISPLAY) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── VehiclePosition (J1939 PGN 0x0FEF7) ──────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct VehiclePosition {
    /// SPN 584: 1e-7 deg/bit, offset −210, 4 bytes.
    pub latitude_deg: f64,
    /// SPN 585: 1e-7 deg/bit, offset −210, 4 bytes.
    pub longitude_deg: f64,
}

impl VehiclePosition {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let lat = offset_scaled_u32_non_na(self.latitude_deg, 210.0, 1e-7);
        data[0..4].copy_from_slice(&lat.to_le_bytes());
        let lon = offset_scaled_u32_non_na(self.longitude_deg, 210.0, 1e-7);
        data[4..8].copy_from_slice(&lon.to_le_bytes());
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let lat = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let lon = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if !u32_data_is_available(lat) || !u32_data_is_available(lon) {
            return None;
        }
        Some(Self {
            latitude_deg: lat as f64 * 1e-7 - 210.0,
            longitude_deg: lon as f64 * 1e-7 - 210.0,
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_VEHICLE_POSITION) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── FuelConsumption (PGN 0x0FEE9) ────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct FuelConsumption {
    pub trip_fuel_l: f64,
    pub total_fuel_l: f64,
}

impl FuelConsumption {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let trip = scaled_u32_non_na(self.trip_fuel_l, 0.5);
        data[0..4].copy_from_slice(&trip.to_le_bytes());
        let total = scaled_u32_non_na(self.total_fuel_l, 0.5);
        data[4..8].copy_from_slice(&total.to_le_bytes());
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let trip = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let total = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        if !u32_data_is_available(trip) || !u32_data_is_available(total) {
            return None;
        }
        Some(Self {
            trip_fuel_l: trip as f64 * 0.5,
            total_fuel_l: total as f64 * 0.5,
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_FUEL_CONSUMPTION) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── Aftertreatment 1 (AT1) ───────────────────────────────────────────
// Repo-owned combined aftertreatment-1 snapshot (DEF tank level + intake/
// outlet NOx). Keyed by `PGN_AT1`, distinct from Ambient Conditions.

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Aftertreatment1 {
    /// SPN 1761: 0.4 % per bit.
    pub def_tank_level: f64,
    /// SPN 3216: 0.05 ppm per bit, 2 bytes.
    pub intake_nox_ppm: f64,
    /// SPN 3226: 0.05 ppm per bit, 2 bytes.
    pub outlet_nox_ppm: f64,
    pub intake_nox_reading_status: u8,
    pub outlet_nox_reading_status: u8,
}

impl Aftertreatment1 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = scaled_u8_non_na(self.def_tank_level, 0.4);
        let intake = scaled_u16_non_na(self.intake_nox_ppm, 0.05);
        data[1] = (intake & 0xFF) as u8;
        data[2] = ((intake >> 8) & 0xFF) as u8;
        let outlet = scaled_u16_non_na(self.outlet_nox_ppm, 0.05);
        data[3] = (outlet & 0xFF) as u8;
        data[4] = ((outlet >> 8) & 0xFF) as u8;
        data[5] = self.intake_nox_reading_status;
        data[6] = self.outlet_nox_reading_status;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 7) {
            return None;
        }
        let intake = (data[1] as u16) | ((data[2] as u16) << 8);
        let outlet = (data[3] as u16) | ((data[4] as u16) << 8);
        if !u8_percent_raw_is_defined(data[0])
            || !u16_data_is_available(intake)
            || !u16_data_is_available(outlet)
            || !u8_status_raw_is_defined(data[5])
            || !u8_status_raw_is_defined(data[6])
        {
            return None;
        }
        Some(Self {
            def_tank_level: data[0] as f64 * 0.4,
            intake_nox_ppm: intake as f64 * 0.05,
            outlet_nox_ppm: outlet as f64 * 0.05,
            intake_nox_reading_status: data[5],
            outlet_nox_reading_status: data[6],
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_AT1) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── Aftertreatment 2 (AT2, PGN 65110) ────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Aftertreatment2 {
    pub dpf_differential_pressure_kpa: f64,
    pub def_concentration: f64,
    pub dpf_soot_load_percent: f64,
    pub dpf_active_regeneration_status: u8,
    pub dpf_passive_regeneration_status: u8,
}

impl Aftertreatment2 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let diff = scaled_u16_non_na(self.dpf_differential_pressure_kpa, 0.1);
        data[0] = (diff & 0xFF) as u8;
        data[1] = ((diff >> 8) & 0xFF) as u8;
        data[2] = scaled_u8_non_na(self.def_concentration, 0.4);
        data[3] = scaled_u8_non_na(self.dpf_soot_load_percent, 0.4);
        data[4] = self.dpf_active_regeneration_status;
        data[5] = self.dpf_passive_regeneration_status;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !exact8_with_ff_tail(data, 6) {
            return None;
        }
        let diff = (data[0] as u16) | ((data[1] as u16) << 8);
        if !u16_data_is_available(diff)
            || !u8_percent_raw_is_defined(data[2])
            || !u8_percent_raw_is_defined(data[3])
        {
            return None;
        }
        Some(Self {
            dpf_differential_pressure_kpa: diff as f64 * 0.1,
            def_concentration: data[2] as f64 * 0.4,
            dpf_soot_load_percent: data[3] as f64 * 0.4,
            dpf_active_regeneration_status: data[4],
            dpf_passive_regeneration_status: data[5],
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_AT2) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── ComponentIdentification (PGN 0x0FEEB) ─────────────────────────────

/// `*`-delimited: `Make*Model*SerialNumber*UnitNumber*`. Multi-frame
/// (TP) when encoded.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComponentIdentification {
    pub make: String,
    pub model: String,
    pub serial_number: String,
    pub unit_number: String,
}

impl ComponentIdentification {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for f in [
            &self.make,
            &self.model,
            &self.serial_number,
            &self.unit_number,
        ] {
            data.extend_from_slice(f.as_bytes());
            data.push(b'*');
        }
        data
    }

    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        let fields = decode_exact_star_fields(raw, 4)?;
        Some(Self {
            make: fields[0].clone(),
            model: fields[1].clone(),
            serial_number: fields[2].clone(),
            unit_number: fields[3].clone(),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_COMPONENT_ID) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── VehicleIdentification (PGN 0x0FEEC) ──────────────────────────────

/// `*`-terminated VIN string.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VehicleIdentification {
    pub vin: String,
}

impl VehicleIdentification {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = self.vin.as_bytes().to_vec();
        data.push(b'*');
        data
    }

    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        let fields = decode_exact_star_fields(raw, 1)?;
        Some(Self {
            vin: fields[0].clone(),
        })
    }

    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_VEHICLE_ID) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHORT_7: [u8; 7] = [0; 7];

    #[test]
    fn eec1_round_trip() {
        let m = Eec1 {
            engine_torque_percent: Signal::Value(50.0),
            driver_demand_percent: Signal::Value(75.0),
            actual_engine_percent: Signal::Value(45.0),
            engine_speed_rpm: Signal::Value(1500.0),
            starter_mode: 0x05,
            source_address: 0x10,
        };
        assert_eq!(m.encode(), [0xAF, 0xC8, 0xAA, 0xE0, 0x2E, 0x10, 0x05, 0xFF]);
        let decoded = Eec1::decode(&m.encode()).unwrap();
        assert!(
            decoded
                .engine_torque_percent
                .value()
                .is_some_and(|v| (v - 50.0).abs() < 1.0)
        );
        assert!(
            decoded
                .engine_speed_rpm
                .value()
                .is_some_and(|v| (v - 1500.0).abs() < 0.125)
        );
        assert_eq!(decoded.starter_mode, 0x05);
        assert_eq!(decoded.source_address, 0x10);
    }

    /// H54 — one absent sub-signal used to drop the whole PG, so a single
    /// faulted sensor made every other parameter in the frame unreadable and a
    /// faulted sensor was indistinguishable from an unplugged bus (G4).
    #[test]
    fn eec1_keeps_the_frame_when_one_parameter_is_absent() {
        let mut bytes = Eec1 {
            engine_torque_percent: Signal::Value(50.0),
            driver_demand_percent: Signal::Value(75.0),
            actual_engine_percent: Signal::Value(45.0),
            engine_speed_rpm: Signal::Value(1500.0),
            starter_mode: 0x05,
            source_address: 0x10,
        }
        .encode();

        // The engine reports torque as *faulted* and speed as not provided.
        bytes[0] = 0xFE;
        bytes[3] = 0xFF;
        bytes[4] = 0xFF;

        let decoded = Eec1::decode(&bytes).expect("the frame still decodes");
        assert_eq!(decoded.engine_torque_percent, Signal::Error);
        assert_eq!(decoded.engine_speed_rpm, Signal::NotAvailable);
        // The parameters that *were* reported survive.
        assert!(
            decoded
                .driver_demand_percent
                .value()
                .is_some_and(|v| (v - 75.0).abs() < 1.0)
        );
        assert_eq!(decoded.source_address, 0x10);

        // And both indicators round-trip rather than flattening to a number.
        let reencoded = decoded.encode();
        assert_eq!(reencoded[0], 0xFE, "error stays error");
        assert_eq!(
            Eec1::decode(&reencoded).unwrap().engine_speed_rpm,
            Signal::NotAvailable
        );
    }

    #[test]
    fn eec1_rejects_reserved_starter_mode_nibble() {
        let mut bytes = Eec1::default().encode();
        bytes[6] |= 0xF0;
        assert!(Eec1::decode(&bytes).is_none());
    }

    #[test]
    fn eec2_round_trip() {
        // Raw 200 at the SPN 91 resolution of 0.4 %/bit is 80 % pedal.
        let m = Eec2 {
            accel_pedal_position: Signal::Value(80.0),
            engine_load_percent: Signal::Value(65.0),
            accel_pedal_low_idle: 1,
            accel_pedal_kickdown: 0,
            road_speed_limit: Signal::Value(80.0),
        };
        assert_eq!(m.encode()[1], 200);
        let decoded = Eec2::decode(&m.encode()).unwrap();
        assert!((decoded.accel_pedal_position.value().unwrap() - 80.0).abs() < 0.4);
        assert_eq!(decoded.road_speed_limit, Signal::Value(80.0));

        // No road speed limiter fitted still leaves a readable pedal position.
        let mut absent = m.encode();
        absent[3] = 0xFF;
        let decoded = Eec2::decode(&absent).unwrap();
        assert_eq!(decoded.road_speed_limit, Signal::NotAvailable);
        assert!((decoded.accel_pedal_position.value().unwrap() - 80.0).abs() < 0.4);
    }

    #[test]
    fn engine_temp1_round_trip() {
        let m = EngineTemp1 {
            coolant_temp_c: Signal::Value(90.0),
            fuel_temp_c: Signal::Value(50.0),
            oil_temp_c: Signal::Value(100.0),
            turbo_oil_temp_c: Signal::Value(110.0),
            intercooler_temp_c: Signal::Value(60.0),
        };
        assert_eq!(m.encode(), [0x82, 0x5A, 0xA0, 0x2E, 0xE0, 0x2F, 0x64, 0xFF]);
        let decoded = EngineTemp1::decode(&m.encode()).unwrap();
        assert!((decoded.coolant_temp_c.value().unwrap() - 90.0).abs() < 1.0);
        assert!((decoded.oil_temp_c.value().unwrap() - 100.0).abs() < 0.1);

        // An uninstrumented turbo oil sensor must not cost the coolant reading.
        let mut absent = m.encode();
        absent[4] = 0xFF;
        absent[5] = 0xFF;
        let decoded = EngineTemp1::decode(&absent).unwrap();
        assert_eq!(decoded.turbo_oil_temp_c, Signal::NotAvailable);
        assert!((decoded.coolant_temp_c.value().unwrap() - 90.0).abs() < 1.0);
    }

    #[test]
    fn engine_temp2_round_trip() {
        let m = EngineTemp2 {
            engine_oil_temp_c: Signal::Value(95.0),
            turbo_oil_temp_c: Signal::Value(105.0),
            engine_intercooler_temp_c: Signal::Value(55.0),
            turbo_1_temp_c: Signal::Value(200.0),
        };
        let d = EngineTemp2::decode(&m.encode()).unwrap();
        assert!((d.engine_oil_temp_c.value().unwrap() - 95.0).abs() < 0.1);
        assert!((d.turbo_1_temp_c.value().unwrap() - 200.0).abs() < 0.1);

        // A default ET2 is "nothing reported", and it survives a round trip.
        let empty = EngineTemp2::decode(&EngineTemp2::default().encode()).unwrap();
        assert_eq!(empty, EngineTemp2::default());
    }

    #[test]
    fn engine_fluid_lp_round_trip() {
        let m = EngineFluidLp {
            oil_pressure_kpa: Signal::Value(400.0),
            coolant_pressure_kpa: Signal::Value(200.0),
            oil_level_percent: Signal::Value(80.0),
            coolant_level_percent: Signal::Value(88.0),
            fuel_delivery_pressure_kpa: Signal::Value(300.0),
            crankcase_pressure_kpa: Signal::Value(0.5),
        };
        let d = EngineFluidLp::decode(&m.encode()).unwrap();
        assert!((d.oil_level_percent.value().unwrap() - 80.0).abs() < 0.4);
        assert!((d.oil_pressure_kpa.value().unwrap() - 400.0).abs() < 4.0);

        // An engine without a crankcase pressure sensor still reports oil.
        let mut absent = m.encode();
        absent[5] = 0xFF;
        absent[6] = 0xFF;
        let d = EngineFluidLp::decode(&absent).unwrap();
        assert_eq!(d.crankcase_pressure_kpa, Signal::NotAvailable);
        assert!((d.oil_pressure_kpa.value().unwrap() - 400.0).abs() < 4.0);
    }

    #[test]
    fn engine_hours_round_trip() {
        let m = EngineHours {
            total_hours: Signal::Value(12_345.7),
            total_revolutions: Signal::Value(1_000_000_000.0),
        };
        assert_eq!(m.encode(), [0x82, 0xC4, 0x03, 0x00, 0x40, 0x42, 0x0F, 0x00]);
        let d = EngineHours::decode(&m.encode()).unwrap();
        assert!((d.total_hours.value().unwrap() - 12_345.7).abs() < 0.1);
        assert!((d.total_revolutions.value().unwrap() - 1_000_000_000.0).abs() < 1000.0);

        // An ECU that counts hours but not revolutions is a valid frame.
        let mut absent = m.encode();
        absent[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let d = EngineHours::decode(&absent).unwrap();
        assert_eq!(d.total_revolutions, Signal::NotAvailable);
        assert!((d.total_hours.value().unwrap() - 12_345.7).abs() < 0.1);
    }

    #[test]
    fn fuel_economy_round_trip() {
        let m = FuelEconomy {
            fuel_rate_lph: Signal::Value(25.0),
            instantaneous_lph: Signal::Value(6.5),
            throttle_position: Signal::Value(80.0),
        };
        assert_eq!(m.encode(), [0xF4, 0x01, 0x00, 0x0D, 0xC8, 0xFF, 0xFF, 0xFF]);
        let d = FuelEconomy::decode(&m.encode()).unwrap();
        assert!((d.fuel_rate_lph.value().unwrap() - 25.0).abs() < 0.1);
        assert!((d.throttle_position.value().unwrap() - 80.0).abs() < 0.5);
    }

    #[test]
    fn eec3_round_trip() {
        let m = Eec3 {
            nominal_friction_percent: Signal::Value(25.0),
            desired_operating_speed_rpm: Signal::Value(1800.0),
            operating_speed_asymmetry: Signal::Value(50.0),
        };
        let d = Eec3::decode(&m.encode()).unwrap();
        assert!((d.nominal_friction_percent.value().unwrap() - 25.0).abs() < 1.0);
        assert_eq!(d.operating_speed_asymmetry, Signal::Value(50.0));
    }

    #[test]
    fn tsc1_round_trip() {
        let m = Tsc1 {
            override_mode: OverrideControlMode::SpeedControl,
            requested_speed_rpm: Signal::Value(1200.0),
            requested_torque_percent: Signal::Value(50.0),
        };
        assert_eq!(m.encode(), [0x01, 0x80, 0x25, 0xAF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let d = Tsc1::decode(&m.encode()).unwrap();
        assert_eq!(d.override_mode, OverrideControlMode::SpeedControl);
        assert!((d.requested_speed_rpm.value().unwrap() - 1200.0).abs() < 0.125);

        // A speed-only request leaves torque at the sentinel, and that is a
        // valid command — not a frame to throw away.
        let speed_only = Tsc1 {
            requested_torque_percent: Signal::NotAvailable,
            ..m
        };
        let d = Tsc1::decode(&speed_only.encode()).unwrap();
        assert_eq!(d.requested_torque_percent, Signal::NotAvailable);
        assert!((d.requested_speed_rpm.value().unwrap() - 1200.0).abs() < 0.125);
    }

    #[test]
    fn vep1_round_trip() {
        let m = Vep1 {
            battery_voltage_v: Signal::Value(12.5),
            alternator_current_a: Signal::Value(50.0),
            charging_system_voltage_v: Signal::Value(14.2),
            key_switch_voltage_v: Signal::Value(12.4),
        };
        let d = Vep1::decode(&m.encode()).unwrap();
        assert!((d.battery_voltage_v.value().unwrap() - 12.5).abs() < 0.05);
        assert!((d.alternator_current_a.value().unwrap() - 50.0).abs() < 1.0);

        // No alternator fitted still leaves a readable battery voltage.
        let mut absent = m.encode();
        absent[6] = 0xFF;
        let d = Vep1::decode(&absent).unwrap();
        assert_eq!(d.alternator_current_a, Signal::NotAvailable);
        assert!((d.battery_voltage_v.value().unwrap() - 12.5).abs() < 0.05);
    }

    #[test]
    fn ambient_conditions_round_trip() {
        let m = AmbientConditions {
            barometric_pressure_kpa: Signal::Value(101.3),
            ambient_air_temp_c: Signal::Value(25.0),
            intake_air_temp_c: Signal::Value(30.0),
            road_surface_temp_c: Signal::Value(22.0),
        };
        let d = AmbientConditions::decode(&m.encode()).unwrap();
        assert!((d.ambient_air_temp_c.value().unwrap() - 25.0).abs() < 0.1);

        // Road surface temperature is rarely fitted; the rest must survive it.
        let mut absent = m.encode();
        absent[4] = 0xFF;
        absent[5] = 0xFF;
        let d = AmbientConditions::decode(&absent).unwrap();
        assert_eq!(d.road_surface_temp_c, Signal::NotAvailable);
        assert!((d.ambient_air_temp_c.value().unwrap() - 25.0).abs() < 0.1);
    }

    #[test]
    fn dash_display_round_trip() {
        let m = DashDisplay {
            fuel_level_percent: 200,
            washer_fluid_level: 180,
            fuel_filter_diff_kpa: 50.0,
            oil_filter_diff_kpa: 25.0,
            cargo_ambient_temp_c: 20.0,
        };
        let d = DashDisplay::decode(&m.encode()).unwrap();
        assert_eq!(d.fuel_level_percent, 200);
        assert!((d.cargo_ambient_temp_c - 20.0).abs() < 0.1);
    }

    #[test]
    fn vehicle_position_round_trip() {
        let m = VehiclePosition {
            latitude_deg: 52.3676,
            longitude_deg: 4.9041,
        };
        let d = VehiclePosition::decode(&m.encode()).unwrap();
        assert!((d.latitude_deg - 52.3676).abs() < 1e-6);
        assert!((d.longitude_deg - 4.9041).abs() < 1e-6);
    }

    #[test]
    fn fuel_consumption_round_trip() {
        let m = FuelConsumption {
            trip_fuel_l: 250.5,
            total_fuel_l: 12_345.0,
        };
        let d = FuelConsumption::decode(&m.encode()).unwrap();
        assert!((d.trip_fuel_l - 250.5).abs() < 0.5);
        assert!((d.total_fuel_l - 12_345.0).abs() < 0.5);
    }

    #[test]
    fn aftertreatment1_round_trip() {
        let m = Aftertreatment1 {
            def_tank_level: 75.0,
            intake_nox_ppm: 1500.0,
            outlet_nox_ppm: 50.0,
            intake_nox_reading_status: 1,
            outlet_nox_reading_status: 1,
        };
        let d = Aftertreatment1::decode(&m.encode()).unwrap();
        assert!((d.def_tank_level - 75.0).abs() < 0.5);
        assert!((d.intake_nox_ppm - 1500.0).abs() < 0.05);
    }

    #[test]
    fn aftertreatment2_round_trip() {
        let m = Aftertreatment2 {
            dpf_differential_pressure_kpa: 5.5,
            def_concentration: 32.5,
            dpf_soot_load_percent: 75.0,
            dpf_active_regeneration_status: 2,
            dpf_passive_regeneration_status: 1,
        };
        let d = Aftertreatment2::decode(&m.encode()).unwrap();
        assert!((d.dpf_differential_pressure_kpa - 5.5).abs() < 0.1);
        assert_eq!(d.dpf_active_regeneration_status, 2);
    }

    #[test]
    fn component_identification_round_trip() {
        let id = ComponentIdentification {
            make: "Acme".into(),
            model: "X1000".into(),
            serial_number: "SN-001".into(),
            unit_number: "U-42".into(),
        };
        let bytes = id.encode();
        assert_eq!(bytes.iter().filter(|b| **b == b'*').count(), 4);
        assert_eq!(ComponentIdentification::decode(&bytes), Some(id));
    }

    #[test]
    fn vehicle_identification_round_trip() {
        let v = VehicleIdentification {
            vin: "1HGBH41JXMN109186".into(),
        };
        let d = VehicleIdentification::decode(&v.encode());
        assert_eq!(d, Some(v));
    }

    #[test]
    fn identification_rejects_malformed_delimited_payloads() {
        assert!(ComponentIdentification::decode(b"Acme*X1000*SN-001*").is_none());
        assert!(ComponentIdentification::decode(b"Acme*X1000*SN-001*U-42*EXTRA*").is_none());
        assert!(ComponentIdentification::decode(b"Acme*X1000*SN-\xFF*U-42*").is_none());

        assert!(VehicleIdentification::decode(b"1HGBH41JXMN109186").is_none());
        assert!(VehicleIdentification::decode(b"1HGBH41JXMN109186*\xFF").is_none());
        assert!(VehicleIdentification::decode(b"1HGBH41JXMN109186\x80*").is_none());
    }

    #[test]
    fn short_payload_returns_none() {
        assert!(Eec1::decode(&SHORT_7).is_none());
        assert!(Eec2::decode(&SHORT_7).is_none());
        assert!(Eec3::decode(&SHORT_7).is_none());
        assert!(EngineTemp1::decode(&SHORT_7).is_none());
        assert!(EngineTemp2::decode(&SHORT_7).is_none());
        assert!(EngineFluidLp::decode(&SHORT_7).is_none());
        assert!(EngineHours::decode(&SHORT_7).is_none());
        assert!(FuelEconomy::decode(&SHORT_7).is_none());
        assert!(Tsc1::decode(&SHORT_7).is_none());
        assert!(Vep1::decode(&SHORT_7).is_none());
        assert!(AmbientConditions::decode(&SHORT_7).is_none());
        assert!(DashDisplay::decode(&SHORT_7).is_none());
        assert!(VehiclePosition::decode(&SHORT_7).is_none());
        assert!(FuelConsumption::decode(&SHORT_7).is_none());
        assert!(Aftertreatment1::decode(&SHORT_7).is_none());
        assert!(Aftertreatment2::decode(&SHORT_7).is_none());
    }

    #[test]
    fn overlong_payload_returns_none() {
        const LONG_9: [u8; 9] = [0u8; 9];
        assert!(Eec1::decode(&LONG_9).is_none());
        assert!(Eec2::decode(&LONG_9).is_none());
        assert!(Eec3::decode(&LONG_9).is_none());
        assert!(EngineTemp1::decode(&LONG_9).is_none());
        assert!(EngineTemp2::decode(&LONG_9).is_none());
        assert!(EngineFluidLp::decode(&LONG_9).is_none());
        assert!(EngineHours::decode(&LONG_9).is_none());
        assert!(FuelEconomy::decode(&LONG_9).is_none());
        assert!(Tsc1::decode(&LONG_9).is_none());
        assert!(Vep1::decode(&LONG_9).is_none());
        assert!(AmbientConditions::decode(&LONG_9).is_none());
        assert!(DashDisplay::decode(&LONG_9).is_none());
        assert!(VehiclePosition::decode(&LONG_9).is_none());
        assert!(FuelConsumption::decode(&LONG_9).is_none());
        assert!(Aftertreatment1::decode(&LONG_9).is_none());
        assert!(Aftertreatment2::decode(&LONG_9).is_none());
    }

    use proptest::prelude::*;

    macro_rules! assert_fixed_decoder_is_canonical {
        ($ty:ty, $data:expr) => {
            if let Some(decoded) = <$ty>::decode($data) {
                let encoded = decoded.encode();
                let decoded_again =
                    <$ty>::decode(&encoded).expect("canonical re-encode must remain decodable");
                prop_assert_eq!(decoded_again.encode(), encoded);
            }
        };
    }

    macro_rules! assert_fixed_decoder_seed_is_canonical {
        ($ty:ty, $data:expr) => {
            if let Some(decoded) = <$ty>::decode($data) {
                let encoded = decoded.encode();
                let decoded_again =
                    <$ty>::decode(&encoded).expect("canonical re-encode must remain decodable");
                assert_eq!(decoded_again.encode(), encoded);
            }
        };
    }

    #[test]
    fn regression_engine_proptest_seed_7307e9c00f46f039_is_canonical() {
        let data = [0, 0, 0, 0, 222, 214, 68, 15];
        assert_fixed_decoder_seed_is_canonical!(Eec1, &data);
        assert_fixed_decoder_seed_is_canonical!(Eec2, &data);
        assert_fixed_decoder_seed_is_canonical!(Eec3, &data);
        assert_fixed_decoder_seed_is_canonical!(EngineTemp1, &data);
        assert_fixed_decoder_seed_is_canonical!(EngineTemp2, &data);
        assert_fixed_decoder_seed_is_canonical!(EngineFluidLp, &data);
        assert_fixed_decoder_seed_is_canonical!(EngineHours, &data);
        assert_fixed_decoder_seed_is_canonical!(FuelEconomy, &data);
        assert_fixed_decoder_seed_is_canonical!(Tsc1, &data);
        assert_fixed_decoder_seed_is_canonical!(Vep1, &data);
        assert_fixed_decoder_seed_is_canonical!(AmbientConditions, &data);
        assert_fixed_decoder_seed_is_canonical!(DashDisplay, &data);
        assert_fixed_decoder_seed_is_canonical!(VehiclePosition, &data);
        assert_fixed_decoder_seed_is_canonical!(FuelConsumption, &data);
        assert_fixed_decoder_seed_is_canonical!(Aftertreatment1, &data);
        assert_fixed_decoder_seed_is_canonical!(Aftertreatment2, &data);
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            failure_persistence: None,
            .. ProptestConfig::default()
        })]

        #[test]
        fn proptest_engine_fixed_size_decoders_accept_or_reject_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=64),
        ) {
            assert_fixed_decoder_is_canonical!(Eec1, &data);
            assert_fixed_decoder_is_canonical!(Eec2, &data);
            assert_fixed_decoder_is_canonical!(Eec3, &data);
            assert_fixed_decoder_is_canonical!(EngineTemp1, &data);
            assert_fixed_decoder_is_canonical!(EngineTemp2, &data);
            assert_fixed_decoder_is_canonical!(EngineFluidLp, &data);
            assert_fixed_decoder_is_canonical!(EngineHours, &data);
            assert_fixed_decoder_is_canonical!(FuelEconomy, &data);
            assert_fixed_decoder_is_canonical!(Tsc1, &data);
            assert_fixed_decoder_is_canonical!(Vep1, &data);
            assert_fixed_decoder_is_canonical!(AmbientConditions, &data);
            assert_fixed_decoder_is_canonical!(DashDisplay, &data);
            assert_fixed_decoder_is_canonical!(VehiclePosition, &data);
            assert_fixed_decoder_is_canonical!(FuelConsumption, &data);
            assert_fixed_decoder_is_canonical!(Aftertreatment1, &data);
            assert_fixed_decoder_is_canonical!(Aftertreatment2, &data);
        }

        #[test]
        fn proptest_engine_identification_decoders_accept_or_reject_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=256),
        ) {
            if let Some(decoded) = ComponentIdentification::decode(&data) {
                prop_assert_eq!(
                    ComponentIdentification::decode(&decoded.encode()),
                    Some(decoded)
                );
            }

            if let Some(decoded) = VehicleIdentification::decode(&data) {
                prop_assert_eq!(
                    VehicleIdentification::decode(&decoded.encode()),
                    Some(decoded)
                );
            }
        }
    }
}
