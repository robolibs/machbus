//! Server-side working-set state per connected VT client.
//!
//! Mirrors the C++ `machbus::isobus::vt::ServerWorkingSet` (319 LOC).
//! On-disk persistence uses a small `VTP1` magic + fixed-layout
//! header. Storage layout:
//!
//! ```text
//! <storage_path>/<addr_hex>/<label>.vtp
//! ```
//!
//! On-disk file format (matches C++ byte-for-byte):
//!
//! ```text
//! [0..4]  Magic "VTP1"
//! [4..12] timestamp_us (u64 LE)
//! [12..16] size_bytes  (u32 LE)
//! [16..18] vt_version  (u16 LE)
//! [18..19] object_count (u8)
//! [19..27] label, NUL-padded to 8 bytes
//! [27..]   pool_data
//! ```

use super::objects::{ObjectID, ObjectPool};
use super::working_set::WorkingSet;
use crate::isobus::{AuxFunctionState, AuxFunctionType};
use crate::net::constants::NULL_ADDRESS;
use crate::net::types::Address;
#[cfg(any(feature = "default", feature = "cli"))]
use crate::vt_storage::{MAX_STORED_POOL_BYTES, VT_STORAGE_HEADER_LEN, VT_STORAGE_MAGIC};
use crate::vt_storage::{StoredPoolVersion, is_valid_classic_label};
use alloc::{collections::BTreeMap as HashMap, string::String, vec::Vec};
#[cfg(any(feature = "default", feature = "cli"))]
use alloc::{format, vec};
#[cfg(any(feature = "default", feature = "cli"))]
use std::fs;
#[cfg(any(feature = "default", feature = "cli"))]
use std::io::{Read, Write};
#[cfg(any(feature = "default", feature = "cli"))]
use std::path::PathBuf;

/// The VT Get Versions response carries the number of stored versions in one byte.
pub const MAX_STORED_VERSIONS: usize = u8::MAX as usize;

/// Server-side state derived from accepted ECU→VT change commands.
///
/// This is a semantic object-state cache, not a renderer. It lets a VT server
/// remember the latest changes applied by one ECU working set so higher layers
/// can render, audit, or persist an activated object pool without replaying the
/// command stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerObjectState {
    pub active_data_mask: ObjectID,
    pub active_soft_key_mask: ObjectID,
    pub selected_input_object: ObjectID,
    /// Object opened for data input by an accepted Select Input Object
    /// command. `NULL` means focus-only/no open edit transaction.
    pub open_input_object: ObjectID,
    pub input_escape_count: u32,
    pub visibility: HashMap<ObjectID, bool>,
    pub enable_state: HashMap<ObjectID, bool>,
    pub numeric_values: HashMap<ObjectID, u32>,
    pub string_values: HashMap<ObjectID, String>,
    /// `data_mask_id -> soft_key_mask_id`.
    pub soft_key_masks: HashMap<ObjectID, ObjectID>,
    /// `(parent_id, child_id) -> (relative_x, relative_y)`.
    pub child_locations: HashMap<(ObjectID, ObjectID), (u8, u8)>,
    /// `(parent_id, child_id) -> (x, y)`.
    pub child_positions: HashMap<(ObjectID, ObjectID), (u16, u16)>,
    /// `object_id -> (width, height)`.
    pub sizes: HashMap<ObjectID, (u16, u16)>,
    pub background_colours: HashMap<ObjectID, u8>,
    pub endpoints: HashMap<ObjectID, (u16, u16, u8)>,
    pub font_attributes: HashMap<ObjectID, ObjectID>,
    pub line_attributes: HashMap<ObjectID, ObjectID>,
    pub fill_attributes: HashMap<ObjectID, ObjectID>,
    /// `(object_id, attribute_id) -> raw 32-bit attribute value`.
    pub attributes: HashMap<(ObjectID, u8), u32>,
    pub priorities: HashMap<ObjectID, u8>,
    /// `(list_id, index) -> new_item_id`.
    pub list_items: HashMap<(ObjectID, u8), ObjectID>,
    pub object_labels: HashMap<ObjectID, ObjectLabelState>,
    /// `(polygon_id, point_index) -> (x, y)`.
    pub polygon_points: HashMap<(ObjectID, u8), (u16, u16)>,
    pub polygon_scales: HashMap<ObjectID, (u16, u16)>,
    pub graphics_contexts: Vec<GraphicsContextCommand>,
    pub selected_colour_map: ObjectID,
    /// `None` means no runtime palette command has selected one yet;
    /// `Some(NULL)` explicitly restores the terminal default palette.
    pub selected_colour_palette: Option<ObjectID>,
    pub mask_locks: HashMap<ObjectID, MaskLockState>,
    pub executed_macros: Vec<ObjectID>,
    pub executed_extended_macros: Vec<ObjectID>,
    /// `aux_input_object_id -> aux_function_object_id`.
    pub aux_assignments: HashMap<ObjectID, ObjectID>,
    /// Latest accepted status for an assigned AUX input object.
    pub aux_input_states: HashMap<ObjectID, AuxInputRuntimeState>,
    pub audio_signal: Option<AudioSignalState>,
    pub audio_volume_percent: Option<u8>,
    /// Accepted ECU→VT command effects in arrival order. This gives hosted
    /// render runtimes a replayable command stream without making the no_std
    /// server depend on the std-gated renderer module.
    pub accepted_effects: Vec<ServerRenderEffect>,
}

