//! J1939 transmission and powertrain status messages.
//!
//! Mirrors the C++ `machbus::j1939::transmission.hpp`. Each struct
//! exposes [`encode`](Etc1::encode) / [`decode`](Etc1::decode) for the
//! 8-byte wire format. SPN scaling preserved exactly.

use crate::net::message::Message;
use crate::net::pgn_defs::{PGN_CRUISE_CONTROL, PGN_ETC1};

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

fn offset_scaled_u16_non_na(value: f64, offset: f64, scale: f64) -> u16 {
    scaled_u16_non_na(value + offset, scale)
}

fn etc1_gear_raw(gear: i8) -> u8 {
    (i16::from(gear) + 125).clamp(0, 250) as u8
}

fn u16_data_is_available(raw: u16) -> bool {
    raw < u16::MAX - 1
}

// ─── ETC1 — Electronic Transmission Controller 1 (PGN 0x0F005) ─────────

/// Output-shaft speed at 0.125 rpm/bit, gears at offset −125. Sent
/// every 10–100 ms by the transmission controller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Etc1 {
    /// SPN 523, offset −125 (range −125 … 125).
    pub current_gear: i8,
    /// SPN 524, offset −125.
    pub selected_gear: i8,
    /// SPN 191, 0.125 rpm/bit.
    pub output_shaft_speed_rpm: f64,
    /// SPN 574, 2 bits.
    pub shift_in_progress: u8,
    /// SPN 573, 2 bits.
    pub torque_converter_lockup: u8,
}

impl Default for Etc1 {
    fn default() -> Self {
        Self {
            current_gear: -125,
            selected_gear: -125,
            output_shaft_speed_rpm: 0.0,
            shift_in_progress: 0x03,
            torque_converter_lockup: 0x03,
        }
    }
}

impl Etc1 {
    /// Encode to 8 bytes. Bytes 5–7 are reserved (`0xFF`).
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = (self.shift_in_progress & 0x03) | ((self.torque_converter_lockup & 0x03) << 2);
        let speed = scaled_u16_non_na(self.output_shaft_speed_rpm, 0.125);
        data[1] = (speed & 0xFF) as u8;
        data[2] = ((speed >> 8) & 0xFF) as u8;
        data[3] = etc1_gear_raw(self.current_gear);
        data[4] = etc1_gear_raw(self.selected_gear);
        data
    }

    /// Decode from a complete 8-byte fixed-size payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || data[0] & 0xF0 != 0 || !has_ff_tail(data, 5) {
            return None;
        }
        let speed = (data[1] as u16) | ((data[2] as u16) << 8);
        if !u16_data_is_available(speed) || data[3] > 250 || data[4] > 250 {
            return None;
        }
        Some(Self {
            shift_in_progress: data[0] & 0x03,
            torque_converter_lockup: (data[0] >> 2) & 0x03,
            output_shaft_speed_rpm: speed as f64 * 0.125,
            current_gear: (data[3] as i16 - 125) as i8,
            selected_gear: (data[4] as i16 - 125) as i8,
        })
    }

    #[inline]
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_ETC1) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── Transmission Oil Temperature (subset of ET2, PGN 0x0FEED) ─────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransmissionOilTemp {
    /// SPN 177: 0.03125 °C/bit, offset −273, 2 bytes.
    pub oil_temp_c: f64,
}

impl Default for TransmissionOilTemp {
    fn default() -> Self {
        Self { oil_temp_c: -40.0 }
    }
}

