//! ISO 11783-6:2018 VT v5 Auxiliary Channel Capability discovery.
//!
//! Mirrors the C++ `machbus::isobus::vt::auxiliary_caps.hpp`. The
//! C++ `AuxCapabilityDiscovery` class is IsoNet-coupled; the Rust
//! port is pump-style:
//!
//! - [`AuxCapabilityDiscovery::request_capabilities`] returns the
//!   8-byte payload to send on `PGN_ECU_TO_VT`.
//! - [`AuxCapabilityDiscovery::handle_response`] decodes an inbound
//!   `Get Supported Objects` response from the VT.

use alloc::{vec, vec::Vec};

use super::commands::cmd;
use crate::net::error::{Error, Result};
use crate::net::message::Message;
use crate::net::pgn_defs::PGN_VT_TO_ECU;

/// One auxiliary input channel descriptor (5 bytes on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AuxChannelCapability {
    pub channel_id: u8,
    /// 0 = boolean, 1 = analog, 2 = bidirectional.
    pub aux_type: u8,
    /// Step count for analog channels.
    pub resolution: u16,
    pub function_type: u8,
}

impl AuxChannelCapability {
    #[must_use]
    pub fn decode(data: &[u8], offset: usize) -> Option<Self> {
        if offset + 5 > data.len() {
            return None;
        }
        let channel = Self {
            channel_id: data[offset],
            aux_type: data[offset + 1],
            resolution: (data[offset + 2] as u16) | ((data[offset + 3] as u16) << 8),
            function_type: data[offset + 4],
        };
        channel.is_valid().then_some(channel)
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        vec![
            self.channel_id,
            self.aux_type,
            (self.resolution & 0xFF) as u8,
            ((self.resolution >> 8) & 0xFF) as u8,
            self.function_type,
        ]
    }

    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.aux_type <= 2
    }
}

/// Aggregated auxiliary capabilities reported by the VT.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuxCapabilities {
    pub channels: Vec<AuxChannelCapability>,
    pub vt_version: u8,
    pub discovery_complete: bool,
}

/// Pump-style discovery state machine.
#[derive(Debug, Default)]
pub struct AuxCapabilityDiscovery {
    caps: AuxCapabilities,
    request_pending: bool,
}

impl AuxCapabilityDiscovery {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build the 8-byte `Get Supported Objects` request payload.
    /// Returns an error if a request is already in flight.
    pub fn request_capabilities(&mut self) -> Result<[u8; 8]> {
        if self.request_pending {
            return Err(Error::invalid_state("request already pending"));
        }
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GET_SUPPORTED_OBJECTS;
        data[1] = 0x01; // Sub-function: aux capabilities query.
        data[2] = 31; // ObjectType::AuxFunction2.
        data[3] = 32; // ObjectType::AuxInput2.
        self.request_pending = true;
        Ok(data)
    }

    /// Decode an inbound capability response. Returns the populated
    /// [`AuxCapabilities`] when the response was valid, otherwise
    /// `None` (wrong command/sub-function, truncated payload, trailing bytes).
    pub fn handle_response(&mut self, msg: &Message) -> Option<&AuxCapabilities> {
        if !self.request_pending
            || !msg.has_usable_envelope_for_pgn(PGN_VT_TO_ECU)
            || msg.data.len() < 3
            || msg.data[0] != cmd::GET_SUPPORTED_OBJECTS
            || msg.data[1] != 0x01
        {
            return None;
        }

        let num_channels = msg.data[2] as usize;
        let expected_len = 3usize.checked_add(num_channels.checked_mul(5)?)?;
        if msg.data.len() != expected_len {
            return None;
        }

        let mut channels = Vec::with_capacity(num_channels);
        let mut offset = 3usize;
        for _ in 0..num_channels {
            channels.push(AuxChannelCapability::decode(&msg.data, offset)?);
            offset += 5;
        }

        self.request_pending = false;
        self.caps.channels = channels;
        self.caps.vt_version = 5;
        self.caps.discovery_complete = true;
        Some(&self.caps)
    }

