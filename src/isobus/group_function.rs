//! ISO 11783-3 §5.4.6 Group Function Protocol.
//!
//! Mirrors the C++ `machbus::isobus::group_function.hpp`. Group
//! functions piggyback on `PGN_ACKNOWLEDGMENT`. The codec is pure
//! data-model code; [`GroupFunctionResponder`] provides a small,
//! deterministic responder policy and the `session` facade wires it into
//! the bus when requested.

use alloc::{format, vec, vec::Vec};

use crate::net::error::{Error, Result};
use crate::net::pgn::pgn_is_valid;
use crate::net::types::Pgn;

/// Group function command type byte (`data\[0\]`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum GroupFunctionType {
    #[default]
    Request = 0,
    Command = 1,
    Acknowledge = 2,
    ReadReply = 3,
    /// Wire sentinel for unknown / not-set type bytes (matches C++).
    Reserved = 0xFF,
}

impl GroupFunctionType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            0 => Self::Request,
            1 => Self::Command,
            2 => Self::Acknowledge,
            3 => Self::ReadReply,
            0xFF => Self::Reserved,
            _ => Self::Reserved,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Request),
            1 => Some(Self::Command),
            2 => Some(Self::Acknowledge),
            3 => Some(Self::ReadReply),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Acknowledgment-side error byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum GroupFunctionError {
    #[default]
    NoError = 0,
    UnsupportedPgn = 1,
    UnsupportedFunction = 2,
    InvalidParameter = 3,
    PermissionDenied = 4,
    Busy = 5,
}

impl GroupFunctionError {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::UnsupportedPgn,
            2 => Self::UnsupportedFunction,
            3 => Self::InvalidParameter,
            4 => Self::PermissionDenied,
            5 => Self::Busy,
            _ => Self::NoError,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::NoError),
            1 => Some(Self::UnsupportedPgn),
            2 => Some(Self::UnsupportedFunction),
            3 => Some(Self::InvalidParameter),
            4 => Some(Self::PermissionDenied),
            5 => Some(Self::Busy),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Wire-format group-function message.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GroupFunctionMsg {
    pub function_type: GroupFunctionType,
    pub target_pgn: Pgn,
    /// Up to 4 parameter bytes (function-specific). Trailing `0xFF`s
    /// are stripped on decode (matches C++).
    pub parameters: Vec<u8>,
}

impl GroupFunctionMsg {
    pub fn encode(&self) -> Result<[u8; 8]> {
        if !pgn_is_valid(self.target_pgn) {
            return Err(Error::invalid_data(format!(
                "Group Function target PGN 0x{:X} exceeds the 18-bit J1939/ISOBUS PGN range",
                self.target_pgn
            )));
        }
        if self.parameters.len() > 4 {
            return Err(Error::invalid_data(format!(
                "Group Function parameter list has {} bytes but the wire format holds at most 4",
                self.parameters.len()
            )));
        }
        let mut data = [0xFFu8; 8];
        data[0] = self.function_type.as_u8();
        data[1] = (self.target_pgn & 0xFF) as u8;
        data[2] = ((self.target_pgn >> 8) & 0xFF) as u8;
        data[3] = ((self.target_pgn >> 16) & 0xFF) as u8;
        for (i, b) in self.parameters.iter().enumerate() {
            data[4 + i] = *b;
        }
        Ok(data)
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        let function_type = GroupFunctionType::try_from_u8(data[0])?;
        let mut parameters = Vec::new();
        let mut padding_started = false;
        for &b in data.iter().skip(4) {
            if padding_started {
                if b != 0xFF {
                    return None;
                }
                continue;
            }
            if b == 0xFF {
                padding_started = true;
                continue;
            }
            parameters.push(b);
        }
        let target_pgn = (data[1] as Pgn) | ((data[2] as Pgn) << 8) | ((data[3] as Pgn) << 16);
        if !pgn_is_valid(target_pgn) {
            return None;
        }
        Some(Self {
            function_type,
            target_pgn,
            parameters,
        })
    }

