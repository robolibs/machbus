//! Request2 / Transfer protocol — codec only.
//!
//! Mirrors the C++ `machbus::j1939::request2.hpp`. Request2 (ISO
//! 11783-3 §5.4.7) extends PGN-Request with up to 3 bytes of extended
//! identifier and an optional flag asking the responder to reply via
//! the [`PGN_TRANSFER`] PGN instead of the requested PGN.
//!
//! A small [`Request2Responder`] helper covers the C++ `Request2Protocol`
//! responder registry shape without coupling this codec module to `IsoNet`:
//! callers register deterministic payloads and route the returned
//! [`Request2Reply`] through their transport of choice.
//!
//! [`PGN_TRANSFER`]: crate::net::pgn_defs::PGN_TRANSFER

use alloc::{collections::BTreeMap, format, vec::Vec};

use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::error::{Error, Result};
use crate::net::message::Message;
use crate::net::pgn::pgn_is_valid;
use crate::net::pgn_defs::{PGN_REQUEST2, PGN_TRANSFER};
use crate::net::types::{Address, Pgn};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Request2Msg {
    pub requested_pgn: Pgn,
    pub extended_id: Vec<u8>,
    pub use_transfer: bool,
}

impl Request2Msg {
    /// Encode to an 8-byte payload. The extended ID is up to 3
    /// bytes at offsets 4–6; unused bytes are `0xFF`.
    pub fn encode(&self) -> Result<[u8; 8]> {
        if !pgn_is_valid(self.requested_pgn) {
            return Err(Error::invalid_data(format!(
                "Request2 requested PGN 0x{:X} exceeds the 18-bit J1939/ISOBUS PGN range",
                self.requested_pgn
            )));
        }
        if self.extended_id.len() > 3 {
            return Err(Error::invalid_data(format!(
                "Request2 extended identifier has {} bytes but the wire format holds at most 3",
                self.extended_id.len()
            )));
        }
        let mut data = [0xFFu8; 8];
        data[0] = (self.requested_pgn & 0xFF) as u8;
        data[1] = ((self.requested_pgn >> 8) & 0xFF) as u8;
        data[2] = ((self.requested_pgn >> 16) & 0xFF) as u8;
        data[3] = u8::from(self.use_transfer);
        for (i, b) in self.extended_id.iter().enumerate() {
            data[4 + i] = *b;
        }
        Ok(data)
    }

    /// Decode from a classic 8-byte payload. Reads up to 3
    /// non-`0xFF` extended-ID bytes from offsets 4–6.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[3] & 0xFE != 0 {
            return None;
        }
        if data[7] != 0xFF {
            return None;
        }
        let requested_pgn = (data[0] as Pgn) | ((data[1] as Pgn) << 8) | ((data[2] as Pgn) << 16);
        if !pgn_is_valid(requested_pgn) {
            return None;
        }
        let use_transfer = (data[3] & 0x01) != 0;
        let mut extended_id = Vec::new();
        let mut seen_padding = false;
        for &b in &data[4..7] {
            if b != 0xFF {
                if seen_padding {
                    return None;
                }
                extended_id.push(b);
            } else {
                seen_padding = true;
            }
        }
        Some(Self {
            requested_pgn,
            extended_id,
            use_transfer,
        })
    }

    #[inline]
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_REQUEST2) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

/// Transfer-PGN payload: prefixed with the 3-byte original PGN
/// followed by the response data.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TransferMsg {
    pub original_pgn: Pgn,
    pub data: Vec<u8>,
}

impl TransferMsg {
    /// Encode to a variable-length payload (`3 + data.len()` bytes).
    pub fn encode(&self) -> Result<Vec<u8>> {
        if !pgn_is_valid(self.original_pgn) {
            return Err(Error::invalid_data(format!(
                "Transfer original PGN 0x{:X} exceeds the 18-bit J1939/ISOBUS PGN range",
                self.original_pgn
            )));
        }
        let mut out = Vec::with_capacity(3 + self.data.len());
        out.push((self.original_pgn & 0xFF) as u8);
        out.push(((self.original_pgn >> 8) & 0xFF) as u8);
        out.push(((self.original_pgn >> 16) & 0xFF) as u8);
        out.extend_from_slice(&self.data);
        Ok(out)
    }

    /// Decode from a payload (must be ≥ 3 bytes).
    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        if raw.len() < 3 {
            return None;
        }
        let original_pgn = (raw[0] as Pgn) | ((raw[1] as Pgn) << 8) | ((raw[2] as Pgn) << 16);
        if !pgn_is_valid(original_pgn) {
            return None;
        }
        Some(Self {
            original_pgn,
            data: raw[3..].to_vec(),
        })
    }

    #[inline]
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_TRANSFER) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

/// Response returned by [`Request2Responder::handle_message`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request2Reply {
    pub pgn: Pgn,
    pub destination: Address,
    pub data: Vec<u8>,
}

impl Request2Reply {
    #[must_use]
    pub fn new(pgn: Pgn, destination: Address, data: Vec<u8>) -> Self {
        Self {
            pgn,
            destination,
            data,
        }
    }
}

