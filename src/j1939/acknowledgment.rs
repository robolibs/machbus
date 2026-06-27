//! J1939 Acknowledgment message (`PGN_ACKNOWLEDGMENT = 0xE800`).
//!
//! Mirrors the C++ `machbus::j1939::acknowledgment.hpp`. Encodes the
//! ACK / NACK / Access-Denied / Cannot-Respond control byte plus the
//! PGN being acknowledged and the address it was acknowledged for.

use alloc::format;

use crate::net::error::{Error, Result};
use crate::net::message::Message;
use crate::net::pgn::pgn_is_valid;
use crate::net::pgn_defs::PGN_ACKNOWLEDGMENT;
use crate::net::types::{Address, Pgn};

/// Acknowledgment control byte (J1939-21 Table 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum AckControl {
    #[default]
    PositiveAck = 0,
    NegativeAck = 1,
    AccessDenied = 2,
    CannotRespond = 3,
}

impl AckControl {
    #[inline]
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match Self::try_from_u8(v) {
            Some(control) => control,
            None => Self::CannotRespond,
        }
    }

    #[inline]
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::PositiveAck),
            1 => Some(Self::NegativeAck),
            2 => Some(Self::AccessDenied),
            3 => Some(Self::CannotRespond),
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
pub struct Acknowledgment {
    pub control: AckControl,
    pub group_function: u8,
    pub acknowledged_pgn: Pgn,
    pub address: Address,
}

impl Default for Acknowledgment {
    fn default() -> Self {
        Self {
            control: AckControl::PositiveAck,
            group_function: 0xFF,
            acknowledged_pgn: 0,
            address: 0xFF,
        }
    }
}

impl Acknowledgment {
    /// Construct a positive ACK for `pgn` from `addr`.
    #[must_use]
    pub const fn ack(pgn: Pgn, addr: Address) -> Self {
        Self {
            control: AckControl::PositiveAck,
            group_function: 0xFF,
            acknowledged_pgn: pgn,
            address: addr,
        }
    }

    /// Construct a NACK for `pgn` from `addr`.
    #[must_use]
    pub const fn nack(pgn: Pgn, addr: Address) -> Self {
        Self {
            control: AckControl::NegativeAck,
            group_function: 0xFF,
            acknowledged_pgn: pgn,
            address: addr,
        }
    }

    /// Encode to the 8-byte wire format.
    pub fn encode(&self) -> Result<[u8; 8]> {
        if !pgn_is_valid(self.acknowledged_pgn) {
            return Err(Error::invalid_data(format!(
                "acknowledged PGN 0x{:X} exceeds the 18-bit J1939/ISOBUS PGN range",
                self.acknowledged_pgn
            )));
        }
        let mut data = [0xFFu8; 8];
        data[0] = self.control.as_u8();
        data[1] = self.group_function;
        data[4] = self.address;
        data[5] = (self.acknowledged_pgn & 0xFF) as u8;
        data[6] = ((self.acknowledged_pgn >> 8) & 0xFF) as u8;
        data[7] = ((self.acknowledged_pgn >> 16) & 0xFF) as u8;
        Ok(data)
    }

    /// Decode from an exact 8-byte payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[2] != 0xFF || data[3] != 0xFF {
            return None;
        }
        let control = AckControl::try_from_u8(data[0])?;
        let acknowledged_pgn =
            (data[5] as Pgn) | ((data[6] as Pgn) << 8) | ((data[7] as Pgn) << 16);
        if !pgn_is_valid(acknowledged_pgn) {
            return None;
        }
        Some(Self {
            control,
            group_function: data[1],
            acknowledged_pgn,
            address: data[4],
        })
    }

    /// Convenience: decode from a [`Message`].
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_ACKNOWLEDGMENT) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_ACKNOWLEDGMENT;

    #[test]
    fn ack_round_trips() {
        let ack = Acknowledgment::ack(0xEA00, 0x42);
        let bytes = ack.encode().unwrap();
        assert_eq!(bytes[0], 0); // PositiveAck
        assert_eq!(bytes[4], 0x42);
        assert_eq!(bytes[5..8], [0x00, 0xEA, 0x00]);
        let decoded = Acknowledgment::decode(&bytes).unwrap();
        assert_eq!(decoded, ack);
    }

    #[test]
    fn nack_round_trips() {
        let nack = Acknowledgment::nack(0xCAFE, 0x10);
        let decoded = Acknowledgment::decode(&nack.encode().unwrap()).unwrap();
        assert_eq!(decoded.control, AckControl::NegativeAck);
        assert_eq!(decoded.acknowledged_pgn, 0xCAFE);
        assert_eq!(decoded.address, 0x10);
    }

    #[test]
    fn from_message_works() {
        let bytes = Acknowledgment::ack(0xCAFE, 0x10).encode().unwrap();
        let msg = Message::new(PGN_ACKNOWLEDGMENT, bytes.to_vec(), 0x10);
        let decoded = Acknowledgment::from_message(&msg).unwrap();
        assert_eq!(decoded.acknowledged_pgn, 0xCAFE);
    }

    #[test]
    fn decode_short_payload_returns_none() {
        assert!(Acknowledgment::decode(&[0u8; 4]).is_none());
    }

    #[test]
    fn decode_oversized_payload_returns_none() {
        let mut bytes = Acknowledgment::ack(0xCAFE, 0x10).encode().unwrap().to_vec();
        bytes.push(0xFF);
        assert!(Acknowledgment::decode(&bytes).is_none());
    }

    #[test]
    fn decode_reserved_control_returns_none() {
        let mut bytes = Acknowledgment::ack(0xCAFE, 0x10).encode().unwrap();
        bytes[0] = 0x04;
        assert!(Acknowledgment::decode(&bytes).is_none());
    }

    #[test]
    fn decode_reserved_padding_returns_none() {
        let mut bytes = Acknowledgment::ack(0xCAFE, 0x10).encode().unwrap();
        bytes[2] = 0x00;
        assert!(Acknowledgment::decode(&bytes).is_none());

        let mut bytes = Acknowledgment::ack(0xCAFE, 0x10).encode().unwrap();
        bytes[3] = 0x00;
        assert!(Acknowledgment::decode(&bytes).is_none());
    }

    #[test]
    fn decode_invalid_pgn_high_bits_returns_none() {
        let bytes = [0x00, 0xFF, 0xFF, 0xFF, 0x42, 0x00, 0xEA, 0x04];
        assert!(Acknowledgment::decode(&bytes).is_none());
    }

    #[test]
    fn encode_rejects_invalid_acknowledged_pgn_high_bits() {
        let err = Acknowledgment::ack(0x40000, 0x10).encode().unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));
    }

    #[test]
    fn ack_control_round_trips() {
        for c in [
            AckControl::PositiveAck,
            AckControl::NegativeAck,
            AckControl::AccessDenied,
            AckControl::CannotRespond,
        ] {
            assert_eq!(AckControl::from_u8(c.as_u8()), c);
            assert_eq!(AckControl::try_from_u8(c.as_u8()), Some(c));
        }
        assert_eq!(AckControl::from_u8(0xFE), AckControl::CannotRespond);
        assert_eq!(AckControl::try_from_u8(0xFE), None);
    }
}
