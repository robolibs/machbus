use alloc::{borrow::ToOwned, format, string::String, vec, vec::Vec};

use crate::j1939::pgn_request::{decode_request, encode_request};
use crate::j1939::text::{decode_iso11783_text_field, encode_iso11783_text_field};
use crate::net::error::{Error, Result};
use crate::net::pgn_defs::PGN_VEHICLE_ID;

const MAX_SPN_19: u32 = 0x7_FFFF;

#[inline]
#[must_use]
const fn clamp_spn_19(spn: u32) -> u32 {
    if spn > MAX_SPN_19 { MAX_SPN_19 } else { spn }
}

// ─── Lamp status / flash ───────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum LampStatus {
    #[default]
    Off = 0,
    On = 1,
    Error = 2,
    NotAvailable = 3,
}

impl LampStatus {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Off,
            1 => Self::On,
            2 => Self::Error,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Off),
            1 => Some(Self::On),
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
pub enum LampFlash {
    SlowFlash = 0,
    FastFlash = 1,
    #[default]
    Off = 2,
    NotAvailable = 3,
}

impl LampFlash {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::SlowFlash,
            1 => Self::FastFlash,
            2 => Self::Off,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::SlowFlash),
            1 => Some(Self::FastFlash),
            2 => Some(Self::Off),
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

// ─── FMI (J1939-73 Annex C, Table C-1) ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Fmi {
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
    #[default]
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

impl Fmi {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x1F {
            0 => Self::AboveNormal,
            1 => Self::BelowNormal,
            2 => Self::Erratic,
            3 => Self::VoltageHigh,
            4 => Self::VoltageLow,
            5 => Self::CurrentLow,
            6 => Self::CurrentHigh,
            7 => Self::MechanicalFail,
            8 => Self::AbnormalFrequency,
            9 => Self::AbnormalUpdate,
            10 => Self::AbnormalRateChange,
            12 => Self::BadDevice,
            13 => Self::OutOfCalibration,
            14 => Self::SpecialInstructions,
            15 => Self::AboveNormalLeast,
            16 => Self::AboveNormalModerate,
            17 => Self::BelowNormalLeast,
            18 => Self::BelowNormalModerate,
            19 => Self::ReceivedNetworkData,
            20 => Self::DataDriftedHigh,
            21 => Self::DataDriftedLow,
            31 => Self::ConditionExists,
            _ => Self::RootCauseUnknown,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::AboveNormal),
            1 => Some(Self::BelowNormal),
            2 => Some(Self::Erratic),
            3 => Some(Self::VoltageHigh),
            4 => Some(Self::VoltageLow),
            5 => Some(Self::CurrentLow),
            6 => Some(Self::CurrentHigh),
            7 => Some(Self::MechanicalFail),
            8 => Some(Self::AbnormalFrequency),
            9 => Some(Self::AbnormalUpdate),
            10 => Some(Self::AbnormalRateChange),
            11 => Some(Self::RootCauseUnknown),
            12 => Some(Self::BadDevice),
            13 => Some(Self::OutOfCalibration),
            14 => Some(Self::SpecialInstructions),
            15 => Some(Self::AboveNormalLeast),
            16 => Some(Self::AboveNormalModerate),
            17 => Some(Self::BelowNormalLeast),
            18 => Some(Self::BelowNormalModerate),
            19 => Some(Self::ReceivedNetworkData),
            20 => Some(Self::DataDriftedHigh),
            21 => Some(Self::DataDriftedLow),
            31 => Some(Self::ConditionExists),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ─── Diagnostic Trouble Code ───────────────────────────────────────────

/// 4-byte DTC: 19-bit SPN, 5-bit FMI, 7-bit occurrence count, 1 reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dtc {
    /// Suspect Parameter Number (19 bits).
    pub spn: u32,
    pub fmi: Fmi,
    pub occurrence_count: u8,
}

/// SPN base for an ISO 11783-5 address-violation diagnostic: the reported SPN
/// is this base plus the conflicting source address.
pub const ADDRESS_VIOLATION_SPN_BASE: u32 = 2000;

impl Dtc {
    /// The diagnostic trouble code a control function activates when it detects
    /// an address violation on `source_address` (ISO 11783-5): SPN = 2000 + SA,
    /// FMI = Condition Exists (31).
    #[must_use]
    pub fn address_violation(source_address: u8) -> Self {
        Self {
            spn: ADDRESS_VIOLATION_SPN_BASE + source_address as u32,
            fmi: Fmi::ConditionExists,
            occurrence_count: 1,
        }
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 4] {
        let mut bytes = [0u8; 4];
        let spn = clamp_spn_19(self.spn);
        bytes[0] = (spn & 0xFF) as u8;
        bytes[1] = ((spn >> 8) & 0xFF) as u8;
        bytes[2] = (((spn >> 16) & 0x07) << 5) as u8 | (self.fmi.as_u8() & 0x1F);
        bytes[3] = self.occurrence_count & 0x7F;
        bytes
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 4 {
            return None;
        }
        if data[3] & 0x80 != 0 {
            return None;
        }
        Some(Self {
            spn: (data[0] as u32)
                | ((data[1] as u32) << 8)
                | (((data[2] >> 5) & 0x07) as u32) << 16,
            fmi: Fmi::try_from_u8(data[2] & 0x1F)?,
            occurrence_count: data[3] & 0x7F,
        })
    }

