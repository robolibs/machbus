//! Operator-event ECU response protocol helpers.
//!
//! The input/runtime layer already emits VT-to-ECU operator notifications for
//! soft keys, buttons, and pointing events. ISO also defines matching
//! ECU-to-VT response payloads with the same command bytes. Keeping those
//! response shapes here avoids growing the already-large input runtime module.

use alloc::{format, vec::Vec};

use crate::isobus::vt::ObjectID;
use crate::isobus::vt::commands::{KeyActivationCode, cmd};
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::pgn_defs::PGN_ECU_TO_VT;
use crate::net::{Address, Error, Message, Result};

/// The activation-response family carried by an ECU-to-VT activation reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlActivationResponseKind {
    SoftKey,
    Button,
}

impl ControlActivationResponseKind {
    #[inline]
    #[must_use]
    pub const fn command(self) -> u8 {
        match self {
            Self::SoftKey => cmd::SOFT_KEY_ACTIVATION,
            Self::Button => cmd::BUTTON_ACTIVATION,
        }
    }

    #[must_use]
    pub const fn from_command(command: u8) -> Option<Self> {
        match command {
            cmd::SOFT_KEY_ACTIVATION => Some(Self::SoftKey),
            cmd::BUTTON_ACTIVATION => Some(Self::Button),
            _ => None,
        }
    }
}

/// ECU response to a VT Soft Key Activation or Button Activation message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlActivationResponse {
    pub kind: ControlActivationResponseKind,
    pub activation_code: KeyActivationCode,
    pub object_id: ObjectID,
    pub parent_id: ObjectID,
    pub key_number: u8,
    pub transfer_sequence_number: Option<u8>,
}

impl ControlActivationResponse {
    pub fn new(
        kind: ControlActivationResponseKind,
        activation_code: KeyActivationCode,
        object_id: ObjectID,
        parent_id: ObjectID,
        key_number: u8,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        validate_object_id(object_id, "activation response object is NULL")?;
        validate_object_id(parent_id, "activation response parent object is NULL")?;
        validate_optional_tan(
            transfer_sequence_number,
            "activation response TAN exceeds 4-bit field",
        )?;
        Ok(Self {
            kind,
            activation_code,
            object_id,
            parent_id,
            key_number,
            transfer_sequence_number,
        })
    }

    pub fn soft_key(
        activation_code: KeyActivationCode,
        object_id: ObjectID,
        parent_id: ObjectID,
        key_number: u8,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        Self::new(
            ControlActivationResponseKind::SoftKey,
            activation_code,
            object_id,
            parent_id,
            key_number,
            transfer_sequence_number,
        )
    }

    pub fn button(
        activation_code: KeyActivationCode,
        object_id: ObjectID,
        parent_id: ObjectID,
        key_number: u8,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        Self::new(
            ControlActivationResponseKind::Button,
            activation_code,
            object_id,
            parent_id,
            key_number,
            transfer_sequence_number,
        )
    }

    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> Result<[u8; 8]> {
        let mut data = [0xFFu8; 8];
        data[0] = self.kind.command();
        data[1] = self.activation_code.as_u8();
        data[2..4].copy_from_slice(&self.object_id.to_le_bytes());
        data[4..6].copy_from_slice(&self.parent_id.to_le_bytes());
        data[6] = self.key_number;
        if vt_version >= 6 {
            let tan = self.transfer_sequence_number.ok_or_else(|| {
                Error::invalid_data("activation response VT6 payload requires TAN")
            })?;
            data[7] = (tan << 4) | 0x0F;
        }
        Ok(data)
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "activation response payload must be 8 bytes",
            ));
        }
        let kind = ControlActivationResponseKind::from_command(data[0])
            .ok_or_else(|| Error::invalid_data("activation response has wrong command byte"))?;
        let activation_code = KeyActivationCode::try_from_u8(data[1]).ok_or_else(|| {
            Error::invalid_data("activation response has invalid activation code")
        })?;
        let object_id = ObjectID(u16::from_le_bytes([data[2], data[3]]));
        let parent_id = ObjectID(u16::from_le_bytes([data[4], data[5]]));
        let transfer_sequence_number = parse_reserved_or_tan_byte(data[7], vt_version)?;
        Self::new(
            kind,
            activation_code,
            object_id,
            parent_id,
            data[6],
            transfer_sequence_number,
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

/// ECU response to a VT Pointing Event message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PointingEventResponse {
    pub x: u16,
    pub y: u16,
    pub touch_state: KeyActivationCode,
    pub parent_mask: Option<ObjectID>,
    pub transfer_sequence_number: Option<u8>,
}