impl Default for ServerObjectState {
    fn default() -> Self {
        Self {
            active_data_mask: ObjectID::default(),
            active_soft_key_mask: ObjectID::default(),
            selected_input_object: ObjectID::NULL,
            open_input_object: ObjectID::NULL,
            input_escape_count: 0,
            visibility: HashMap::new(),
            enable_state: HashMap::new(),
            numeric_values: HashMap::new(),
            string_values: HashMap::new(),
            soft_key_masks: HashMap::new(),
            child_locations: HashMap::new(),
            child_positions: HashMap::new(),
            sizes: HashMap::new(),
            background_colours: HashMap::new(),
            endpoints: HashMap::new(),
            font_attributes: HashMap::new(),
            line_attributes: HashMap::new(),
            fill_attributes: HashMap::new(),
            attributes: HashMap::new(),
            priorities: HashMap::new(),
            list_items: HashMap::new(),
            object_labels: HashMap::new(),
            polygon_points: HashMap::new(),
            polygon_scales: HashMap::new(),
            graphics_contexts: Vec::new(),
            selected_colour_map: ObjectID::NULL,
            selected_colour_palette: None,
            mask_locks: HashMap::new(),
            executed_macros: Vec::new(),
            executed_extended_macros: Vec::new(),
            aux_assignments: HashMap::new(),
            aux_input_states: HashMap::new(),
            audio_signal: None,
            audio_volume_percent: None,
            accepted_effects: Vec::new(),
        }
    }
}

/// Renderer-facing semantic effects accepted by `VTServer`.
///
/// This enum deliberately lives next to the server state instead of inside the
/// std-gated renderer. Embedded builds can keep recording/auditing command
/// effects while hosted code maps them into `render::VtRuntimeCommand`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServerRenderEffect {
    HideShow {
        id: ObjectID,
        visible: bool,
    },
    EnableDisable {
        id: ObjectID,
        enabled: bool,
    },
    SelectInputObject {
        id: ObjectID,
        open_for_input: bool,
    },
    Esc,
    ChangeChildLocation {
        parent: ObjectID,
        child: ObjectID,
        x: u8,
        y: u8,
    },
    ChangeChildPosition {
        parent: ObjectID,
        child: ObjectID,
        x: u16,
        y: u16,
    },
    ChangeSize {
        id: ObjectID,
        width: u16,
        height: u16,
    },
    ChangeEndPoint {
        id: ObjectID,
        width: u16,
        height: u16,
        line_direction: u8,
    },
    ChangeBackgroundColour {
        id: ObjectID,
        colour: u8,
    },
    ChangeNumericValue {
        id: ObjectID,
        value: u32,
    },
    ChangeStringValue {
        id: ObjectID,
        text: String,
    },
    ChangeFontAttributes {
        id: ObjectID,
        attributes: ObjectID,
    },
    ChangeFontAttributeValues {
        id: ObjectID,
        colour: u8,
        size: u8,
        font_type: u8,
        style: u8,
    },
    ChangeLineAttributes {
        id: ObjectID,
        attributes: ObjectID,
    },
    ChangeLineAttributeValues {
        id: ObjectID,
        colour: u8,
        width: u8,
        line_art: u16,
    },
    ChangeFillAttributes {
        id: ObjectID,
        attributes: ObjectID,
    },
    ChangeFillAttributeValues {
        id: ObjectID,
        fill_type: u8,
        colour: u8,
        pattern: ObjectID,
    },
    ChangeActiveMask {
        mask: ObjectID,
    },
    ChangeSoftKeyMask {
        data_mask: ObjectID,
        soft_key_mask: ObjectID,
    },
    ChangeGenericAttribute {
        id: ObjectID,
        attribute_id: u8,
        value: u32,
    },
    ChangePriority {
        id: ObjectID,
        priority: u8,
    },
    ChangeListItem {
        list: ObjectID,
        index: u8,
        item: ObjectID,
    },
    LockUnlockMask {
        id: ObjectID,
        locked: bool,
        timeout_ms: u16,
    },
    ExecuteMacro {
        id: ObjectID,
        extended: bool,
    },
    ChangeObjectLabel {
        id: ObjectID,
        label: ObjectLabelState,
    },
    ChangePolygonPoint {
        id: ObjectID,
        index: u8,
        x: u16,
        y: u16,
    },
    ChangePolygonScale {
        id: ObjectID,
        width: u16,
        height: u16,
    },
    SelectColourMap {
        id: ObjectID,
    },
    GraphicsContext {
        id: ObjectID,
        subcommand: u8,
        payload: Vec<u8>,
    },
    AudioSignal,
    SetAudioVolume {
        percent: u8,
    },
}

