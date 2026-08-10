//! ISO 11783-10 Task Controller — DDOP object types.
//!
//! Mirrors the C++ `machbus::isobus::tc::objects.hpp`. Five object
//! kinds (Device, DeviceElement, DeviceProcessData, DeviceProperty,
//! DeviceValuePresentation) and their wire-format serializers.

use alloc::{format, string::String, vec::Vec};

use crate::net::error::{Error, ErrorCode, Result};

/// Data Dictionary Identifier (ISO 11783-11).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct DDI(pub u16);

impl DDI {
    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        Self(v)
    }
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
    #[inline]
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
    #[inline]
    #[must_use]
    pub const fn try_new_i32(v: i32) -> Option<Self> {
        if v >= 0 && v <= u16::MAX as i32 {
            Some(Self(v as u16))
        } else {
            None
        }
    }
}

impl From<u16> for DDI {
    #[inline]
    fn from(v: u16) -> Self {
        Self(v)
    }
}
impl From<DDI> for u16 {
    #[inline]
    fn from(v: DDI) -> Self {
        v.0
    }
}
impl PartialEq<u16> for DDI {
    #[inline]
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}
impl PartialEq<i32> for DDI {
    #[inline]
    fn eq(&self, other: &i32) -> bool {
        self.0 as i32 == *other
    }
}
impl PartialEq<DDI> for u16 {
    #[inline]
    fn eq(&self, other: &DDI) -> bool {
        *self == other.0
    }
}
impl core::fmt::Debug for DDI {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "DDI({})", self.0)
    }
}
impl core::fmt::Display for DDI {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// TC element number (ISO 11783-10).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct ElementNumber(pub u16);

impl ElementNumber {
    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        Self(v)
    }
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
    #[inline]
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
    #[inline]
    #[must_use]
    pub const fn try_new_i32(v: i32) -> Option<Self> {
        if v >= 0 && v <= u16::MAX as i32 {
            Some(Self(v as u16))
        } else {
            None
        }
    }
}

impl From<u16> for ElementNumber {
    #[inline]
    fn from(v: u16) -> Self {
        Self(v)
    }
}
impl From<ElementNumber> for u16 {
    #[inline]
    fn from(v: ElementNumber) -> Self {
        v.0
    }
}
impl PartialEq<u16> for ElementNumber {
    #[inline]
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}
impl PartialEq<i32> for ElementNumber {
    #[inline]
    fn eq(&self, other: &i32) -> bool {
        self.0 as i32 == *other
    }
}
impl PartialEq<ElementNumber> for u16 {
    #[inline]
    fn eq(&self, other: &ElementNumber) -> bool {
        *self == other.0
    }
}
impl core::fmt::Debug for ElementNumber {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ElementNumber({})", self.0)
    }
}
impl core::fmt::Display for ElementNumber {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// TC pool object identifier — distinct from
/// [`crate::isobus::vt::ObjectID`].
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct ObjectID(pub u16);

impl ObjectID {
    pub const NULL: Self = Self(0xFFFF);

    #[inline]
    #[must_use]
    pub const fn new(v: u16) -> Self {
        Self(v)
    }
    #[inline]
    #[must_use]
    pub const fn raw(self) -> u16 {
        self.0
    }
    #[inline]
    #[must_use]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.0.to_le_bytes()
    }
    #[inline]
    #[must_use]
    pub const fn try_new_i32(v: i32) -> Option<Self> {
        if v >= 0 && v <= u16::MAX as i32 {
            Some(Self(v as u16))
        } else {
            None
        }
    }
}