    #[inline]
    #[must_use]
    pub fn capabilities(&self) -> &AuxCapabilities {
        &self.caps
    }

    #[inline]
    #[must_use]
    pub const fn is_request_pending(&self) -> bool {
        self.request_pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::pgn_defs::PGN_VT_TO_ECU;

    #[test]
    fn channel_round_trip() {
        let c = AuxChannelCapability {
            channel_id: 7,
            aux_type: 2,
            resolution: 1024,
            function_type: 5,
        };
        let bytes = c.encode();
        assert_eq!(AuxChannelCapability::decode(&bytes, 0), Some(c));
        assert_eq!(AuxChannelCapability::decode(&bytes[..4], 0), None);
    }

    #[test]
    fn request_then_response() {
        let mut d = AuxCapabilityDiscovery::new();
        let payload = d.request_capabilities().unwrap();
        assert_eq!(payload[0], cmd::GET_SUPPORTED_OBJECTS);
        assert!(d.is_request_pending());
        // Duplicate request should error.
        assert!(d.request_capabilities().is_err());

        let mut data = vec![cmd::GET_SUPPORTED_OBJECTS, 0x01, 2u8];
        data.extend(
            AuxChannelCapability {
                channel_id: 1,
                aux_type: 0,
                resolution: 0,
                function_type: 1,
            }
            .encode(),
        );
        data.extend(
            AuxChannelCapability {
                channel_id: 2,
                aux_type: 1,
                resolution: 1024,
                function_type: 2,
            }
            .encode(),
        );

        let msg = Message::new(PGN_VT_TO_ECU, data, 0x10);
        let caps = d.handle_response(&msg).unwrap();
        assert_eq!(caps.channels.len(), 2);
        assert_eq!(caps.channels[1].resolution, 1024);
        assert!(caps.discovery_complete);
        assert!(!d.is_request_pending());
    }

    #[test]
    fn response_with_wrong_command_byte_is_ignored() {
        let mut d = AuxCapabilityDiscovery::new();
        let _ = d.request_capabilities();
        let msg = Message::new(PGN_VT_TO_ECU, vec![0x12, 0x01, 0x00], 0x10);
        assert!(d.handle_response(&msg).is_none());
        assert!(d.is_request_pending());
    }

    #[test]
    fn response_with_wrong_subfunction_is_ignored() {
        let mut d = AuxCapabilityDiscovery::new();
        let _ = d.request_capabilities();
        let msg = Message::new(
            PGN_VT_TO_ECU,
            vec![cmd::GET_SUPPORTED_OBJECTS, 0x02, 0x00],
            0x10,
        );
        assert!(d.handle_response(&msg).is_none());
        assert!(d.is_request_pending());
    }

    #[test]
    fn response_rejects_truncated_channel_list() {
        let mut d = AuxCapabilityDiscovery::new();
        let _ = d.request_capabilities();
        // Header claims 5 channels but only 1 fits.
        let mut data = vec![cmd::GET_SUPPORTED_OBJECTS, 0x01, 5u8];
        data.extend(
            AuxChannelCapability {
                channel_id: 1,
                aux_type: 0,
                resolution: 0,
                function_type: 1,
            }
            .encode(),
        );
        let msg = Message::new(PGN_VT_TO_ECU, data, 0x10);
        assert!(d.handle_response(&msg).is_none());
        assert!(d.is_request_pending());
    }

    #[test]
    fn response_rejects_trailing_bytes() {
        let mut d = AuxCapabilityDiscovery::new();
        let _ = d.request_capabilities();
        let mut data = vec![cmd::GET_SUPPORTED_OBJECTS, 0x01, 0u8];
        data.push(0xFF);
        let msg = Message::new(PGN_VT_TO_ECU, data, 0x10);
        assert!(d.handle_response(&msg).is_none());
        assert!(d.is_request_pending());
    }
}
