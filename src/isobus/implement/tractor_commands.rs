//! ISO 11783-7 §11 tractor implement command codecs.
//!
//! Mirrors the C++ `machbus::isobus::implement::tractor_commands.hpp`.
//! Five message families:
//!
//! - `PGN_REAR_HITCH_CMD` / `PGN_FRONT_HITCH_CMD` → `HitchCommandMsg`
//! - `PGN_REAR_PTO_CMD` / `PGN_FRONT_PTO_CMD` → `PtoCommandMsg`
//! - `PGN_AUX_VALVE_CMD + index` (16 valves) → `AuxValveCommandMsg`
//! - `PGN_TRACTOR_CONTROL_MODE` → `TractorControlModeMsg`
//!
//! The C++ `TractorCommands` interface (IsoNet-coupled) is not ported.

use super::aux_valve_status::MAX_AUX_VALVES;
use crate::net::pgn_defs::PGN_AUX_VALVE_CMD;
use crate::net::types::Pgn;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum HitchCommand {
    #[default]
    NoAction = 0,
    Lower = 1,
    Raise = 2,
    Position = 3,
}

impl HitchCommand {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::NoAction,
            1 => Self::Lower,
            2 => Self::Raise,
            _ => Self::Position,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoAction),
            1 => Some(Self::Lower),
            2 => Some(Self::Raise),
            3 => Some(Self::Position),
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
pub enum PtoCommand {
    #[default]
    NoAction = 0,
    Engage = 1,
    Disengage = 2,
    SetSpeed = 3,
}

impl PtoCommand {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::NoAction,
            1 => Self::Engage,
            2 => Self::Disengage,
            _ => Self::SetSpeed,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoAction),
            1 => Some(Self::Engage),
            2 => Some(Self::Disengage),
            3 => Some(Self::SetSpeed),
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
pub enum ValveCommand {
    #[default]
    NoAction = 0,
    Extend = 1,
    Retract = 2,
    Float = 3,
    Block = 4,
}

impl ValveCommand {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            0 => Self::NoAction,
            1 => Self::Extend,
            2 => Self::Retract,
            3 => Self::Float,
            4 => Self::Block,
            _ => Self::NoAction,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoAction),
            1 => Some(Self::Extend),
            2 => Some(Self::Retract),
            3 => Some(Self::Float),
            4 => Some(Self::Block),
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
pub enum TractorMode {
    Manual = 0,
    Automatic = 1,
    Reserved2 = 2,
    #[default]
    NotAvailable = 3,
}

impl TractorMode {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::Manual,
            1 => Self::Automatic,
            2 => Self::Reserved2,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Manual),
            1 => Some(Self::Automatic),
            2 => None,
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

fn fixed8_with_ff_at(data: &[u8], ff_indices: &[usize]) -> bool {
    data.len() == 8 && ff_indices.iter().all(|&idx| data[idx] == 0xFF)
}

/// Hitch position command (front or rear). Position is `0..=40000`
/// at 0.0025 % per bit; `0xFFFF` = N/A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitchCommandMsg {
    pub command: HitchCommand,
    pub target_position: u16,
    /// Rate of change; `0xFF` = N/A.
    pub rate: u8,
}

impl Default for HitchCommandMsg {
    fn default() -> Self {
        Self {
            command: HitchCommand::NoAction,
            target_position: 0xFFFF,
            rate: 0xFF,
        }
    }
}

impl HitchCommandMsg {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.target_position & 0xFF) as u8;
        data[1] = ((self.target_position >> 8) & 0xFF) as u8;
        data[3] = self.rate;
        data[4] = self.command.as_u8();
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_at(data, &[2, 5, 6, 7]) {
            return None;
        }
        Some(Self {
            target_position: (data[0] as u16) | ((data[1] as u16) << 8),
            rate: data[3],
            command: HitchCommand::try_from_u8(data[4])?,
        })
    }
}

/// PTO speed/engage command (front or rear). Speed at 0.125 rpm/bit;
/// `0xFFFF` = N/A.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PtoCommandMsg {
    pub command: PtoCommand,
    pub target_speed_rpm: u16,
    pub ramp_rate: u8,
}

impl Default for PtoCommandMsg {
    fn default() -> Self {
        Self {
            command: PtoCommand::NoAction,
            target_speed_rpm: 0xFFFF,
            ramp_rate: 0xFF,
        }
    }
}

impl PtoCommandMsg {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.target_speed_rpm & 0xFF) as u8;
        data[1] = ((self.target_speed_rpm >> 8) & 0xFF) as u8;
        data[3] = self.ramp_rate;
        data[4] = self.command.as_u8();
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_at(data, &[2, 5, 6, 7]) {
            return None;
        }
        Some(Self {
            target_speed_rpm: (data[0] as u16) | ((data[1] as u16) << 8),
            ramp_rate: data[3],
            command: PtoCommand::try_from_u8(data[4])?,
        })
    }
}

