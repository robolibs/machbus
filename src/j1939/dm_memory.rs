//! Memory access protocol — DM14 / DM15 / DM16 plus ECU
//! Identification.
//!
//! Mirrors the C++ `machbus::j1939::dm_memory.hpp`. Used by service
//! tools to read/write/erase ECU memory and to query identification
//! strings.
//!
//! - `PGN_DM14` (0xD900): memory-access **request** (tool → ECU)
//! - `PGN_DM15` (0xD800): memory-access **response** (ECU → tool)
//! - `PGN_DM16` (0xD700): binary data transfer
//! - `PGN_ECU_IDENTIFICATION` (0xFDC5): `*`-delimited identification
//!   strings

use alloc::{format, string::String, vec::Vec};

use crate::j1939::text::{decode_iso11783_text_field, encode_iso11783_text_field};
use crate::net::error::{Error, Result};
use crate::net::message::Message;
use crate::net::pgn_defs::PGN_DM14;

// ─── DM14 Request ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Dm14Command {
    #[default]
    Read = 0,
    Write = 1,
    StatusRequest = 2,
    Erase = 3,
    BootLoad = 4,
    EdcpGeneration = 5,
}

impl Dm14Command {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            1 => Self::Write,
            2 => Self::StatusRequest,
            3 => Self::Erase,
            4 => Self::BootLoad,
            5 => Self::EdcpGeneration,
            _ => Self::Read,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Read),
            1 => Some(Self::Write),
            2 => Some(Self::StatusRequest),
            3 => Some(Self::Erase),
            4 => Some(Self::BootLoad),
            5 => Some(Self::EdcpGeneration),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Dm14PointerType {
    #[default]
    DirectPhysical = 0,
    DirectVirtual = 1,
    Indirect = 2,
    NotAvailable = 3,
}

impl Dm14PointerType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x03 {
            0 => Self::DirectPhysical,
            1 => Self::DirectVirtual,
            2 => Self::Indirect,
            _ => Self::NotAvailable,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::DirectPhysical),
            1 => Some(Self::DirectVirtual),
            2 => Some(Self::Indirect),
            3 => Some(Self::NotAvailable),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dm14Request {
    pub command: Dm14Command,
    pub pointer_type: Dm14PointerType,
    /// 24-bit memory address.
    pub address: u32,
    pub length: u16,
    /// Security key / seed response (`0xFF` if unused).
    pub key: u8,
}

impl Dm14Request {
    pub fn encode(&self) -> Result<[u8; 8]> {
        if self.address > 0xFF_FFFF {
            return Err(Error::invalid_data(format!(
                "DM14 address 0x{:X} exceeds the 24-bit wire field",
                self.address
            )));
        }
        let mut data = [0xFFu8; 8];
        data[0] = (self.command.as_u8() & 0x07) | ((self.pointer_type.as_u8() & 0x03) << 4);
        data[1] = (self.length & 0xFF) as u8;
        data[2] = ((self.length >> 8) & 0xFF) as u8;
        data[3] = (self.address & 0xFF) as u8;
        data[4] = ((self.address >> 8) & 0xFF) as u8;
        data[5] = ((self.address >> 16) & 0xFF) as u8;
        data[6] = self.key;
        Ok(data)
    }

    /// Decode from a classic 8-byte DM14 payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[0] & 0b1100_1000 != 0 {
            return None;
        }
        if data[7] != 0xFF {
            return None;
        }
        let command = Dm14Command::try_from_u8(data[0] & 0x07)?;
        Some(Self {
            command,
            pointer_type: Dm14PointerType::try_from_u8(data[0] >> 4)?,
            length: (data[1] as u16) | ((data[2] as u16) << 8),
            address: (data[3] as u32) | ((data[4] as u32) << 8) | ((data[5] as u32) << 16),
            key: data[6],
        })
    }

    #[inline]
    #[must_use]
    pub fn from_message(msg: &Message) -> Option<Self> {
        if !msg.has_usable_envelope_for_pgn(PGN_DM14) {
            return None;
        }
        Self::decode(&msg.data)
    }
}

// ─── DM15 Response ─────────────────────────────────────────────────────