impl From<u16> for ObjectID {
    #[inline]
    fn from(v: u16) -> Self {
        Self(v)
    }
}
impl From<ObjectID> for u16 {
    #[inline]
    fn from(v: ObjectID) -> Self {
        v.0
    }
}
impl PartialEq<u16> for ObjectID {
    #[inline]
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}
impl PartialEq<ObjectID> for u16 {
    #[inline]
    fn eq(&self, other: &ObjectID) -> bool {
        *self == other.0
    }
}
impl PartialEq<i32> for ObjectID {
    #[inline]
    fn eq(&self, other: &i32) -> bool {
        self.0 as i32 == *other
    }
}
impl PartialOrd<ObjectID> for u16 {
    #[inline]
    fn partial_cmp(&self, other: &ObjectID) -> Option<core::cmp::Ordering> {
        self.partial_cmp(&other.0)
    }
}
impl PartialOrd<u16> for ObjectID {
    #[inline]
    fn partial_cmp(&self, other: &u16) -> Option<core::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

/// Increment by `u16` for next-id allocators.
impl core::ops::AddAssign<u16> for ObjectID {
    #[inline]
    fn add_assign(&mut self, rhs: u16) {
        self.0 = self.0.wrapping_add(rhs);
    }
}

impl core::ops::Add<u16> for ObjectID {
    type Output = ObjectID;
    #[inline]
    fn add(self, rhs: u16) -> Self::Output {
        Self(self.0.wrapping_add(rhs))
    }
}

impl core::fmt::Debug for ObjectID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "tc::ObjectID({})", self.0)
    }
}
impl core::fmt::Display for ObjectID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// TC object kind.
///
/// On the wire this is the **3-byte ASCII "Table ID"** that opens every DDOP
/// object record (Tables A.1–A.5: `Type = String`, `Size = 3`, record bytes
/// 1–3). The serializer emitted a single numeric byte instead, leaving every
/// object two bytes short and the whole pool unparsable by a conformant TC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TCObjectType {
    #[default]
    Device = 0,
    DeviceElement = 1,
    DeviceProcessData = 2,
    DeviceProperty = 3,
    DeviceValuePresentation = 4,
}

impl TCObjectType {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// The 3-byte ASCII Table ID this object opens with (Tables A.1–A.5).
    #[inline]
    #[must_use]
    pub const fn table_id(self) -> [u8; 3] {
        match self {
            Self::Device => *b"DVC",
            Self::DeviceElement => *b"DET",
            Self::DeviceProcessData => *b"DPD",
            Self::DeviceProperty => *b"DPT",
            Self::DeviceValuePresentation => *b"DVP",
        }
    }

    /// Parse a 3-byte ASCII Table ID.
    #[must_use]
    pub const fn from_table_id(id: &[u8]) -> Option<Self> {
        if id.len() < 3 {
            return None;
        }
        match [id[0], id[1], id[2]] {
            [b'D', b'V', b'C'] => Some(Self::Device),
            [b'D', b'E', b'T'] => Some(Self::DeviceElement),
            [b'D', b'P', b'D'] => Some(Self::DeviceProcessData),
            [b'D', b'P', b'T'] => Some(Self::DeviceProperty),
            [b'D', b'V', b'P'] => Some(Self::DeviceValuePresentation),
            _ => None,
        }
    }
}

/// `DeviceElementType` (per `<DET A="...">` in ISO XML form).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum DeviceElementType {
    #[default]
    Device = 1,
    Function = 2,
    Bin = 3,
    Section = 4,
    Unit = 5,
    Connector = 6,
    NavigationReference = 7,
}

impl DeviceElementType {
    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Process-data trigger methods. Bitmask, OR multiple together.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum TriggerMethod {
    #[default]
    TimeInterval = 0x01,
    DistanceInterval = 0x02,
    ThresholdLimits = 0x04,
    OnChange = 0x08,
    Total = 0x10,
}

impl TriggerMethod {
    pub const ALL_BITS: u8 = Self::TimeInterval as u8
        | Self::DistanceInterval as u8
        | Self::ThresholdLimits as u8
        | Self::OnChange as u8
        | Self::Total as u8;

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

// ─── DeviceObject ─────────────────────────────────────────────────────

/// Top-level Device object.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceObject {
    pub id: ObjectID,
    pub designator: String,
    pub software_version: String,
    /// The device's 64-bit ISO 11783-5 NAME (Table A.1, "ClientNAME",
    /// `Double integer`, 8 bytes, record byte 7+N).
    ///
    /// This was omitted from the record entirely, so every field after the
    /// software version sat 8 bytes early on the wire.
    pub client_name: u64,
    pub serial_number: String,
    pub structure_label: [u8; 7],
    pub localization_label: [u8; 7],
    /// Table A.1, introduced in ISO 11783-10 version 4: 0..=32 bytes.
    ///
    /// Only transmitted when **both** the TC and the client report version 4
    /// or higher; a version-3 connection must not carry it at all, and a
    /// version-4 client that does not use it sends a length of 0. Use
    /// [`DeviceObject::serialize_for_version`] rather than assuming.
    pub extended_structure_label: Vec<u8>,
}

/// First DDOP version that defines the Extended Structure Label (Table A.1).
pub const DDOP_VERSION_EXTENDED_STRUCTURE_LABEL: u8 = 4;

impl DeviceObject {
    #[must_use]
    pub fn with_id(mut self, v: impl Into<ObjectID>) -> Self {
        let v = v.into();
        self.id = v;
        self
    }

