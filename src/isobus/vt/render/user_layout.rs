//! User-layout visibility protocol helpers.
//!
//! The render runtime can emit the ISO VT On User-Layout Hide/Show
//! notification when Window Mask / Key Group objects, or an inactive visible
//! Working Set's active Data Mask / Soft Key Mask, change visibility. This
//! module models the matching ECU-to-VT response shape so host integrations can
//! validate replies without treating the response as an ordinary render event.

use alloc::vec::Vec;

use crate::isobus::vt::ObjectID;
use crate::isobus::vt::commands::cmd;
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::pgn_defs::PGN_ECU_TO_VT;
use crate::net::{Address, Error, Message, Result};

/// One object/state pair carried by a user-layout hide/show notification or
/// response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserLayoutHideShowRecord {
    pub object_id: ObjectID,
    pub shown: bool,
}

impl UserLayoutHideShowRecord {
    #[must_use]
    pub const fn new(object_id: ObjectID, shown: bool) -> Self {
        Self { object_id, shown }
    }
}

/// ECU response to a VT On User-Layout Hide/Show notification.
///
/// The payload mirrors the notification payload: one required record, one
/// optional second record, and an optional VT version 6+ transfer sequence
/// number nibble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserLayoutHideShowResponse {
    pub first: UserLayoutHideShowRecord,
    pub second: Option<UserLayoutHideShowRecord>,
    pub transfer_sequence_number: Option<u8>,
}

impl UserLayoutHideShowResponse {
    pub fn new(
        first: UserLayoutHideShowRecord,
        second: Option<UserLayoutHideShowRecord>,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        validate_record(first, "first user-layout hide/show response record")?;
        if let Some(record) = second {
            validate_record(record, "second user-layout hide/show response record")?;
        }
        if let Some(tan) = transfer_sequence_number
            && tan > 0x0F
        {
            return Err(Error::invalid_data(
                "user-layout hide/show response transfer sequence number exceeds 4-bit field",
            ));
        }
        Ok(Self {
            first,
            second,
            transfer_sequence_number,
        })
    }

    /// Build the 8-byte ECU-to-VT response payload.
    ///
    /// For VT versions before 6, byte 8 is the reserved `0xFF` shape. For VT6+
    /// callers may provide a TAN nibble; omitting it keeps the legacy `0xFF`
    /// byte, which is also wire-compatible with TAN 15.
    #[must_use]
    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::USER_LAYOUT_HIDE_SHOW;
        data[1..3].copy_from_slice(&self.first.object_id.to_le_bytes());
        data[3] = u8::from(self.first.shown);
        if let Some(second) = self.second {
            data[4..6].copy_from_slice(&second.object_id.to_le_bytes());
            data[6] = u8::from(second.shown);
        } else {
            data[4..6].copy_from_slice(&ObjectID::NULL.to_le_bytes());
            data[6] = 0;
        }
        if vt_version >= 6
            && let Some(tan) = self.transfer_sequence_number
        {
            data[7] = (tan << 4) | 0x0F;
        }
        data
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "user-layout hide/show response must be 8 bytes",
            ));
        }
        if data[0] != cmd::USER_LAYOUT_HIDE_SHOW {
            return Err(Error::invalid_data(
                "user-layout hide/show response has wrong command byte",
            ));
        }
        let first_id = ObjectID(u16::from_le_bytes([data[1], data[2]]));
        let first_shown = parse_status(data[3], "first user-layout hide/show response status")?;
        if first_id == ObjectID::NULL {
            return Err(Error::invalid_data(
                "user-layout hide/show response first object is NULL",
            ));
        }

        let second_id = ObjectID(u16::from_le_bytes([data[4], data[5]]));
        let second_shown = parse_status(data[6], "second user-layout hide/show response status")?;
        let second = if second_id == ObjectID::NULL {
            if second_shown {
                return Err(Error::invalid_data(
                    "user-layout hide/show response NULL second object must be hidden",
                ));
            }
            None
        } else {
            Some(UserLayoutHideShowRecord::new(second_id, second_shown))
        };

        Self::new(
            UserLayoutHideShowRecord::new(first_id, first_shown),
            second,
            parse_transfer_sequence_number(data[7], vt_version)?,
        )
    }

    pub fn to_message(
        &self,
        ecu_source: Address,
        vt_destination: Address,
        vt_version: u16,
    ) -> Result<Message> {
        validate_ecu_to_vt_response_envelope(ecu_source, vt_destination)?;
        Ok(Message::with_addressing(
            PGN_ECU_TO_VT,
            Vec::from(self.to_payload_for_vt_version(vt_version)),
            ecu_source,
            vt_destination,
            Default::default(),
        ))
    }

    pub fn from_message(msg: &Message, vt_version: u16) -> Result<Self> {
        if msg.pgn != PGN_ECU_TO_VT {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        validate_ecu_to_vt_response_envelope(msg.source, msg.destination)?;
        Self::from_payload_for_vt_version(&msg.data, vt_version)
    }
}

/// Validate the full-message envelope used by ECU-to-VT user-layout
/// responses. The helper follows the hosted VT convention used by the render
/// runtime full-message helpers: both source and destination must name a real
/// control function address.
pub fn validate_ecu_to_vt_response_envelope(
    ecu_source: Address,
    vt_destination: Address,
) -> Result<()> {
    if ecu_source == NULL_ADDRESS || ecu_source == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(ecu_source));
    }
    if vt_destination == NULL_ADDRESS || vt_destination == BROADCAST_ADDRESS {
        return Err(Error::invalid_address(vt_destination));
    }
    Ok(())
}

fn validate_record(record: UserLayoutHideShowRecord, label: &'static str) -> Result<()> {
    if record.object_id == ObjectID::NULL {
        return Err(Error::invalid_data(label));
    }
    Ok(())
}

fn parse_status(value: u8, label: &'static str) -> Result<bool> {
    if value & !0x01 != 0 {
        return Err(Error::invalid_data(label));
    }
    Ok(value & 0x01 != 0)
}

fn parse_transfer_sequence_number(value: u8, vt_version: u16) -> Result<Option<u8>> {
    if vt_version >= 6 {
        if value & 0x0F != 0x0F {
            return Err(Error::invalid_data(
                "user-layout hide/show response reserved TAN bits are not set",
            ));
        }
        Ok(Some(value >> 4))
    } else if value == 0xFF {
        Ok(None)
    } else {
        Err(Error::invalid_data(
            "user-layout hide/show response reserved byte is not 0xFF",
        ))
    }
}
