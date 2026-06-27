//! VT mask-change error notifications and ECU response helpers.
//!
//! ISO 11783-6 Annex H defines VT-to-ECU error notifications for active-mask
//! and soft-key-mask drawing/reference failures, plus matching ECU-to-VT
//! acknowledgements. These helpers keep the wire shapes explicit and separate
//! from the render runtime's local mask switching logic.

use alloc::vec::Vec;

use crate::isobus::vt::ObjectID;
use crate::isobus::vt::commands::cmd;
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::pgn_defs::{PGN_ECU_TO_VT, PGN_VT_TO_ECU};
use crate::net::{Address, Error, Message, Result};

const MASK_ERROR_ALLOWED_BITS: u8 = 0x3C;

/// Error bitset used by H.14/H.16 VT mask-change error notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskErrorFlags {
    raw: u8,
}

impl MaskErrorFlags {
    pub const MISSING_OBJECTS: u8 = 0x04;
    pub const OBJECT_HAS_ERRORS: u8 = 0x08;
    pub const OTHER_ERROR: u8 = 0x10;
    pub const POOL_BEING_DELETED: u8 = 0x20;

    pub fn new(raw: u8) -> Result<Self> {
        if raw == 0 {
            return Err(Error::invalid_data(
                "mask-change error notification has no error bits set",
            ));
        }
        if raw & !MASK_ERROR_ALLOWED_BITS != 0 {
            return Err(Error::invalid_data(
                "mask-change error notification has reserved error bits set",
            ));
        }
        Ok(Self { raw })
    }

    #[must_use]
    pub const fn raw(self) -> u8 {
        self.raw
    }
}

/// H.14 VT Change Active Mask error notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeActiveMaskError {
    pub mask_id: ObjectID,
    pub error_flags: MaskErrorFlags,
    pub object_id_with_error: ObjectID,
    pub parent_object_id: ObjectID,
}

impl ChangeActiveMaskError {
    pub fn new(
        mask_id: ObjectID,
        error_flags: MaskErrorFlags,
        object_id_with_error: ObjectID,
        parent_object_id: ObjectID,
    ) -> Result<Self> {
        validate_object_id(mask_id, "active-mask error notification mask is NULL")?;
        Ok(Self {
            mask_id,
            error_flags,
            object_id_with_error,
            parent_object_id,
        })
    }

    #[must_use]
    pub fn to_payload(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_CHANGE_ACTIVE_MASK;
        data[1..3].copy_from_slice(&self.mask_id.to_le_bytes());
        data[3] = self.error_flags.raw();
        data[4..6].copy_from_slice(&self.object_id_with_error.to_le_bytes());
        data[6..8].copy_from_slice(&self.parent_object_id.to_le_bytes());
        data
    }

    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "active-mask error notification payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::VT_CHANGE_ACTIVE_MASK {
            return Err(Error::invalid_data(
                "active-mask error notification has wrong command byte",
            ));
        }
        Self::new(
            ObjectID(u16::from_le_bytes([data[1], data[2]])),
            MaskErrorFlags::new(data[3])?,
            ObjectID(u16::from_le_bytes([data[4], data[5]])),
            ObjectID(u16::from_le_bytes([data[6], data[7]])),
        )
    }

    pub fn to_message(&self, vt_source: Address, ecu_destination: Address) -> Result<Message> {
        validate_destination_specific_envelope(vt_source, ecu_destination)?;
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
        validate_destination_specific_envelope(msg.source, msg.destination)?;
        Self::from_payload(&msg.data)
    }
}

/// H.15 ECU response to a VT Change Active Mask error notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeActiveMaskResponse {
    pub mask_id: ObjectID,
}

impl ChangeActiveMaskResponse {
    pub fn new(mask_id: ObjectID) -> Result<Self> {
        validate_object_id(mask_id, "active-mask error response mask is NULL")?;
        Ok(Self { mask_id })
    }

    #[must_use]
    pub fn to_payload(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_CHANGE_ACTIVE_MASK;
        data[1..3].copy_from_slice(&self.mask_id.to_le_bytes());
        data
    }

    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "active-mask error response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::VT_CHANGE_ACTIVE_MASK {
            return Err(Error::invalid_data(
                "active-mask error response has wrong command byte",
            ));
        }
        validate_reserved_tail(&data[3..], "active-mask error response")?;
        Self::new(ObjectID(u16::from_le_bytes([data[1], data[2]])))
    }

    pub fn to_message(&self, ecu_source: Address, vt_destination: Address) -> Result<Message> {
        validate_destination_specific_envelope(ecu_source, vt_destination)?;
        Ok(Message::with_addressing(
            PGN_ECU_TO_VT,
            Vec::from(self.to_payload()),
            ecu_source,
            vt_destination,
            Default::default(),
        ))
    }

    pub fn from_message(msg: &Message) -> Result<Self> {
        if msg.pgn != PGN_ECU_TO_VT {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        validate_destination_specific_envelope(msg.source, msg.destination)?;
        Self::from_payload(&msg.data)
    }
}

