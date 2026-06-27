//! Wheel / Ground / Machine speed and distance — J1939-71.
//!
//! Mirrors the C++ `machbus::j1939::speed_distance.hpp`. Three PGNs
//! share the same payload shape (`speed_raw u16` LE @ 0..2,
//! `distance_raw u32` LE @ 2..6, both at 0.001 m resolution).
//!
//! The C++ has separate handlers per PGN that fan into a single
//! `SpeedData` struct; the Rust port factors out the codec into a
//! single [`SpeedAndDistance`] type and lets the caller pair it with
//! the PGN they care about.

use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_GROUND_SPEED, PGN_MACHINE_SPEED, PGN_WHEEL_SPEED};

fn has_ff_tail(data: &[u8], used: usize) -> bool {
    data[used..].iter().all(|&byte| byte == 0xFF)
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
        raw as u32
    }
}

fn u16_error_indicator(raw: u16) -> bool {
    raw == u16::MAX - 1
}

fn u32_error_indicator(raw: u32) -> bool {
    raw == u32::MAX - 1
}

/// Speed (m/s) and accumulated distance (m) carried by any of:
///
/// - `PGN_WHEEL_SPEED` (0xFE48)
/// - `PGN_GROUND_SPEED` (0xFE49)
/// - `PGN_MACHINE_SPEED` (0xF022)
///
/// Either field is [`None`] if the wire bytes are the J1939
/// "not available" sentinel (`0xFFFF` / `0xFFFFFFFF`).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct SpeedAndDistance {
    /// Speed in metres per second.
    pub speed_mps: Option<f64>,
    /// Accumulated distance in metres.
    pub distance_m: Option<f64>,
    pub timestamp_us: u64,
}