    /// Two DTCs are equal-by-key iff (spn, fmi) match — ignoring
    /// occurrence count. Use [`PartialEq`] for full structural equality.
    #[must_use]
    pub fn matches(&self, other: &Self) -> bool {
        self.spn == other.spn && self.fmi == other.fmi
    }
}

/// Tracks a DTC's occurrence count after it has been cleared from the
/// active list (used by the C++ `previously_active_dtcs_` registry).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PreviouslyActiveDtc {
    pub dtc: Dtc,
    pub occurrence_count: u8,
}

// ─── Lamp status block (2 bytes, used by DM1/2/3/6/12/23) ─────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiagnosticLamps {
    pub malfunction: LampStatus,
    pub malfunction_flash: LampFlash,
    pub red_stop: LampStatus,
    pub red_stop_flash: LampFlash,
    pub amber_warning: LampStatus,
    pub amber_warning_flash: LampFlash,
    pub engine_protect: LampStatus,
    pub engine_protect_flash: LampFlash,
}

impl DiagnosticLamps {
    #[must_use]
    pub fn encode(&self) -> [u8; 2] {
        [
            (self.engine_protect.as_u8() << 6)
                | (self.amber_warning.as_u8() << 4)
                | (self.red_stop.as_u8() << 2)
                | self.malfunction.as_u8(),
            (self.engine_protect_flash.as_u8() << 6)
                | (self.amber_warning_flash.as_u8() << 4)
                | (self.red_stop_flash.as_u8() << 2)
                | self.malfunction_flash.as_u8(),
        ]
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 2 {
            return None;
        }
        Some(Self {
            malfunction: LampStatus::try_from_u8(data[0] & 0x03)?,
            red_stop: LampStatus::try_from_u8((data[0] >> 2) & 0x03)?,
            amber_warning: LampStatus::try_from_u8((data[0] >> 4) & 0x03)?,
            engine_protect: LampStatus::try_from_u8(data[0] >> 6)?,
            malfunction_flash: LampFlash::try_from_u8(data[1] & 0x03)?,
            red_stop_flash: LampFlash::try_from_u8((data[1] >> 2) & 0x03)?,
            amber_warning_flash: LampFlash::try_from_u8((data[1] >> 4) & 0x03)?,
            engine_protect_flash: LampFlash::try_from_u8(data[1] >> 6)?,
        })
    }
}

// ─── DM1 / DM2 — active and previously active DTC list ────────────────

/// Lamp status + DTC list, shared by DM1 (active), DM2 (previously
/// active), DM3 (clear-PA response), DM6 (pending), DM12 (emissions),
/// DM23 (previously MIL-off). DM6 / DM12 / DM23 expose this same
/// shape under semantic type aliases — see [`Dm6Message`],
/// [`Dm12Message`], [`Dm23Message`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DmDtcList {
    pub lamps: DiagnosticLamps,
    pub dtcs: Vec<Dtc>,
}

/// DM6 — pending DTCs (not yet confirmed active). Same wire format as
/// [`DmDtcList`].
pub type Dm6Message = DmDtcList;

/// DM12 — emissions-related active DTCs. Same wire format as
/// [`DmDtcList`].
pub type Dm12Message = DmDtcList;

/// DM23 — previously MIL-off DTCs. Same wire format as
/// [`DmDtcList`].
pub type Dm23Message = DmDtcList;

impl DmDtcList {
    /// Encode the full message (≥ 8 bytes; padded with `0xFF`).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&self.lamps.encode());
        if self.dtcs.is_empty() {
            data.extend_from_slice(&[0u8; 4]);
        } else {
            for dtc in &self.dtcs {
                data.extend_from_slice(&dtc.encode());
            }
        }
        while data.len() < 8 {
            data.push(0xFF);
        }
        data
    }

    /// Decode lamps + zero-or-more DTCs. SPN=0 / FMI=0 placeholder is
    /// filtered out (matches C++ `decode_dtc_message`).
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        if data.len() == 8 && data[6..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        if data.len() > 8 && !(data.len() - 2).is_multiple_of(4) {
            return None;
        }
        let lamps = DiagnosticLamps::decode(&data[..2])?;
        let mut dtcs = Vec::new();
        let mut i = 2;
        let dtc_end = if data.len() == 8 { 6 } else { data.len() };
        while i + 4 <= dtc_end {
            let dtc = Dtc::decode(&data[i..i + 4])?;
            if dtc.spn == 0 && dtc.fmi.as_u8() == 0 {
                let is_single_frame_empty_placeholder = data.len() == 8 && i == 2;
                if dtc.occurrence_count != 0 || !is_single_frame_empty_placeholder {
                    return None;
                }
            } else {
                dtcs.push(dtc);
            }
            i += 4;
        }
        Some(Self { lamps, dtcs })
    }
}