/// Runtime object-label assignment from the ISO 11783-6 Change Object Label
/// command. The label text is referenced through a String Variable object; the
/// optional graphic designator is a normal VT object reference clipped by a
/// display backend to a soft-key designator area when shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ObjectLabelState {
    pub string_variable: ObjectID,
    pub font_type: u8,
    pub graphic_designator: ObjectID,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioSignalState {
    pub activations: u8,
    pub frequency_hz: u16,
    pub duration_ms: u16,
    pub off_time_ms: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaskLockState {
    pub locked: bool,
    pub timeout_ms: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphicsContextCommand {
    pub object_id: ObjectID,
    pub subcommand: u8,
    pub payload: Vec<u8>,
}

/// `true` when this VT server/render replay path knows the Graphics Context
/// subcommand ID and can validate its payload shape.
#[must_use]
pub const fn graphics_context_subcommand_is_supported(subcommand: u8) -> bool {
    matches!(subcommand, 0x00..=0x14)
}

/// Return the number of parameter bytes used by a known Graphics Context
/// subcommand, excluding any single-frame `FF16` padding.
#[must_use]
pub fn graphics_context_payload_parameter_len(subcommand: u8, payload: &[u8]) -> Option<usize> {
    match subcommand {
        // Set cursor, move cursor, set viewport position.
        0x00 | 0x01 | 0x08 | 0x09 | 0x0E => Some(4),
        // Set foreground/background colour.
        0x02 | 0x03 => Some(1),
        // Select line/fill/font attrs, Draw VT Object, Copy Canvas/Viewport.
        0x04 | 0x05 | 0x06 | 0x12 | 0x13 | 0x14 => Some(2),
        // Size-bearing primitive / viewport-size subcommands.
        0x07 | 0x0A | 0x0B | 0x11 => Some(4),
        // Polygon point list: count byte followed by count signed x/y pairs.
        0x0C => payload
            .first()
            .and_then(|count| usize::from(*count).checked_mul(4))
            .and_then(|bytes| bytes.checked_add(1)),
        // DrawText: canonical bool, counted byte length, exact counted bytes.
        0x0D => payload
            .get(1)
            .and_then(|count| 2usize.checked_add(usize::from(*count))),
        // Zoom uses a raw IEEE-754 `f32`; PanAndZoom is x/y plus zoom.
        0x0F => Some(4),
        0x10 => Some(8),
        _ => None,
    }
}

/// Return a canonical Graphics Context payload slice after accepting optional
/// `FF16` padding from a padded single-frame command.
#[must_use]
pub fn graphics_context_payload_without_padding(subcommand: u8, payload: &[u8]) -> Option<&[u8]> {
    let used = graphics_context_payload_parameter_len(subcommand, payload)?;
    if used > payload.len() || payload[used..].iter().any(|&byte| byte != 0xFF) {
        return None;
    }
    Some(&payload[..used])
}

/// `true` when a Graphics Context subcommand payload has the canonical shape
/// for the subcommands currently modelled by the server/render replay path.
///
/// Unknown subcommands are rejected so the server can report the F.57 invalid
/// sub-command bit instead of preserving opaque commands as if they were
/// renderable replay records. Known subcommands are checked before they mutate
/// retained state.
#[must_use]
pub fn graphics_context_payload_is_canonical(subcommand: u8, payload: &[u8]) -> bool {
    if graphics_context_payload_parameter_len(subcommand, payload) != Some(payload.len()) {
        return false;
    }
    match subcommand {
        // Set cursor, move cursor, set viewport position.
        0x00 | 0x01 | 0x08 | 0x09 | 0x0E => payload.len() == 4,
        // Set foreground/background colour.
        0x02 | 0x03 => payload.len() == 1,
        // Select line/fill/font attrs, Draw VT Object, Copy Canvas/Viewport.
        0x04 | 0x05 | 0x06 | 0x12 | 0x13 | 0x14 => payload.len() == 2,
        // Size-bearing primitive / viewport-size subcommands.
        0x07 | 0x0A | 0x0B | 0x11 => payload.len() == 4,
        // Polygon point list: count byte followed by count signed x/y pairs.
        0x0C => {
            payload
                .first()
                .and_then(|count| usize::from(*count).checked_mul(4))
                .and_then(|bytes| bytes.checked_add(1))
                == Some(payload.len())
        }
        // DrawText: canonical bool, counted byte length, exact counted bytes.
        0x0D => {
            payload.len() >= 2
                && matches!(payload[0], 0 | 1)
                && 2usize
                    .checked_add(usize::from(payload[1]))
                    .is_some_and(|expected| expected == payload.len())
        }
        // Zoom uses a raw IEEE-754 `f32`; PanAndZoom is x/y plus zoom.
        0x0F => graphics_context_zoom_is_canonical(payload),
        0x10 => payload.len() == 8 && graphics_context_zoom_is_canonical(&payload[4..8]),
        _ => false,
    }
}

fn graphics_context_zoom_is_canonical(payload: &[u8]) -> bool {
    if payload.len() != 4 {
        return false;
    }
    let zoom = f32::from_bits(u32::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ]));
    zoom.is_finite() && zoom > 0.0 && zoom <= 32.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuxRuntimeStyle {
    AuxO,
    AuxN,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuxInputRuntimeState {
    pub style: AuxRuntimeStyle,
    pub input_object: ObjectID,
    pub function_object: ObjectID,
    pub function_number: u8,
    pub r#type: AuxFunctionType,
    pub state: AuxFunctionState,
    pub setpoint: u16,
    pub source: Address,
}

/// Server-side per-client tracking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerWorkingSet {
    pub client_address: Address,
    pub pool: ObjectPool,
    pub working_set: WorkingSet,
    pub pool_uploaded: bool,
    pub pool_upload_allowed: bool,
    pub pool_activation_pending: bool,
    pub pool_activated: bool,
    pub last_status_ms: u32,
    pub stored_versions: Vec<StoredPoolVersion>,
    #[cfg(any(feature = "default", feature = "cli"))]
    pub storage_path: PathBuf,
    pub object_state: ServerObjectState,
}

impl Default for ServerWorkingSet {
    fn default() -> Self {
        Self {
            client_address: NULL_ADDRESS,
            pool: ObjectPool::default(),
            working_set: WorkingSet::default(),
            pool_uploaded: false,
            pool_upload_allowed: false,
            pool_activation_pending: false,
            pool_activated: false,
            last_status_ms: 0,
            stored_versions: Vec::new(),
            #[cfg(any(feature = "default", feature = "cli"))]
            storage_path: PathBuf::from("./vt_storage"),
            object_state: ServerObjectState::default(),
        }
    }
}

impl ServerWorkingSet {
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn set_storage_path(&mut self, path: impl Into<PathBuf>) {
        self.storage_path = path.into();
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    #[must_use]
    pub fn get_client_storage_dir(&self) -> PathBuf {
        self.storage_path
            .join(format!("{:02X}", self.client_address))
    }

    /// Ensure the per-client storage directory exists. Returns `true`
    /// if the directory exists (or was created) at the end of the call.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn ensure_storage_dir(&self) -> bool {
        let dir = self.get_client_storage_dir();
        fs::create_dir_all(&dir).is_ok()
    }

    /// Find a stored version by label.
    pub fn find_version(&mut self, label: &str) -> Option<&mut StoredPoolVersion> {
        self.stored_versions.iter_mut().find(|v| v.label == label)
    }

    /// Store the current pool with a label, persisting to disk.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn store_version(&mut self, label: impl Into<String>, vt_ver: u16) -> bool {
        let Some(ver) = self.build_stored_version(label, vt_ver) else {
            return false;
        };
        if !self.save_version_to_disk(&ver) {
            return false;
        }
        self.insert_stored_version(ver)
    }

    fn build_stored_version(
        &self,
        label: impl Into<String>,
        vt_ver: u16,
    ) -> Option<StoredPoolVersion> {
        if !self.pool_uploaded || self.pool.is_empty() {
            return None;
        }
        let label = label.into();
        if !is_valid_classic_label(&label) {
            return None;
        }
        let data = self.pool.serialize().ok()?;
        let mut ver = StoredPoolVersion {
            label,
            pool_data: data,
            ..Default::default()
        };
        #[cfg(any(feature = "default", feature = "cli"))]
        ver.update_metadata(vt_ver);
        #[cfg(feature = "embedded")]
        ver.update_metadata_at(vt_ver, 0);
        Some(ver)
    }

    fn insert_stored_version(&mut self, ver: StoredPoolVersion) -> bool {
        if let Some(pos) = self
            .stored_versions
            .iter()
            .position(|v| v.label == ver.label)
        {
            self.stored_versions[pos] = ver;
            return true;
        }
        if self.stored_versions.len() >= MAX_STORED_VERSIONS {
            return false;
        }
        self.stored_versions.push(ver);
        true
    }

    /// Store the current pool with a label in memory only.
    ///
    /// Use [`StoredPoolVersion::to_storage_bytes`] on the cached version when an
    /// embedded application wants to persist the blob through its own storage
    /// stack instead of the hosted filesystem helpers.
    pub fn store_version_in_memory(&mut self, label: impl Into<String>, vt_ver: u16) -> bool {
        self.build_stored_version(label, vt_ver)
            .is_some_and(|ver| self.insert_stored_version(ver))
    }

    #[cfg(feature = "embedded")]
    pub fn store_version(&mut self, label: impl Into<String>, vt_ver: u16) -> bool {
        self.store_version_in_memory(label, vt_ver)
    }

    /// Load a stored version into the active pool. Tries the
    /// in-memory cache first, falls back to disk.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn load_version(&mut self, label: &str) -> bool {
        if !is_valid_classic_label(label) {
            return false;
        }
        if self.load_cached_version(label) {
            return true;
        }
        let Some(disk_ver) = self.load_version_from_disk(label) else {
            return false;
        };
        if self.stored_versions.len() >= MAX_STORED_VERSIONS {
            return false;
        }
        let pool_data = disk_ver.pool_data.clone();
        let Ok(restored) = ObjectPool::deserialize(&pool_data) else {
            return false;
        };
        self.stored_versions.push(disk_ver);
        self.pool = restored;
        self.pool_uploaded = true;
        self.pool_upload_allowed = false;
        self.pool_activation_pending = false;
        self.pool_activated = true;
        true
    }

    /// Load a cached in-memory version into the active pool without touching
    /// the filesystem.
    pub fn load_cached_version(&mut self, label: &str) -> bool {
        if !is_valid_classic_label(label) {
            return false;
        }
        let Some(ver) = self.stored_versions.iter().find(|v| v.label == label) else {
            return false;
        };
        let Ok(restored) = ObjectPool::deserialize(&ver.pool_data) else {
            return false;
        };
        self.pool = restored;
        self.pool_uploaded = true;
        self.pool_upload_allowed = false;
        self.pool_activation_pending = false;
        self.pool_activated = true;
        true
    }

    #[cfg(feature = "embedded")]
    pub fn load_version(&mut self, label: &str) -> bool {
        self.load_cached_version(label)
    }

    /// Load a stored version blob supplied by the caller into the active pool.
    ///
    /// This is the storage-agnostic counterpart to [`Self::load_version`].
    /// Embedded applications own flash/SD/EEPROM lookup and pass the bytes here.
    pub fn load_version_from_storage_bytes(&mut self, expected_label: &str, bytes: &[u8]) -> bool {
        if !is_valid_classic_label(expected_label) {
            return false;
        }
        let Some(ver) = StoredPoolVersion::from_storage_bytes(bytes) else {
            return false;
        };
        if ver.label != expected_label {
            return false;
        }
        let Ok(restored) = ObjectPool::deserialize(&ver.pool_data) else {
            return false;
        };
        if let Some(existing) = self
            .stored_versions
            .iter_mut()
            .find(|existing| existing.label == ver.label)
        {
            *existing = ver;
        } else {
            if self.stored_versions.len() >= MAX_STORED_VERSIONS {
                return false;
            }
            self.stored_versions.push(ver);
        }
        self.pool = restored;
        self.pool_uploaded = true;
        self.pool_upload_allowed = false;
        self.pool_activation_pending = false;
        self.pool_activated = true;
        true
    }

    /// Export a cached version as a storage-agnostic `VTP1` blob.
    #[must_use]
    pub fn export_version_storage_bytes(&self, label: &str) -> Option<Vec<u8>> {
        self.stored_versions
            .iter()
            .find(|ver| ver.label == label)
            .and_then(StoredPoolVersion::to_storage_bytes)
    }

    /// Remove a version from the in-memory cache and from disk.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn delete_version(&mut self, label: &str) -> bool {
        if !self.delete_cached_version(label) {
            return false;
        }
        let _ = self.delete_version_from_disk(label);
        true
    }

    /// Remove a version from the in-memory cache only.
    pub fn delete_cached_version(&mut self, label: &str) -> bool {
        if !is_valid_classic_label(label) {
            return false;
        }
        let len_before = self.stored_versions.len();
        self.stored_versions.retain(|v| v.label != label);
        self.stored_versions.len() != len_before
    }

    #[cfg(feature = "embedded")]
    pub fn delete_version(&mut self, label: &str) -> bool {
        self.delete_cached_version(label)
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn save_version_to_disk(&self, ver: &StoredPoolVersion) -> bool {
        let Some(filename) = version_filename(&ver.label) else {
            return false;
        };
        let Some(buf) = ver.to_storage_bytes() else {
            return false;
        };
        if !self.ensure_storage_dir() {
            return false;
        }
        let filepath = self.get_client_storage_dir().join(filename);
        let Ok(mut file) = fs::File::create(&filepath) else {
            return false;
        };
        file.write_all(&buf).is_ok()
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn load_version_from_disk(&self, label: &str) -> Option<StoredPoolVersion> {
        let filename = version_filename(label)?;
        let filepath = self.get_client_storage_dir().join(filename);
        let ver = self.load_version_file(filepath)?;
        (ver.label == label).then_some(ver)
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    fn load_version_file(&self, filepath: PathBuf) -> Option<StoredPoolVersion> {
        let mut file = fs::File::open(&filepath).ok()?;
        let mut header = [0u8; VT_STORAGE_HEADER_LEN];
        file.read_exact(&mut header).ok()?;
        if &header[0..4] != VT_STORAGE_MAGIC {
            return None;
        }
        let size_bytes = u32::from_le_bytes(header[12..16].try_into().ok()?);
        if size_bytes as usize > MAX_STORED_POOL_BYTES {
            return None;
        }
        let mut pool_data = vec![0u8; size_bytes as usize];
        file.read_exact(&mut pool_data).ok()?;
        let mut trailing = [0u8; 1];
        if file.read(&mut trailing).ok()? != 0 {
            return None;
        }
        let mut bytes = Vec::with_capacity(VT_STORAGE_HEADER_LEN + pool_data.len());
        bytes.extend_from_slice(&header);
        bytes.extend_from_slice(&pool_data);
        StoredPoolVersion::from_storage_bytes(&bytes)
    }

    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn delete_version_from_disk(&self, label: &str) -> bool {
        let Some(filename) = version_filename(label) else {
            return false;
        };
        let filepath = self.get_client_storage_dir().join(filename);
        fs::remove_file(filepath).is_ok()
    }

    /// Walk the per-client directory and load every `.vtp` file we
    /// haven't seen yet. Returns the number of newly-loaded versions.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn load_all_versions_from_disk(&mut self) -> u32 {
        let dir = self.get_client_storage_dir();
        let Ok(entries) = fs::read_dir(&dir) else {
            return 0;
        };
        let mut loaded = 0u32;
        for entry in entries.flatten() {
            if self.stored_versions.len() >= MAX_STORED_VERSIONS {
                break;
            }
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("vtp") {
                continue;
            }
            if let Some(ver) = self.load_version_file(path)
                && !self.stored_versions.iter().any(|v| v.label == ver.label)
            {
                self.stored_versions.push(ver);
                loaded += 1;
            }
        }
        loaded
    }

    /// Drop expired versions from cache and disk. Returns count.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn cleanup_expired_versions(&mut self, max_age_days: u32) -> u32 {
        let mut deleted = 0u32;
        let mut idx = 0;
        while idx < self.stored_versions.len() {
            if self.stored_versions[idx].is_expired(max_age_days) {
                let label = core::mem::take(&mut self.stored_versions[idx].label);
                self.stored_versions.remove(idx);
                let _ = self.delete_version_from_disk(&label);
                deleted += 1;
            } else {
                idx += 1;
            }
        }
        deleted
    }

    /// Persist every cached version. Returns count actually written.
    #[cfg(any(feature = "default", feature = "cli"))]
    pub fn save_all_versions_to_disk(&self) -> u32 {
        self.stored_versions
            .iter()
            .filter(|v| self.save_version_to_disk(v))
            .count() as u32
    }
}