/// DM15 response status code. The C++ enum also defines
/// `Reserved = 0xFF`, but its encoder masks the field to 3 bits
/// (`& 0x07`), which silently truncates `0xFF` to `0x07` on the
/// wire — making `Reserved` not round-trippable. The Rust port omits
/// it (see `book/src/reference/behavior-differences.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Dm15Status {
    #[default]
    Proceed = 0,
    Busy = 1,
    Completed = 2,
    Error = 3,
    EdcpFault = 4,
}

impl Dm15Status {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v & 0x07 {
            1 => Self::Busy,
            2 => Self::Completed,
            3 => Self::Error,
            4 => Self::EdcpFault,
            _ => Self::Proceed,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Proceed),
            1 => Some(Self::Busy),
            2 => Some(Self::Completed),
            3 => Some(Self::Error),
            4 => Some(Self::EdcpFault),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dm15Response {
    pub status: Dm15Status,
    pub length: u16,
    pub address: u32,
    pub edcp_extension: u8,
    pub seed: u8,
}

impl Default for Dm15Response {
    fn default() -> Self {
        Self {
            status: Dm15Status::Proceed,
            length: 0,
            address: 0,
            edcp_extension: 0xFF,
            seed: 0xFF,
        }
    }
}

impl Dm15Response {
    pub fn encode(&self) -> Result<[u8; 8]> {
        if self.address > 0xFF_FFFF {
            return Err(Error::invalid_data(format!(
                "DM15 address 0x{:X} exceeds the 24-bit wire field",
                self.address
            )));
        }
        let mut data = [0xFFu8; 8];
        data[0] = self.status.as_u8() & 0x07;
        data[1] = (self.length & 0xFF) as u8;
        data[2] = ((self.length >> 8) & 0xFF) as u8;
        data[3] = (self.address & 0xFF) as u8;
        data[4] = ((self.address >> 8) & 0xFF) as u8;
        data[5] = ((self.address >> 16) & 0xFF) as u8;
        data[6] = self.edcp_extension;
        data[7] = self.seed;
        Ok(data)
    }

    /// Decode from a classic 8-byte DM15 payload.
    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 8 {
            return None;
        }
        if data[0] & 0b1111_1000 != 0 {
            return None;
        }
        let status = Dm15Status::try_from_u8(data[0])?;
        Some(Self {
            status,
            length: (data[1] as u16) | ((data[2] as u16) << 8),
            address: (data[3] as u32) | ((data[4] as u32) << 8) | ((data[5] as u32) << 16),
            edcp_extension: data[6],
            seed: data[7],
        })
    }
}

// ─── DM16 Binary Data Transfer ─────────────────────────────────────────

/// Binary data carried via PGN 0xD700. Single-frame fits 7 bytes;
/// larger payloads use TP underneath.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Dm16Transfer {
    pub num_bytes: u8,
    pub data: Vec<u8>,
}

impl Dm16Transfer {
    /// Encode to the 8-byte single-frame format.
    pub fn encode(&self) -> Result<[u8; 8]> {
        if self.num_bytes > 7 {
            return Err(Error::invalid_data(format!(
                "DM16 single-frame num_bytes {} exceeds the 7-byte payload field",
                self.num_bytes
            )));
        }
        if self.data.len() != self.num_bytes as usize {
            return Err(Error::invalid_data(format!(
                "DM16 single-frame declared {} bytes but data has {} bytes",
                self.num_bytes,
                self.data.len()
            )));
        }
        let mut out = [0xFFu8; 8];
        out[0] = self.num_bytes;
        let n = self.data.len();
        out[1..1 + n].copy_from_slice(&self.data);
        Ok(out)
    }

    /// Decode from a payload (handles both single-frame and TP-larger
    /// payloads).
    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        let num_bytes = *raw.first()?;
        let declared_len = num_bytes as usize;
        let payload_len = raw.len().checked_sub(1)?;
        if raw.len() == 8 {
            if declared_len > 7 || payload_len < declared_len {
                return None;
            }
            if raw[1 + declared_len..].iter().any(|b| *b != 0xFF) {
                return None;
            }
        } else if payload_len != declared_len {
            return None;
        }
        Some(Self {
            num_bytes,
            data: raw[1..1 + declared_len].to_vec(),
        })
    }
}