    #[must_use]
    pub fn with_designator(mut self, v: impl Into<String>) -> Self {
        self.designator = v.into();
        self
    }

    #[must_use]
    pub fn with_software_version(mut self, v: impl Into<String>) -> Self {
        self.software_version = v.into();
        self
    }

    #[must_use]
    pub fn with_serial_number(mut self, v: impl Into<String>) -> Self {
        self.serial_number = v.into();
        self
    }

    /// Set the version-4 Extended Structure Label (0..=32 bytes).
    #[must_use]
    pub fn with_extended_structure_label(mut self, v: impl Into<Vec<u8>>) -> Self {
        self.extended_structure_label = v.into();
        self
    }

    /// Set the device's ISO 11783-5 NAME (Table A.1 "ClientNAME").
    #[must_use]
    pub const fn with_client_name(mut self, v: u64) -> Self {
        self.client_name = v;
        self
    }

    #[must_use]
    pub const fn with_structure_label(mut self, v: [u8; 7]) -> Self {
        self.structure_label = v;
        self
    }

    #[must_use]
    pub const fn with_localization_label(mut self, v: [u8; 7]) -> Self {
        self.localization_label = v;
        self
    }

    /// Serialize in the version-3 form (no Extended Structure Label).
    ///
    /// # Errors
    /// Propagates text-field encoding failures.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        self.serialize_for_version(DDOP_VERSION_EXTENDED_STRUCTURE_LABEL - 1)
    }

    /// Serialize for a negotiated DDOP `version`.
    ///
    /// The Extended Structure Label is emitted only from version 4, per
    /// Table A.1 and the accompanying rule that a version-3 connection "shall
    /// fall back to the lowest common version and the 2 attributes for the
    /// extended structure label shall both not be used".
    ///
    /// # Errors
    /// A label longer than 32 bytes, or a text field that cannot be encoded.
    pub fn serialize_for_version(&self, version: u8) -> Result<Vec<u8>> {
        if self.extended_structure_label.len() > 32 {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "device extended structure label exceeds 32 bytes",
            ));
        }
        let mut data = Vec::with_capacity(
            3 + 1
                + self.designator.len()
                + 1
                + self.software_version.len()
                + 8
                + 1
                + self.serial_number.len()
                + 14,
        );
        data.extend_from_slice(&TCObjectType::Device.table_id());
        push_u16_le(&mut data, self.id);
        push_str_with_len(&mut data, "device designator", &self.designator)?;
        push_str_with_len(&mut data, "device software version", &self.software_version)?;
        // ClientNAME sits between the software version and the serial number
        // (Table A.1). Omitting it shifted every later field 8 bytes early.
        data.extend_from_slice(&self.client_name.to_le_bytes());
        push_str_with_len(&mut data, "device serial number", &self.serial_number)?;
        data.extend_from_slice(&self.structure_label);
        data.extend_from_slice(&self.localization_label);
        if version >= DDOP_VERSION_EXTENDED_STRUCTURE_LABEL {
            data.push(self.extended_structure_label.len() as u8);
            data.extend_from_slice(&self.extended_structure_label);
        }
        Ok(data)
    }
}

// ─── DeviceElement ────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DeviceElement {
    pub id: ObjectID,
    pub r#type: DeviceElementType,
    pub number: ElementNumber,
    pub parent_id: ObjectID,
    pub designator: String,
    pub child_objects: Vec<ObjectID>,
}

impl DeviceElement {
    #[must_use]
    pub fn with_id(mut self, v: impl Into<ObjectID>) -> Self {
        let v = v.into();
        self.id = v;
        self
    }

    #[must_use]
    pub fn with_type(mut self, v: DeviceElementType) -> Self {
        self.r#type = v;
        self
    }

    #[must_use]
    pub fn with_number(mut self, v: impl Into<ElementNumber>) -> Self {
        let v = v.into();
        self.number = v;
        self
    }

    #[must_use]
    pub fn with_parent(mut self, v: impl Into<ObjectID>) -> Self {
        self.parent_id = v.into();
        self
    }

    #[must_use]
    pub fn with_designator(mut self, v: impl Into<String>) -> Self {
        self.designator = v.into();
        self
    }