impl PointingEventResponse {
    pub fn new(
        x: u16,
        y: u16,
        touch_state: KeyActivationCode,
        parent_mask: Option<ObjectID>,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        validate_pointing_touch_state(touch_state)?;
        if let Some(parent_mask) = parent_mask {
            validate_object_id(parent_mask, "pointing-event response parent mask is NULL")?;
        }
        validate_optional_tan(
            transfer_sequence_number,
            "pointing-event response TAN exceeds 4-bit field",
        )?;
        Ok(Self {
            x,
            y,
            touch_state,
            parent_mask,
            transfer_sequence_number,
        })
    }

    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> Result<[u8; 8]> {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::POINTING_EVENT;
        data[1..3].copy_from_slice(&self.x.to_le_bytes());
        data[3..5].copy_from_slice(&self.y.to_le_bytes());
        if vt_version <= 3 {
            data[5] = 0xFF;
        } else if vt_version < 6 {
            data[5] = self.touch_state.as_u8();
        } else {
            let tan = self.transfer_sequence_number.ok_or_else(|| {
                Error::invalid_data("pointing-event response VT6 payload requires TAN")
            })?;
            let parent_mask = self.parent_mask.ok_or_else(|| {
                Error::invalid_data("pointing-event response VT6 payload requires parent mask")
            })?;
            data[5] = (tan << 4) | self.touch_state.as_u8();
            data[6..8].copy_from_slice(&parent_mask.to_le_bytes());
        }
        Ok(data)
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "pointing-event response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::POINTING_EVENT {
            return Err(Error::invalid_data(
                "pointing-event response has wrong command byte",
            ));
        }
        let x = u16::from_le_bytes([data[1], data[2]]);
        let y = u16::from_le_bytes([data[3], data[4]]);
        if vt_version <= 3 {
            if data[5..8].iter().any(|&byte| byte != 0xFF) {
                return Err(Error::invalid_data(
                    "pointing-event response VT3 reserved bytes are not 0xFF",
                ));
            }
            return Self::new(x, y, KeyActivationCode::Pressed, None, None);
        }
        if vt_version < 6 {
            if data[6..8].iter().any(|&byte| byte != 0xFF) {
                return Err(Error::invalid_data(
                    "pointing-event response VT5 reserved bytes are not 0xFF",
                ));
            }
            return Self::new(x, y, parse_pointing_touch_state(data[5])?, None, None);
        }

        let tan = data[5] >> 4;
        let touch_state = parse_pointing_touch_state(data[5] & 0x0F)?;
        let parent_mask = ObjectID(u16::from_le_bytes([data[6], data[7]]));
        Self::new(x, y, touch_state, Some(parent_mask), Some(tan))
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

/// ECU response to a VT Select Input Object notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectInputObjectResponse {
    pub object_id: ObjectID,
    pub selected: bool,
    pub open_for_input: bool,
    pub transfer_sequence_number: Option<u8>,
}

