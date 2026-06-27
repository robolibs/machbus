//! VT audio-side-effect protocol helpers.
//!
//! Control Audio Signal itself is an ECU-to-VT side effect handled by the VT
//! server/runtime. When a VT stops that signal before completion, ISO defines a
//! separate VT-to-ECU termination notification and, for VT6+, a matching
//! ECU-to-VT acknowledgement. These helpers keep that wire shape out of the
//! oversized input runtime module.

use alloc::vec::Vec;

use crate::isobus::vt::commands::cmd;
use crate::net::constants::{BROADCAST_ADDRESS, NULL_ADDRESS};
use crate::net::pgn_defs::{PGN_ECU_TO_VT, PGN_VT_TO_ECU};
use crate::net::{Address, Error, Message, Result};

const TERMINATION_CAUSE_AUDIO_TERMINATED: u8 = 0x01;

/// VT-to-ECU Control Audio Signal Termination notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlAudioSignalTermination {
    pub transfer_sequence_number: Option<u8>,
}

impl ControlAudioSignalTermination {
    pub fn new(transfer_sequence_number: Option<u8>) -> Result<Self> {
        validate_optional_tan(
            transfer_sequence_number,
            "control-audio termination transfer sequence number exceeds 4-bit field",
        )?;
        Ok(Self {
            transfer_sequence_number,
        })
    }

    /// Build the 8-byte VT-to-ECU termination payload.
    ///
    /// VT5 and prior use an `0xFF` reserved byte in byte 3. VT6+ use byte 3
    /// as TAN in the high nibble and reserved `0xF` in the low nibble.
    #[must_use]
    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> [u8; 8] {
        audio_termination_payload(vt_version, self.transfer_sequence_number)
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        let transfer_sequence_number = parse_audio_termination_payload(data, vt_version, true)?;
        Self::new(transfer_sequence_number)
    }

    pub fn to_message(
        &self,
        vt_source: Address,
        ecu_destination: Address,
        vt_version: u16,
    ) -> Result<Message> {
        validate_destination_specific_envelope(vt_source, ecu_destination)?;
        Ok(Message::with_addressing(
            PGN_VT_TO_ECU,
            Vec::from(self.to_payload_for_vt_version(vt_version)),
            vt_source,
            ecu_destination,
            Default::default(),
        ))
    }

    pub fn from_message(msg: &Message, vt_version: u16) -> Result<Self> {
        if msg.pgn != PGN_VT_TO_ECU {
            return Err(Error::invalid_pgn(msg.pgn));
        }
        validate_destination_specific_envelope(msg.source, msg.destination)?;
        Self::from_payload_for_vt_version(&msg.data, vt_version)
    }
}

/// ECU-to-VT Control Audio Signal Termination response.
///
/// ISO defines this response only for VT6 and later, so it always carries a
/// concrete TAN nibble.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlAudioSignalTerminationResponse {
    pub transfer_sequence_number: u8,
}

impl ControlAudioSignalTerminationResponse {
    pub fn new(transfer_sequence_number: u8) -> Result<Self> {
        if transfer_sequence_number > 0x0F {
            return Err(Error::invalid_data(
                "control-audio termination response TAN exceeds 4-bit field",
            ));
        }
        Ok(Self {
            transfer_sequence_number,
        })
    }

    pub fn to_payload_for_vt_version(&self, vt_version: u16) -> Result<[u8; 8]> {
        if vt_version < 6 {
            return Err(Error::invalid_state(
                "control-audio termination response is defined only for VT6+",
            ));
        }
        Ok(audio_termination_payload(
            vt_version,
            Some(self.transfer_sequence_number),
        ))
    }

    pub fn from_payload_for_vt_version(data: &[u8], vt_version: u16) -> Result<Self> {
        if vt_version < 6 {
            return Err(Error::invalid_state(
                "control-audio termination response is defined only for VT6+",
            ));
        }
        let Some(tan) = parse_audio_termination_payload(data, vt_version, false)? else {
            return Err(Error::invalid_data(
                "control-audio termination response missing TAN",
            ));
        };
        Self::new(tan)
    }

    pub fn to_message(
        &self,
        ecu_source: Address,
        vt_destination: Address,
        vt_version: u16,
    ) -> Result<Message> {
        validate_destination_specific_envelope(ecu_source, vt_destination)?;
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
        validate_destination_specific_envelope(msg.source, msg.destination)?;
        Self::from_payload_for_vt_version(&msg.data, vt_version)
    }
}

fn audio_termination_payload(vt_version: u16, transfer_sequence_number: Option<u8>) -> [u8; 8] {
    let mut data = [0xFFu8; 8];
    data[0] = cmd::CONTROL_AUDIO_SIGNAL_TERMINATION;
    data[1] = TERMINATION_CAUSE_AUDIO_TERMINATED;
    if vt_version >= 6
        && let Some(tan) = transfer_sequence_number
    {
        data[2] = (tan << 4) | 0x0F;
    }
    data
}

fn parse_audio_termination_payload(
    data: &[u8],
    vt_version: u16,
    allow_legacy_reserved_byte: bool,
) -> Result<Option<u8>> {
    if data.len() != 8 {
        return Err(Error::invalid_data(
            "control-audio termination payload must be 8 bytes",
        ));
    }
    if data[0] != cmd::CONTROL_AUDIO_SIGNAL_TERMINATION {
        return Err(Error::invalid_data(
            "control-audio termination has wrong command byte",
        ));
    }
    if data[1] != TERMINATION_CAUSE_AUDIO_TERMINATED {
        return Err(Error::invalid_data(
            "control-audio termination cause must be audio-terminated bit only",
        ));
    }
    if data[3..].iter().any(|&byte| byte != 0xFF) {
        return Err(Error::invalid_data(
            "control-audio termination reserved tail bytes are not 0xFF",
        ));
    }
    if vt_version >= 6 {
        if data[2] & 0x0F != 0x0F {
            return Err(Error::invalid_data(
                "control-audio termination reserved TAN bits are not set",
            ));
        }
        Ok(Some(data[2] >> 4))
    } else if allow_legacy_reserved_byte && data[2] == 0xFF {
        Ok(None)
    } else {
        Err(Error::invalid_data(
            "control-audio termination reserved byte is not 0xFF",
        ))
    }
}

fn validate_optional_tan(value: Option<u8>, message: &'static str) -> Result<()> {
    if let Some(tan) = value
        && tan > 0x0F
    {
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