// ─── ECU Identification ────────────────────────────────────────────────

/// Five or six `*`-delimited identification fields. Multi-frame; sent via TP
/// when the encoded payload exceeds 8 bytes.
///
/// ISO 11783 ECU Identification appends a hardware ID field after the J1939
/// fields. J1939 mode omits that final hardware ID.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EcuIdentification {
    pub ecu_part_number: String,
    pub ecu_serial_number: String,
    pub ecu_location: String,
    pub ecu_type: String,
    pub ecu_manufacturer: String,
    pub ecu_hardware_id: Option<String>,
}

impl EcuIdentification {
    /// Encode each field followed by `'*'`.
    ///
    /// If [`Self::ecu_hardware_id`] is present, this emits the ISO 11783
    /// six-field form. Otherwise it emits the J1939 five-field form.
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.encode_with_hardware_id(self.ecu_hardware_id.as_deref())
    }

    /// Encode the J1939 five-field ECU Identification form, even when a
    /// hardware ID is present.
    pub fn encode_j1939(&self) -> Result<Vec<u8>> {
        self.encode_with_hardware_id(None)
    }

    /// Encode the ISO 11783 six-field ECU Identification form, appending an
    /// empty hardware ID field if no hardware ID has been configured.
    pub fn encode_iso11783(&self) -> Result<Vec<u8>> {
        self.encode_with_hardware_id(Some(self.ecu_hardware_id.as_deref().unwrap_or("")))
    }

    fn encode_with_hardware_id(&self, hardware_id: Option<&str>) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        for (name, field) in [
            ("ecu_part_number", &self.ecu_part_number),
            ("ecu_serial_number", &self.ecu_serial_number),
            ("ecu_location", &self.ecu_location),
            ("ecu_type", &self.ecu_type),
            ("ecu_manufacturer", &self.ecu_manufacturer),
        ] {
            validate_ecu_identification_field(name, field)?;
            data.extend_from_slice(&encode_iso11783_text_field(name, field, &[])?);
            data.push(b'*');
        }
        if let Some(field) = hardware_id {
            validate_ecu_identification_field("ecu_hardware_id", field)?;
            data.extend_from_slice(&encode_iso11783_text_field(
                "ecu_hardware_id",
                field,
                &['#'],
            )?);
            data.push(b'*');
        }
        Ok(data)
    }

    /// Encode fields without validation. This is only useful for tests/tools
    /// that intentionally need malformed payload bytes; normal send paths
    /// should use [`Self::encode`].
    #[must_use]
    pub fn encode_lossy_for_malformed_fixture(&self) -> Vec<u8> {
        let mut data = Vec::new();
        for field in [
            &self.ecu_part_number,
            &self.ecu_serial_number,
            &self.ecu_location,
            &self.ecu_type,
            &self.ecu_manufacturer,
        ] {
            data.extend_from_slice(field.as_bytes());
            data.push(b'*');
        }
        if let Some(field) = &self.ecu_hardware_id {
            data.extend_from_slice(field.as_bytes());
            data.push(b'*');
        }
        data
    }

    /// Decode from exactly five J1939 fields or six ISO 11783 fields.
    #[must_use]
    pub fn decode(raw: &[u8]) -> Option<Self> {
        let mut fields = Vec::with_capacity(6);
        let mut start = 0usize;
        for (idx, &byte) in raw.iter().enumerate() {
            if byte == b'*' {
                if fields.len() == 6 {
                    return None;
                }
                let field = decode_iso11783_text_field(&raw[start..idx])?;
                fields.push(field);
                start = idx + 1;
            } else if !(0x20..=0x7E).contains(&byte) {
                decode_iso11783_text_field(&[byte])?;
            }
        }
        if start != raw.len() || !(fields.len() == 5 || fields.len() == 6) {
            return None;
        }
        if fields
            .get(5)
            .is_some_and(|hardware_id| hardware_id.contains('#'))
        {
            return None;
        }
        Some(Self {
            ecu_part_number: fields[0].clone(),
            ecu_serial_number: fields[1].clone(),
            ecu_location: fields[2].clone(),
            ecu_type: fields[3].clone(),
            ecu_manufacturer: fields[4].clone(),
            ecu_hardware_id: fields.get(5).cloned(),
        })
    }
}

