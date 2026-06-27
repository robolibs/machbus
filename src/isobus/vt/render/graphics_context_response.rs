//! Graphics Context command response helpers.
//!
//! ISO 11783-6 F.57 uses the same VT function byte as the ECU-to-VT
//! Graphics Context command, but in the VT-to-ECU direction and with a fixed
//! eight-byte response payload. Keeping this parser/builder separate from the
//! retained render replay prevents command bytes and response error bits from
//! being handled as anonymous arrays.

use alloc::vec::Vec;

use crate::isobus::vt::ObjectID;
use crate::isobus::vt::commands::cmd;
use crate::net::pgn_defs::PGN_VT_TO_ECU;
use crate::net::{Address, Error, Message, Result};

use super::bus_message::validate_vt_to_ecu_envelope;

const GRAPHICS_CONTEXT_RESPONSE_ALLOWED_ERROR_BITS: u8 = 0x1F;

/// F.57 Graphics Context response error bitset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GraphicsContextErrorFlags {
    raw: u8,
}

impl GraphicsContextErrorFlags {
    pub const INVALID_OBJECT_ID: u8 = 0x01;
    pub const INVALID_SUBCOMMAND_ID: u8 = 0x02;
    pub const INVALID_PARAMETER: u8 = 0x04;
    pub const INVALID_RESULTS: u8 = 0x08;
    pub const OTHER_ERROR: u8 = 0x10;

    pub fn new(raw: u8) -> Result<Self> {
        if raw & !GRAPHICS_CONTEXT_RESPONSE_ALLOWED_ERROR_BITS != 0 {
            return Err(Error::invalid_data(
                "graphics-context response has reserved error bits set",
            ));
        }
        Ok(Self { raw })
    }

    #[must_use]
    pub const fn none() -> Self {
        Self { raw: 0 }
    }

    #[must_use]
    pub const fn other_error() -> Self {
        Self {
            raw: Self::OTHER_ERROR,
        }
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self.raw
    }

    #[must_use]
    pub const fn has_errors(self) -> bool {
        self.raw != 0
    }
}

/// F.57 VT-to-ECU response to one Graphics Context command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphicsContextResponse {
    pub object_id: ObjectID,
    pub subcommand: u8,
    pub errors: GraphicsContextErrorFlags,
}

impl GraphicsContextResponse {
    #[must_use]
    pub const fn new(
        object_id: ObjectID,
        subcommand: u8,
        errors: GraphicsContextErrorFlags,
    ) -> Self {
        Self {
            object_id,
            subcommand,
            errors,
        }
    }

    pub fn with_error_bits(object_id: ObjectID, subcommand: u8, error_bits: u8) -> Result<Self> {
        Ok(Self::new(
            object_id,
            subcommand,
            GraphicsContextErrorFlags::new(error_bits)?,
        ))
    }

    #[must_use]
    pub fn to_payload(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::GRAPHICS_CONTEXT;
        data[1..3].copy_from_slice(&self.object_id.to_le_bytes());
        data[3] = self.subcommand;
        data[4] = self.errors.raw();
        data
    }

    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "graphics-context response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::GRAPHICS_CONTEXT {
            return Err(Error::invalid_data(
                "graphics-context response has wrong command byte",
            ));
        }
        if data[5..8].iter().any(|&byte| byte != 0xFF) {
            return Err(Error::invalid_data(
                "graphics-context response reserved tail bytes are not 0xFF",
            ));
        }
        Self::with_error_bits(
            ObjectID(u16::from_le_bytes([data[1], data[2]])),
            data[3],
            data[4],
        )
    }

    pub fn to_message(&self, vt_source: Address, ecu_destination: Address) -> Result<Message> {
        validate_vt_to_ecu_envelope(vt_source, ecu_destination)?;
        Ok(Message::with_addressing(
            PGN_VT_TO_ECU,
            Vec::from(self.to_payload()),
            vt_source,
            ecu_destination,
            Default::default(),
        ))
    }

    pub fn from_message(msg: &Message) -> Result<Self> {
        if msg.pgn != PGN_VT_TO_ECU {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        validate_vt_to_ecu_envelope(msg.source, msg.destination)?;
        Self::from_payload(&msg.data)
    }
}
