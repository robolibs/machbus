//! ISO 11783-13 v2 File-Server property/volume codecs and NACK helper.
//!
//! Mirrors the C++ `machbus::isobus::fs::properties.hpp`. The C++
//! redefines `FileServerProperties` and `VolumeState` with
//! different layouts than `types.hpp`. The Rust port renames the v2
//! versions to [`FileServerPropertiesV2`] and [`VolumeStateV2`] for
//! disambiguation. See `book/src/reference/behavior-differences.md`.

use alloc::{borrow::ToOwned, string::String, vec::Vec};

use super::types::{FS_SUPPORTED_COUNT_MAX, fs_count_is_supported, is_valid_volume_name};
use crate::net::error::{Error, Result};

// ─── Enhanced FS command codes ────────────────────────────────────────

pub const FS_CMD_GET_PROPERTIES: u8 = 0x70;
pub const FS_CMD_GET_VOLUME_STATUS: u8 = 0x71;
pub const FS_CMD_PREPARE_VOLUME_REMOVAL: u8 = 0x72;
pub const FS_CMD_MAINTAIN_VOLUME: u8 = 0x73;
pub const FS_V2_PROPERTIES_VERSION: u8 = 2;

// ─── NACK error codes ─────────────────────────────────────────────────

pub const FS_NACK_NOT_SUPPORTED: u8 = 0x01;
pub const FS_NACK_INVALID_ACCESS: u8 = 0x02;
pub const FS_NACK_VOLUME_BUSY: u8 = 0x03;

// ─── FileServerPropertiesV2 ───────────────────────────────────────────

/// V2 properties block (ISO 11783-13 extensions). Use this when
/// talking to v2 file servers; the classic
/// [`super::types::FileServerProperties`] mirrors the v1 layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileServerPropertiesV2 {
    pub version_number: u8,
    pub max_open_files: u8,
    pub supports_volumes: bool,
    pub supports_long_filenames: bool,
    pub max_simultaneous_clients: u8,
}

impl Default for FileServerPropertiesV2 {
    fn default() -> Self {
        Self {
            version_number: 2,
            max_open_files: 16,
            supports_volumes: true,
            supports_long_filenames: true,
            max_simultaneous_clients: 4,
        }
    }
}

impl FileServerPropertiesV2 {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = FS_CMD_GET_PROPERTIES;
        data[1] = self.version_number;
        data[2] = self.max_open_files.min(FS_SUPPORTED_COUNT_MAX);
        let mut caps = 0u8;
        if self.supports_volumes {
            caps |= 0x01;
        }
        if self.supports_long_filenames {
            caps |= 0x02;
        }
        data[3] = caps;
        data[4] = self.max_simultaneous_clients.min(FS_SUPPORTED_COUNT_MAX);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        // Skip the leading command byte if present.
        let offset = if data.first().copied() == Some(FS_CMD_GET_PROPERTIES) {
            1
        } else {
            0
        };
        if data.len() < offset + 4 || data.len() > 8 {
            return None;
        }
        if data[offset + 4..].iter().any(|&b| b != 0xFF) {
            return None;
        }
        let caps = data[offset + 2];
        if caps & !0x03 != 0 {
            return None;
        }
        if data[offset] != FS_V2_PROPERTIES_VERSION
            || !fs_count_is_supported(data[offset + 1])
            || !fs_count_is_supported(data[offset + 3])
        {
            return None;
        }
        Some(Self {
            version_number: data[offset],
            max_open_files: data[offset + 1],
            supports_volumes: caps & 0x01 != 0,
            supports_long_filenames: caps & 0x02 != 0,
            max_simultaneous_clients: data[offset + 3],
        })
    }
}

// ─── VolumeStateV2 ────────────────────────────────────────────────────

/// V2 volume state (different from [`super::types::VolumeState`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum VolumeStateV2 {
    #[default]
    Mounted = 0,
    NotMounted = 1,
    PrepareForRemoval = 2,
    Maintenance = 3,
}

