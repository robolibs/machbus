//! Storage-agnostic Virtual Terminal stored-pool blobs.
//!
//! ISO 11783-6 VT servers can cache uploaded object pools under short version
//! labels. Hosted builds may put these blobs on disk, but embedded
//! applications usually own flash, EEPROM, SD, or LittleFS directly. This
//! module keeps the portable byte format independent from any filesystem.

use alloc::{string::String, vec::Vec};

/// Magic prefix for stored VT pool blobs.
pub const VT_STORAGE_MAGIC: &[u8; 4] = b"VTP1";

/// Header length for stored VT pool blobs.
pub const VT_STORAGE_HEADER_LEN: usize = 4 + 8 + 4 + 2 + 1 + 8;

/// Maximum accepted stored-pool payload size.
pub const MAX_STORED_POOL_BYTES: usize = 16 * 1024 * 1024;

/// Stored pool version with metadata.
///
/// The encoded format is:
///
/// ```text
/// [0..4]  Magic "VTP1"
/// [4..12] timestamp_us (u64 LE)
/// [12..16] size_bytes  (u32 LE)
/// [16..18] vt_version  (u16 LE)
/// [18..19] object_count (u8)
/// [19..27] label, NUL-padded to 8 bytes
/// [27..]   pool_data
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StoredPoolVersion {
    /// 7-character version label.
    pub label: String,
    /// Raw serialized object-pool bytes.
    pub pool_data: Vec<u8>,
    /// Microseconds since UNIX epoch, or a caller-owned monotonic/persistent
    /// timestamp if the embedded application does not use wall-clock time.
    pub timestamp_us: u64,
    pub size_bytes: u32,
    pub vt_version: u16,
    pub object_count: u8,
}

impl StoredPoolVersion {
    /// Refresh size, VT version, object count, and caller-supplied timestamp.
    pub fn update_metadata_at(&mut self, vt_ver: u16, timestamp_us: u64) {
        self.size_bytes = u32::try_from(self.pool_data.len()).unwrap_or(u32::MAX);
        self.vt_version = vt_ver;
        if self.pool_data.len() >= 2 {
            self.object_count = self.pool_data[0]; // C++ ORs in [1] but truncates to u8
        }
        self.timestamp_us = timestamp_us;
    }

    /// Refresh size, VT version, object count, and host wall-clock timestamp.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn update_metadata(&mut self, vt_ver: u16) {
        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        self.update_metadata_at(vt_ver, timestamp_us);
    }

    /// `true` once `now_us - timestamp_us > max_age_days`.
    #[must_use]
    pub fn is_expired_at(&self, now_us: u64, max_age_days: u32) -> bool {
        if self.timestamp_us == 0 {
            return false;
        }
        let age_us = now_us.saturating_sub(self.timestamp_us);
        let max_age_us = (max_age_days as u64) * 24 * 3600 * 1_000_000;
        age_us > max_age_us
    }

    /// `true` once host wall-clock `now - timestamp_us > max_age_days`.
    #[cfg(any(feature = "default", feature = "cli"))]
    #[must_use]
    pub fn is_expired(&self, max_age_days: u32) -> bool {
        if self.timestamp_us == 0 {
            return false;
        }
        let now_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        self.is_expired_at(now_us, max_age_days)
    }

    /// Serialize this stored pool version to the portable `VTP1` blob format.
    ///
    /// Embedded applications can write this returned byte buffer to flash,
    /// EEPROM, SD, LittleFS, etc. themselves.
    #[must_use]
    pub fn to_storage_bytes(&self) -> Option<Vec<u8>> {
        if !is_valid_classic_label(&self.label)
            || self.pool_data.len() > MAX_STORED_POOL_BYTES
            || u32::try_from(self.pool_data.len()).ok() != Some(self.size_bytes)
        {
            return None;
        }
        let mut buf = Vec::with_capacity(VT_STORAGE_HEADER_LEN + self.pool_data.len());
        buf.extend_from_slice(VT_STORAGE_MAGIC);
        buf.extend_from_slice(&self.timestamp_us.to_le_bytes());
        buf.extend_from_slice(&self.size_bytes.to_le_bytes());
        buf.extend_from_slice(&self.vt_version.to_le_bytes());
        buf.push(self.object_count);
        let mut label_buf = [0u8; 8];
        for (i, b) in self.label.as_bytes().iter().take(7).enumerate() {
            label_buf[i] = *b;
        }
        buf.extend_from_slice(&label_buf);
        buf.extend_from_slice(&self.pool_data);
        Some(buf)
    }

    /// Decode a portable `VTP1` blob produced by [`Self::to_storage_bytes`].
    #[must_use]
    pub fn from_storage_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < VT_STORAGE_HEADER_LEN || &bytes[0..4] != VT_STORAGE_MAGIC {
            return None;
        }
        let timestamp_us = u64::from_le_bytes(bytes[4..12].try_into().ok()?);
        let size_bytes = u32::from_le_bytes(bytes[12..16].try_into().ok()?);
        let size = size_bytes as usize;
        if size > MAX_STORED_POOL_BYTES || bytes.len() != VT_STORAGE_HEADER_LEN + size {
            return None;
        }
        let vt_version = u16::from_le_bytes(bytes[16..18].try_into().ok()?);
        let object_count = bytes[18];
        let stored_label = decode_stored_label_field(&bytes[19..27])?;
        let pool_data = bytes[VT_STORAGE_HEADER_LEN..].to_vec();
        Some(Self {
            label: stored_label,
            pool_data,
            timestamp_us,
            size_bytes,
            vt_version,
            object_count,
        })
    }
}