/// Minimal Request2 responder registry.
///
/// It intentionally returns plain reply metadata instead of sending on a bus.
/// If the request sets `use_transfer`, the registered payload is wrapped in a
/// [`TransferMsg`] and the reply PGN is [`PGN_TRANSFER`]; otherwise the
/// registered payload is returned on the requested PGN.
#[derive(Debug, Clone, Default)]
pub struct Request2Responder {
    responses: BTreeMap<Pgn, Vec<u8>>,
}

impl Request2Responder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a deterministic response payload for `pgn`.
    pub fn register_response(&mut self, pgn: Pgn, data: impl Into<Vec<u8>>) -> Result<()> {
        if !pgn_is_valid(pgn) {
            return Err(Error::invalid_data(format!(
                "Request2 response PGN 0x{pgn:X} exceeds the 18-bit J1939/ISOBUS PGN range"
            )));
        }
        self.responses.insert(pgn, data.into());
        Ok(())
    }

    /// Fluent registration helper.
    pub fn with_response(mut self, pgn: Pgn, data: impl Into<Vec<u8>>) -> Result<Self> {
        self.register_response(pgn, data)?;
        Ok(self)
    }

    #[must_use]
    pub fn contains(&self, pgn: Pgn) -> bool {
        self.responses.contains_key(&pgn)
    }

    #[must_use]
    pub fn response_count(&self) -> usize {
        self.responses.len()
    }

    pub fn remove_response(&mut self, pgn: Pgn) -> Option<Vec<u8>> {
        self.responses.remove(&pgn)
    }

    /// Handle a decoded Request2 message from `source`.
    pub fn handle_request(&self, source: Address, request: &Request2Msg) -> Option<Request2Reply> {
        if source == NULL_ADDRESS || source == BROADCAST_ADDRESS {
            return None;
        }
        let response = self.responses.get(&request.requested_pgn)?;
        if request.use_transfer {
            let data = TransferMsg {
                original_pgn: request.requested_pgn,
                data: response.clone(),
            }
            .encode()
            .ok()?;
            Some(Request2Reply::new(PGN_TRANSFER, source, data))
        } else {
            Some(Request2Reply::new(
                request.requested_pgn,
                source,
                response.clone(),
            ))
        }
    }

    /// Decode and handle an incoming PGN Request2 network message.
    #[must_use]
    pub fn handle_message(&self, msg: &Message) -> Option<Request2Reply> {
        if msg.pgn != PGN_REQUEST2 {
            return None;
        }
        let request = Request2Msg::from_message(msg)?;
        self.handle_request(msg.source, &request)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_TIME_DATE;

    #[test]
    fn request2_round_trips() {
        let req = Request2Msg {
            requested_pgn: 0xCAFE,
            extended_id: vec![0x01, 0x02, 0x03],
            use_transfer: true,
        };
        let decoded = Request2Msg::decode(&req.encode().unwrap()).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn request2_no_extended_id_round_trips() {
        let req = Request2Msg {
            requested_pgn: 0xEA00,
            extended_id: vec![],
            use_transfer: false,
        };
        let decoded = Request2Msg::decode(&req.encode().unwrap()).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn request2_short_payload_returns_none() {
        assert!(Request2Msg::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn request2_oversized_payload_returns_none() {
        assert!(Request2Msg::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn request2_rejects_reserved_control_and_padding() {
        let mut payload = Request2Msg {
            requested_pgn: 0xCAFE,
            extended_id: vec![0x01],
            use_transfer: true,
        }
        .encode()
        .unwrap();
        payload[3] |= 0x02;
        assert!(Request2Msg::decode(&payload).is_none());

        let mut payload = Request2Msg {
            requested_pgn: 0xCAFE,
            extended_id: vec![0x01],
            use_transfer: true,
        }
        .encode()
        .unwrap();
        payload[5] = 0xFF;
        payload[6] = 0x03;
        assert!(Request2Msg::decode(&payload).is_none());

        let mut payload = Request2Msg {
            requested_pgn: 0xCAFE,
            extended_id: vec![0x01],
            use_transfer: true,
        }
        .encode()
        .unwrap();
        payload[7] = 0x00;
        assert!(Request2Msg::decode(&payload).is_none());
    }

    #[test]
    fn request2_rejects_invalid_pgn_high_bits() {
        assert!(Request2Msg::decode(&[0xFE, 0xCA, 0x04, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]).is_none());
    }

    #[test]
    fn request2_encode_rejects_invalid_pgn_and_overlong_extended_id() {
        let err = Request2Msg {
            requested_pgn: 0x40000,
            ..Default::default()
        }
        .encode()
        .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));

        let err = Request2Msg {
            requested_pgn: 0xCAFE,
            extended_id: vec![1, 2, 3, 4],
            use_transfer: false,
        }
        .encode()
        .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("at most 3"));
    }

    #[test]
    fn transfer_round_trips() {
        let t = TransferMsg {
            original_pgn: 0x1234,
            data: vec![0xDE, 0xAD, 0xBE, 0xEF],
        };
        let decoded = TransferMsg::decode(&t.encode().unwrap()).unwrap();
        assert_eq!(decoded, t);
    }

    #[test]
    fn transfer_short_payload_returns_none() {
        assert!(TransferMsg::decode(&[0u8; 2]).is_none());
    }

    #[test]
    fn transfer_rejects_invalid_original_pgn_high_bits() {
        assert!(TransferMsg::decode(&[0xE6, 0xFE, 0x04, 0x12]).is_none());
    }

    #[test]
    fn transfer_encode_rejects_invalid_original_pgn_high_bits() {
        let err = TransferMsg {
            original_pgn: 0x40000,
            data: vec![0x12],
        }
        .encode()
        .unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));
    }

    #[test]
    fn from_message_works() {
        let req = Request2Msg {
            requested_pgn: 0xABCD,
            ..Default::default()
        };
        let msg = Message::new(PGN_REQUEST2, req.encode().unwrap().to_vec(), 0x10);
        assert_eq!(Request2Msg::from_message(&msg).unwrap(), req);
    }

    #[test]
    fn request2_responder_returns_direct_reply_for_registered_pgn() {
        let responder = Request2Responder::new()
            .with_response(
                PGN_TIME_DATE,
                [0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78],
            )
            .unwrap();
        let request = Request2Msg {
            requested_pgn: PGN_TIME_DATE,
            extended_id: vec![],
            use_transfer: false,
        };
        let msg = Message::new(PGN_REQUEST2, request.encode().unwrap().to_vec(), 0xA5);
        let reply = responder.handle_message(&msg).unwrap();

        assert_eq!(reply.pgn, PGN_TIME_DATE);
        assert_eq!(reply.destination, 0xA5);
        assert_eq!(
            reply.data,
            vec![0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78]
        );
    }

    #[test]
    fn request2_responder_wraps_reply_when_transfer_is_requested() {
        let mut responder = Request2Responder::new();
        responder
            .register_response(
                PGN_TIME_DATE,
                [0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78],
            )
            .unwrap();
        let request = Request2Msg {
            requested_pgn: PGN_TIME_DATE,
            extended_id: vec![0x01, 0x02],
            use_transfer: true,
        };
        let msg = Message::new(PGN_REQUEST2, request.encode().unwrap().to_vec(), 0xA5);
        let reply = responder.handle_message(&msg).unwrap();
        let wrapped = TransferMsg::decode(&reply.data).unwrap();

        assert_eq!(reply.pgn, PGN_TRANSFER);
        assert_eq!(reply.destination, 0xA5);
        assert_eq!(wrapped.original_pgn, PGN_TIME_DATE);
        assert_eq!(
            wrapped.data,
            vec![0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78]
        );
    }

    #[test]
    fn request2_responder_ignores_unknown_malformed_wrong_pgn_and_invalid_sources() {
        let responder = Request2Responder::new()
            .with_response(
                PGN_TIME_DATE,
                [0xA4, 0x31, 0x16, 0x08, 0x1C, 0x26, 0x7D, 0x78],
            )
            .unwrap();
        let request = Request2Msg {
            requested_pgn: PGN_TIME_DATE,
            extended_id: vec![],
            use_transfer: false,
        };
        let valid_payload = request.encode().unwrap().to_vec();

        assert!(
            responder
                .handle_message(&Message::new(PGN_TIME_DATE, valid_payload.clone(), 0xA5))
                .is_none(),
            "non-Request2 PGNs must not be interpreted as Request2"
        );
        assert!(
            responder
                .handle_message(&Message::new(PGN_REQUEST2, vec![0; 7], 0xA5))
                .is_none(),
            "malformed Request2 payloads must not produce replies"
        );
        assert!(
            responder
                .handle_message(&Message::new(
                    PGN_REQUEST2,
                    Request2Msg {
                        requested_pgn: 0x1234,
                        ..Default::default()
                    }
                    .encode()
                    .unwrap()
                    .to_vec(),
                    0xA5,
                ))
                .is_none(),
            "unregistered requested PGNs must not produce replies"
        );
        assert!(
            responder
                .handle_message(&Message::new(
                    PGN_REQUEST2,
                    valid_payload.clone(),
                    NULL_ADDRESS,
                ))
                .is_none(),
            "NULL source addresses must not produce replies"
        );
        assert!(
            responder
                .handle_message(&Message::new(
                    PGN_REQUEST2,
                    valid_payload,
                    BROADCAST_ADDRESS
                ))
                .is_none(),
            "broadcast source addresses must not produce replies"
        );
    }

    #[test]
    fn request2_responder_rejects_invalid_registered_pgn_and_removes_entries() {
        let mut responder = Request2Responder::new();
        let err = responder.register_response(0x40000, [0x00]).unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert_eq!(responder.response_count(), 0);

        responder.register_response(PGN_TIME_DATE, [0x01]).unwrap();
        assert!(responder.contains(PGN_TIME_DATE));
        assert_eq!(responder.remove_response(PGN_TIME_DATE), Some(vec![0x01]));
        assert!(!responder.contains(PGN_TIME_DATE));
    }
}