    #[must_use]
    pub fn with_children<I, T>(mut self, v: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<ObjectID>,
    {
        self.child_objects = v.into_iter().map(Into::into).collect();
        self
    }

    pub fn add_child(&mut self, v: impl Into<ObjectID>) -> &mut Self {
        self.child_objects.push(v.into());
        self
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        if self.child_objects.len() > u16::MAX as usize {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "device element has too many child object references",
            ));
        }
        let mut data = Vec::new();
        data.extend_from_slice(&TCObjectType::DeviceElement.table_id());
        push_u16_le(&mut data, self.id);
        data.push(self.r#type.as_u8());
        push_str_with_len(&mut data, "device element designator", &self.designator)?;
        push_u16_le(&mut data, self.number);
        push_u16_le(&mut data, self.parent_id);
        push_u16_le(&mut data, self.child_objects.len() as u16);
        for &cid in &self.child_objects {
            push_u16_le(&mut data, cid);
        }
        Ok(data)
    }
}

// ─── DeviceProcessData ────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProcessData {
    pub id: ObjectID,
    pub ddi: DDI,
    /// Table A.3 record byte 8, "Process data properties": a bitset of
    /// bit 1 = member of default set, bit 2 = settable, bit 3 = control source.
    /// Bits 2 and 3 are mutually exclusive.
    ///
    /// The field was absent from the record entirely, so a TC could not tell a
    /// settable process datum from a read-only one.
    pub properties: u8,
    pub trigger_methods: u8,
    /// `0xFFFF` = no presentation reference.
    pub presentation_object_id: ObjectID,
    pub designator: String,
}

impl Default for DeviceProcessData {
    fn default() -> Self {
        Self {
            id: ObjectID(0),
            ddi: DDI(0),
            properties: 0,
            trigger_methods: 0,
            presentation_object_id: ObjectID::NULL,
            designator: String::new(),
        }
    }
}

impl DeviceProcessData {
    pub fn validate_trigger_methods(&self) -> Result<()> {
        if self.trigger_methods & !TriggerMethod::ALL_BITS != 0 {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "process data trigger methods contain reserved bits",
            ));
        }
        Ok(())
    }

    /// Validate the Table A.3 properties bitset.
    ///
    /// # Errors
    /// Reserved bits set, or both "settable" and "control source" — the table
    /// notes these two are mutually exclusive.
    pub fn validate_properties(&self) -> Result<()> {
        const DEFAULT_SET: u8 = 0b001;
        const SETTABLE: u8 = 0b010;
        const CONTROL_SOURCE: u8 = 0b100;
        if self.properties & !(DEFAULT_SET | SETTABLE | CONTROL_SOURCE) != 0 {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "process data properties contain reserved bits",
            ));
        }
        if self.properties & SETTABLE != 0 && self.properties & CONTROL_SOURCE != 0 {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "process data properties: settable and control source are mutually exclusive",
            ));
        }
        Ok(())
    }

    /// Set the Table A.3 properties bitset.
    #[must_use]
    pub const fn with_properties(mut self, v: u8) -> Self {
        self.properties = v;
        self
    }

    #[must_use]
    pub fn with_id(mut self, v: impl Into<ObjectID>) -> Self {
        let v = v.into();
        self.id = v;
        self
    }

    #[must_use]
    pub fn with_ddi(mut self, v: impl Into<DDI>) -> Self {
        let v = v.into();
        self.ddi = v;
        self
    }

    #[must_use]
    pub fn with_triggers(mut self, v: u8) -> Self {
        self.trigger_methods = v;
        self
    }

    #[must_use]
    pub fn with_trigger(mut self, t: TriggerMethod) -> Self {
        self.trigger_methods |= t.as_u8();
        self
    }

    #[must_use]
    pub fn with_presentation(mut self, v: impl Into<ObjectID>) -> Self {
        let v = v.into();
        self.presentation_object_id = v;
        self
    }

    #[must_use]
    pub fn with_designator(mut self, v: impl Into<String>) -> Self {
        self.designator = v.into();
        self
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        self.validate_trigger_methods()?;
        let mut data = Vec::new();
        self.validate_properties()?;
        data.extend_from_slice(&TCObjectType::DeviceProcessData.table_id());
        push_u16_le(&mut data, self.id);
        push_u16_le(&mut data, self.ddi);
        // Table A.3 order: properties, triggers, designator, then the DVP
        // reference. The DVP ID used to precede the designator, so everything
        // after it was misaligned for a conformant reader.
        data.push(self.properties);
        data.push(self.trigger_methods);
        push_str_with_len(&mut data, "process data designator", &self.designator)?;
        push_u16_le(&mut data, self.presentation_object_id);
        Ok(data)
    }
}

