//! Time/Date — J1939-71 PGN 65254 (`PGN_TIME_DATE`).
//!
//! Mirrors the C++ `machbus::j1939::time_date.hpp`. SPN scaling
//! preserved: seconds at 0.25 s/bit, day at 0.25 day/bit, year offset
//! 1985, UTC offsets at offset −125. Each field is `Option<...>` —
//! `None` corresponds to the J1939 "not available" `0xFF` byte.

use crate::net::message::Message;
use crate::net::pgn_defs::PGN_TIME_DATE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeDate {
    pub seconds: Option<u8>,          // SPN 959, 0.25 s/bit
    pub minutes: Option<u8>,          // SPN 960
    pub hours: Option<u8>,            // SPN 961
    pub day: Option<u8>,              // SPN 963, 0.25 day/bit
    pub month: Option<u8>,            // SPN 962
    pub year: Option<u16>,            // SPN 964, offset 1985
    pub utc_offset_min: Option<i16>,  // SPN 1601, offset -125
    pub utc_offset_hours: Option<i8>, // SPN 1602, offset -125
    pub timestamp_us: u64,
}

impl TimeDate {
    /// Encode to the 8-byte J1939-71 PGN 65254 wire format.
    ///
    /// Unset fields are encoded as `0xFF`. Out-of-range `Some` values are
    /// clamped to the highest non-`0xFF` wire byte instead of wrapping or
    /// accidentally colliding with the not-available sentinel.
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        if let Some(s) = self.seconds {
            data[0] = encode_quarter_byte(s);
        }
        if let Some(m) = self.minutes {
            data[1] = encode_non_na_byte(m);
        }
        if let Some(h) = self.hours {
            data[2] = encode_non_na_byte(h);
        }
        if let Some(month) = self.month {
            data[3] = encode_non_na_byte(month);
        }
        if let Some(d) = self.day {
            data[4] = encode_quarter_byte(d);
        }
        if let Some(y) = self.year {
            data[5] = y.saturating_sub(1985).min(0xFE) as u8;
        }
        if let Some(o) = self.utc_offset_min {
            data[6] = (o + 125).clamp(0, 250) as u8;
        }
        if let Some(o) = self.utc_offset_hours {
            data[7] = (o as i16 + 125).clamp(0, 250) as u8;
        }
        data
    }

    /// Decode from a message. Returns [`None`] unless the payload is the
    /// exact 8-byte J1939-71 PGN 65254 wire format.
    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_TIME_DATE) {
            return None;
        }
        if msg.data.len() != 8 {
            return None;
        }
        let d = &msg.data;
        Some(Self {
            seconds: na_u8(d[0]).map(|b| b / 4),
            minutes: na_u8(d[1]),
            hours: na_u8(d[2]),
            month: na_u8(d[3]),
            day: na_u8(d[4]).map(|b| b / 4),
            year: na_u8(d[5]).map(|b| b as u16 + 1985),
            utc_offset_min: decode_utc_offset_byte(d[6])?.map(i16::from),
            utc_offset_hours: decode_utc_offset_byte(d[7])?,
            timestamp_us: msg.timestamp_us,
        })
    }
}

#[inline]
fn encode_non_na_byte(value: u8) -> u8 {
    value.min(0xFE)
}

#[inline]
fn encode_quarter_byte(value: u8) -> u8 {
    ((value as u16) * 4).min(0xFE) as u8
}

#[inline]
fn na_u8(b: u8) -> Option<u8> {
    if b == 0xFF { None } else { Some(b) }
}

#[inline]
fn decode_utc_offset_byte(b: u8) -> Option<Option<i8>> {
    match b {
        0xFF => Some(None),
        0..=250 => Some(Some((i16::from(b) - 125) as i8)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_TIME_DATE;

    #[test]
    fn round_trip_full_fields() {
        let td = TimeDate {
            seconds: Some(45),
            minutes: Some(30),
            hours: Some(14),
            day: Some(15),
            month: Some(6),
            year: Some(2024),
            utc_offset_min: Some(0),
            utc_offset_hours: Some(2),
            timestamp_us: 0,
        };
        let payload = td.encode();
        let msg = Message::new(PGN_TIME_DATE, payload.to_vec(), 0x10);
        let decoded = TimeDate::decode(&msg).unwrap();
        assert_eq!(decoded.seconds, Some(45));
        assert_eq!(decoded.minutes, Some(30));
        assert_eq!(decoded.hours, Some(14));
        assert_eq!(decoded.day, Some(15));
        assert_eq!(decoded.month, Some(6));
        assert_eq!(decoded.year, Some(2024));
        assert_eq!(decoded.utc_offset_min, Some(0));
        assert_eq!(decoded.utc_offset_hours, Some(2));
    }

    #[test]
    fn unset_fields_encode_as_ff_and_decode_as_none() {
        let td = TimeDate::default();
        let payload = td.encode();
        assert_eq!(payload, [0xFFu8; 8]);
        let msg = Message::new(PGN_TIME_DATE, payload.to_vec(), 0);
        let decoded = TimeDate::decode(&msg).unwrap();
        assert!(decoded.seconds.is_none());
        assert!(decoded.year.is_none());
    }

    #[test]
    fn decode_short_payload_returns_none() {
        let msg = Message::new(PGN_TIME_DATE, vec![0u8; 4], 0);
        assert!(TimeDate::decode(&msg).is_none());
    }

    #[test]
    fn decode_overlong_payload_returns_none() {
        let msg = Message::new(PGN_TIME_DATE, vec![0u8; 9], 0);
        assert!(TimeDate::decode(&msg).is_none());
    }

    #[test]
    fn negative_utc_offset_round_trips() {
        let td = TimeDate {
            utc_offset_min: Some(-60),
            utc_offset_hours: Some(-5),
            ..Default::default()
        };
        let payload = td.encode();
        let msg = Message::new(PGN_TIME_DATE, payload.to_vec(), 0);
        let decoded = TimeDate::decode(&msg).unwrap();
        assert_eq!(decoded.utc_offset_min, Some(-60));
        assert_eq!(decoded.utc_offset_hours, Some(-5));
    }

    #[test]
    fn encode_clamps_out_of_range_values_without_emitting_na_or_wrapping() {
        let td = TimeDate {
            seconds: Some(250),
            minutes: Some(255),
            hours: Some(255),
            day: Some(250),
            month: Some(255),
            year: Some(3000),
            utc_offset_min: Some(500),
            utc_offset_hours: Some(127),
            timestamp_us: 0,
        };
        let payload = td.encode();
        assert_eq!(payload, [0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFE, 0xFA, 0xFA]);
        let msg = Message::new(PGN_TIME_DATE, payload.to_vec(), 0);
        let decoded = TimeDate::decode(&msg).unwrap();
        assert_eq!(decoded.seconds, Some(63));
        assert_eq!(decoded.minutes, Some(254));
        assert_eq!(decoded.hours, Some(254));
        assert_eq!(decoded.day, Some(63));
        assert_eq!(decoded.month, Some(254));
        assert_eq!(decoded.year, Some(2239));
        assert_eq!(decoded.utc_offset_min, Some(125));
        assert_eq!(decoded.utc_offset_hours, Some(125));
    }

    #[test]
    fn decode_rejects_reserved_utc_offset_bytes() {
        for idx in [6, 7] {
            let mut data = [0u8; 8];
            data[idx] = 0xFB;
            let msg = Message::new(PGN_TIME_DATE, data.to_vec(), 0);
            assert!(TimeDate::decode(&msg).is_none());
        }
    }
}