impl TransmissionOilTemp {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let raw = offset_scaled_u16_non_na(self.oil_temp_c, 273.0, 0.03125);
        data[0] = (raw & 0xFF) as u8;
        data[1] = ((raw >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || !has_ff_tail(data, 2) {
            return None;
        }
        let raw = (data[0] as u16) | ((data[1] as u16) << 8);
        if !u16_data_is_available(raw) {
            return None;
        }
        Some(Self {
            oil_temp_c: raw as f64 * 0.03125 - 273.0,
        })
    }
}

// ─── Cruise Control / Vehicle Speed (CCVS, PGN 0x0FEF1) ───────────────

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CruiseControl {
    /// SPN 84: 1/256 km/h per bit, 2 bytes.
    pub wheel_speed_kmh: f64,
    /// SPN 595, 2 bits (0=off, 1=on, 2=error, 3=N/A).
    pub cc_active: u8,
    /// SPN 597, 2 bits.
    pub brake_switch: u8,
    /// SPN 598, 2 bits.
    pub clutch_switch: u8,
    /// SPN 70, 2 bits.
    pub park_brake: u8,
    /// SPN 86: 1/256 km/h per bit, 2 bytes.
    pub cc_set_speed_kmh: f64,
}

impl CruiseControl {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        let speed = scaled_u16_non_na(self.wheel_speed_kmh, 1.0 / 256.0);
        data[0] = (speed & 0xFF) as u8;
        data[1] = ((speed >> 8) & 0xFF) as u8;
        data[2] = (self.cc_active & 0x03)
            | ((self.brake_switch & 0x03) << 2)
            | ((self.clutch_switch & 0x03) << 4)
            | ((self.park_brake & 0x03) << 6);
        let set_speed = scaled_u16_non_na(self.cc_set_speed_kmh, 1.0 / 256.0);
        data[3] = (set_speed & 0xFF) as u8;
        data[4] = ((set_speed >> 8) & 0xFF) as u8;
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 || !has_ff_tail(data, 5) {
            return None;
        }
        let speed = (data[0] as u16) | ((data[1] as u16) << 8);
        let set_speed = (data[3] as u16) | ((data[4] as u16) << 8);
        if !u16_data_is_available(speed) || !u16_data_is_available(set_speed) {
            return None;
        }
        Some(Self {
            wheel_speed_kmh: speed as f64 / 256.0,
            cc_active: data[2] & 0x03,
            brake_switch: (data[2] >> 2) & 0x03,
            clutch_switch: (data[2] >> 4) & 0x03,
            park_brake: (data[2] >> 6) & 0x03,
            cc_set_speed_kmh: set_speed as f64 / 256.0,
        })
    }