#[must_use]
pub fn is_valid_classic_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 7
        && label != "."
        && label != ".."
        && bytes.iter().all(|&b| {
            matches!(b, b'!'..=b'~')
                && !matches!(
                    b,
                    b'/' | b'\\' | b':' | b'*' | b'?' | b'"' | b'<' | b'>' | b'|'
                )
        })
}

fn decode_stored_label_field(field: &[u8]) -> Option<String> {
    let label_len = field.iter().position(|&b| b == 0).unwrap_or(field.len());
    if field[label_len..].iter().any(|&b| b != 0) {
        return None;
    }
    let label = core::str::from_utf8(&field[..label_len]).ok()?;
    is_valid_classic_label(label).then(|| label.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};

    #[test]
    fn explicit_timestamp_helpers_are_storage_agnostic() {
        let mut v = StoredPoolVersion {
            label: "V1".to_string(),
            pool_data: vec![1, 2, 3],
            ..Default::default()
        };
        v.update_metadata_at(7, 1_000_000);
        assert_eq!(v.size_bytes, 3);
        assert_eq!(v.vt_version, 7);
        assert_eq!(v.timestamp_us, 1_000_000);
        assert!(!v.is_expired_at(1_000_000, 1));
        assert!(v.is_expired_at(1_000_000 + 2 * 24 * 3600 * 1_000_000, 1));
    }

    #[test]
    fn storage_blob_round_trips_without_filesystem() {
        let mut v = StoredPoolVersion {
            label: "LABEL01".to_string(),
            pool_data: vec![2, 0, 1, 2, 3],
            ..Default::default()
        };
        v.update_metadata_at(5, 123);
        let bytes = v.to_storage_bytes().unwrap();
        let restored = StoredPoolVersion::from_storage_bytes(&bytes).unwrap();
        assert_eq!(restored, v);
    }

    #[test]
    fn rejects_bad_labels_and_trailing_label_garbage() {
        let mut v = StoredPoolVersion {
            label: "BAD/ONE".to_string(),
            pool_data: vec![0],
            ..Default::default()
        };
        v.update_metadata_at(1, 0);
        assert!(v.to_storage_bytes().is_none());

        let mut ok = StoredPoolVersion {
            label: "OK".to_string(),
            pool_data: vec![0],
            ..Default::default()
        };
        ok.update_metadata_at(1, 0);
        let mut bytes = ok.to_storage_bytes().unwrap();
        bytes[22] = b'X';
        assert!(StoredPoolVersion::from_storage_bytes(&bytes).is_none());
    }
}