// ─── DeviceProperty ───────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceProperty {
    pub id: ObjectID,
    pub ddi: DDI,
    /// Fixed property value (part of definition, unlike
    /// [`DeviceProcessData`]).
    pub value: i32,
    pub presentation_object_id: ObjectID,
    pub designator: String,
}

impl Default for DeviceProperty {
    fn default() -> Self {
        Self {
            id: ObjectID(0),
            ddi: DDI(0),
            value: 0,
            presentation_object_id: ObjectID::NULL,
            designator: String::new(),
        }
    }
}

impl DeviceProperty {
    #[must_use]
    pub fn with_id(mut self, v: impl Into<ObjectID>) -> Self {
        let v = v.into();
        self.id = v;
        self
    }

    #[must_use]
    pub fn with_ddi(mut self, v: impl Into<DDI>) -> Self {
        let v = v.into();
        self.ddi = v;
        self
    }

    #[must_use]
    pub fn with_value(mut self, v: i32) -> Self {
        self.value = v;
        self
    }

    #[must_use]
    pub fn with_presentation(mut self, v: impl Into<ObjectID>) -> Self {
        let v = v.into();
        self.presentation_object_id = v;
        self
    }

    #[must_use]
    pub fn with_designator(mut self, v: impl Into<String>) -> Self {
        self.designator = v.into();
        self
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        data.extend_from_slice(&TCObjectType::DeviceProperty.table_id());
        push_u16_le(&mut data, self.id);
        push_u16_le(&mut data, self.ddi);
        data.extend_from_slice(&self.value.to_le_bytes());
        // Table A.4: designator at record byte 13, then the DVP reference at
        // 13+N. The DVP ID used to precede the designator.
        push_str_with_len(&mut data, "property designator", &self.designator)?;
        push_u16_le(&mut data, self.presentation_object_id);
        Ok(data)
    }
}

// ─── DeviceValuePresentation ──────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceValuePresentation {
    pub id: ObjectID,
    pub offset: i32,
    pub scale: f32,
    pub decimal_digits: u8,
    pub unit_designator: String,
}

impl Default for DeviceValuePresentation {
    fn default() -> Self {
        Self {
            id: ObjectID(0),
            offset: 0,
            scale: 1.0,
            decimal_digits: 0,
            unit_designator: String::new(),
        }
    }
}

impl DeviceValuePresentation {
    #[must_use]
    pub fn with_id(mut self, v: impl Into<ObjectID>) -> Self {
        let v = v.into();
        self.id = v;
        self
    }

    #[must_use]
    pub fn with_offset(mut self, v: i32) -> Self {
        self.offset = v;
        self
    }

    #[must_use]
    pub fn with_scale(mut self, v: f32) -> Self {
        self.scale = v;
        self
    }

    #[must_use]
    pub fn with_decimals(mut self, v: u8) -> Self {
        self.decimal_digits = v;
        self
    }

    #[must_use]
    pub fn with_unit(mut self, v: impl Into<String>) -> Self {
        self.unit_designator = v.into();
        self
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        if !self.scale.is_finite() {
            return Err(Error::with_message(
                ErrorCode::PoolValidation,
                "value presentation scale must be finite",
            ));
        }
        let mut data = Vec::new();
        data.extend_from_slice(&TCObjectType::DeviceValuePresentation.table_id());
        push_u16_le(&mut data, self.id);
        data.extend_from_slice(&self.offset.to_le_bytes());
        data.extend_from_slice(&self.scale.to_le_bytes());
        data.push(self.decimal_digits);
        push_str_with_len(
            &mut data,
            "value presentation unit designator",
            &self.unit_designator,
        )?;
        Ok(data)
    }
}

// ─── helpers ──────────────────────────────────────────────────────────

#[inline]
fn push_u16_le<T: Into<u16>>(out: &mut Vec<u8>, v: T) {
    let v = v.into();
    out.push((v & 0xFF) as u8);
    out.push(((v >> 8) & 0xFF) as u8);
}