/// H.16 VT Change Soft Key Mask error notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeSoftKeyMaskError {
    pub mask_id: ObjectID,
    pub soft_key_mask_id: ObjectID,
    pub error_flags: MaskErrorFlags,
}

impl ChangeSoftKeyMaskError {
    pub fn new(
        mask_id: ObjectID,
        soft_key_mask_id: ObjectID,
        error_flags: MaskErrorFlags,
    ) -> Result<Self> {
        validate_object_id(mask_id, "soft-key-mask error notification mask is NULL")?;
        validate_object_id(
            soft_key_mask_id,
            "soft-key-mask error notification soft-key mask is NULL",
        )?;
        Ok(Self {
            mask_id,
            soft_key_mask_id,
            error_flags,
        })
    }

    #[must_use]
    pub fn to_payload(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_CHANGE_SOFT_KEY_MASK;
        data[1..3].copy_from_slice(&self.mask_id.to_le_bytes());
        data[3..5].copy_from_slice(&self.soft_key_mask_id.to_le_bytes());
        data[5] = self.error_flags.raw();
        data
    }

    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "soft-key-mask error notification payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::VT_CHANGE_SOFT_KEY_MASK {
            return Err(Error::invalid_data(
                "soft-key-mask error notification has wrong command byte",
            ));
        }
        validate_reserved_tail(&data[6..], "soft-key-mask error notification")?;
        Self::new(
            ObjectID(u16::from_le_bytes([data[1], data[2]])),
            ObjectID(u16::from_le_bytes([data[3], data[4]])),
            MaskErrorFlags::new(data[5])?,
        )
    }

    pub fn to_message(&self, vt_source: Address, ecu_destination: Address) -> Result<Message> {
        validate_destination_specific_envelope(vt_source, ecu_destination)?;
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
        validate_destination_specific_envelope(msg.source, msg.destination)?;
        Self::from_payload(&msg.data)
    }
}

/// H.17 ECU response to a VT Change Soft Key Mask error notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeSoftKeyMaskResponse {
    pub mask_id: ObjectID,
    pub soft_key_mask_id: ObjectID,
}

impl ChangeSoftKeyMaskResponse {
    pub fn new(mask_id: ObjectID, soft_key_mask_id: ObjectID) -> Result<Self> {
        validate_object_id(mask_id, "soft-key-mask error response mask is NULL")?;
        validate_object_id(
            soft_key_mask_id,
            "soft-key-mask error response soft-key mask is NULL",
        )?;
        Ok(Self {
            mask_id,
            soft_key_mask_id,
        })
    }

    #[must_use]
    pub fn to_payload(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_CHANGE_SOFT_KEY_MASK;
        data[1..3].copy_from_slice(&self.mask_id.to_le_bytes());
        data[3..5].copy_from_slice(&self.soft_key_mask_id.to_le_bytes());
        data
    }

    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "soft-key-mask error response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::VT_CHANGE_SOFT_KEY_MASK {
            return Err(Error::invalid_data(
                "soft-key-mask error response has wrong command byte",
            ));
        }
        validate_reserved_tail(&data[5..], "soft-key-mask error response")?;
        Self::new(
            ObjectID(u16::from_le_bytes([data[1], data[2]])),
            ObjectID(u16::from_le_bytes([data[3], data[4]])),
        )
    }

    pub fn to_message(&self, ecu_source: Address, vt_destination: Address) -> Result<Message> {
        validate_destination_specific_envelope(ecu_source, vt_destination)?;
        Ok(Message::with_addressing(
            PGN_ECU_TO_VT,
            Vec::from(self.to_payload()),
            ecu_source,
            vt_destination,
            Default::default(),
        ))
    }

    pub fn from_message(msg: &Message) -> Result<Self> {
        if msg.pgn != PGN_ECU_TO_VT {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        validate_destination_specific_envelope(msg.source, msg.destination)?;
        Self::from_payload(&msg.data)
    }
}

fn validate_reserved_tail(data: &[u8], label: &'static str) -> Result<()> {
    if data.iter().any(|&byte| byte != 0xFF) {
        return Err(Error::invalid_data(label));
    }
    Ok(())
}

fn validate_object_id(object_id: ObjectID, message: &'static str) -> Result<()> {
    if object_id == ObjectID::NULL {
        return Err(Error::invalid_data(message));
    }
    Ok(())
}

fn validate_destination_specific_envelope(source: Address, destination: Address) -> Result<()> {
    if source == NULL_ADDRESS || source == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(source));
    }
    if destination == NULL_ADDRESS || destination == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(destination));
    }
    Ok(())
}