#[cfg(any(feature = "default", feature = "cli"))]
fn version_filename(label: &str) -> Option<String> {
    if !is_valid_classic_label(label) {
        return None;
    }
    let mut out = String::with_capacity(label.len() * 2 + 4);
    for b in label.as_bytes() {
        use core::fmt::Write as _;
        let _ = write!(&mut out, "{b:02X}");
    }
    out.push_str(".vtp");
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::super::objects::{
        DataMaskBody, WorkingSetBody, create_data_mask, create_working_set,
    };
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("machbus_vt_sws_{nanos}"));
        let _ = fs::create_dir_all(&p);
        p
    }

    fn dummy_pool() -> ObjectPool {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
    }

    #[test]
    fn storage_blob_round_trip_without_disk() {
        let mut sws = ServerWorkingSet {
            client_address: 0x42,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        assert!(sws.store_version_in_memory("LABEL01", 5));
        let bytes = sws.export_version_storage_bytes("LABEL01").unwrap();
        let decoded = StoredPoolVersion::from_storage_bytes(&bytes).unwrap();
        assert_eq!(decoded.label, "LABEL01");
        assert_eq!(decoded.vt_version, 5);

        let mut restored = ServerWorkingSet::default();
        assert!(restored.load_version_from_storage_bytes("LABEL01", &bytes));
        assert_eq!(restored.pool.size(), 2);
        assert_eq!(restored.stored_versions.len(), 1);
        assert!(!restored.load_version_from_storage_bytes("WRONG", &bytes));
    }

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
    fn store_then_load_round_trip_via_disk() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet {
            client_address: 0x42,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        sws.set_storage_path(&dir);
        assert!(sws.store_version("LABEL01", 5));

        // Drop the in-memory cache; force disk read.
        sws.stored_versions.clear();
        assert!(sws.load_version("LABEL01"));
        assert_eq!(sws.pool.size(), 2);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_version_removes_from_memory_and_disk() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet {
            client_address: 0x10,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        sws.set_storage_path(&dir);
        sws.store_version("V1", 5);
        assert_eq!(sws.stored_versions.len(), 1);
        let path = sws
            .get_client_storage_dir()
            .join(version_filename("V1").unwrap());
        assert!(path.exists());
        assert!(sws.delete_version("V1"));
        assert!(sws.stored_versions.is_empty());
        assert!(!path.exists());
        assert!(!sws.delete_version("V1"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_all_from_disk_skips_existing() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet {
            client_address: 0x20,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        sws.set_storage_path(&dir);
        sws.store_version("V1", 5);
        sws.store_version("V2", 5);
        // Already cached; load_all should add zero.
        assert_eq!(sws.load_all_versions_from_disk(), 0);
        // Drop cache and reload.
        sws.stored_versions.clear();
        assert_eq!(sws.load_all_versions_from_disk(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_returns_false_when_pool_not_uploaded() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet::default();
        sws.set_storage_path(&dir);
        sws.client_address = 0x42;
        // pool_uploaded defaults false.
        assert!(!sws.store_version("X", 5));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn store_version_rejects_new_entries_above_one_byte_advertised_count() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet {
            client_address: 0x42,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        sws.set_storage_path(&dir);
        for i in 0..MAX_STORED_VERSIONS {
            sws.stored_versions.push(StoredPoolVersion {
                label: format!("V{i:06}"),
                ..Default::default()
            });
        }
        assert_eq!(sws.stored_versions.len(), MAX_STORED_VERSIONS);
        assert!(!sws.store_version("NEW", 5));
        assert_eq!(sws.stored_versions.len(), MAX_STORED_VERSIONS);

        sws.pool = dummy_pool();
        assert!(
            sws.store_version("V000000", 5),
            "replacing an existing label must still be allowed at capacity"
        );
        assert_eq!(sws.stored_versions.len(), MAX_STORED_VERSIONS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_version_rejects_uncached_disk_entries_at_capacity() {
        let dir = temp_dir();
        let mut writer = ServerWorkingSet {
            client_address: 0x42,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        writer.set_storage_path(&dir);
        assert!(writer.store_version("ONDISK", 5));

        let mut full = ServerWorkingSet {
            client_address: 0x42,
            ..Default::default()
        };
        full.set_storage_path(&dir);
        for i in 0..MAX_STORED_VERSIONS {
            full.stored_versions.push(StoredPoolVersion {
                label: format!("F{i:06}"),
                ..Default::default()
            });
        }
        assert!(!full.load_version("ONDISK"));
        assert_eq!(full.stored_versions.len(), MAX_STORED_VERSIONS);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_filenames_are_hex_encoded() {
        assert_eq!(version_filename("V1").as_deref(), Some("5631.vtp"));
        assert_eq!(
            version_filename("LABEL01").as_deref(),
            Some("4C4142454C3031.vtp")
        );
    }

    #[test]
    fn invalid_version_labels_are_rejected_before_disk_access() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet {
            client_address: 0x42,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        sws.set_storage_path(&dir);

        for label in [
            "", "TOOLONG8", ".", "..", "../BAD", "A/B", "A\\B", "A:B", "A*B", "A?B", "A B", "A\0B",
        ] {
            assert!(
                !sws.store_version(label, 5),
                "stored invalid label {label:?}"
            );
            assert!(!sws.load_version(label), "loaded invalid label {label:?}");
            assert!(
                !sws.delete_version(label),
                "deleted invalid label {label:?}"
            );
        }

        assert!(fs::read_dir(sws.get_client_storage_dir()).is_err());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_mismatched_header_label() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet {
            client_address: 0x10,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        sws.set_storage_path(&dir);
        assert!(sws.store_version("V1", 5));
        let v1_path = sws
            .get_client_storage_dir()
            .join(version_filename("V1").unwrap());
        let v2_path = sws
            .get_client_storage_dir()
            .join(version_filename("V2").unwrap());
        fs::copy(v1_path, v2_path).unwrap();
        assert!(sws.load_version_from_disk("V1").is_some());
        assert!(
            sws.load_version_from_disk("V2").is_none(),
            "filename and on-disk header label must agree"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_oversized_header_before_pool_allocation() {
        let dir = temp_dir();
        let sws = ServerWorkingSet {
            client_address: 0x10,
            storage_path: dir.clone(),
            ..Default::default()
        };
        assert!(sws.ensure_storage_dir());
        let path = sws
            .get_client_storage_dir()
            .join(version_filename("V1").unwrap());
        let mut header = Vec::new();
        header.extend_from_slice(VT_STORAGE_MAGIC);
        header.extend_from_slice(&0u64.to_le_bytes());
        header.extend_from_slice(&(u32::MAX).to_le_bytes());
        header.extend_from_slice(&5u16.to_le_bytes());
        header.push(1);
        let mut label = [0u8; 8];
        label[0] = b'V';
        label[1] = b'1';
        header.extend_from_slice(&label);
        fs::write(path, header).unwrap();
        assert!(sws.load_version_from_disk("V1").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn save_rejects_mismatched_or_oversized_pool_size_metadata() {
        let dir = temp_dir();
        let sws = ServerWorkingSet {
            client_address: 0x10,
            storage_path: dir.clone(),
            ..Default::default()
        };
        let good = StoredPoolVersion {
            label: "V1".into(),
            pool_data: vec![1, 2, 3],
            size_bytes: 3,
            ..Default::default()
        };
        assert!(sws.save_version_to_disk(&good));

        let mismatched = StoredPoolVersion {
            size_bytes: 2,
            ..good.clone()
        };
        assert!(
            !sws.save_version_to_disk(&mismatched),
            "header size must match payload length"
        );

        let oversized = StoredPoolVersion {
            label: "BIG".into(),
            pool_data: vec![0; MAX_STORED_POOL_BYTES + 1],
            size_bytes: (MAX_STORED_POOL_BYTES as u32) + 1,
            ..Default::default()
        };
        assert!(
            !sws.save_version_to_disk(&oversized),
            "oversized pools must not be persisted"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_rejects_trailing_bytes_after_declared_pool_data() {
        let dir = temp_dir();
        let mut sws = ServerWorkingSet {
            client_address: 0x10,
            pool: dummy_pool(),
            pool_uploaded: true,
            ..Default::default()
        };
        sws.set_storage_path(&dir);
        assert!(sws.store_version("V1", 5));
        let path = sws
            .get_client_storage_dir()
            .join(version_filename("V1").unwrap());
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(0x00);
        fs::write(path, bytes).unwrap();
        assert!(sws.load_version_from_disk("V1").is_none());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn is_expired_zero_timestamp_is_never_expired() {
        let v = StoredPoolVersion {
            timestamp_us: 0,
            ..Default::default()
        };
        assert!(!v.is_expired(0));
    }
}