#[inline]
fn push_str_with_len(out: &mut Vec<u8>, field: &'static str, s: &str) -> Result<()> {
    if !s.is_ascii() {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            format!("{field} must be ASCII"),
        ));
    }
    if s.len() > u8::MAX as usize {
        return Err(Error::with_message(
            ErrorCode::PoolValidation,
            format!("{field} exceeds one-byte length field"),
        ));
    }
    out.push(s.len() as u8);
    out.extend_from_slice(s.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tc_id_wrappers_checked_constructors_reject_unencodable_values() {
        assert_eq!(DDI::try_new_i32(0), Some(DDI(0)));
        assert_eq!(DDI::try_new_i32(u16::MAX as i32), Some(DDI(u16::MAX)));
        assert_eq!(DDI::try_new_i32(-1), None);
        assert_eq!(DDI::try_new_i32(u16::MAX as i32 + 1), None);

        assert_eq!(ElementNumber::try_new_i32(0), Some(ElementNumber(0)));
        assert_eq!(
            ElementNumber::try_new_i32(u16::MAX as i32),
            Some(ElementNumber(u16::MAX))
        );
        assert_eq!(ElementNumber::try_new_i32(-1), None);
        assert_eq!(ElementNumber::try_new_i32(u16::MAX as i32 + 1), None);

        assert_eq!(ObjectID::try_new_i32(0), Some(ObjectID(0)));
        assert_eq!(ObjectID::try_new_i32(u16::MAX as i32), Some(ObjectID::NULL));
        assert_eq!(ObjectID::try_new_i32(-1), None);
        assert_eq!(ObjectID::try_new_i32(u16::MAX as i32 + 1), None);
    }

    #[test]
    fn device_serialize_layout() {
        let d = DeviceObject::default()
            .with_id(0x1234)
            .with_designator("Sprayer")
            .with_software_version("1.0")
            .with_serial_number("ABC")
            .with_structure_label([0x01; 7])
            .with_localization_label([0x02; 7]);
        let bytes = d.serialize().unwrap();
        // Tables A.1-A.5: bytes 1-3 are the ASCII Table ID, 4-5 the object ID.
        assert_eq!(&bytes[0..3], b"DVC");
        assert_eq!(bytes[3], 0x34);
        assert_eq!(bytes[4], 0x12);
        assert_eq!(bytes[5], 7);
        assert_eq!(&bytes[6..13], b"Sprayer");
    }

    #[test]
    fn device_element_serialize_with_children() {
        let de = DeviceElement::default()
            .with_id(1)
            .with_type(DeviceElementType::Section)
            .with_number(5)
            .with_parent(10)
            .with_designator("S1")
            .with_children(vec![100, 200]);
        let bytes = de.serialize().unwrap();
        assert_eq!(&bytes[0..3], b"DET");
        assert_eq!(bytes[5], DeviceElementType::Section.as_u8());
        // Designator length 2 + "S1" + number(2) + parent(2) + children_count(2) + 2*2.
        assert_eq!(bytes[6], 2);
        assert_eq!(&bytes[7..9], b"S1");
        let num_offset = 9;
        assert_eq!(bytes[num_offset], 5);
        let children_count_offset = num_offset + 4;
        assert_eq!(bytes[children_count_offset], 2);
        let first_child_offset = children_count_offset + 2;
        assert_eq!(bytes[first_child_offset], 100);
    }

    #[test]
    fn device_process_data_round_trip_triggers() {
        let pd = DeviceProcessData::default()
            .with_id(7)
            .with_ddi(0x1234)
            .with_trigger(TriggerMethod::TimeInterval)
            .with_trigger(TriggerMethod::OnChange);
        assert_eq!(
            pd.trigger_methods,
            TriggerMethod::TimeInterval.as_u8() | TriggerMethod::OnChange.as_u8()
        );
        let bytes = pd.serialize().unwrap();
        assert_eq!(&bytes[0..3], b"DPD");
    }

    #[test]
    fn device_property_serializes_value() {
        let p = DeviceProperty::default()
            .with_id(9)
            .with_ddi(0xABCD)
            .with_value(-42);
        let bytes = p.serialize().unwrap();
        assert_eq!(&bytes[0..3], b"DPT");
        // value at offset 7..11 (3 + 2 + 2 + 4)
        let v = i32::from_le_bytes(bytes[7..11].try_into().unwrap());
        assert_eq!(v, -42);
    }

    #[test]
    fn device_value_presentation_serializes_scale() {
        let vp = DeviceValuePresentation::default()
            .with_id(1)
            .with_offset(100)
            .with_scale(0.001)
            .with_decimals(3)
            .with_unit("m");
        let bytes = vp.serialize().unwrap();
        assert_eq!(&bytes[0..3], b"DVP");
        // scale at offset 9..13 (3 + 2 + 4)
        let scale = f32::from_le_bytes(bytes[9..13].try_into().unwrap());
        assert!((scale - 0.001).abs() < 1e-6);
        assert_eq!(bytes[13], 3); // decimals
    }

    #[test]
    fn direct_serializers_reject_unencodable_strings_counts_and_scales() {
        let overlong = "A".repeat(u8::MAX as usize + 1);
        assert!(
            DeviceObject::default()
                .with_id(1)
                .with_designator(overlong.clone())
                .serialize()
                .is_err()
        );
        assert!(
            DeviceProcessData::default()
                .with_id(2)
                .with_designator("µ")
                .serialize()
                .is_err()
        );
        assert!(
            DeviceProperty::default()
                .with_id(3)
                .with_designator(overlong.clone())
                .serialize()
                .is_err()
        );
        assert!(
            DeviceValuePresentation::default()
                .with_id(4)
                .with_scale(f32::NAN)
                .serialize()
                .is_err()
        );
        assert!(
            DeviceValuePresentation::default()
                .with_id(5)
                .with_unit(overlong)
                .serialize()
                .is_err()
        );

        let too_many_children = DeviceElement::default()
            .with_id(6)
            .with_children((0..=u16::MAX).map(ObjectID).collect::<Vec<_>>());
        assert!(too_many_children.serialize().is_err());
    }

    /// 6E — ISO 11783-10 Tables A.1-A.5 define the first field of every DDOP
    /// object record as `Table ID`: `Type = String`, `Size = 3`, record bytes
    /// 1-3, with values "DVC", "DET", "DPD", "DPT", "DVP". The serializer
    /// emitted a single numeric byte, so every object was two bytes short and
    /// the whole pool was unparsable by a conformant Task Controller.
    #[test]
    fn every_object_record_opens_with_its_ascii_table_id() {
        assert_eq!(&TCObjectType::Device.table_id(), b"DVC");
        assert_eq!(&TCObjectType::DeviceElement.table_id(), b"DET");
        assert_eq!(&TCObjectType::DeviceProcessData.table_id(), b"DPD");
        assert_eq!(&TCObjectType::DeviceProperty.table_id(), b"DPT");
        assert_eq!(&TCObjectType::DeviceValuePresentation.table_id(), b"DVP");

        for (id, expected) in [
            (&b"DVC"[..], TCObjectType::Device),
            (b"DET", TCObjectType::DeviceElement),
            (b"DPD", TCObjectType::DeviceProcessData),
            (b"DPT", TCObjectType::DeviceProperty),
            (b"DVP", TCObjectType::DeviceValuePresentation),
        ] {
            assert_eq!(TCObjectType::from_table_id(id), Some(expected));
        }

        // A numeric tag is not a Table ID, and neither is a short slice.
        assert_eq!(TCObjectType::from_table_id(&[0, 0, 0]), None);
        assert_eq!(TCObjectType::from_table_id(b"DV"), None);
        assert_eq!(TCObjectType::from_table_id(b"XXX"), None);

        // And it really is what reaches the wire.
        let bytes = DeviceObject::default()
            .with_id(1)
            .with_designator("X")
            .serialize()
            .unwrap();
        assert_eq!(&bytes[0..3], b"DVC");
    }

    /// 6E — Table A.1 places `ClientNAME` (Double integer, 8 bytes) between the
    /// software version and the serial number. It was omitted from the record
    /// entirely, so every field after the software version sat 8 bytes early
    /// and a conformant TC read the serial-number length out of the NAME.
    #[test]
    fn device_record_carries_the_client_name() {
        let name = 0x0123_4567_89AB_CDEFu64;
        let bytes = DeviceObject::default()
            .with_id(1)
            .with_designator("D")
            .with_software_version("v")
            .with_client_name(name)
            .with_serial_number("S")
            .serialize()
            .unwrap();

        // DVC(3) + id(2) + [1]"D" + [1]"v" = 9 bytes before the NAME.
        assert_eq!(&bytes[0..3], b"DVC");
        assert_eq!(
            u64::from_le_bytes(bytes[9..17].try_into().unwrap()),
            name,
            "ClientNAME follows the software version (Table A.1 record byte 7+N)"
        );
        // And the serial-number length follows the NAME, not the version.
        assert_eq!(bytes[17], 1);
        assert_eq!(bytes[18], b'S');
    }

    /// 6E — Table A.3 orders the DPD record as Table ID, object ID, DDI,
    /// **properties**, triggers, designator, then the DVP reference. The
    /// properties byte was absent entirely (so a TC could not tell a settable
    /// process datum from a read-only one) and the DVP ID preceded the
    /// designator, misaligning everything after it.
    #[test]
    fn process_data_record_follows_table_a3_order() {
        let pd = DeviceProcessData::default()
            .with_id(10)
            .with_ddi(0x1234)
            .with_properties(0b011) // default set + settable
            .with_trigger(TriggerMethod::OnChange)
            .with_designator("PD1")
            .with_presentation(0xFFFF);
        let bytes = pd.serialize().unwrap();

        assert_eq!(&bytes[0..3], b"DPD");
        assert_eq!(&bytes[3..5], &[10, 0], "object ID");
        assert_eq!(&bytes[5..7], &[0x34, 0x12], "DDI");
        assert_eq!(bytes[7], 0b011, "properties at record byte 8");
        assert_eq!(bytes[8], TriggerMethod::OnChange.as_u8(), "triggers follow");
        assert_eq!(bytes[9], 3, "then the designator length");
        assert_eq!(&bytes[10..13], b"PD1");
        assert_eq!(&bytes[13..15], &[0xFF, 0xFF], "DVP reference last");
    }

    /// Table A.3 notes that "settable" and "control source" are mutually
    /// exclusive, and only three bits are defined.
    #[test]
    fn process_data_properties_reject_impossible_combinations() {
        let base = DeviceProcessData::default().with_id(1).with_ddi(1);

        assert!(base.clone().with_properties(0b001).serialize().is_ok());
        assert!(base.clone().with_properties(0b010).serialize().is_ok());
        assert!(base.clone().with_properties(0b100).serialize().is_ok());

        // settable + control source
        assert!(base.clone().with_properties(0b110).serialize().is_err());
        // reserved bits
        assert!(base.with_properties(0b1000).serialize().is_err());
    }

    /// 6E — Table A.4 puts the property designator at record byte 13 and the
    /// DVP reference at 13+N. The DVP ID used to precede the designator, so
    /// every field after it was misaligned for a conformant reader.
    #[test]
    fn property_record_puts_the_dvp_reference_after_the_designator() {
        let bytes = DeviceProperty::default()
            .with_id(20)
            .with_ddi(0xABCD)
            .with_value(-42)
            .with_designator("Prop1")
            .with_presentation(0x1234)
            .serialize()
            .unwrap();

        assert_eq!(&bytes[0..3], b"DPT");
        assert_eq!(&bytes[3..5], &[20, 0]);
        assert_eq!(&bytes[5..7], &[0xCD, 0xAB], "DDI");
        assert_eq!(
            i32::from_le_bytes(bytes[7..11].try_into().unwrap()),
            -42,
            "value at record bytes 8-11"
        );
        assert_eq!(bytes[11], 5, "designator length at record byte 12");
        assert_eq!(&bytes[12..17], b"Prop1");
        assert_eq!(&bytes[17..19], &[0x34, 0x12], "DVP reference last");
    }

    /// 6E — the Extended Structure Label was introduced in ISO 11783-10
    /// version 4 and "shall only be used when the versions of the TC and of the
    /// client are both reported as version 4 or higher". It was missing
    /// entirely; emitting it unconditionally would be just as wrong.
    #[test]
    fn extended_structure_label_is_version_gated() {
        let device = DeviceObject::default()
            .with_id(1)
            .with_designator("D")
            .with_extended_structure_label(*b"CFG-A");

        // Version 3 must not carry it at all.
        let v3 = device.serialize_for_version(3).unwrap();
        assert!(!v3.ends_with(b"CFG-A"));

        // Version 4 carries a length-prefixed array.
        let v4 = device
            .serialize_for_version(DDOP_VERSION_EXTENDED_STRUCTURE_LABEL)
            .unwrap();
        assert_eq!(v4.len(), v3.len() + 1 + 5);
        assert_eq!(v4[v3.len()], 5, "length prefix");
        assert!(v4.ends_with(b"CFG-A"));

        // A v4 client not using it reports a length of zero, not an omission.
        let unused = DeviceObject::default().with_id(1).with_designator("D");
        let v4_empty = unused
            .serialize_for_version(DDOP_VERSION_EXTENDED_STRUCTURE_LABEL)
            .unwrap();
        assert_eq!(*v4_empty.last().unwrap(), 0);

        // 0..=32 bytes is the defined range.
        let overlong = DeviceObject::default()
            .with_id(1)
            .with_extended_structure_label(vec![b'X'; 33]);
        assert!(overlong.serialize_for_version(4).is_err());
    }
}
