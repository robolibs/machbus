//! ISO 11783-7 guidance — `PGN_GUIDANCE_MACHINE` (0xFE44) and
//! `PGN_GUIDANCE_SYSTEM` (0xFE45).
//!
//! Mirrors the C++ `machbus::isobus::guidance.hpp`. Both PGNs share
//! the same wire format. The C++ `GuidanceInterface` (IsoNet-coupled)
//! is not ported.
//!
//! # Wire format
//!
//! | bytes | field | scaling |
//! |---|---|---|
//! | 0..2 | curvature | 0.25 km⁻¹ per bit, offset −8032 km⁻¹ |
//! | 2 | status | as-is |
//! | 3..8 | reserved | `0xFF` |

use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_GUIDANCE_MACHINE, PGN_GUIDANCE_SYSTEM};

const CURVATURE_MIN_PER_KM: f64 = -8032.0;
const CURVATURE_MAX_PER_KM: f64 = 8031.75;
const CURVATURE_OFFSET_PER_KM: f64 = 8032.0;
const CURVATURE_RESOLUTION_PER_KM: f64 = 0.25;
const CURVATURE_NOT_AVAILABLE_RAW: u16 = 0xFFFF;

fn encode_curvature_per_km(curvature_per_km: f64) -> u16 {
    if !curvature_per_km.is_finite() {
        return CURVATURE_NOT_AVAILABLE_RAW;
    }
    let clamped = curvature_per_km.clamp(CURVATURE_MIN_PER_KM, CURVATURE_MAX_PER_KM);
    ((clamped + CURVATURE_OFFSET_PER_KM) / CURVATURE_RESOLUTION_PER_KM) as u16
}

fn decode_curvature_per_km(raw: u16) -> Option<f64> {
    if raw == CURVATURE_NOT_AVAILABLE_RAW {
        None
    } else {
        Some(raw as f64 * CURVATURE_RESOLUTION_PER_KM - CURVATURE_OFFSET_PER_KM)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GuidanceData {
    /// Inverse-radius (1/km). Positive = right turn.
    pub curvature: Option<f64>,
    /// Heading in radians (0 = north, CW positive). Reserved on the
    /// wire; preserved here for parity with the C++ struct.
    pub heading_rad: Option<f64>,
    /// Cross-track error in metres (positive = right of line).
    /// Reserved on the wire; same caveat as `heading_rad`.
    pub cross_track_m: Option<f64>,
    pub status: Option<u8>,
    pub timestamp_us: u64,
}

impl GuidanceData {
    /// Encode to the 8-byte wire format. `heading_rad` and
    /// `cross_track_m` are not on the wire and are ignored.
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        if let Some(curvature_per_km) = self.curvature {
            let raw = encode_curvature_per_km(curvature_per_km);
            let bytes = raw.to_le_bytes();
            data[0] = bytes[0];
            data[1] = bytes[1];
        }
        if let Some(s) = self.status {
            data[2] = s;
        }
        data
    }

    /// Decode from a payload (must be exactly the classic 8-byte CAN payload).
    /// Curvature stays
    /// [`None`] on the J1939 not-available sentinel `0xFFFF`.
    #[must_use]
    pub fn decode(msg: &Message) -> Option<Self> {
        if msg.pgn != PGN_GUIDANCE_MACHINE && msg.pgn != PGN_GUIDANCE_SYSTEM {
            return None;
        }
        if !msg.has_usable_envelope_for_pgn(msg.pgn) {
            return None;
        }
        if msg.data.len() != 8 {
            return None;
        }
        if msg.data[3..8] != [0xFF; 5] {
            return None;
        }
        let curv_raw = msg.get_u16_le(0);
        let curvature = decode_curvature_per_km(curv_raw);
        let status = if msg.data[2] == 0xFF {
            None
        } else {
            Some(msg.data[2])
        };
        Some(Self {
            curvature,
            heading_rad: None,
            cross_track_m: None,
            status,
            timestamp_us: msg.timestamp_us,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::{PGN_GUIDANCE_MACHINE, PGN_GUIDANCE_SYSTEM};

    #[test]
    fn round_trip_with_curvature_and_status() {
        let gd = GuidanceData {
            curvature: Some(0.05),
            status: Some(2),
            ..Default::default()
        };
        let bytes = gd.encode();
        let msg = Message::new(PGN_GUIDANCE_MACHINE, bytes.to_vec(), 0x10);
        let decoded = GuidanceData::decode(&msg).unwrap();
        assert!((decoded.curvature.unwrap() - 0.05).abs() < 0.25);
        assert_eq!(decoded.status, Some(2));
    }

    #[test]
    fn unset_fields_round_trip_as_none() {
        let gd = GuidanceData::default();
        let bytes = gd.encode();
        assert_eq!(&bytes[..], &[0xFFu8; 8]);
        let msg = Message::new(PGN_GUIDANCE_SYSTEM, bytes.to_vec(), 0);
        let decoded = GuidanceData::decode(&msg).unwrap();
        assert!(decoded.curvature.is_none());
        assert!(decoded.status.is_none());
    }

    #[test]
    fn decode_short_payload_returns_none() {
        let msg = Message::new(PGN_GUIDANCE_MACHINE, vec![0xFF; 7], 0);
        assert!(GuidanceData::decode(&msg).is_none());
    }

    #[test]
    fn negative_curvature_round_trips() {
        let gd = GuidanceData {
            curvature: Some(-0.02),
            ..Default::default()
        };
        let bytes = gd.encode();
        let msg = Message::new(PGN_GUIDANCE_MACHINE, bytes.to_vec(), 0);
        let decoded = GuidanceData::decode(&msg).unwrap();
        assert!((decoded.curvature.unwrap() - -0.02).abs() < 0.25);
    }

    #[test]
    fn zero_curvature_uses_iso11783_offset() {
        let gd = GuidanceData {
            curvature: Some(0.0),
            ..Default::default()
        };
        let bytes = gd.encode();
        assert_eq!(&bytes[0..2], &32128u16.to_le_bytes());

        let msg = Message::new(PGN_GUIDANCE_MACHINE, bytes.to_vec(), 0);
        let decoded = GuidanceData::decode(&msg).unwrap();
        assert_eq!(decoded.curvature, Some(0.0));
    }

    #[test]
    fn curvature_encode_clamps_and_rejects_non_finite() {
        let too_low = GuidanceData {
            curvature: Some(CURVATURE_MIN_PER_KM - 1.0),
            ..Default::default()
        };
        assert_eq!(&too_low.encode()[0..2], &0u16.to_le_bytes());

        let too_high = GuidanceData {
            curvature: Some(CURVATURE_MAX_PER_KM + 1.0),
            ..Default::default()
        };
        assert_eq!(&too_high.encode()[0..2], &64255u16.to_le_bytes());

        let nan = GuidanceData {
            curvature: Some(f64::NAN),
            ..Default::default()
        };
        assert_eq!(
            &nan.encode()[0..2],
            &CURVATURE_NOT_AVAILABLE_RAW.to_le_bytes()
        );
        let msg = Message::new(PGN_GUIDANCE_MACHINE, nan.encode().to_vec(), 0);
        assert_eq!(GuidanceData::decode(&msg).unwrap().curvature, None);
    }
}