fn validate_ecu_identification_field(field_name: &'static str, value: &str) -> Result<()> {
    let forbidden = if field_name == "ecu_hardware_id" {
        &['#'][..]
    } else {
        &[][..]
    };
    encode_iso11783_text_field(field_name, value, forbidden)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dm14_round_trip() {
        let req = Dm14Request {
            command: Dm14Command::Write,
            pointer_type: Dm14PointerType::DirectVirtual,
            address: 0x12_3456,
            length: 0xCAFE,
            key: 0xAB,
        };
        let decoded = Dm14Request::decode(&req.encode().unwrap()).unwrap();
        assert_eq!(decoded, req);
    }

    #[test]
    fn dm14_encode_rejects_unencodable_address() {
        let req = Dm14Request {
            address: 0x0100_0000,
            ..Dm14Request::default()
        };
        assert!(req.encode().is_err());
    }

    #[test]
    fn dm14_short_returns_none() {
        assert!(Dm14Request::decode(&[0u8; 7]).is_none());
    }

    #[test]
    fn dm14_oversized_returns_none() {
        assert!(Dm14Request::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn dm14_reserved_control_bits_and_commands_return_none() {
        let mut reserved_bit = Dm14Request::default().encode().unwrap();
        reserved_bit[0] |= 0b0000_1000;
        assert!(Dm14Request::decode(&reserved_bit).is_none());

        let mut reserved_command = Dm14Request::default().encode().unwrap();
        reserved_command[0] = 0b0000_0110;
        assert!(Dm14Request::decode(&reserved_command).is_none());

        let mut reserved_tail = Dm14Request::default().encode().unwrap();
        reserved_tail[7] = 0x00;
        assert!(Dm14Request::decode(&reserved_tail).is_none());
    }

    #[test]
    fn dm15_round_trip() {
        let resp = Dm15Response {
            status: Dm15Status::Completed,
            length: 0xBEEF,
            address: 0x98_7654,
            edcp_extension: 0x42,
            seed: 0x99,
        };
        let decoded = Dm15Response::decode(&resp.encode().unwrap()).unwrap();
        assert_eq!(decoded, resp);
    }

    #[test]
    fn dm15_encode_rejects_unencodable_address() {
        let resp = Dm15Response {
            address: 0x0100_0000,
            ..Dm15Response::default()
        };
        assert!(resp.encode().is_err());
    }

    #[test]
    fn dm15_short_and_oversized_return_none() {
        assert!(Dm15Response::decode(&[0u8; 7]).is_none());
        assert!(Dm15Response::decode(&[0u8; 9]).is_none());
    }

    #[test]
    fn dm15_reserved_status_bits_and_values_return_none() {
        let mut reserved_bit = Dm15Response::default().encode().unwrap();
        reserved_bit[0] |= 0b0000_1000;
        assert!(Dm15Response::decode(&reserved_bit).is_none());

        let mut reserved_status = Dm15Response::default().encode().unwrap();
        reserved_status[0] = 0b0000_0101;
        assert!(Dm15Response::decode(&reserved_status).is_none());
    }

    #[test]
    fn dm16_round_trip_single_frame() {
        let t = Dm16Transfer {
            num_bytes: 5,
            data: vec![1, 2, 3, 4, 5],
        };
        let bytes = t.encode().unwrap();
        assert_eq!(bytes[0], 5);
        assert_eq!(&bytes[1..6], &[1, 2, 3, 4, 5]);
        let decoded = Dm16Transfer::decode(&bytes).unwrap();
        assert_eq!(decoded.num_bytes, 5);
        assert_eq!(decoded.data, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn dm16_decode_requires_declared_length_and_padding() {
        let padded = [3u8, 0xAA, 0xBB, 0xCC, 0xFF, 0xFF, 0xFF, 0xFF];
        let decoded = Dm16Transfer::decode(&padded).unwrap();
        assert_eq!(decoded.data, vec![0xAA, 0xBB, 0xCC]);

        let hidden_tail = [3u8, 0xAA, 0xBB, 0xCC, 0x00, 0xFF, 0xFF, 0xFF];
        assert!(Dm16Transfer::decode(&hidden_tail).is_none());

        let too_long_classic = [8u8, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0xFF];
        assert!(Dm16Transfer::decode(&too_long_classic).is_none());

        let exact_large = [8u8, 1, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(
            Dm16Transfer::decode(&exact_large).unwrap().data,
            vec![1, 2, 3, 4, 5, 6, 7, 8]
        );

        let truncated_large = [8u8, 1, 2, 3, 4, 5, 6, 7];
        assert!(Dm16Transfer::decode(&truncated_large).is_none());
    }

    #[test]
    fn dm16_empty_returns_none() {
        assert!(Dm16Transfer::decode(&[]).is_none());
    }

    #[test]
    fn dm16_encode_rejects_single_frame_length_mismatches() {
        assert!(
            Dm16Transfer {
                num_bytes: 8,
                data: vec![0; 8],
            }
            .encode()
            .is_err()
        );
        assert!(
            Dm16Transfer {
                num_bytes: 3,
                data: vec![1, 2],
            }
            .encode()
            .is_err()
        );
        assert!(
            Dm16Transfer {
                num_bytes: 2,
                data: vec![1, 2, 3],
            }
            .encode()
            .is_err()
        );
    }

    #[test]
    fn ecu_identification_round_trips() {
        let id = EcuIdentification {
            ecu_part_number: "ABC123".into(),
            ecu_serial_number: "SN-001".into(),
            ecu_location: "MainCab".into(),
            ecu_type: "TECU".into(),
            ecu_manufacturer: "Acme".into(),
            ecu_hardware_id: None,
        };
        let bytes = id.encode().unwrap();
        // 4 commas... I mean, one '*' per field.
        assert_eq!(bytes.iter().filter(|b| **b == b'*').count(), 5);
        let decoded = EcuIdentification::decode(&bytes).unwrap();
        assert_eq!(decoded, id);

        let iso_id = EcuIdentification {
            ecu_hardware_id: Some("HW-1".into()),
            ..id
        };
        let iso_bytes = iso_id.encode_iso11783().unwrap();
        assert_eq!(iso_bytes.iter().filter(|b| **b == b'*').count(), 6);
        assert_eq!(EcuIdentification::decode(&iso_bytes), Some(iso_id));
    }

    #[test]
    fn ecu_identification_rejects_partial_extra_or_invalid_text_fields() {
        assert!(EcuIdentification::decode(b"ABC*SN-001*").is_none());
        assert!(EcuIdentification::decode(b"ABC*SN-001*MainCab*TECU*Acme*HW*extra*").is_none());
        assert_eq!(
            EcuIdentification::decode(b"ABC*SN-001*MainCab*TECU*Acme\xFF*"),
            Some(EcuIdentification {
                ecu_part_number: "ABC".into(),
                ecu_serial_number: "SN-001".into(),
                ecu_location: "MainCab".into(),
                ecu_type: "TECU".into(),
                ecu_manufacturer: "Acmeÿ".into(),
                ecu_hardware_id: None,
            })
        );
        assert!(EcuIdentification::decode(b"ABC*SN-001*MainCab*TECU*Acme\x80*").is_none());
        assert!(EcuIdentification::decode(b"ABC*SN-001*MainCab*TECU*Acme*HW#1*").is_none());
    }

    #[test]
    fn ecu_identification_encode_rejects_delimiters_and_non_printable_text() {
        let with_delimiter = EcuIdentification {
            ecu_part_number: "ABC*123".into(),
            ..EcuIdentification::default()
        };
        assert!(with_delimiter.encode().is_err());

        let with_newline = EcuIdentification {
            ecu_serial_number: "SN\n001".into(),
            ..EcuIdentification::default()
        };
        assert!(with_newline.encode().is_err());

        let with_reserved_hardware_char = EcuIdentification {
            ecu_hardware_id: Some("HW#1".into()),
            ..EcuIdentification::default()
        };
        assert!(with_reserved_hardware_char.encode_iso11783().is_err());
    }
}