    /// Construct a Group Function Acknowledge for an inbound request/command.
    ///
    /// The first parameter byte carries [`GroupFunctionError`]. The remaining
    /// parameter bytes are wire padding (`0xFF`) through [`Self::encode`].
    #[must_use]
    pub fn acknowledge(target_pgn: Pgn, error: GroupFunctionError) -> Self {
        Self {
            function_type: GroupFunctionType::Acknowledge,
            target_pgn,
            parameters: vec![error.as_u8()],
        }
    }
}

/// Supported Group Function operations for one target PGN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupFunctionSupport {
    pub target_pgn: Pgn,
    pub request: bool,
    pub command: bool,
}

impl GroupFunctionSupport {
    /// Support Group Function Request for `target_pgn`.
    #[must_use]
    pub const fn request(target_pgn: Pgn) -> Self {
        Self {
            target_pgn,
            request: true,
            command: false,
        }
    }

    /// Support Group Function Command for `target_pgn`.
    #[must_use]
    pub const fn command(target_pgn: Pgn) -> Self {
        Self {
            target_pgn,
            request: false,
            command: true,
        }
    }

    /// Support both Group Function Request and Command for `target_pgn`.
    #[must_use]
    pub const fn request_and_command(target_pgn: Pgn) -> Self {
        Self {
            target_pgn,
            request: true,
            command: true,
        }
    }

    /// Override request support.
    #[must_use]
    pub const fn with_request(mut self, enabled: bool) -> Self {
        self.request = enabled;
        self
    }

    /// Override command support.
    #[must_use]
    pub const fn with_command(mut self, enabled: bool) -> Self {
        self.command = enabled;
        self
    }

    #[must_use]
    pub const fn supports(self, function_type: GroupFunctionType) -> bool {
        match function_type {
            GroupFunctionType::Request => self.request,
            GroupFunctionType::Command => self.command,
            GroupFunctionType::Acknowledge
            | GroupFunctionType::ReadReply
            | GroupFunctionType::Reserved => false,
        }
    }
}

/// Deterministic local responder policy for Group Function requests.
///
/// This type intentionally only answers request-like traffic (`Request` and
/// `Command`). It returns `None` for `Acknowledge` and `ReadReply` frames so a
/// stack cannot create acknowledge loops by replying to replies.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GroupFunctionResponder {
    supported: Vec<GroupFunctionSupport>,
}