// ─── DM3 / DM11 — clear all diagnostic data requests ──────────────────

/// DM3 / DM11 clear-all request payload.
///
/// The PGN distinguishes the target:
///
/// - `PGN_DM3` clears previously-active DTCs.
/// - `PGN_DM11` clears active DTCs.
///
/// The data field carries no selector; this codec emits and accepts only the
/// canonical all-`0xFF` reserved payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct DmClearAllRequest;

/// DM3 — clear/reset previously-active DTCs.
pub type Dm3ClearPreviouslyActiveRequest = DmClearAllRequest;
/// DM11 — clear/reset active DTCs.
pub type Dm11ClearActiveRequest = DmClearAllRequest;

impl DmClearAllRequest {
    pub const WIRE_BYTES: [u8; 8] = [0xFF; 8];

    #[inline]
    #[must_use]
    pub const fn encode(self) -> [u8; 8] {
        Self::WIRE_BYTES
    }

    #[inline]
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data != Self::WIRE_BYTES {
            return None;
        }
        Some(Self)
    }
}

// ─── DM4 — Driver's Information Message ────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dm4Message {
    pub mil_status: LampStatus,
    pub red_stop_lamp: LampStatus,
    pub amber_warning: LampStatus,
    pub protect_lamp: LampStatus,
    pub dtcs: Vec<Dtc>,
}

impl Dm4Message {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(
            (self.protect_lamp.as_u8() << 6)
                | (self.amber_warning.as_u8() << 4)
                | (self.red_stop_lamp.as_u8() << 2)
                | self.mil_status.as_u8(),
        );
        data.push(0xFF);
        for dtc in &self.dtcs {
            data.extend_from_slice(&dtc.encode());
        }
        while data.len() < 8 {
            data.push(0xFF);
        }
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        if data[1] != 0xFF {
            return None;
        }
        let mut dtcs = Vec::new();
        if data.len() == 8 {
            if data[2..].iter().all(|&byte| byte == 0xFF) {
                return Some(Self {
                    mil_status: LampStatus::try_from_u8(data[0] & 0x03)?,
                    red_stop_lamp: LampStatus::try_from_u8((data[0] >> 2) & 0x03)?,
                    amber_warning: LampStatus::try_from_u8((data[0] >> 4) & 0x03)?,
                    protect_lamp: LampStatus::try_from_u8(data[0] >> 6)?,
                    dtcs,
                });
            }
            if data[6..].iter().any(|&byte| byte != 0xFF) {
                return None;
            }
            dtcs.push(Dtc::decode(&data[2..6])?);
        } else {
            if !(data.len() - 2).is_multiple_of(4) {
                return None;
            }
            let mut i = 2;
            while i + 4 <= data.len() {
                dtcs.push(Dtc::decode(&data[i..i + 4])?);
                i += 4;
            }
        }
        Some(Self {
            mil_status: LampStatus::try_from_u8(data[0] & 0x03)?,
            red_stop_lamp: LampStatus::try_from_u8((data[0] >> 2) & 0x03)?,
            amber_warning: LampStatus::try_from_u8((data[0] >> 4) & 0x03)?,
            protect_lamp: LampStatus::try_from_u8(data[0] >> 6)?,
            dtcs,
        })
    }
}

// ─── DM7 / DM8 — non-continuous monitor test command + result ─────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dm7Command {
    pub spn: u32,
    pub test_id: u8,
}

