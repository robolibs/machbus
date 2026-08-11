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
    /// `None` when the terminal sent a code this edition does not define —
    /// including the all-ones "no action / not available" convention this
    /// struct already documents for the country code.
    ///
    /// These used to be plain enums decoded with `?`, so one unrecognised
    /// 2-bit field discarded the *entire* Language Command: the application
    /// never learned the operator's language, or any of the units the terminal
    /// did specify, and kept its defaults. G4 — an unrecognised sub-field says
    /// nothing about its siblings.
    pub decimal: Option<DecimalSymbol>,
    pub time_format: Option<TimeFormat>,
    pub date_format: Option<DateFormat>,
    pub distance: Option<DistanceUnit>,
    pub area: Option<AreaUnit>,
    pub volume: Option<VolumeUnit>,
    pub mass: Option<MassUnit>,
    pub temperature: Option<TemperatureUnit>,
    pub pressure: Option<PressureUnit>,
    pub force: Option<ForceUnit>,
    /// ISO 3166 two-character country code. `0xFF 0xFF` means no action /
    /// not available on the wire.
    pub country_code: [u8; 2],
    pub generic: Option<UnitSystem>,
}

impl Default for LanguageData {
    fn default() -> Self {
        Self {
            language_code: [b'e', b'n'],
            decimal: Some(DecimalSymbol::default()),
            time_format: Some(TimeFormat::default()),
            date_format: Some(DateFormat::default()),
            distance: Some(DistanceUnit::default()),
            area: Some(AreaUnit::default()),
            volume: Some(VolumeUnit::default()),
            mass: Some(MassUnit::default()),
            temperature: Some(TemperatureUnit::default()),
            pressure: Some(PressureUnit::default()),
            force: Some(ForceUnit::default()),
            country_code: [0xFF, 0xFF],
            generic: Some(UnitSystem::default()),
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
        // A field we could not interpret goes back out as the all-ones "no
        // action / not available" code for its width, never as a fabricated
        // default.
        let two_bit = |v: Option<u8>| v.unwrap_or(0x03) & 0x03;
        data[2] = 0x0F
            | (two_bit(self.time_format.map(TimeFormat::as_u8)) << 4)
            | (two_bit(self.decimal.map(DecimalSymbol::as_u8)) << 6);
        data[3] = self.date_format.map_or(0xFF, DateFormat::as_u8);
        data[4] = two_bit(self.mass.map(MassUnit::as_u8))
            | (two_bit(self.volume.map(VolumeUnit::as_u8)) << 2)
            | (two_bit(self.area.map(AreaUnit::as_u8)) << 4)
            | (two_bit(self.distance.map(DistanceUnit::as_u8)) << 6);
        data[5] = two_bit(self.generic.map(UnitSystem::as_u8))
            | (two_bit(self.force.map(ForceUnit::as_u8)) << 2)
            | (two_bit(self.pressure.map(PressureUnit::as_u8)) << 4)
            | (two_bit(self.temperature.map(TemperatureUnit::as_u8)) << 6);
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
        // G3 — §5.4: byte 3 bits 1-4 are reserved and ignored on receive.
        // G4 — a sub-field code this edition does not define is that field's
        // problem, not the message's: the language code in bytes 1-2 is still
        // exactly what the operator selected.
        let d = &msg.data;
        Some(Self {
            language_code: [d[0], d[1]],
            decimal: DecimalSymbol::try_from_u8((d[2] >> 6) & 0x03),
            time_format: TimeFormat::try_from_u8((d[2] >> 4) & 0x03),
            date_format: DateFormat::try_from_u8(d[3]),
            distance: DistanceUnit::try_from_u8((d[4] >> 6) & 0x03),
            area: AreaUnit::try_from_u8((d[4] >> 4) & 0x03),
            volume: VolumeUnit::try_from_u8((d[4] >> 2) & 0x03),
            mass: MassUnit::try_from_u8(d[4] & 0x03),
            temperature: TemperatureUnit::try_from_u8((d[5] >> 6) & 0x03),
            pressure: PressureUnit::try_from_u8((d[5] >> 4) & 0x03),
            force: ForceUnit::try_from_u8((d[5] >> 2) & 0x03),
            generic: UnitSystem::try_from_u8(d[5] & 0x03),
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
        assert_eq!(ld.distance, Some(DistanceUnit::Metric));
        assert_eq!(ld.decimal, Some(DecimalSymbol::Period));
    }

    #[test]
    fn round_trip_imperial_us() {
        let ld = LanguageData {
            language_code: [b'd', b'e'],
            decimal: Some(DecimalSymbol::Comma),
            time_format: Some(TimeFormat::TwelveHour),
            date_format: Some(DateFormat::YyyyMmDd),
            distance: Some(DistanceUnit::Imperial),
            area: Some(AreaUnit::Us),
            volume: Some(VolumeUnit::Imperial),
            mass: Some(MassUnit::Us),
            temperature: Some(TemperatureUnit::Imperial),
            pressure: Some(PressureUnit::Imperial),
            force: Some(ForceUnit::Imperial),
            country_code: [b'D', b'E'],
            generic: Some(UnitSystem::Us),
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

    /// J6 — §5.4 makes byte 3 bits 1-4 reserved and don't-care on receive, and
    /// G4 says an unrecognised sub-field code is that field's problem, not the
    /// message's. Rejecting the whole PG meant the application never learned
    /// the operator's language, or any of the units the terminal *did* specify,
    /// and kept its defaults — the plugin returns early on `None`, so no event
    /// was emitted at all.
    #[test]
    fn an_unknown_unit_code_still_yields_the_language_code() {
        let mut payload = LanguageData::default().encode();
        payload[0] = b'd';
        payload[1] = b'e';
        payload[2] &= !0x01; // a reserved bit cleared in byte 3
        payload[4] |= 0x03; // an undefined mass code
        payload[5] |= 0x02; // an undefined generic-unit code

        let decoded =
            LanguageData::decode(&Message::new(PGN_LANGUAGE_COMMAND, payload.to_vec(), 0))
                .expect("the Language Command still decodes");
        assert_eq!(
            decoded.language_code,
            [b'd', b'e'],
            "the operator's language selection survives an unknown unit code"
        );
        assert_eq!(decoded.mass, None, "an undefined code is not a fabricated default");
        assert_eq!(decoded.generic, None);
        assert_eq!(
            decoded.distance,
            Some(DistanceUnit::Metric),
            "the fields that did decode are still delivered"
        );
    }

}