impl VolumeStateV2 {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::NotMounted,
            2 => Self::PrepareForRemoval,
            3 => Self::Maintenance,
            _ => Self::Mounted,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::Mounted),
            1 => Some(Self::NotMounted),
            2 => Some(Self::PrepareForRemoval),
            3 => Some(Self::Maintenance),
            _ => None,
        }
    }
}

// ─── VolumeStatus ─────────────────────────────────────────────────────

/// Variable-length volume status payload.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VolumeStatus {
    pub name: String,
    pub state: VolumeStateV2,
    pub total_bytes: u32,
    pub free_bytes: u32,
    pub removable: bool,
}

impl VolumeStatus {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if !self.name.is_empty() && !is_valid_volume_name(&self.name) {
            return Err(Error::invalid_data(
                "FS volume status names must use the canonical volume-name grammar",
            ));
        }
        if self.name.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "FS volume status name exceeds one-byte length field",
            ));
        }
        if self.free_bytes > self.total_bytes {
            return Err(Error::invalid_data(
                "FS volume status free space must not exceed total space",
            ));
        }

        let mut data = Vec::with_capacity(12 + self.name.len());
        data.push(FS_CMD_GET_VOLUME_STATUS);
        data.push(self.state.as_u8());
        data.push(if self.removable { 0x01 } else { 0x00 });
        data.extend_from_slice(&self.total_bytes.to_le_bytes());
        data.extend_from_slice(&self.free_bytes.to_le_bytes());
        let name_len = self.name.len() as u8;
        data.push(name_len);
        data.extend_from_slice(self.name.as_bytes());
        Ok(data)
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }
        let offset = if data[0] == FS_CMD_GET_VOLUME_STATUS {
            1
        } else {
            0
        };
        if offset + 11 > data.len() {
            return None;
        }
        let name_len = data[offset + 10] as usize;
        let end = offset + 11 + name_len;
        if end != data.len() {
            return None;
        }
        if data[offset + 1] & !0x01 != 0 {
            return None;
        }
        let name_bytes = &data[offset + 11..end];
        if !name_bytes.is_ascii() {
            return None;
        }
        let name = core::str::from_utf8(name_bytes).ok()?;
        if !name.is_empty() && !is_valid_volume_name(name) {
            return None;
        }
        let total_bytes = u32::from_le_bytes(data[offset + 2..offset + 6].try_into().unwrap());
        let free_bytes = u32::from_le_bytes(data[offset + 6..offset + 10].try_into().unwrap());
        if free_bytes > total_bytes {
            return None;
        }
        Some(Self {
            state: VolumeStateV2::try_from_u8(data[offset])?,
            removable: data[offset + 1] & 0x01 != 0,
            total_bytes,
            free_bytes,
            name: name.to_owned(),
        })
    }
}

// ─── NACK helper ──────────────────────────────────────────────────────

/// NACK frame for unsupported commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FSNack {
    pub command_code: u8,
    pub error_code: u8,
}