impl SelectInputObjectResponse {
    pub fn new(
        object_id: ObjectID,
        selected: bool,
        open_for_input: bool,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        validate_object_id(object_id, "select-input response object is NULL")?;
        if open_for_input && !selected {
            return Err(Error::invalid_data(
                "select-input response cannot open a deselected object",
            ));
        }
        validate_optional_tan(
            transfer_sequence_number,
            "select-input response TAN exceeds 4-bit field",
        )?;
        Ok(Self {
            object_id,
            selected,
            open_for_input,
            transfer_sequence_number,
        })
    }

    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> Result<[u8; 8]> {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_INPUT_OBJECT;
        data[1..3].copy_from_slice(&self.object_id.to_le_bytes());
        data[3] = u8::from(self.selected);
        if vt_version >= 5 {
            data[4] = u8::from(self.open_for_input);
        }
        if vt_version >= 6 {
            let tan = self.transfer_sequence_number.ok_or_else(|| {
                Error::invalid_data("select-input response VT6 payload requires TAN")
            })?;
            data[7] = (tan << 4) | 0x0F;
        }
        Ok(data)
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "select-input response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::SELECT_INPUT_OBJECT {
            return Err(Error::invalid_data(
                "select-input response has wrong command byte",
            ));
        }
        let object_id = ObjectID(u16::from_le_bytes([data[1], data[2]]));
        let selected = parse_flag(data[3], "select-input response selection byte")?;
        let open_for_input = if vt_version >= 5 {
            let open = parse_flag(data[4], "select-input response open bitmask")?;
            if open && !selected {
                return Err(Error::invalid_data(
                    "select-input response open bit requires selection",
                ));
            }
            open
        } else {
            if data[4] != 0xFF {
                return Err(Error::invalid_data(
                    "select-input response pre-VT5 open byte is not 0xFF",
                ));
            }
            false
        };
        if data[5..7].iter().any(|&byte| byte != 0xFF) {
            return Err(Error::invalid_data(
                "select-input response reserved bytes are not 0xFF",
            ));
        }
        Self::new(
            object_id,
            selected,
            open_for_input,
            parse_reserved_or_tan_byte_for_command(data[7], vt_version, "select-input response")?,
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

/// ECU response to a VT ESC notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VtEscResponse {
    pub object_id: ObjectID,
    pub transfer_sequence_number: Option<u8>,
}

impl VtEscResponse {
    pub fn new(object_id: ObjectID, transfer_sequence_number: Option<u8>) -> Result<Self> {
        validate_optional_tan(
            transfer_sequence_number,
            "VT ESC response TAN exceeds 4-bit field",
        )?;
        Ok(Self {
            object_id,
            transfer_sequence_number,
        })
    }

    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> Result<[u8; 8]> {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_ESC;
        data[1..3].copy_from_slice(&self.object_id.to_le_bytes());
        if vt_version >= 6 {
            let tan = self
                .transfer_sequence_number
                .ok_or_else(|| Error::invalid_data("VT ESC response VT6 payload requires TAN"))?;
            data[7] = (tan << 4) | 0x0F;
        }
        Ok(data)
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        if data.len() != 8 {
            return Err(Error::invalid_data(
                "VT ESC response payload must be 8 bytes",
            ));
        }
        if data[0] != cmd::VT_ESC {
            return Err(Error::invalid_data(
                "VT ESC response has wrong command byte",
            ));
        }
        if data[3..7].iter().any(|&byte| byte != 0xFF) {
            return Err(Error::invalid_data(
                "VT ESC response reserved bytes are not 0xFF",
            ));
        }
        Self::new(
            ObjectID(u16::from_le_bytes([data[1], data[2]])),
            parse_reserved_or_tan_byte_for_command(data[7], vt_version, "VT ESC response")?,
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

fn parse_reserved_or_tan_byte(value: u8, vt_version: u16) -> Result<Option<u8>> {
    parse_reserved_or_tan_byte_for_command(value, vt_version, "activation response")
}

fn parse_reserved_or_tan_byte_for_command(
    value: u8,
    vt_version: u16,
    label: &'static str,
) -> Result<Option<u8>> {
    if vt_version >= 6 {
        if value & 0x0F != 0x0F {
            return Err(Error::invalid_data(format!(
                "{label} reserved TAN bits are not set"
            )));
        }
        Ok(Some(value >> 4))
    } else if value == 0xFF {
        Ok(None)
    } else {
        Err(Error::invalid_data(format!(
            "{label} reserved byte is not 0xFF"
        )))
    }
}

fn parse_flag(value: u8, label: &'static str) -> Result<bool> {
    if value & !0x01 != 0 {
        return Err(Error::invalid_data(label));
    }
    Ok(value & 0x01 != 0)
}

fn parse_pointing_touch_state(value: u8) -> Result<KeyActivationCode> {
    let Some(touch_state) = KeyActivationCode::try_from_u8(value) else {
        return Err(Error::invalid_data(
            "pointing-event response has invalid touch state",
        ));
    };
    validate_pointing_touch_state(touch_state)?;
    Ok(touch_state)
}

fn validate_pointing_touch_state(touch_state: KeyActivationCode) -> Result<()> {
    if touch_state == KeyActivationCode::Aborted {
        return Err(Error::invalid_data(
            "pointing-event response touch state cannot be Aborted",
        ));
    }
    Ok(())
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