impl GroupFunctionResponder {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            supported: Vec::new(),
        }
    }

    /// Build a responder from static support entries.
    #[must_use]
    pub fn with_supported(entries: impl IntoIterator<Item = GroupFunctionSupport>) -> Self {
        let mut responder = Self::new();
        for entry in entries {
            responder.set_support(entry);
        }
        responder
    }

    /// Add or replace support for `entry.target_pgn`.
    pub fn set_support(&mut self, entry: GroupFunctionSupport) {
        if let Some(existing) = self
            .supported
            .iter_mut()
            .find(|existing| existing.target_pgn == entry.target_pgn)
        {
            *existing = entry;
        } else {
            self.supported.push(entry);
        }
    }

    /// Convenience builder for [`Self::set_support`].
    #[must_use]
    pub fn supporting(mut self, entry: GroupFunctionSupport) -> Self {
        self.set_support(entry);
        self
    }

    /// Remove a target PGN from the support table.
    pub fn remove_support(&mut self, target_pgn: Pgn) {
        self.supported
            .retain(|entry| entry.target_pgn != target_pgn);
    }

    /// `true` if this responder has an entry for `target_pgn`.
    #[must_use]
    pub fn supports_pgn(&self, target_pgn: Pgn) -> bool {
        self.supported
            .iter()
            .any(|entry| entry.target_pgn == target_pgn)
    }

    /// `true` if this responder supports `function_type` for `target_pgn`.
    #[must_use]
    pub fn supports_function(&self, target_pgn: Pgn, function_type: GroupFunctionType) -> bool {
        self.supported
            .iter()
            .find(|entry| entry.target_pgn == target_pgn)
            .is_some_and(|entry| entry.supports(function_type))
    }

    /// Expose the configured support table for diagnostics/tests.
    #[must_use]
    pub fn supported(&self) -> &[GroupFunctionSupport] {
        &self.supported
    }

    /// Build the local response for one inbound Group Function message.
    ///
    /// Returns `None` for response-like input (`Acknowledge`, `ReadReply`, or
    /// `Reserved`) to avoid protocol loops. Unknown target PGNs receive
    /// `UnsupportedPgn`; unsupported operations for a known target receive
    /// `UnsupportedFunction`; supported request/command operations receive
    /// `NoError`.
    #[must_use]
    pub fn response_for(&self, msg: &GroupFunctionMsg) -> Option<GroupFunctionMsg> {
        match msg.function_type {
            GroupFunctionType::Request | GroupFunctionType::Command => {}
            GroupFunctionType::Acknowledge
            | GroupFunctionType::ReadReply
            | GroupFunctionType::Reserved => return None,
        }

        let error = self
            .supported
            .iter()
            .find(|entry| entry.target_pgn == msg.target_pgn)
            .map_or(GroupFunctionError::UnsupportedPgn, |entry| {
                if entry.supports(msg.function_type) {
                    GroupFunctionError::NoError
                } else {
                    GroupFunctionError::UnsupportedFunction
                }
            });
        Some(GroupFunctionMsg::acknowledge(msg.target_pgn, error))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_request_with_params() {
        let m = GroupFunctionMsg {
            function_type: GroupFunctionType::Command,
            target_pgn: 0xCAFE,
            parameters: vec![0x01, 0x02, 0x03],
        };
        let bytes = m.encode().unwrap();
        let decoded = GroupFunctionMsg::decode(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn round_trip_no_params() {
        let m = GroupFunctionMsg {
            function_type: GroupFunctionType::Request,
            target_pgn: 0xEA00,
            parameters: vec![],
        };
        let bytes = m.encode().unwrap();
        // Bytes 4..8 should all be 0xFF.
        assert_eq!(&bytes[4..], &[0xFFu8; 4]);
        let decoded = GroupFunctionMsg::decode(&bytes).unwrap();
        assert_eq!(decoded, m);
    }

    #[test]
    fn parameters_capped_at_four_bytes() {
        let m = GroupFunctionMsg {
            function_type: GroupFunctionType::Acknowledge,
            target_pgn: 0xABCD,
            parameters: vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
        };
        let err = m.encode().unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("at most 4"));
    }

    #[test]
    fn decode_short_payload_returns_none() {
        assert!(GroupFunctionMsg::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn decode_oversized_payload_returns_none() {
        assert!(GroupFunctionMsg::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn decode_rejects_reserved_function_and_bad_parameter_padding() {
        assert!(
            GroupFunctionMsg::decode(&[0xFF, 0xFE, 0xCA, 0x00, 0xFF, 0xFF, 0xFF, 0xFF]).is_none()
        );
        assert!(
            GroupFunctionMsg::decode(&[0x01, 0xFE, 0xCA, 0x00, 0x01, 0xFF, 0x02, 0xFF]).is_none()
        );
    }

    #[test]
    fn function_type_round_trips() {
        for t in [
            GroupFunctionType::Request,
            GroupFunctionType::Command,
            GroupFunctionType::Acknowledge,
            GroupFunctionType::ReadReply,
            GroupFunctionType::Reserved,
        ] {
            assert_eq!(GroupFunctionType::from_u8(t.as_u8()), t);
        }
        // Unknown bytes route to Reserved (the wire sentinel).
        assert_eq!(
            GroupFunctionType::from_u8(0x42),
            GroupFunctionType::Reserved
        );
        assert!(GroupFunctionType::try_from_u8(0xFF).is_none());
        assert!(GroupFunctionType::try_from_u8(0x42).is_none());
    }

    #[test]
    fn group_function_error_round_trips() {
        for e in [
            GroupFunctionError::NoError,
            GroupFunctionError::UnsupportedPgn,
            GroupFunctionError::UnsupportedFunction,
            GroupFunctionError::InvalidParameter,
            GroupFunctionError::PermissionDenied,
            GroupFunctionError::Busy,
        ] {
            assert_eq!(GroupFunctionError::from_u8(e.as_u8()), e);
        }
    }

    #[test]
    fn acknowledge_constructor_uses_first_parameter_error_byte() {
        let ack = GroupFunctionMsg::acknowledge(0xCAFE, GroupFunctionError::UnsupportedFunction);
        assert_eq!(ack.function_type, GroupFunctionType::Acknowledge);
        assert_eq!(ack.target_pgn, 0xCAFE);
        assert_eq!(
            ack.parameters,
            vec![GroupFunctionError::UnsupportedFunction.as_u8()]
        );
        assert_eq!(
            ack.encode().unwrap(),
            [0x02, 0xFE, 0xCA, 0x00, 0x02, 0xFF, 0xFF, 0xFF]
        );
    }

    #[test]
    fn encode_rejects_invalid_target_pgn() {
        let msg = GroupFunctionMsg {
            function_type: GroupFunctionType::Request,
            target_pgn: 0x40000,
            parameters: vec![],
        };
        let err = msg.encode().unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
        assert!(err.message.contains("PGN"));
    }

    #[test]
    fn decode_rejects_invalid_target_pgn_high_bits() {
        assert!(
            GroupFunctionMsg::decode(&[0x00, 0x00, 0x00, 0x04, 0xFF, 0xFF, 0xFF, 0xFF]).is_none()
        );
    }

    #[test]
    fn responder_acknowledges_supported_request_and_command() {
        let responder = GroupFunctionResponder::new()
            .supporting(GroupFunctionSupport::request_and_command(0xCAFE));
        for function_type in [GroupFunctionType::Request, GroupFunctionType::Command] {
            let msg = GroupFunctionMsg {
                function_type,
                target_pgn: 0xCAFE,
                parameters: vec![],
            };
            let response = responder.response_for(&msg).unwrap();
            assert_eq!(
                response,
                GroupFunctionMsg::acknowledge(0xCAFE, GroupFunctionError::NoError)
            );
        }
    }

    #[test]
    fn responder_distinguishes_unknown_pgn_and_unsupported_function() {
        let responder =
            GroupFunctionResponder::new().supporting(GroupFunctionSupport::request(0xCAFE));
        let unsupported_pgn = GroupFunctionMsg {
            function_type: GroupFunctionType::Request,
            target_pgn: 0xBEEF,
            parameters: vec![],
        };
        assert_eq!(
            responder.response_for(&unsupported_pgn),
            Some(GroupFunctionMsg::acknowledge(
                0xBEEF,
                GroupFunctionError::UnsupportedPgn
            ))
        );

        let unsupported_function = GroupFunctionMsg {
            function_type: GroupFunctionType::Command,
            target_pgn: 0xCAFE,
            parameters: vec![0x01],
        };
        assert_eq!(
            responder.response_for(&unsupported_function),
            Some(GroupFunctionMsg::acknowledge(
                0xCAFE,
                GroupFunctionError::UnsupportedFunction
            ))
        );
    }

    #[test]
    fn responder_does_not_answer_replies() {
        let responder = GroupFunctionResponder::new()
            .supporting(GroupFunctionSupport::request_and_command(0xCAFE));
        for function_type in [GroupFunctionType::Acknowledge, GroupFunctionType::ReadReply] {
            let msg = GroupFunctionMsg {
                function_type,
                target_pgn: 0xCAFE,
                parameters: vec![],
            };
            assert_eq!(responder.response_for(&msg), None);
        }
    }
}
