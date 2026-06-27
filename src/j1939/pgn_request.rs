//! PGN Request (`PGN_REQUEST = 0xEA00`) — codec only.
//!
//! Mirrors the C++ `machbus::j1939::pgn_request.hpp`. The C++
//! `PGNRequestProtocol` class (responder registry, automatic NACK
//! synthesis, IsoNet integration) is intentionally not ported —
//! responder dispatch belongs at the `IsoNet` layer where users
//! already have the network handle. The wire codec is the ported
//! piece.

use alloc::format;

use crate::net::error::{Error, Result};
use crate::net::message::Message;
use crate::net::pgn::pgn_is_valid;
use crate::net::pgn_defs::PGN_REQUEST;
use crate::net::types::Pgn;

/// Decode a PGN-Request payload into the requested PGN.
///
/// The canonical Request payload is 3 bytes. Some stacks put the same
/// payload in an 8-byte classic-CAN frame with `0xFF` padding, so that
/// compatibility shape is accepted too. Other prefix-compatible payloads are
/// rejected instead of silently ignoring trailing data.
#[must_use]
pub fn decode_request(data: &[u8]) -> Option<Pgn> {
    let valid_len = data.len() == 3 || (data.len() == 8 && data[3..].iter().all(|&b| b == 0xFF));
    if !valid_len {
        return None;
    }
    let pgn = (data[0] as Pgn) | ((data[1] as Pgn) << 8) | ((data[2] as Pgn) << 16);
    pgn_is_valid(pgn).then_some(pgn)
}

/// Convenience: decode the requested PGN from a [`Message`].
#[must_use]
pub fn requested_pgn(msg: &Message) -> Option<Pgn> {
    if !msg.has_usable_envelope_for_pgn(PGN_REQUEST) {
        return None;
    }
    decode_request(&msg.data)
}

/// Encode a PGN-Request payload (3 bytes).
pub fn encode_request(pgn: Pgn) -> Result<[u8; 3]> {
    if !pgn_is_valid(pgn) {
        return Err(Error::invalid_data(format!(
            "requested PGN 0x{pgn:X} exceeds the 18-bit J1939/ISOBUS PGN range"
        )));
    }
    Ok([
        (pgn & 0xFF) as u8,
        ((pgn >> 8) & 0xFF) as u8,
        ((pgn >> 16) & 0xFF) as u8,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::{PGN_ADDRESS_CLAIMED, PGN_REQUEST};
    use crate::net::{BROADCAST_ADDRESS, NULL_ADDRESS};

    #[test]
    fn request_round_trips() {
        for pgn in [PGN_ADDRESS_CLAIMED, 0xFEDA, 0x1EF00] {
            let bytes = encode_request(pgn).unwrap();
            assert_eq!(decode_request(&bytes), Some(pgn));
        }
    }

    #[test]
    fn decode_short_payload_returns_none() {
        assert!(decode_request(&[0u8; 2]).is_none());
    }

    #[test]
    fn decode_accepts_ff_padded_classic_frame() {
        let bytes = [0x00, 0xEE, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF];
        assert_eq!(decode_request(&bytes), Some(PGN_ADDRESS_CLAIMED));
    }

    #[test]
    fn decode_rejects_malformed_trailing_data() {
        assert!(decode_request(&[0x00, 0xEE, 0x00, 0xFF]).is_none());
        assert!(decode_request(&[0x00, 0xEE, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]).is_none());
        assert!(decode_request(&[0x00, 0xEE, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_none());
    }

    #[test]
    fn decode_rejects_invalid_pgn_high_bits() {
        assert!(decode_request(&[0x00, 0xEE, 0x04]).is_none());
        assert!(decode_request(&[0x00, 0xEE, 0x04, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]).is_none());
    }

    #[test]
    fn from_message_works() {
        let bytes = encode_request(PGN_ADDRESS_CLAIMED).unwrap();
        let msg = Message::new(PGN_REQUEST, bytes.to_vec(), 0x10);
        assert_eq!(requested_pgn(&msg), Some(PGN_ADDRESS_CLAIMED));
        assert_eq!(
            requested_pgn(&Message::new(PGN_ADDRESS_CLAIMED, bytes.to_vec(), 0x10)),
            None
        );
        assert_eq!(
            requested_pgn(&Message::new(PGN_REQUEST, bytes.to_vec(), NULL_ADDRESS)),
            None
        );
        assert_eq!(
            requested_pgn(&Message::new(
                PGN_REQUEST,
                bytes.to_vec(),
                BROADCAST_ADDRESS
            )),
            None
        );
    }

    #[test]
    fn encode_rejects_invalid_pgn_high_bits() {
        let err = encode_request(0x40000).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));
    }
}
