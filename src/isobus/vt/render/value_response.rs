//! Value-change ECU response protocol helpers.
//!
//! The render/input runtime emits VT-to-ECU value-change notifications for
//! numeric and string input transactions. ISO defines matching ECU-to-VT
//! acknowledgements. These helpers model those acknowledgement payloads without
//! growing the already-large input runtime module.

use alloc::vec::Vec;

use crate::isobus::vt::ObjectID;
use crate::isobus::vt::commands::cmd;
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::pgn_defs::PGN_ECU_TO_VT;
use crate::net::{Address, Error, Message, Result};

/// ECU response to a VT Change Numeric Value message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericValueChangeResponse {
    pub object_id: ObjectID,
    pub value_bytes: [u8; 4],
    pub transfer_sequence_number: Option<u8>,
}

impl NumericValueChangeResponse {
    pub fn new(
        object_id: ObjectID,
        value_bytes: [u8; 4],
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        validate_object_id(object_id, "numeric-value response object is NULL")?;
        validate_optional_tan(
            transfer_sequence_number,
            "numeric-value response TAN exceeds 4-bit field",
        )?;
        Ok(Self {
            object_id,
            value_bytes,
            transfer_sequence_number,
        })
    }

    pub fn from_u32(
        object_id: ObjectID,
        value: u32,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        Self::new(object_id, value.to_le_bytes(), transfer_sequence_number)
    }

    #[must_use]
    pub fn value_u32(self) -> u32 {
        u32::from_le_bytes(self.value_bytes)
    }

    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> Result<[u8; 8]> {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::NUMERIC_VALUE_CHANGE;
        data[1..3].copy_from_slice(&self.object_id.to_le_bytes());
        if vt_version >= 6 {
            let tan = self.transfer_sequence_number.ok_or_else(|| {
                Error::invalid_data("numeric-value response VT6 payload requires TAN")
            })?;
            data[3] = (tan << 4) | 0x0F;
        }
        data[4..8].copy_from_slice(&self.value_bytes);
        Ok(data)
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "numeric-value response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::NUMERIC_VALUE_CHANGE {
            return Err(Error::invalid_data(
                "numeric-value response has wrong command byte",
            ));
        }
        Self::new(
            ObjectID(u16::from_le_bytes([data[1], data[2]])),
            [data[4], data[5], data[6], data[7]],
            parse_reserved_or_tan_byte(data[3], vt_version, "numeric-value response")?,
        )
    }

    pub fn to_message(
        &self,
        ecu_source: Address,
        vt_destination: Address,
        vt_version: u16,
    ) -> Result<Message> {
        validate_ecu_to_vt_envelope(ecu_source, vt_destination)?;
        Ok(Message::with_addressing(
            PGN_ECU_TO_VT,
            Vec::from(self.to_payload_for_vt_version(vt_version)?),
            ecu_source,
            vt_destination,
            Default::default(),
        ))
    }

    pub fn from_message(msg: &Message, vt_version: u16) -> Result<Self> {
        if msg.pgn != PGN_ECU_TO_VT {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        validate_ecu_to_vt_envelope(msg.source, msg.destination)?;
        Self::from_payload_for_vt_version(&msg.data, vt_version)
    }
}

/// ECU response to a VT Change String Value message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringValueChangeResponse {
    pub object_id: ObjectID,
}

impl StringValueChangeResponse {
    pub fn new(object_id: ObjectID) -> Result<Self> {
        validate_object_id(object_id, "string-value response object is NULL")?;
        Ok(Self { object_id })
    }

    #[must_use]
    pub fn to_payload(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::STRING_VALUE_CHANGE;
        data[3..5].copy_from_slice(&self.object_id.to_le_bytes());
        data
    }

    pub fn from_payload(data: &[u8]) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "string-value response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::STRING_VALUE_CHANGE {
            return Err(Error::invalid_data(
                "string-value response has wrong command byte",
            ));
        }
        if data[1..3].iter().any(|&byte| byte != 0xFF)
            || data[5..8].iter().any(|&byte| byte != 0xFF)
        {
            return Err(Error::invalid_data(
                "string-value response reserved bytes are not 0xFF",
            ));
        }
        Self::new(ObjectID(u16::from_le_bytes([data[3], data[4]])))
    }

    pub fn to_message(&self, ecu_source: Address, vt_destination: Address) -> Result<Message> {
        validate_ecu_to_vt_envelope(ecu_source, vt_destination)?;
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
        validate_ecu_to_vt_envelope(msg.source, msg.destination)?;
        Self::from_payload(&msg.data)
    }
}

fn parse_reserved_or_tan_byte(
    value: u8,
    vt_version: u16,
    label: &'static str,
) -> Result<Option<u8>> {
    if vt_version >= 6 {
        if value & 0x0F != 0x0F {
            return Err(Error::invalid_data(label));
        }
        Ok(Some(value >> 4))
    } else if value == 0xFF {
        Ok(None)
    } else {
        Err(Error::invalid_data(label))
    }
}

fn validate_object_id(object_id: ObjectID, message: &'static str) -> Result<()> {
    if object_id == ObjectID::NULL {
        return Err(Error::invalid_data(message));
    }
    Ok(())
}

fn validate_optional_tan(value: Option<u8>, message: &'static str) -> Result<()> {
    if let Some(tan) = value
        && tan > 0x0F
    {
        return Err(Error::invalid_data(message));
    }
    Ok(())
}

fn validate_ecu_to_vt_envelope(ecu_source: Address, vt_destination: Address) -> Result<()> {
    if ecu_source == NULL_ADDRESS || ecu_source == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(ecu_source));
    }
    if vt_destination == NULL_ADDRESS || vt_destination == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(vt_destination));
    }
    Ok(())
}