impl Dm7Command {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let spn = clamp_spn_19(self.spn);
        data[0] = (spn & 0xFF) as u8;
        data[1] = ((spn >> 8) & 0xFF) as u8;
        data[2] = ((spn >> 16) & 0x07) as u8;
        data[3] = self.test_id;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[2] & 0xF8 != 0 || data[4..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        Some(Self {
            spn: (data[0] as u32) | ((data[1] as u32) << 8) | (((data[2] & 0x07) as u32) << 16),
            test_id: data[3],
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dm8TestResult {
    pub spn: u32,
    pub test_id: u8,
    /// `0` = passed, `1` = failed; rest test-defined.
    pub test_result: u8,
    pub test_value: u16,
    pub test_limit_min: u16,
    pub test_limit_max: u16,
}

impl Default for Dm8TestResult {
    fn default() -> Self {
        Self {
            spn: 0,
            test_id: 0xFF,
            test_result: 0xFF,
            test_value: 0xFFFF,
            test_limit_min: 0xFFFF,
            test_limit_max: 0xFFFF,
        }
    }
}

impl Dm8TestResult {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let spn = clamp_spn_19(self.spn);
        vec![
            (spn & 0xFF) as u8,
            ((spn >> 8) & 0xFF) as u8,
            ((spn >> 16) & 0x07) as u8,
            self.test_id,
            self.test_result,
            (self.test_value & 0xFF) as u8,
            ((self.test_value >> 8) & 0xFF) as u8,
            (self.test_limit_min & 0xFF) as u8,
            ((self.test_limit_min >> 8) & 0xFF) as u8,
            (self.test_limit_max & 0xFF) as u8,
            ((self.test_limit_max >> 8) & 0xFF) as u8,
        ]
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 11 {
            return None;
        }
        if data[2] & 0xF8 != 0 {
            return None;
        }
        Some(Self {
            spn: (data[0] as u32) | ((data[1] as u32) << 8) | (((data[2] & 0x07) as u32) << 16),
            test_id: data[3],
            test_result: data[4],
            test_value: (data[5] as u16) | ((data[6] as u16) << 8),
            test_limit_min: (data[7] as u16) | ((data[8] as u16) << 8),
            test_limit_max: (data[9] as u16) | ((data[10] as u16) << 8),
        })
    }
}

// ─── DM13 — suspend / resume broadcast ─────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Dm13Command {
    SuspendBroadcast = 0,
    ResumeBroadcast = 1,
    Undefined = 2,
    #[default]
    DoNotCare = 3,
}

impl Dm13Command {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::SuspendBroadcast,
            1 => Self::ResumeBroadcast,
            2 => Self::Undefined,
            _ => Self::DoNotCare,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::SuspendBroadcast),
            1 => Some(Self::ResumeBroadcast),
            2 => None,
            3 => Some(Self::DoNotCare),
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
pub enum Dm13SuspendSignal {
    IndefiniteSuspension = 0,
    PartialIndefiniteSuspension = 1,
    TemporarySuspension = 2,
    PartialTemporarySuspension = 3,
    Resuming = 4,
    #[default]
    NotAvailable = 15,
}

impl Dm13SuspendSignal {
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::IndefiniteSuspension),
            1 => Some(Self::PartialIndefiniteSuspension),
            2 => Some(Self::TemporarySuspension),
            3 => Some(Self::PartialTemporarySuspension),
            4 => Some(Self::Resuming),
            15 | 0xFF => Some(Self::NotAvailable),
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
pub struct Dm13Signals {
    pub primary_vehicle_network: Dm13Command,
    pub sae_j1922_network: Dm13Command,
    pub sae_j1587_network: Dm13Command,
    pub current_data_link: Dm13Command,
    pub suspend_signal: Dm13SuspendSignal,
    pub suspend_duration_s: u16,
}

impl Default for Dm13Signals {
    fn default() -> Self {
        Self {
            primary_vehicle_network: Dm13Command::DoNotCare,
            sae_j1922_network: Dm13Command::DoNotCare,
            sae_j1587_network: Dm13Command::DoNotCare,
            current_data_link: Dm13Command::DoNotCare,
            suspend_signal: Dm13SuspendSignal::NotAvailable,
            suspend_duration_s: 0xFFFF,
        }
    }
}

impl Dm13Signals {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.primary_vehicle_network.as_u8()
            | (self.sae_j1922_network.as_u8() << 2)
            | (self.sae_j1587_network.as_u8() << 4)
            | (self.current_data_link.as_u8() << 6);
        data[3] = if self.suspend_signal == Dm13SuspendSignal::NotAvailable {
            0xFF
        } else {
            self.suspend_signal.as_u8()
        };
        data[4] = (self.suspend_duration_s & 0xFF) as u8;
        data[5] = ((self.suspend_duration_s >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[1] != 0xFF || data[2] != 0xFF || data[6] != 0xFF || data[7] != 0xFF {
            return None;
        }
        if data[3] != 0xFF && data[3] & 0xF0 != 0 {
            return None;
        }
        Some(Self {
            primary_vehicle_network: Dm13Command::try_from_u8(data[0] & 0x03)?,
            sae_j1922_network: Dm13Command::try_from_u8((data[0] >> 2) & 0x03)?,
            sae_j1587_network: Dm13Command::try_from_u8((data[0] >> 4) & 0x03)?,
            current_data_link: Dm13Command::try_from_u8(data[0] >> 6)?,
            suspend_signal: Dm13SuspendSignal::from_u8(data[3])?,
            suspend_duration_s: (data[4] as u16) | ((data[5] as u16) << 8),
        })
    }
}

// ─── DM21 — diagnostic readiness 2 ─────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dm21Readiness {
    pub distance_with_mil_on_km: u16,
    pub distance_since_codes_cleared_km: u16,
    pub minutes_with_mil_on: u16,
    pub time_since_codes_cleared_min: u16,
    pub comprehensive_component: u8,
    pub fuel_system: u8,
    pub misfire: u8,
}

impl Default for Dm21Readiness {
    fn default() -> Self {
        Self {
            distance_with_mil_on_km: 0xFFFF,
            distance_since_codes_cleared_km: 0xFFFF,
            minutes_with_mil_on: 0xFFFF,
            time_since_codes_cleared_min: 0xFFFF,
            comprehensive_component: 0xFF,
            fuel_system: 0xFF,
            misfire: 0xFF,
        }
    }
}

impl Dm21Readiness {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        vec![
            (self.distance_with_mil_on_km & 0xFF) as u8,
            ((self.distance_with_mil_on_km >> 8) & 0xFF) as u8,
            (self.distance_since_codes_cleared_km & 0xFF) as u8,
            ((self.distance_since_codes_cleared_km >> 8) & 0xFF) as u8,
            (self.minutes_with_mil_on & 0xFF) as u8,
            ((self.minutes_with_mil_on >> 8) & 0xFF) as u8,
            (self.time_since_codes_cleared_min & 0xFF) as u8,
            ((self.time_since_codes_cleared_min >> 8) & 0xFF) as u8,
            self.comprehensive_component,
            self.fuel_system,
            self.misfire,
        ]
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 11 {
            return None;
        }
        Some(Self {
            distance_with_mil_on_km: (data[0] as u16) | ((data[1] as u16) << 8),
            distance_since_codes_cleared_km: (data[2] as u16) | ((data[3] as u16) << 8),
            minutes_with_mil_on: (data[4] as u16) | ((data[5] as u16) << 8),
            time_since_codes_cleared_min: (data[6] as u16) | ((data[7] as u16) << 8),
            comprehensive_component: data[8],
            fuel_system: data[9],
            misfire: data[10],
        })
    }
}

// ─── DM22 — individual DTC clear/reset ────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Dm22Control {
    ClearPreviouslyActive = 0x01,
    AckClearPreviouslyActive = 0x02,
    NackClearPreviouslyActive = 0x03,
    ClearActive = 0x11,
    AckClearActive = 0x12,
    NackClearActive = 0x13,
}

