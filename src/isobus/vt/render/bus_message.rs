//! VT-to-ECU bus-message payload helpers for render-runtime events.
//!
//! This module owns the Annex H-style payload shapes emitted by the
//! backend-neutral render/input runtime. It keeps the raw event runtime focused
//! on operator state while this layer validates wire-facing VT-to-ECU payloads,
//! optional VT v6 transfer-sequence-number nibbles, and full PGN envelopes.

use alloc::vec::Vec;

use crate::isobus::vt::ObjectID;
use crate::isobus::vt::commands::{KeyActivationCode, VT_STRING_VALUE_MAX_LEN, cmd};
use crate::net::pgn_defs::PGN_VT_TO_ECU;
use crate::net::{Address, Error, Message, Result};

/// VT-to-ECU bus message family produced from a semantic render event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VtBusMessageKind {
    SoftKeyActivation,
    ButtonActivation,
    NumericValueChange,
    StringValueChange,
    SelectInputObject,
    VtEsc,
    UserLayoutHideShow,
    PointingEvent,
}

impl VtBusMessageKind {
    /// Map a VT-to-ECU function byte to its render-runtime bus-message family.
    #[must_use]
    pub const fn from_command(command: u8) -> Option<Self> {
        match command {
            cmd::SOFT_KEY_ACTIVATION => Some(Self::SoftKeyActivation),
            cmd::BUTTON_ACTIVATION => Some(Self::ButtonActivation),
            cmd::NUMERIC_VALUE_CHANGE => Some(Self::NumericValueChange),
            cmd::STRING_VALUE_CHANGE => Some(Self::StringValueChange),
            cmd::SELECT_INPUT_OBJECT => Some(Self::SelectInputObject),
            cmd::VT_ESC => Some(Self::VtEsc),
            cmd::USER_LAYOUT_HIDE_SHOW => Some(Self::UserLayoutHideShow),
            cmd::POINTING_EVENT => Some(Self::PointingEvent),
            _ => None,
        }
    }
}

/// Payload-ready VT-to-ECU message.
///
/// The payload is the VT function byte plus parameter bytes. The host still
/// owns CAN/J1939 routing (PGN, destination, source address, transport
/// protocol segmentation for variable-length strings).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VtBusMessage {
    pub kind: VtBusMessageKind,
    pub data: Vec<u8>,
}

impl VtBusMessage {
    #[must_use]
    pub fn soft_key_activation(
        code: KeyActivationCode,
        object_id: ObjectID,
        parent_id: ObjectID,
        key_number: u8,
    ) -> Self {
        Self::fixed(
            VtBusMessageKind::SoftKeyActivation,
            build_key_activation(
                cmd::SOFT_KEY_ACTIVATION,
                code,
                object_id,
                parent_id,
                key_number,
                None,
            ),
        )
    }

    pub fn soft_key_activation_with_transfer_sequence_number(
        code: KeyActivationCode,
        object_id: ObjectID,
        parent_id: ObjectID,
        key_number: u8,
        transfer_sequence_number: u8,
    ) -> Result<Self> {
        validate_transfer_sequence_number(
            transfer_sequence_number,
            "soft-key activation transfer sequence number exceeds 4-bit field",
        )?;
        Ok(Self::fixed(
            VtBusMessageKind::SoftKeyActivation,
            build_key_activation(
                cmd::SOFT_KEY_ACTIVATION,
                code,
                object_id,
                parent_id,
                key_number,
                Some(transfer_sequence_number),
            ),
        ))
    }

    #[must_use]
    pub fn button_activation(
        code: KeyActivationCode,
        object_id: ObjectID,
        parent_id: ObjectID,
        key_number: u8,
    ) -> Self {
        Self::fixed(
            VtBusMessageKind::ButtonActivation,
            build_key_activation(
                cmd::BUTTON_ACTIVATION,
                code,
                object_id,
                parent_id,
                key_number,
                None,
            ),
        )
    }

    pub fn button_activation_with_transfer_sequence_number(
        code: KeyActivationCode,
        object_id: ObjectID,
        parent_id: ObjectID,
        key_number: u8,
        transfer_sequence_number: u8,
    ) -> Result<Self> {
        validate_transfer_sequence_number(
            transfer_sequence_number,
            "button activation transfer sequence number exceeds 4-bit field",
        )?;
        Ok(Self::fixed(
            VtBusMessageKind::ButtonActivation,
            build_key_activation(
                cmd::BUTTON_ACTIVATION,
                code,
                object_id,
                parent_id,
                key_number,
                Some(transfer_sequence_number),
            ),
        ))
    }