impl FSNack {
    #[must_use]
    pub fn encode(&self) -> [u8; 8] {
        let mut data = [0xFFu8; 8];
        data[0] = self.command_code;
        data[1] = self.error_code;
        data
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn properties_v2_round_trip() {
        let p = FileServerPropertiesV2 {
            version_number: 2,
            max_open_files: 32,
            supports_volumes: false,
            supports_long_filenames: true,
            max_simultaneous_clients: 8,
        };
        let bytes = p.encode();
        // First byte is command code, then payload.
        assert_eq!(bytes[0], FS_CMD_GET_PROPERTIES);
        assert_eq!(FileServerPropertiesV2::decode(&bytes), Some(p));
    }

    #[test]
    fn properties_v2_decode_without_command_byte() {
        let p = FileServerPropertiesV2::default();
        let mut bytes = p.encode().to_vec();
        bytes.remove(0); // drop command code
        bytes.resize(8, 0xFF); // pad back to 8 bytes
        // With command byte stripped, decode should still work via the
        // `offset = 0` branch.
        let d = FileServerPropertiesV2::decode(&bytes).unwrap();
        assert_eq!(d.version_number, p.version_number);
    }

    #[test]
    fn volume_state_v2_round_trip() {
        for s in [
            VolumeStateV2::Mounted,
            VolumeStateV2::NotMounted,
            VolumeStateV2::PrepareForRemoval,
            VolumeStateV2::Maintenance,
        ] {
            assert_eq!(VolumeStateV2::from_u8(s.as_u8()), s);
            assert_eq!(VolumeStateV2::try_from_u8(s.as_u8()), Some(s));
        }
        assert_eq!(VolumeStateV2::try_from_u8(4), None);
    }

    #[test]
    fn volume_status_round_trip() {
        let v = VolumeStatus {
            name: "DISK".to_string(),
            state: VolumeStateV2::Mounted,
            total_bytes: 1_000_000,
            free_bytes: 500_000,
            removable: true,
        };
        let bytes = v.encode().unwrap();
        let d = VolumeStatus::decode(&bytes);
        assert_eq!(d, Some(v));
    }

    #[test]
    fn volume_status_encode_rejects_unencodable_names() {
        let mut v = VolumeStatus {
            name: "D".repeat(u8::MAX as usize),
            state: VolumeStateV2::Mounted,
            total_bytes: 1_000_000,
            free_bytes: 500_000,
            removable: true,
        };
        let max_len = v.encode().unwrap();
        assert_eq!(max_len[11], u8::MAX);
        assert_eq!(max_len.len(), 12 + u8::MAX as usize);

        v.name.push('X');
        let err = v.encode().unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);

        v.name = "DÍSK".to_string();
        let err = v.encode().unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);

        v.name = "BAD\\DISK".to_string();
        let err = v.encode().unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);

        v.name = "DISK".to_string();
        v.total_bytes = 10;
        v.free_bytes = 11;
        let err = v.encode().unwrap_err();
        assert_eq!(err.code, crate::net::error::ErrorCode::InvalidData);
    }

    #[test]
    fn properties_v2_rejects_short_overlong_and_bad_padding() {
        let props = FileServerPropertiesV2::default().encode();
        assert!(FileServerPropertiesV2::decode(&props[..4]).is_none());
        assert!(FileServerPropertiesV2::decode(&[props.as_slice(), &[0xFF]].concat()).is_none());

        let mut bad_padding = props;
        bad_padding[5] = 0x00;
        assert!(FileServerPropertiesV2::decode(&bad_padding).is_none());
    }

    #[test]
    fn volume_status_rejects_truncated_and_trailing_payloads() {
        let v = VolumeStatus {
            name: "DISK".to_string(),
            state: VolumeStateV2::Mounted,
            total_bytes: 1_000_000,
            free_bytes: 500_000,
            removable: true,
        };
        let bytes = v.encode().unwrap();
        assert!(VolumeStatus::decode(&bytes[..bytes.len() - 1]).is_none());
        assert!(VolumeStatus::decode(&[bytes.as_slice(), &[0xFF]].concat()).is_none());

        let mut wrong_len = bytes.clone();
        wrong_len[11] = wrong_len[11].saturating_add(1);
        assert!(VolumeStatus::decode(&wrong_len).is_none());

        let mut reserved_state = bytes;
        reserved_state[1] = 4;
        assert!(VolumeStatus::decode(&reserved_state).is_none());
    }

    #[test]
    fn fs_nack_layout() {
        let nack = FSNack {
            command_code: 0x10,
            error_code: FS_NACK_NOT_SUPPORTED,
        };
        let bytes = nack.encode();
        assert_eq!(bytes[0], 0x10);
        assert_eq!(bytes[1], 0x01);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_fs_v2_decoders_accept_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=512),
        ) {
            if let Some(props) = FileServerPropertiesV2::decode(&data) {
                let _ = props.encode();
            }

            if let Some(volume) = VolumeStatus::decode(&data) {
                let _ = volume.encode();
                prop_assert!(volume.name.len() <= data.len().saturating_mul(2));
            }
        }
    }
}