/// Auxiliary-valve command. PGN = `PGN_AUX_VALVE_CMD + valve_index`
/// for valves 0..=15.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxValveCommandMsg {
    pub valve_index: u8,
    pub command: ValveCommand,
    /// `0.4 %` per bit; `0xFFFF` = N/A.
    pub flow_rate: u16,
}

impl Default for AuxValveCommandMsg {
    fn default() -> Self {
        Self {
            valve_index: 0,
            command: ValveCommand::NoAction,
            flow_rate: 0xFFFF,
        }
    }
}

impl AuxValveCommandMsg {
    /// `true` when `valve_index` maps to one of the protocol-defined command
    /// PGNs (`PGN_AUX_VALVE_CMD + 0..=15`).
    #[inline]
    #[must_use]
    pub const fn has_valid_valve_index(&self) -> bool {
        self.valve_index < MAX_AUX_VALVES
    }

    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.valve_index;
        data[1] = (self.flow_rate & 0xFF) as u8;
        data[2] = ((self.flow_rate >> 8) & 0xFF) as u8;
        data[3] = self.command.as_u8();
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_at(data, &[4, 5, 6, 7]) {
            return None;
        }
        if data[0] >= MAX_AUX_VALVES {
            return None;
        }
        Some(Self {
            valve_index: data[0],
            flow_rate: (data[1] as u16) | ((data[2] as u16) << 8),
            command: ValveCommand::try_from_u8(data[3])?,
        })
    }

    /// PGN this command should be transmitted on (= base + index).
    #[inline]
    #[must_use]
    pub const fn pgn(&self) -> Pgn {
        PGN_AUX_VALVE_CMD + self.valve_index as Pgn
    }

    /// Fallible PGN helper for user-facing send paths.
    ///
    /// The legacy [`Self::pgn`] helper is intentionally a simple arithmetic
    /// mapping, but only indexes below [`MAX_AUX_VALVES`] are valid protocol
    /// command PGNs for this message family.
    #[inline]
    #[must_use]
    pub const fn try_pgn(&self) -> Option<Pgn> {
        if self.has_valid_valve_index() {
            Some(self.pgn())
        } else {
            None
        }
    }
}

/// Tractor Control Mode (PGN 0xFE0B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TractorControlModeMsg {
    pub hitch_mode: TractorMode,
    pub pto_mode: TractorMode,
    pub front_hitch_mode: TractorMode,
    pub front_pto_mode: TractorMode,
    /// `0xFF` = N/A; otherwise bit-encoded speed control modes.
    pub speed_control_state: u8,
}

impl Default for TractorControlModeMsg {
    fn default() -> Self {
        Self {
            hitch_mode: TractorMode::NotAvailable,
            pto_mode: TractorMode::NotAvailable,
            front_hitch_mode: TractorMode::NotAvailable,
            front_pto_mode: TractorMode::NotAvailable,
            speed_control_state: 0xFF,
        }
    }
}

