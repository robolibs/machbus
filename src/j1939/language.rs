//! Language Command — `PGN_LANGUAGE_COMMAND`.
//!
//! Mirrors the C++ `machbus::j1939::language.hpp`. 2-byte ISO 639-1
//! language code plus a packed unit-system descriptor.

use crate::net::message::Message;
use crate::net::pgn_defs::PGN_LANGUAGE_COMMAND;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DistanceUnit {
    #[default]
    Metric = 0,
    Imperial = 1,
    Us = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum AreaUnit {
    #[default]
    Metric = 0,
    Imperial = 1,
    Us = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum VolumeUnit {
    #[default]
    Metric = 0,
    Imperial = 1,
    Us = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum MassUnit {
    #[default]
    Metric = 0,
    Imperial = 1,
    Us = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TemperatureUnit {
    #[default]
    Metric = 0,
    Imperial = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum PressureUnit {
    #[default]
    Metric = 0,
    Imperial = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ForceUnit {
    #[default]
    Metric = 0,
    Imperial = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum UnitSystem {
    #[default]
    Metric = 0,
    Us = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TimeFormat {
    #[default]
    TwentyFourHour = 0,
    TwelveHour = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DateFormat {
    #[default]
    DdMmYyyy = 0,
    MmDdYyyy = 1,
    YyyyMmDd = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DecimalSymbol {
    Comma = 0,
    #[default]
    Period = 1,
}

macro_rules! impl_strict_unit_decoder {
    ($ty:ty, {$($raw:literal => $variant:path),+ $(,)?}) => {
        impl $ty {
            #[must_use]
            pub const fn try_from_u8(v: u8) -> Option<Self> {
                match v {
                    $($raw => Some($variant),)+
                    _ => None,
                }
            }

            #[inline]
            #[must_use]
            pub const fn as_u8(self) -> u8 {
                self as u8
            }
        }
    };
}

impl_strict_unit_decoder!(DistanceUnit, {
    0 => DistanceUnit::Metric,
    1 => DistanceUnit::Imperial,
    2 => DistanceUnit::Us,
});
impl_strict_unit_decoder!(AreaUnit, {
    0 => AreaUnit::Metric,
    1 => AreaUnit::Imperial,
    2 => AreaUnit::Us,
});
impl_strict_unit_decoder!(VolumeUnit, {
    0 => VolumeUnit::Metric,
    1 => VolumeUnit::Imperial,
    2 => VolumeUnit::Us,
});
impl_strict_unit_decoder!(MassUnit, {
    0 => MassUnit::Metric,
    1 => MassUnit::Imperial,
    2 => MassUnit::Us,
});
impl_strict_unit_decoder!(TemperatureUnit, {
    0 => TemperatureUnit::Metric,
    1 => TemperatureUnit::Imperial,
});
impl_strict_unit_decoder!(PressureUnit, {
    0 => PressureUnit::Metric,
    1 => PressureUnit::Imperial,
});
impl_strict_unit_decoder!(ForceUnit, {
    0 => ForceUnit::Metric,
    1 => ForceUnit::Imperial,
});
impl_strict_unit_decoder!(UnitSystem, {
    0 => UnitSystem::Metric,
    1 => UnitSystem::Us,
});
impl_strict_unit_decoder!(TimeFormat, {
    0 => TimeFormat::TwentyFourHour,
    1 => TimeFormat::TwelveHour,
});
impl_strict_unit_decoder!(DateFormat, {
    0 => DateFormat::DdMmYyyy,
    1 => DateFormat::MmDdYyyy,
    4 => DateFormat::YyyyMmDd,
});
impl_strict_unit_decoder!(DecimalSymbol, {
    0 => DecimalSymbol::Comma,
    1 => DecimalSymbol::Period,
});

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageData {
    pub language_code: [u8; 2],
    pub decimal: DecimalSymbol,
    pub time_format: TimeFormat,
    pub date_format: DateFormat,
    pub distance: DistanceUnit,
    pub area: AreaUnit,
    pub volume: VolumeUnit,
    pub mass: MassUnit,
    pub temperature: TemperatureUnit,
    pub pressure: PressureUnit,
    pub force: ForceUnit,
    /// ISO 3166 two-character country code. `0xFF 0xFF` means no action /
    /// not available on the wire.
    pub country_code: [u8; 2],
    pub generic: UnitSystem,
}

impl Default for LanguageData {
    fn default() -> Self {
        Self {
            language_code: [b'e', b'n'],
            decimal: DecimalSymbol::default(),
            time_format: TimeFormat::default(),
            date_format: DateFormat::default(),
            distance: DistanceUnit::default(),
            area: AreaUnit::default(),
            volume: VolumeUnit::default(),
            mass: MassUnit::default(),
            temperature: TemperatureUnit::default(),
            pressure: PressureUnit::default(),
            force: ForceUnit::default(),
            country_code: [0xFF, 0xFF],
            generic: UnitSystem::default(),
        }
    }
}

impl LanguageData {
    /// Encode to the 8-byte ISO 11783 Language Command wire format.
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.language_code[0];
        data[1] = self.language_code[1];
        data[2] = 0x0F | ((self.time_format as u8) << 4) | ((self.decimal as u8) << 6);
        data[3] = self.date_format as u8;
        data[4] = (self.mass as u8)
            | ((self.volume as u8) << 2)
            | ((self.area as u8) << 4)
            | ((self.distance as u8) << 6);
        data[5] = (self.generic as u8)
            | ((self.force as u8) << 2)
            | ((self.pressure as u8) << 4)
            | ((self.temperature as u8) << 6);
        data[6] = self.country_code[0];
        data[7] = self.country_code[1];
        data
    }

    /// Decode from a classic 8-byte message payload. Returns [`None`] for
    /// malformed fixed-size payloads.
    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_LANGUAGE_COMMAND) {
            return None;
        }
        if msg.data.len() != 8 {
            return None;
        }
        let d = &msg.data;
        if d[2] & 0x0F != 0x0F {
            return None;
        }
        Some(Self {
            language_code: [d[0], d[1]],
            decimal: DecimalSymbol::try_from_u8((d[2] >> 6) & 0x03)?,
            time_format: TimeFormat::try_from_u8((d[2] >> 4) & 0x03)?,
            date_format: DateFormat::try_from_u8(d[3])?,
            distance: DistanceUnit::try_from_u8((d[4] >> 6) & 0x03)?,
            area: AreaUnit::try_from_u8((d[4] >> 4) & 0x03)?,
            volume: VolumeUnit::try_from_u8((d[4] >> 2) & 0x03)?,
            mass: MassUnit::try_from_u8(d[4] & 0x03)?,
            temperature: TemperatureUnit::try_from_u8((d[5] >> 6) & 0x03)?,
            pressure: PressureUnit::try_from_u8((d[5] >> 4) & 0x03)?,
            force: ForceUnit::try_from_u8((d[5] >> 2) & 0x03)?,
            generic: UnitSystem::try_from_u8(d[5] & 0x03)?,
            country_code: [d[6], d[7]],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_LANGUAGE_COMMAND;

    #[test]
    fn defaults_are_metric_english() {
        let ld = LanguageData::default();
        assert_eq!(ld.language_code, [b'e', b'n']);
        assert_eq!(ld.distance, DistanceUnit::Metric);
        assert_eq!(ld.decimal, DecimalSymbol::Period);
    }

    #[test]
    fn round_trip_imperial_us() {
        let ld = LanguageData {
            language_code: [b'd', b'e'],
            decimal: DecimalSymbol::Comma,
            time_format: TimeFormat::TwelveHour,
            date_format: DateFormat::YyyyMmDd,
            distance: DistanceUnit::Imperial,
            area: AreaUnit::Us,
            volume: VolumeUnit::Imperial,
            mass: MassUnit::Us,
            temperature: TemperatureUnit::Imperial,
            pressure: PressureUnit::Imperial,
            force: ForceUnit::Imperial,
            country_code: [b'D', b'E'],
            generic: UnitSystem::Us,
        };
        let payload = ld.encode();
        let msg = Message::new(PGN_LANGUAGE_COMMAND, payload.to_vec(), 0);
        let decoded = LanguageData::decode(&msg).unwrap();
        assert_eq!(decoded, ld);
    }

    #[test]
    fn decode_short_payload_returns_none() {
        let msg = Message::new(PGN_LANGUAGE_COMMAND, vec![0u8; 4], 0);
        assert!(LanguageData::decode(&msg).is_none());
    }

    #[test]
    fn decode_oversized_payload_returns_none() {
        let msg = Message::new(PGN_LANGUAGE_COMMAND, vec![0xFFu8; 9], 0);
        assert!(LanguageData::decode(&msg).is_none());
    }

    #[test]
    fn decode_rejects_reserved_unit_values_and_tail_bytes() {
        let mut payload = LanguageData::default().encode();
        payload[2] &= !0x01;
        assert!(
            LanguageData::decode(&Message::new(PGN_LANGUAGE_COMMAND, payload.to_vec(), 0))
                .is_none()
        );

        let mut payload = LanguageData::default().encode();
        payload[4] |= 0x03;
        assert!(
            LanguageData::decode(&Message::new(PGN_LANGUAGE_COMMAND, payload.to_vec(), 0))
                .is_none()
        );

        let mut payload = LanguageData::default().encode();
        payload[5] |= 0x02;
        assert!(
            LanguageData::decode(&Message::new(PGN_LANGUAGE_COMMAND, payload.to_vec(), 0))
                .is_none()
        );
    }
}