/// DM22 NACK reason byte (J1939-73).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Dm22NackReason {
    #[default]
    GeneralNack = 0x00,
    AccessDenied = 0x01,
    UnknownDtc = 0x02,
    DtcNoLongerPrevious = 0x03,
    DtcNoLongerActive = 0x04,
}

impl Dm22NackReason {
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::GeneralNack),
            0x01 => Some(Self::AccessDenied),
            0x02 => Some(Self::UnknownDtc),
            0x03 => Some(Self::DtcNoLongerPrevious),
            0x04 => Some(Self::DtcNoLongerActive),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl Dm22Control {
    #[must_use]
    pub const fn from_u8(v: u8) -> Option<Self> {
        Self::try_from_u8(v)
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::ClearPreviouslyActive),
            0x02 => Some(Self::AckClearPreviouslyActive),
            0x03 => Some(Self::NackClearPreviouslyActive),
            0x11 => Some(Self::ClearActive),
            0x12 => Some(Self::AckClearActive),
            0x13 => Some(Self::NackClearActive),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    #[must_use]
    pub const fn is_nack(self) -> bool {
        matches!(
            self,
            Self::NackClearPreviouslyActive | Self::NackClearActive
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dm22Message {
    pub control: Dm22Control,
    pub nack_reason: Option<Dm22NackReason>,
    pub spn: u32,
    pub fmi: Fmi,
}

impl Dm22Message {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.control.as_u8();
        if self.control.is_nack() {
            data[1] = self
                .nack_reason
                .unwrap_or(Dm22NackReason::GeneralNack)
                .as_u8();
        }
        let spn = clamp_spn_19(self.spn);
        data[5] = (spn & 0xFF) as u8;
        data[6] = ((spn >> 8) & 0xFF) as u8;
        data[7] = (((spn >> 16) & 0x07) << 5) as u8 | (self.fmi.as_u8() & 0x1F);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[2..5].iter().any(|&b| b != 0xFF) {
            return None;
        }
        let control = Dm22Control::from_u8(data[0])?;
        let nack_reason = if control.is_nack() {
            Some(Dm22NackReason::from_u8(data[1])?)
        } else {
            if data[1] != 0xFF {
                return None;
            }
            None
        };
        Some(Self {
            control,
            nack_reason,
            spn: (data[5] as u32)
                | ((data[6] as u32) << 8)
                | (((data[7] >> 5) & 0x07) as u32) << 16,
            fmi: Fmi::try_from_u8(data[7] & 0x1F)?,
        })
    }
}

// ─── DM5 — Diagnostic Protocol Identification ─────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DiagProtocol {
    None = 0x00,
    /// SAE J1939-73.
    J1939_73 = 0x01,
    /// ISO 14230 / KWP 2000.
    Iso14230 = 0x02,
    /// ISO 14229-3 / UDS-on-CAN.
    Iso14229_3 = 0x04,
}

impl DiagProtocol {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[inline]
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0x00 => Some(Self::None),
            0x01 => Some(Self::J1939_73),
            0x02 => Some(Self::Iso14230),
            0x04 => Some(Self::Iso14229_3),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticProtocolId {
    pub protocols: u8,
}

/// DM5 — Diagnostic Readiness / protocol identification.
pub type Dm5Message = DiagnosticProtocolId;

impl Default for DiagnosticProtocolId {
    fn default() -> Self {
        Self {
            protocols: DiagProtocol::J1939_73.as_u8(),
        }
    }
}

impl DiagnosticProtocolId {
    #[inline]
    #[must_use]
    pub const fn known_protocol_bits() -> u8 {
        DiagProtocol::J1939_73.as_u8()
            | DiagProtocol::Iso14230.as_u8()
            | DiagProtocol::Iso14229_3.as_u8()
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.protocols & Self::known_protocol_bits();
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[1..].iter().any(|b| *b != 0xFF) {
            return None;
        }
        if data[0] & !Self::known_protocol_bits() != 0 {
            return None;
        }
        Some(Self {
            protocols: data.first().copied().unwrap_or(0),
        })
    }

    #[must_use]
    pub const fn supports(&self, p: DiagProtocol) -> bool {
        (self.protocols & p.as_u8()) != 0
    }
}

// ─── DM9 / DM10 — Vehicle Identification request / response ───────────

/// DM9 helper: request the Vehicle Identification Number response PGN.
///
/// The wire request is a normal PGN Request whose payload names
/// [`PGN_VEHICLE_ID`] (`0xFEEC`). This wrapper makes the diagnostic intent
/// explicit and keeps callers from accidentally requesting a different PGN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dm9VehicleIdentificationRequest;

impl Dm9VehicleIdentificationRequest {
    pub fn encode(self) -> Result<[u8; 3]> {
        encode_request(PGN_VEHICLE_ID)
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        (decode_request(data) == Some(PGN_VEHICLE_ID)).then_some(Self)
    }
}

/// DM10 helper: Vehicle Identification Number response.
///
/// The payload is one printable ASCII VIN field terminated by `*`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dm10VehicleIdentification {
    pub vin: String,
}

impl Dm10VehicleIdentification {
    pub fn encode(&self) -> Result<Vec<u8>> {
        validate_ascii_star_field("Dm10VehicleIdentification.vin", &self.vin)?;
        let mut data = encode_iso11783_text_field("Dm10VehicleIdentification.vin", &self.vin, &[])?;
        data.push(b'*');
        Ok(data)
    }

    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        let fields = decode_exact_ascii_star_fields(raw, 1)?;
        Some(Self {
            vin: fields[0].clone(),
        })
    }
}

