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

/// TC object kind (first byte of every serialized object).
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
    pub serial_number: String,
    pub structure_label: [u8; 7],
    pub localization_label: [u8; 7],
}

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

    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(
            3 + 1
                + self.designator.len()
                + 1
                + self.software_version.len()
                + 1
                + self.serial_number.len()
                + 14,
        );
        data.push(TCObjectType::Device.as_u8());
        push_u16_le(&mut data, self.id);
        push_str_with_len(&mut data, "device designator", &self.designator)?;
        push_str_with_len(&mut data, "device software version", &self.software_version)?;
        push_str_with_len(&mut data, "device serial number", &self.serial_number)?;
        data.extend_from_slice(&self.structure_label);
        data.extend_from_slice(&self.localization_label);
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
        data.push(TCObjectType::DeviceElement.as_u8());
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
        data.push(TCObjectType::DeviceProcessData.as_u8());
        push_u16_le(&mut data, self.id);
        push_u16_le(&mut data, self.ddi);
        data.push(self.trigger_methods);
        push_u16_le(&mut data, self.presentation_object_id);
        push_str_with_len(&mut data, "process data designator", &self.designator)?;
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
        data.push(TCObjectType::DeviceProperty.as_u8());
        push_u16_le(&mut data, self.id);
        push_u16_le(&mut data, self.ddi);
        data.extend_from_slice(&self.value.to_le_bytes());
        push_u16_le(&mut data, self.presentation_object_id);
        push_str_with_len(&mut data, "property designator", &self.designator)?;
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
        data.push(TCObjectType::DeviceValuePresentation.as_u8());
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
        assert_eq!(bytes[0], TCObjectType::Device.as_u8());
        assert_eq!(bytes[1], 0x34);
        assert_eq!(bytes[2], 0x12);
        assert_eq!(bytes[3], 7);
        assert_eq!(&bytes[4..11], b"Sprayer");
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
        assert_eq!(bytes[0], TCObjectType::DeviceElement.as_u8());
        assert_eq!(bytes[3], DeviceElementType::Section.as_u8());
        // Designator length 2 + "S1" + number(2) + parent(2) + children_count(2) + 2*2.
        assert_eq!(bytes[4], 2);
        assert_eq!(&bytes[5..7], b"S1");
        let num_offset = 7;
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
        assert_eq!(bytes[0], TCObjectType::DeviceProcessData.as_u8());
    }

    #[test]
    fn device_property_serializes_value() {
        let p = DeviceProperty::default()
            .with_id(9)
            .with_ddi(0xABCD)
            .with_value(-42);
        let bytes = p.serialize().unwrap();
        assert_eq!(bytes[0], TCObjectType::DeviceProperty.as_u8());
        // value at offset 5..9 (1+2+2+4)
        let v = i32::from_le_bytes(bytes[5..9].try_into().unwrap());
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
        assert_eq!(bytes[0], TCObjectType::DeviceValuePresentation.as_u8());
        // scale at offset 7..11 (1+2+4)
        let scale = f32::from_le_bytes(bytes[7..11].try_into().unwrap());
        assert!((scale - 0.001).abs() < 1e-6);
        assert_eq!(bytes[11], 3); // decimals
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
}