impl TractorControlModeMsg {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.hitch_mode.as_u8() & 0x03)
            | ((self.pto_mode.as_u8() & 0x03) << 2)
            | ((self.front_hitch_mode.as_u8() & 0x03) << 4)
            | ((self.front_pto_mode.as_u8() & 0x03) << 6);
        data[1] = self.speed_control_state;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if !fixed8_with_ff_at(data, &[2, 3, 4, 5, 6, 7]) {
            return None;
        }
        Some(Self {
            hitch_mode: TractorMode::try_from_u8(data[0] & 0x03)?,
            pto_mode: TractorMode::try_from_u8((data[0] >> 2) & 0x03)?,
            front_hitch_mode: TractorMode::try_from_u8((data[0] >> 4) & 0x03)?,
            front_pto_mode: TractorMode::try_from_u8(data[0] >> 6)?,
            speed_control_state: data[1],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hitch_command_round_trip() {
        let m = HitchCommandMsg {
            command: HitchCommand::Position,
            target_position: 30_000,
            rate: 100,
        };
        let decoded = HitchCommandMsg::decode(&m.encode()).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn pto_command_round_trip() {
        let m = PtoCommandMsg {
            command: PtoCommand::Engage,
            target_speed_rpm: 4320, // 540 rpm
            ramp_rate: 50,
        };
        let decoded = PtoCommandMsg::decode(&m.encode()).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn aux_valve_command_round_trip() {
        let m = AuxValveCommandMsg {
            valve_index: 5,
            command: ValveCommand::Extend,
            flow_rate: 250, // 100 % at 0.4 %/bit
        };
        let decoded = AuxValveCommandMsg::decode(&m.encode()).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn aux_valve_pgn_routing() {
        let m = AuxValveCommandMsg {
            valve_index: 7,
            ..Default::default()
        };
        assert_eq!(m.pgn(), PGN_AUX_VALVE_CMD + 7);
        assert_eq!(m.try_pgn(), Some(PGN_AUX_VALVE_CMD + 7));
    }

    #[test]
    fn aux_valve_try_pgn_rejects_out_of_range_index() {
        let max = AuxValveCommandMsg {
            valve_index: MAX_AUX_VALVES - 1,
            ..Default::default()
        };
        assert!(max.has_valid_valve_index());
        assert_eq!(max.try_pgn(), Some(PGN_AUX_VALVE_CMD + 15));

        let invalid = AuxValveCommandMsg {
            valve_index: MAX_AUX_VALVES,
            ..Default::default()
        };
        assert!(!invalid.has_valid_valve_index());
        assert_eq!(invalid.try_pgn(), None);
    }

    #[test]
    fn tractor_control_mode_round_trip() {
        let m = TractorControlModeMsg {
            hitch_mode: TractorMode::Automatic,
            pto_mode: TractorMode::Manual,
            front_hitch_mode: TractorMode::NotAvailable,
            front_pto_mode: TractorMode::Automatic,
            speed_control_state: 0xAA,
        };
        let decoded = TractorControlModeMsg::decode(&m.encode()).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn enums_round_trip() {
        for c in [
            HitchCommand::NoAction,
            HitchCommand::Lower,
            HitchCommand::Raise,
            HitchCommand::Position,
        ] {
            assert_eq!(HitchCommand::from_u8(c.as_u8()), c);
        }
        for c in [
            PtoCommand::NoAction,
            PtoCommand::Engage,
            PtoCommand::Disengage,
            PtoCommand::SetSpeed,
        ] {
            assert_eq!(PtoCommand::from_u8(c.as_u8()), c);
        }
        for c in [
            ValveCommand::NoAction,
            ValveCommand::Extend,
            ValveCommand::Retract,
            ValveCommand::Float,
            ValveCommand::Block,
        ] {
            assert_eq!(ValveCommand::from_u8(c.as_u8()), c);
        }
    }

    #[test]
    fn command_decoders_reject_reserved_bytes_and_bad_padding() {
        let mut hitch = HitchCommandMsg {
            command: HitchCommand::Raise,
            target_position: 30_000,
            rate: 100,
        }
        .encode();
        hitch[4] = 0x04;
        assert!(HitchCommandMsg::decode(&hitch).is_none());
        let mut hitch_bad_padding = HitchCommandMsg::default().encode();
        hitch_bad_padding[2] = 0x00;
        assert!(HitchCommandMsg::decode(&hitch_bad_padding).is_none());

        let mut pto = PtoCommandMsg {
            command: PtoCommand::Engage,
            target_speed_rpm: 4320,
            ramp_rate: 50,
        }
        .encode();
        pto[4] = 0x04;
        assert!(PtoCommandMsg::decode(&pto).is_none());
        let mut pto_bad_padding = PtoCommandMsg::default().encode();
        pto_bad_padding[2] = 0x00;
        assert!(PtoCommandMsg::decode(&pto_bad_padding).is_none());

        let mut aux = AuxValveCommandMsg {
            valve_index: 5,
            command: ValveCommand::Extend,
            flow_rate: 250,
        }
        .encode();
        aux[3] = 0x05;
        assert!(AuxValveCommandMsg::decode(&aux).is_none());
        let mut aux_bad_index = AuxValveCommandMsg::default().encode();
        aux_bad_index[0] = MAX_AUX_VALVES;
        assert!(AuxValveCommandMsg::decode(&aux_bad_index).is_none());
        let mut aux_bad_padding = AuxValveCommandMsg::default().encode();
        aux_bad_padding[4] = 0x00;
        assert!(AuxValveCommandMsg::decode(&aux_bad_padding).is_none());

        let mut tractor_control_bad_padding = TractorControlModeMsg::default().encode();
        tractor_control_bad_padding[2] = 0x00;
        assert!(TractorControlModeMsg::decode(&tractor_control_bad_padding).is_none());

        for bad_mode in [0x02, 0x08, 0x20, 0x80] {
            let mut tractor_control_reserved_mode = TractorControlModeMsg::default().encode();
            tractor_control_reserved_mode[0] = bad_mode;
            assert!(TractorControlModeMsg::decode(&tractor_control_reserved_mode).is_none());
        }
    }

    #[test]
    fn short_payloads_return_none() {
        assert!(HitchCommandMsg::decode(&[0u8; 7]).is_none());
        assert!(PtoCommandMsg::decode(&[0u8; 7]).is_none());
        assert!(AuxValveCommandMsg::decode(&[0u8; 7]).is_none());
        assert!(TractorControlModeMsg::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn overlong_payloads_return_none() {
        assert!(HitchCommandMsg::decode(&[0u8; 9]).is_none());
        assert!(PtoCommandMsg::decode(&[0u8; 9]).is_none());
        assert!(AuxValveCommandMsg::decode(&[0u8; 9]).is_none());
        assert!(TractorControlModeMsg::decode(&[0u8; 9]).is_none());
    }
}