// ─── Product / Software identification (`*`-delimited strings) ──────

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProductIdentification {
    pub make: String,
    pub model: String,
    pub serial_number: String,
}

fn decode_exact_star_fields(raw: &[u8], expected_fields: usize) -> Option<Vec<String>> {
    let mut fields = Vec::with_capacity(expected_fields);
    let mut start = 0usize;
    for (idx, &byte) in raw.iter().enumerate() {
        if byte == b'*' {
            if fields.len() == expected_fields {
                return None;
            }
            let field = decode_iso11783_text_field(&raw[start..idx])?;
            fields.push(field);
            start = idx + 1;
        } else if !(0x20..=0x7E).contains(&byte) {
            decode_iso11783_text_field(&[byte])?;
        }
    }
    if start != raw.len() || fields.len() != expected_fields {
        return None;
    }
    Some(fields)
}

fn decode_exact_ascii_star_fields(raw: &[u8], expected_fields: usize) -> Option<Vec<String>> {
    let mut fields = Vec::with_capacity(expected_fields);
    let mut start = 0usize;
    for (idx, &byte) in raw.iter().enumerate() {
        if byte == b'*' {
            if fields.len() == expected_fields {
                return None;
            }
            let field = core::str::from_utf8(&raw[start..idx]).ok()?;
            fields.push(field.to_owned());
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

fn decode_star_fields(raw: &[u8]) -> Option<Vec<String>> {
    if raw.is_empty() {
        return None;
    }
    let mut fields = Vec::new();
    let mut start = 0usize;
    for (idx, &byte) in raw.iter().enumerate() {
        if byte == b'*' {
            let field = decode_iso11783_text_field(&raw[start..idx])?;
            fields.push(field);
            start = idx + 1;
        } else if !(0x20..=0x7E).contains(&byte) {
            decode_iso11783_text_field(&[byte])?;
        }
    }
    if start != raw.len() || fields.is_empty() {
        return None;
    }
    Some(fields)
}

fn validate_star_field(field_name: &'static str, value: &str) -> Result<()> {
    encode_iso11783_text_field(field_name, value, &[])?;
    Ok(())
}

fn validate_ascii_star_field(field_name: &'static str, value: &str) -> Result<()> {
    if value.as_bytes().contains(&b'*') {
        return Err(Error::invalid_data(format!(
            "{field_name} contains a reserved delimiter character"
        )));
    }
    if value
        .as_bytes()
        .iter()
        .any(|&byte| !(0x20..=0x7E).contains(&byte))
    {
        return Err(Error::invalid_data(format!(
            "{field_name} contains a non-printable ASCII character"
        )));
    }
    Ok(())
}

impl ProductIdentification {
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        for (name, f) in [
            ("ProductIdentification.make", &self.make),
            ("ProductIdentification.model", &self.model),
            ("ProductIdentification.serial_number", &self.serial_number),
        ] {
            validate_star_field(name, f)?;
            data.extend_from_slice(&encode_iso11783_text_field(name, f, &[])?);
            data.push(b'*');
        }
        Ok(data)
    }

    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        let fields = decode_exact_star_fields(raw, 3)?;
        Some(Self {
            make: fields[0].clone(),
            model: fields[1].clone(),
            serial_number: fields[2].clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SoftwareIdentification {
    pub versions: Vec<String>,
}

impl SoftwareIdentification {
    /// Encode as `v0*v1*...vN*`.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        for v in &self.versions {
            validate_star_field("SoftwareIdentification.version", v)?;
            data.extend_from_slice(&encode_iso11783_text_field(
                "SoftwareIdentification.version",
                v,
                &[],
            )?);
            data.push(b'*');
        }
        Ok(data)
    }

    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        Some(Self {
            versions: decode_star_fields(raw)?,
        })
    }
}

// ─── DM20 — performance ratios ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MonitorPerformanceRatio {
    pub spn: u32,
    pub numerator: u16,
    pub denominator: u16,
}

impl MonitorPerformanceRatio {
    #[must_use]
    pub fn encode(&self) -> [u8; 7] {
        let spn = clamp_spn_19(self.spn);
        [
            (spn & 0xFF) as u8,
            ((spn >> 8) & 0xFF) as u8,
            ((spn >> 16) & 0x07) as u8,
            (self.numerator & 0xFF) as u8,
            ((self.numerator >> 8) & 0xFF) as u8,
            (self.denominator & 0xFF) as u8,
            ((self.denominator >> 8) & 0xFF) as u8,
        ]
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 7 {
            return None;
        }
        if data[2] & 0xF8 != 0 {
            return None;
        }
        Some(Self {
            spn: (data[0] as u32) | ((data[1] as u32) << 8) | (((data[2] & 0x07) as u32) << 16),
            numerator: (data[3] as u16) | ((data[4] as u16) << 8),
            denominator: (data[5] as u16) | ((data[6] as u16) << 8),
        })
    }

    /// `0..=100`. Returns `0` if `denominator == 0`.
    #[must_use]
    pub fn percentage(&self) -> u8 {
        if self.denominator == 0 {
            return 0;
        }
        let p = (self.numerator as u32 * 100) / self.denominator as u32;
        p.min(100) as u8
    }

    #[must_use]
    pub fn meets_threshold(&self, threshold: u8) -> bool {
        self.percentage() >= threshold
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dm20Response {
    pub ignition_cycles: u8,
    pub obd_monitoring_conditions_met: u8,
    pub ratios: Vec<MonitorPerformanceRatio>,
}

impl Dm20Response {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(self.ignition_cycles);
        data.push(self.obd_monitoring_conditions_met);
        for r in &self.ratios {
            data.extend_from_slice(&r.encode());
        }
        while data.len() < 8 {
            data.push(0xFF);
        }
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        if data.len() == 8 && data[2..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        if data.len() > 8 && !(data.len() - 2).is_multiple_of(7) {
            return None;
        }
        let mut ratios = Vec::new();
        let mut offset = 2;
        while offset + 7 <= data.len() {
            ratios.push(MonitorPerformanceRatio::decode(&data[offset..offset + 7])?);
            offset += 7;
        }
        Some(Self {
            ignition_cycles: data[0],
            obd_monitoring_conditions_met: data[1],
            ratios,
        })
    }
}

// ─── DM25 — freeze frame ──────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SpnSnapshot {
    pub spn: u32,
    pub value: u32,
}

impl SpnSnapshot {
    #[must_use]
    pub fn encode(&self) -> [u8; 7] {
        let spn = clamp_spn_19(self.spn);
        [
            (spn & 0xFF) as u8,
            ((spn >> 8) & 0xFF) as u8,
            ((spn >> 16) & 0x07) as u8,
            (self.value & 0xFF) as u8,
            ((self.value >> 8) & 0xFF) as u8,
            ((self.value >> 16) & 0xFF) as u8,
            ((self.value >> 24) & 0xFF) as u8,
        ]
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 7 {
            return None;
        }
        if data[2] & 0xF8 != 0 {
            return None;
        }
        Some(Self {
            spn: (data[0] as u32) | ((data[1] as u32) << 8) | (((data[2] & 0x07) as u32) << 16),
            value: u32::from_le_bytes([data[3], data[4], data[5], data[6]]),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FreezeFrame {
    pub dtc: Dtc,
    pub timestamp_ms: u32,
    pub snapshots: Vec<SpnSnapshot>,
}

impl FreezeFrame {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.snapshots.len() > u8::MAX as usize {
            return Err(Error::invalid_data(format!(
                "FreezeFrame snapshot count {} exceeds the u8 wire field",
                self.snapshots.len()
            )));
        }
        let mut data = Vec::new();
        data.extend_from_slice(&self.dtc.encode());
        data.extend_from_slice(&self.timestamp_ms.to_le_bytes());
        data.push(self.snapshots.len() as u8);
        for s in &self.snapshots {
            data.extend_from_slice(&s.encode());
        }
        Ok(data)
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() < 9 {
            return None;
        }
        let dtc = Dtc::decode(&data[..4])?;
        let timestamp_ms = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let count = data[8] as usize;
        let expected_len = 9usize.checked_add(count.checked_mul(7)?)?;
        if data.len() != expected_len {
            return None;
        }
        let mut snapshots = Vec::with_capacity(count);
        let mut offset = 9;
        for _ in 0..count {
            snapshots.push(SpnSnapshot::decode(&data[offset..offset + 7])?);
            offset += 7;
        }
        Some(Self {
            dtc,
            timestamp_ms,
            snapshots,
        })
    }

    /// Encode as one DM25 Expanded Freeze Frame entry (SAE J1939-73, PGN 64951):
    /// `[freeze-frame length][DTC: 4 bytes][SPN snapshots: 7 bytes each]`, where
    /// the length byte counts the DTC plus the SPN data. `timestamp_ms` is
    /// repo-internal and not part of the wire entry. Each snapshot is the
    /// self-describing 19-bit-SPN + 32-bit-value record, so the data is
    /// recoverable without separate DM24 SPN-support configuration.
    #[must_use]
    pub fn encode_dm25(&self) -> Vec<u8> {
        let data_len = 4 + self.snapshots.len() * 7;
        let mut out = Vec::with_capacity(1 + data_len);
        out.push(u8::try_from(data_len).unwrap_or(u8::MAX));
        out.extend_from_slice(&self.dtc.encode());
        for snapshot in &self.snapshots {
            out.extend_from_slice(&snapshot.encode());
        }
        out
    }

    /// Decode one DM25 entry produced by [`encode_dm25`], returning the frame and
    /// the number of bytes consumed (so a multi-frame DM25 payload can be walked).
    ///
    /// [`encode_dm25`]: Self::encode_dm25
    #[must_use]
    pub fn decode_dm25(data: &[u8]) -> Option<(Self, usize)> {
        if data.is_empty() {
            return None;
        }
        let len = data[0] as usize;
        // The entry must hold at least the 4-byte DTC and fit in the buffer.
        if len < 4 || 1 + len > data.len() {
            return None;
        }
        let dtc = Dtc::decode(&data[1..5])?;
        let snap_bytes = &data[5..1 + len];
        if !snap_bytes.len().is_multiple_of(7) {
            return None;
        }
        let mut snapshots = Vec::with_capacity(snap_bytes.len() / 7);
        for chunk in snap_bytes.chunks_exact(7) {
            snapshots.push(SpnSnapshot::decode(chunk)?);
        }
        Some((
            Self {
                dtc,
                timestamp_ms: 0,
                snapshots,
            },
            1 + len,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dm25Request {
    pub spn: u32,
    pub fmi: Fmi,
    /// `0` = most recent, `1` = next most recent, etc.
    pub frame_number: u8,
}

impl Dm25Request {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let spn = clamp_spn_19(self.spn);
        data[0] = (spn & 0xFF) as u8;
        data[1] = ((spn >> 8) & 0xFF) as u8;
        data[2] = ((spn >> 16) & 0x07) as u8;
        data[3] = self.fmi.as_u8();
        data[4] = self.frame_number;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[2] & 0xF8 != 0 || data[3] & 0xE0 != 0 || data[5..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        Some(Self {
            spn: (data[0] as u32) | ((data[1] as u32) << 8) | (((data[2] & 0x07) as u32) << 16),
            fmi: Fmi::try_from_u8(data[3])?,
            frame_number: data[4],
        })
    }
}