    #[must_use]
    pub fn numeric_value_change(object_id: ObjectID, value: u32) -> Self {
        Self::numeric_value_change_payload(object_id, value, None)
    }

    pub fn numeric_value_change_with_transfer_sequence_number(
        object_id: ObjectID,
        value: u32,
        transfer_sequence_number: u8,
    ) -> Result<Self> {
        validate_transfer_sequence_number(
            transfer_sequence_number,
            "numeric-value transfer sequence number exceeds 4-bit field",
        )?;
        Ok(Self::numeric_value_change_payload(
            object_id,
            value,
            Some(transfer_sequence_number),
        ))
    }

    fn numeric_value_change_payload(
        object_id: ObjectID,
        value: u32,
        transfer_sequence_number: Option<u8>,
    ) -> Self {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::NUMERIC_VALUE_CHANGE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        if let Some(tan) = transfer_sequence_number {
            data[3] = encode_tan(tan);
        }
        data[4..8].copy_from_slice(&value.to_le_bytes());
        Self::fixed(VtBusMessageKind::NumericValueChange, data)
    }

    pub fn string_value_change(object_id: ObjectID, value: &str) -> Result<Self> {
        if value.len() > VT_STRING_VALUE_MAX_LEN {
            return Err(Error::invalid_data(
                "VT string-value notification exceeds u16 length field",
            ));
        }
        let mut data = Vec::with_capacity(5 + value.len());
        data.push(cmd::STRING_VALUE_CHANGE);
        data.extend_from_slice(&object_id.to_le_bytes());
        data.extend_from_slice(&(value.len() as u16).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
        Ok(Self {
            kind: VtBusMessageKind::StringValueChange,
            data,
        })
    }

    #[must_use]
    pub fn select_input_object(object_id: ObjectID, selected: bool, open_for_input: bool) -> Self {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_INPUT_OBJECT;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = u8::from(selected);
        data[4] = u8::from(open_for_input);
        Self::fixed(VtBusMessageKind::SelectInputObject, data)
    }

    #[must_use]
    pub fn vt_esc(object_id: ObjectID, error_code: u8) -> Self {
        Self::vt_esc_payload(object_id, error_code, None)
    }

    pub fn vt_esc_with_transfer_sequence_number(
        object_id: ObjectID,
        error_code: u8,
        transfer_sequence_number: u8,
    ) -> Result<Self> {
        validate_transfer_sequence_number(
            transfer_sequence_number,
            "VT ESC transfer sequence number exceeds 4-bit field",
        )?;
        Ok(Self::vt_esc_payload(
            object_id,
            error_code,
            Some(transfer_sequence_number),
        ))
    }

    fn vt_esc_payload(
        object_id: ObjectID,
        error_code: u8,
        transfer_sequence_number: Option<u8>,
    ) -> Self {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::VT_ESC;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = error_code;
        if let Some(tan) = transfer_sequence_number {
            data[7] = encode_tan(tan);
        }
        Self::fixed(VtBusMessageKind::VtEsc, data)
    }

    pub fn user_layout_hide_show(
        first: (ObjectID, bool),
        second: Option<(ObjectID, bool)>,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        if first.0 == ObjectID::NULL {
            return Err(Error::invalid_data(
                "user-layout hide/show first object is NULL",
            ));
        }
        if let Some((id, _)) = second
            && id == ObjectID::NULL
        {
            return Err(Error::invalid_data(
                "user-layout hide/show second object is NULL",
            ));
        }
        if let Some(tan) = transfer_sequence_number {
            validate_transfer_sequence_number(
                tan,
                "user-layout hide/show transfer sequence number exceeds 4-bit field",
            )?;
        }

        let mut data = [0xFFu8; 8];
        data[0] = cmd::USER_LAYOUT_HIDE_SHOW;
        data[1..3].copy_from_slice(&first.0.to_le_bytes());
        data[3] = u8::from(first.1);
        if let Some((id, shown)) = second {
            data[4..6].copy_from_slice(&id.to_le_bytes());
            data[6] = u8::from(shown);
        } else {
            data[4..6].copy_from_slice(&ObjectID::NULL.to_le_bytes());
            data[6] = 0;
        }
        if let Some(tan) = transfer_sequence_number {
            data[7] = encode_tan(tan);
        }
        Ok(Self::fixed(VtBusMessageKind::UserLayoutHideShow, data))
    }

    pub fn pointing_event(
        x: u16,
        y: u16,
        touch_state: KeyActivationCode,
        parent_mask: ObjectID,
        transfer_sequence_number: Option<u8>,
    ) -> Result<Self> {
        if parent_mask == ObjectID::NULL {
            return Err(Error::invalid_data(
                "pointing event targets NULL parent mask",
            ));
        }
        if touch_state == KeyActivationCode::Aborted {
            return Err(Error::invalid_data(
                "pointing event touch state cannot be Aborted",
            ));
        }
        if let Some(tan) = transfer_sequence_number {
            validate_transfer_sequence_number(
                tan,
                "pointing event transfer sequence number exceeds 4-bit field",
            )?;
        }

        let mut data = [0xFFu8; 8];
        data[0] = cmd::POINTING_EVENT;
        data[1..3].copy_from_slice(&x.to_le_bytes());
        data[3..5].copy_from_slice(&y.to_le_bytes());
        data[5] = touch_state.as_u8();
        if let Some(tan) = transfer_sequence_number {
            data[5] = (tan << 4) | touch_state.as_u8();
            data[6..8].copy_from_slice(&parent_mask.to_le_bytes());
        }
        Ok(Self::fixed(VtBusMessageKind::PointingEvent, data))
    }

    /// Parse and validate one VT-to-ECU payload.
    ///
    /// The returned value preserves the exact payload bytes after validating the
    /// command family, byte count, reserved bytes, object IDs where the
    /// standard requires non-NULL IDs, boolean fields, string length/UTF-8, and
    /// VT v6 transfer-sequence-number nibble layout.
    pub fn from_payload(data: &[u8]) -> Result<Self> {
        let Some((&command, _)) = data.split_first() else {
            return Err(Error::invalid_data("VT-to-ECU payload is empty"));
        };
        let kind = VtBusMessageKind::from_command(command)
            .ok_or_else(|| Error::invalid_data("unsupported VT-to-ECU payload command"))?;
        match kind {
            VtBusMessageKind::SoftKeyActivation => {
                parse_key_activation(data, cmd::SOFT_KEY_ACTIVATION, "soft-key activation")?;
            }
            VtBusMessageKind::ButtonActivation => {
                parse_key_activation(data, cmd::BUTTON_ACTIVATION, "button activation")?;
            }
            VtBusMessageKind::NumericValueChange => parse_numeric_value_change(data)?,
            VtBusMessageKind::StringValueChange => parse_string_value_change(data)?,
            VtBusMessageKind::SelectInputObject => parse_select_input_object(data)?,
            VtBusMessageKind::VtEsc => parse_vt_esc(data)?,
            VtBusMessageKind::UserLayoutHideShow => parse_user_layout_hide_show(data)?,
            VtBusMessageKind::PointingEvent => parse_pointing_event(data)?,
        }
        Ok(Self {
            kind,
            data: data.to_vec(),
        })
    }

    /// Parse and validate one full destination-specific `PGN_VT_TO_ECU`
    /// message envelope plus payload.
    pub fn from_message(msg: &Message) -> Result<Self> {
        if msg.pgn != PGN_VT_TO_ECU {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        validate_vt_to_ecu_envelope(msg.source, msg.destination)?;
        Self::from_payload(&msg.data)
    }

    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Wrap this VT-to-ECU payload in a full PGN/address envelope.
    ///
    /// The destination is explicit because `PGN_VT_TO_ECU` is a
    /// destination-specific VT message PGN. Hosts that only need the raw
    /// protocol payload can keep using [`Self::as_bytes`].
    #[must_use]
    pub fn to_message(&self, vt_source: Address, ecu_destination: Address) -> Message {
        Message::with_addressing(
            PGN_VT_TO_ECU,
            self.data.clone(),
            vt_source,
            ecu_destination,
            Default::default(),
        )
    }

    /// Checked form of [`Self::to_message`].
    ///
    /// This rejects null/broadcast VT source addresses and null/broadcast ECU
    /// destinations before the host puts an unusable destination-specific VT
    /// message on the bus.
    pub fn try_to_message(&self, vt_source: Address, ecu_destination: Address) -> Result<Message> {
        validate_vt_to_ecu_envelope(vt_source, ecu_destination)?;
        Ok(self.to_message(vt_source, ecu_destination))
    }

    /// Consuming form of [`Self::to_message`].
    #[must_use]
    pub fn into_message(self, vt_source: Address, ecu_destination: Address) -> Message {
        Message::with_addressing(
            PGN_VT_TO_ECU,
            self.data,
            vt_source,
            ecu_destination,
            Default::default(),
        )
    }

    /// Checked consuming form of [`Self::to_message`].
    pub fn try_into_message(self, vt_source: Address, ecu_destination: Address) -> Result<Message> {
        validate_vt_to_ecu_envelope(vt_source, ecu_destination)?;
        Ok(self.into_message(vt_source, ecu_destination))
    }

    fn fixed(kind: VtBusMessageKind, data: [u8; 8]) -> Self {
        Self {
            kind,
            data: data.to_vec(),
        }
    }
}

/// Validate the destination-specific VT-to-ECU message envelope used by full
/// [`Message`] wrapping helpers.
///
/// This is public so stateful render-runtime helpers can reject unusable
/// source/destination pairs before mutating input/edit state.
pub fn validate_vt_to_ecu_envelope(vt_source: Address, ecu_destination: Address) -> Result<()> {
    if vt_source == crate::net::constants::NULL_ADDRESS
        || vt_source == crate::net::constants::BROADCAST_ADDRESS
    {
        return Err(Error::invalid_address(vt_source));
    }
    if ecu_destination == crate::net::constants::NULL_ADDRESS
        || ecu_destination == crate::net::constants::BROADCAST_ADDRESS
    {
        return Err(Error::invalid_address(ecu_destination));
    }
    Ok(())
}

fn build_key_activation(
    function: u8,
    code: KeyActivationCode,
    object_id: ObjectID,
    parent_id: ObjectID,
    key_number: u8,
    transfer_sequence_number: Option<u8>,
) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    data[0] = function;
    data[1] = code.as_u8();
    data[2..4].copy_from_slice(&object_id.to_le_bytes());
    data[4..6].copy_from_slice(&parent_id.to_le_bytes());
    data[6] = key_number;
    if let Some(tan) = transfer_sequence_number {
        data[7] = encode_tan(tan);
    }
    data
}

fn parse_key_activation(data: &[u8], command: u8, label: &'static str) -> Result<()> {
    exact_len(data, 8, label)?;
    if data[0] != command {
        return Err(Error::invalid_data("activation command byte mismatch"));
    }
    validate_activation_code(data[1], label)?;
    validate_non_null_object_id(object_id_at(data, 2), "activation object ID is NULL")?;
    validate_non_null_object_id(object_id_at(data, 4), "activation parent object ID is NULL")?;
    validate_reserved_or_tan(data[7], "activation reserved/TAN byte")?;
    Ok(())
}

fn parse_numeric_value_change(data: &[u8]) -> Result<()> {
    exact_len(data, 8, "numeric value change")?;
    validate_non_null_object_id(
        object_id_at(data, 1),
        "numeric value change object ID is NULL",
    )?;
    validate_reserved_or_tan(data[3], "numeric value change reserved/TAN byte")?;
    Ok(())
}

fn parse_string_value_change(data: &[u8]) -> Result<()> {
    if data.len() < 5 {
        return Err(Error::invalid_data(
            "string value change payload is shorter than header",
        ));
    }
    validate_non_null_object_id(
        object_id_at(data, 1),
        "string value change object ID is NULL",
    )?;
    let len = u16::from_le_bytes([data[3], data[4]]) as usize;
    if data.len() != 5 + len {
        return Err(Error::invalid_data(
            "string value change declared length does not match payload",
        ));
    }
    core::str::from_utf8(&data[5..])
        .map_err(|_| Error::invalid_data("string value change payload is not UTF-8"))?;
    Ok(())
}

fn parse_select_input_object(data: &[u8]) -> Result<()> {
    exact_len(data, 8, "select input object")?;
    let selected = validate_bool_byte(data[3], "select input object selected byte")?;
    let open = validate_bool_byte(data[4], "select input object open byte")?;
    if open && !selected {
        return Err(Error::invalid_data(
            "select input object cannot be open when it is not selected",
        ));
    }
    let object_id = object_id_at(data, 1);
    if selected && object_id == ObjectID::NULL {
        return Err(Error::invalid_data(
            "select input object selected target is NULL",
        ));
    }
    validate_reserved_tail(&data[5..8], "select input object reserved tail")?;
    Ok(())
}

fn parse_vt_esc(data: &[u8]) -> Result<()> {
    exact_len(data, 8, "VT ESC")?;
    validate_non_null_object_id(object_id_at(data, 1), "VT ESC object ID is NULL")?;
    validate_reserved_tail(&data[4..7], "VT ESC reserved bytes")?;
    validate_reserved_or_tan(data[7], "VT ESC reserved/TAN byte")?;
    Ok(())
}

fn parse_user_layout_hide_show(data: &[u8]) -> Result<()> {
    exact_len(data, 8, "user-layout hide/show")?;
    validate_non_null_object_id(object_id_at(data, 1), "user-layout first object ID is NULL")?;
    validate_bool_byte(data[3], "user-layout first shown byte")?;
    let second_id = object_id_at(data, 4);
    let second_shown = validate_bool_byte(data[6], "user-layout second shown byte")?;
    if second_id == ObjectID::NULL && second_shown {
        return Err(Error::invalid_data(
            "user-layout NULL second object cannot be shown",
        ));
    }
    validate_reserved_or_tan(data[7], "user-layout reserved/TAN byte")?;
    Ok(())
}

fn parse_pointing_event(data: &[u8]) -> Result<()> {
    exact_len(data, 8, "pointing event")?;
    if data[6] == 0xFF && data[7] == 0xFF {
        let state = validate_activation_code(data[5], "legacy pointing event touch state")?;
        if state == KeyActivationCode::Aborted {
            return Err(Error::invalid_data(
                "legacy pointing event touch state cannot be Aborted",
            ));
        }
        return Ok(());
    }

    let state_byte = data[5] & 0x0F;
    let state = validate_activation_code(state_byte, "VT6 pointing event touch state")?;
    if state == KeyActivationCode::Aborted {
        return Err(Error::invalid_data(
            "VT6 pointing event touch state cannot be Aborted",
        ));
    }
    validate_non_null_object_id(
        object_id_at(data, 6),
        "VT6 pointing event parent mask is NULL",
    )?;
    Ok(())
}

fn exact_len(data: &[u8], expected: usize, label: &'static str) -> Result<()> {
    if data.len() != expected {
        return Err(Error::invalid_data(match expected {
            8 => "fixed VT-to-ECU payload must be exactly 8 bytes",
            _ => label,
        }));
    }
    Ok(())
}

fn object_id_at(data: &[u8], offset: usize) -> ObjectID {
    ObjectID::from_le_bytes([data[offset], data[offset + 1]])
}

fn validate_non_null_object_id(object_id: ObjectID, message: &'static str) -> Result<()> {
    if object_id == ObjectID::NULL {
        return Err(Error::invalid_data(message));
    }
    Ok(())
}

fn validate_activation_code(value: u8, label: &'static str) -> Result<KeyActivationCode> {
    KeyActivationCode::try_from_u8(value).ok_or_else(|| {
        Error::invalid_data(match label {
            "soft-key activation" | "button activation" => "invalid activation code",
            "legacy pointing event touch state" => "invalid legacy pointing event touch state",
            "VT6 pointing event touch state" => "invalid VT6 pointing event touch state",
            _ => "invalid activation code",
        })
    })
}

fn validate_bool_byte(value: u8, message: &'static str) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(Error::invalid_data(message)),
    }
}

fn validate_reserved_tail(bytes: &[u8], message: &'static str) -> Result<()> {
    if bytes.iter().any(|b| *b != 0xFF) {
        return Err(Error::invalid_data(message));
    }
    Ok(())
}

fn validate_reserved_or_tan(value: u8, message: &'static str) -> Result<Option<u8>> {
    if value == 0xFF {
        return Ok(None);
    }
    if value & 0x0F == 0x0F {
        return Ok(Some(value >> 4));
    }
    Err(Error::invalid_data(message))
}

fn validate_transfer_sequence_number(value: u8, message: &'static str) -> Result<()> {
    if value > 0x0F {
        return Err(Error::invalid_data(message));
    }
    Ok(())
}

#[inline]
const fn encode_tan(value: u8) -> u8 {
    (value << 4) | 0x0F
}