impl SpeedAndDistance {
    /// Encode to the 8-byte payload. Unset fields are written as the
    /// J1939 not-available sentinel.
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        if let Some(s) = self.speed_mps {
            let raw = scaled_u16_non_na(s, 0.001);
            data[0] = (raw & 0xFF) as u8;
            data[1] = ((raw >> 8) & 0xFF) as u8;
        }
        if let Some(d) = self.distance_m {
            let raw = scaled_u32_non_na(d, 0.001);
            data[2] = (raw & 0xFF) as u8;
            data[3] = ((raw >> 8) & 0xFF) as u8;
            data[4] = ((raw >> 16) & 0xFF) as u8;
            data[5] = ((raw >> 24) & 0xFF) as u8;
        }
        data
    }

    /// Decode from a complete 8-byte fixed-size payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || !has_ff_tail(data, 6) {
            return None;
        }
        Self::decode_measurement_prefix(data)
    }

    /// Decode only the common `speed_raw` / `distance_raw` prefix from an
    /// ISO 11783/J1939 speed-distance payload.
    ///
    /// Several real speed-distance PGNs share bytes `0..6` but use bytes
    /// `6..8` for PGN-specific status/flags instead of `0xFF` padding
    /// (for example wheel-based speed, ground-based speed, and machine
    /// selected speed in AgIsoStack). Use this helper when the caller already
    /// knows the PGN and wants the common measurement subset.
    #[must_use]
    pub fn decode_measurement_prefix(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let speed_raw = (data[0] as u16) | ((data[1] as u16) << 8);
        let dist_raw = (data[2] as u32)
            | ((data[3] as u32) << 8)
            | ((data[4] as u32) << 16)
            | ((data[5] as u32) << 24);
        if u16_error_indicator(speed_raw) || u32_error_indicator(dist_raw) {
            return None;
        }
        Some(Self {
            speed_mps: if speed_raw == 0xFFFF {
                None
            } else {
                Some(speed_raw as f64 * 0.001)
            },
            distance_m: if dist_raw == 0xFFFF_FFFF {
                None
            } else {
                Some(dist_raw as f64 * 0.001)
            },
            timestamp_us: 0,
        })
    }

    /// Decode and attach the message timestamp.
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if msg.data.len() != 8 {
            return None;
        }
        if !matches!(
            msg.pgn,
            PGN_WHEEL_SPEED | PGN_GROUND_SPEED | PGN_MACHINE_SPEED
        ) || !msg.has_usable_envelope_for_pgn(msg.pgn)
        {
            return None;
        }
        let mut s = Self::decode_measurement_prefix(&msg.data)?;
        s.timestamp_us = msg.timestamp_us;
        Some(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::{PGN_GROUND_SPEED, PGN_MACHINE_SPEED, PGN_WHEEL_SPEED};

    #[test]
    fn round_trip_speed_and_distance() {
        let s = SpeedAndDistance {
            speed_mps: Some(5.0),
            distance_m: Some(1234.5),
            timestamp_us: 0,
        };
        let bytes = s.encode();
        assert_eq!(bytes, [0x88, 0x13, 0x44, 0xD6, 0x12, 0x00, 0xFF, 0xFF]);
        let decoded = SpeedAndDistance::decode(&bytes).unwrap();
        assert!((decoded.speed_mps.unwrap() - 5.0).abs() < 0.001);
        assert!((decoded.distance_m.unwrap() - 1234.5).abs() < 0.001);
    }

    #[test]
    fn unset_fields_round_trip_as_none() {
        let s = SpeedAndDistance::default();
        let bytes = s.encode();
        assert_eq!(bytes, [0xFFu8; 8]);
        let decoded = SpeedAndDistance::decode(&bytes).unwrap();
        assert_eq!(decoded.speed_mps, None);
        assert_eq!(decoded.distance_m, None);
    }

    #[test]
    fn from_message_attaches_timestamp() {
        let s = SpeedAndDistance {
            speed_mps: Some(2.0),
            distance_m: Some(100.0),
            timestamp_us: 0,
        };
        let bytes = s.encode();
        for pgn in [PGN_WHEEL_SPEED, PGN_GROUND_SPEED, PGN_MACHINE_SPEED] {
            let mut msg = Message::new(pgn, bytes.to_vec(), 0x10);
            msg.timestamp_us = 12345;
            let d = SpeedAndDistance::from_message(&msg).unwrap();
            assert_eq!(d.timestamp_us, 12345);
        }
    }

    #[test]
    fn short_payload_returns_none() {
        assert!(SpeedAndDistance::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn overlong_payload_returns_none() {
        assert!(SpeedAndDistance::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn reserved_padding_is_rejected() {
        let mut data = SpeedAndDistance {
            speed_mps: Some(5.0),
            distance_m: Some(1234.5),
            timestamp_us: 0,
        }
        .encode();
        data[6] = 0x00;
        assert!(SpeedAndDistance::decode(&data).is_none());
    }

    #[test]
    fn encode_clamps_some_values_away_from_not_available_sentinels() {
        let high = SpeedAndDistance {
            speed_mps: Some(1_000.0),
            distance_m: Some(10_000_000.0),
            timestamp_us: 0,
        }
        .encode();
        assert_eq!(high, [0xFD, 0xFF, 0xFD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let decoded = SpeedAndDistance::decode(&high).unwrap();
        assert!(decoded.speed_mps.is_some());
        assert!(decoded.distance_m.is_some());

        let non_finite = SpeedAndDistance {
            speed_mps: Some(f64::NAN),
            distance_m: Some(f64::INFINITY),
            timestamp_us: 0,
        }
        .encode();
        assert_eq!(non_finite, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF]);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_speed_distance_decoder_accepts_or_rejects_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=64),
        ) {
            if let Some(decoded) = SpeedAndDistance::decode(&data) {
                let encoded = decoded.encode();
                let decoded_again = SpeedAndDistance::decode(&encoded)
                    .expect("canonical re-encode must remain decodable");
                prop_assert_eq!(decoded_again.encode(), encoded);
            }
        }
    }
}