    #[inline]
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_CRUISE_CONTROL) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etc1_round_trip() {
        let m = Etc1 {
            current_gear: 5,
            selected_gear: 6,
            output_shaft_speed_rpm: 1500.0,
            shift_in_progress: 1,
            torque_converter_lockup: 2,
        };
        let bytes = m.encode();
        assert_eq!(bytes, [0x09, 0xE0, 0x2E, 0x82, 0x83, 0xFF, 0xFF, 0xFF]);
        let decoded = Etc1::decode(&bytes).unwrap();
        assert_eq!(decoded.current_gear, 5);
        assert_eq!(decoded.selected_gear, 6);
        assert_eq!(decoded.shift_in_progress, 1);
        assert_eq!(decoded.torque_converter_lockup, 2);
        assert!((decoded.output_shaft_speed_rpm - 1500.0).abs() < 0.125);
    }

    #[test]
    fn etc1_negative_gears_round_trip() {
        let m = Etc1 {
            current_gear: -10,
            selected_gear: -125,
            ..Default::default()
        };
        let decoded = Etc1::decode(&m.encode()).unwrap();
        assert_eq!(decoded.current_gear, -10);
        assert_eq!(decoded.selected_gear, -125);
    }

    #[test]
    fn transmission_oil_temp_round_trip() {
        let t = TransmissionOilTemp { oil_temp_c: 80.0 };
        assert_eq!(t.encode(), [0x20, 0x2C, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let decoded = TransmissionOilTemp::decode(&t.encode()).unwrap();
        assert!((decoded.oil_temp_c - 80.0).abs() < 0.05);
    }

    #[test]
    fn cruise_control_round_trip() {
        let cc = CruiseControl {
            wheel_speed_kmh: 65.5,
            cc_active: 1,
            brake_switch: 0,
            clutch_switch: 0,
            park_brake: 0,
            cc_set_speed_kmh: 65.0,
        };
        assert_eq!(
            cc.encode(),
            [0x80, 0x41, 0x01, 0x00, 0x41, 0xFF, 0xFF, 0xFF]
        );
        let decoded = CruiseControl::decode(&cc.encode()).unwrap();
        assert!((decoded.wheel_speed_kmh - 65.5).abs() < 1.0 / 256.0);
        assert_eq!(decoded.cc_active, 1);
        assert!((decoded.cc_set_speed_kmh - 65.0).abs() < 1.0 / 256.0);
    }

    #[test]
    fn short_payloads_return_none() {
        assert!(Etc1::decode(&[0u8; 7]).is_none());
        assert!(TransmissionOilTemp::decode(&[0u8; 7]).is_none());
        assert!(CruiseControl::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn overlong_payloads_return_none() {
        assert!(Etc1::decode(&[0u8; 9]).is_none());
        assert!(TransmissionOilTemp::decode(&[0u8; 9]).is_none());
        assert!(CruiseControl::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn reserved_padding_and_bits_are_rejected() {
        let mut etc1 = Etc1 {
            current_gear: 5,
            selected_gear: 6,
            output_shaft_speed_rpm: 1500.0,
            shift_in_progress: 1,
            torque_converter_lockup: 2,
        }
        .encode();
        etc1[5] = 0x00;
        assert!(Etc1::decode(&etc1).is_none());

        let mut etc1 = Etc1::default().encode();
        etc1[0] |= 0x10;
        assert!(Etc1::decode(&etc1).is_none());

        let mut oil = TransmissionOilTemp { oil_temp_c: 80.0 }.encode();
        oil[2] = 0x00;
        assert!(TransmissionOilTemp::decode(&oil).is_none());

        let mut cruise = CruiseControl {
            wheel_speed_kmh: 65.5,
            cc_active: 1,
            brake_switch: 0,
            clutch_switch: 0,
            park_brake: 0,
            cc_set_speed_kmh: 65.0,
        }
        .encode();
        cruise[5] = 0x00;
        assert!(CruiseControl::decode(&cruise).is_none());
    }

    #[test]
    fn unrepresentable_not_available_sentinels_are_rejected() {
        let mut etc1 = Etc1::default().encode();
        etc1[1] = 0xFF;
        etc1[2] = 0xFF;
        assert!(Etc1::decode(&etc1).is_none());

        let mut etc1 = Etc1::default().encode();
        etc1[3] = 0xFB;
        assert!(Etc1::decode(&etc1).is_none());

        let mut etc1 = Etc1::default().encode();
        etc1[4] = 0xFF;
        assert!(Etc1::decode(&etc1).is_none());

        assert!(TransmissionOilTemp::decode(&[0xFF; 8]).is_none());

        let mut cruise = CruiseControl::default().encode();
        cruise[0] = 0xFF;
        cruise[1] = 0xFF;
        assert!(CruiseControl::decode(&cruise).is_none());

        let mut cruise = CruiseControl::default().encode();
        cruise[3] = 0xFF;
        cruise[4] = 0xFF;
        assert!(CruiseControl::decode(&cruise).is_none());
    }

    #[test]
    fn encode_clamps_ranges_away_from_not_available_sentinels() {
        let etc1 = Etc1 {
            current_gear: -128,
            selected_gear: 127,
            output_shaft_speed_rpm: 99_999.0,
            shift_in_progress: 2,
            torque_converter_lockup: 3,
        };
        assert_eq!(
            etc1.encode(),
            [0x0E, 0xFD, 0xFF, 0x00, 0xFA, 0xFF, 0xFF, 0xFF]
        );

        assert_eq!(
            TransmissionOilTemp {
                oil_temp_c: 10_000.0,
            }
            .encode(),
            [0xFD, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );
        assert_eq!(
            TransmissionOilTemp {
                oil_temp_c: f64::NAN
            }
            .encode(),
            [0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]
        );

        let cruise = CruiseControl {
            wheel_speed_kmh: 10_000.0,
            cc_active: 0,
            brake_switch: 0,
            clutch_switch: 0,
            park_brake: 0,
            cc_set_speed_kmh: 10_000.0,
        };
        assert_eq!(
            cruise.encode(),
            [0xFD, 0xFF, 0x00, 0xFD, 0xFF, 0xFF, 0xFF, 0xFF]
        );
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

    proptest! {
        #[test]
        fn proptest_transmission_decoders_accept_or_reject_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=64),
        ) {
            assert_fixed_decoder_is_canonical!(Etc1, &data);
            assert_fixed_decoder_is_canonical!(TransmissionOilTemp, &data);
            assert_fixed_decoder_is_canonical!(CruiseControl, &data);
        }
    }
}
