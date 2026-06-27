use alloc::{format, string::String, vec, vec::Vec};

use crate::net::error::{Error, ErrorCode, Result};

/// VT object headers store the serialized body length in a two-byte field.
pub const VT_OBJECT_BODY_MAX_LEN: usize = u16::MAX as usize;

/// VT object identifier — **distinct** from [`crate::isobus::tc::ObjectID`]
/// (Task Controller object IDs). Infallible conversions are intentionally
/// limited to `u16`; wider or signed caller values must use the checked
/// constructors so invalid IDs cannot silently wrap on the wire.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
#[repr(transparent)]
pub struct ObjectID(pub u16);

impl ObjectID {
    pub const NULL: Self = Self(0xFFFF);

    #[inline]
    #[must_use]
    pub const fn new(id: u16) -> Self {
        Self(id)
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
    pub const fn from_le_bytes(bytes: [u8; 2]) -> Self {
        Self(u16::from_le_bytes(bytes))
    }

    #[inline]
    #[must_use]
    pub const fn try_new_i32(id: i32) -> Option<Self> {
        if id >= 0 && id <= u16::MAX as i32 {
            Some(Self(id as u16))
        } else {
            None
        }
    }

    #[inline]
    #[must_use]
    pub const fn try_new_usize(id: usize) -> Option<Self> {
        if id <= u16::MAX as usize {
            Some(Self(id as u16))
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

/// Lets test assertions compare against integer literals directly:
/// `assert_eq!(obj.id, 7)`.
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

impl PartialEq<ObjectID> for i32 {
    #[inline]
    fn eq(&self, other: &ObjectID) -> bool {
        *self == other.0 as i32
    }
}

impl core::fmt::Debug for ObjectID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ObjectID({})", self.0)
    }
}

impl core::fmt::Display for ObjectID {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ─── Object types ──────────────────────────────────────────────────────

/// A child reference as encoded in a parent object's record: the child
/// object id plus its signed X/Y location relative to the parent's
/// top-left corner (ISO 11783-6 §4.6.12). The standard encodes each
/// child as 6 bytes: `[object_id:2][x:i16][y:i16]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChildRef {
    pub id: ObjectID,
    pub x: i16,
    pub y: i16,
}

impl ChildRef {
    #[inline]
    #[must_use]
    pub const fn new(id: ObjectID, x: i16, y: i16) -> Self {
        Self { id, x, y }
    }

    /// Construct from a bare object id at the origin (0, 0). Useful when
    /// a caller only has the legacy `Vec<ObjectID>` form.
    #[inline]
    #[must_use]
    pub const fn at_origin(id: ObjectID) -> Self {
        Self { id, x: 0, y: 0 }
    }
}

impl From<ObjectID> for ChildRef {
    #[inline]
    fn from(id: ObjectID) -> Self {
        Self::at_origin(id)
    }
}

/// A macro reference as encoded after a parent object's child list:
/// `[event_id:1][macro_id:1]`. A VT-version-5 16-bit macro object id
/// occupies two consecutive entries per the standard; this struct
/// stores the 8-bit form which is the common case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MacroRef {
    pub event_id: u8,
    pub macro_id: u8,
}

impl MacroRef {
    #[inline]
    #[must_use]
    pub const fn new(event_id: u8, macro_id: u8) -> Self {
        Self { event_id, macro_id }
    }
}

/// ISO 11783-6 §4.6 object type codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ObjectType {
    #[default]
    WorkingSet = 0,
    DataMask = 1,
    AlarmMask = 2,
    Container = 3,
    SoftKeyMask = 4,
    Key = 5,
    Button = 6,
    InputBoolean = 7,
    InputString = 8,
    InputNumber = 9,
    InputList = 10,
    OutputString = 11,
    OutputNumber = 12,
    Line = 13,
    Rectangle = 14,
    Ellipse = 15,
    Polygon = 16,
    Meter = 17,
    LinearBarGraph = 18,
    ArchedBarGraph = 19,
    PictureGraphic = 20,
    NumberVariable = 21,
    StringVariable = 22,
    FontAttributes = 23,
    LineAttributes = 24,
    FillAttributes = 25,
    InputAttributes = 26,
    ObjectPointer = 27,
    Macro = 28,
    AuxFunction = 29,
    AuxInput = 30,
    AuxFunction2 = 31,
    AuxInput2 = 32,
    AuxControlDesig = 33,
    WindowMask = 34,
    KeyGroup = 35,
    GraphicContext = 36,
    OutputList = 37,
    ExtendedInputAttributes = 38,
    ColourMap = 39,
    ObjectLabelRef = 40,
    // VT 6 (ISO 11783-6 Ed. 4, 2018)
    ExternalObjectDefinition = 41,
    ExternalReferenceName = 42,
    ExternalObjectPointer = 43,
    Animation = 44,
    ColourPalette = 45,
    GraphicData = 46,
    WorkingSetSpecialControls = 47,
    ScaledGraphic = 48,
    // machbus extension / draft object retained for compatibility with
    // pre-correction object pools. ISO 11783-6:2018 reserves this code.
    ScaledBitmap = 49,
    // machbus extension / draft object retained for compatibility with
    // pre-correction object pools. The standard graphics context is type 36.
    GraphicsContext = 50,
}

impl ObjectType {
    /// Decode a raw byte. Unknown values fall through to
    /// [`ObjectType::WorkingSet`] (matches the C++ `static_cast`).
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::DataMask,
            2 => Self::AlarmMask,
            3 => Self::Container,
            4 => Self::SoftKeyMask,
            5 => Self::Key,
            6 => Self::Button,
            7 => Self::InputBoolean,
            8 => Self::InputString,
            9 => Self::InputNumber,
            10 => Self::InputList,
            11 => Self::OutputString,
            12 => Self::OutputNumber,
            13 => Self::Line,
            14 => Self::Rectangle,
            15 => Self::Ellipse,
            16 => Self::Polygon,
            17 => Self::Meter,
            18 => Self::LinearBarGraph,
            19 => Self::ArchedBarGraph,
            20 => Self::PictureGraphic,
            21 => Self::NumberVariable,
            22 => Self::StringVariable,
            23 => Self::FontAttributes,
            24 => Self::LineAttributes,
            25 => Self::FillAttributes,
            26 => Self::InputAttributes,
            27 => Self::ObjectPointer,
            28 => Self::Macro,
            29 => Self::AuxFunction,
            30 => Self::AuxInput,
            31 => Self::AuxFunction2,
            32 => Self::AuxInput2,
            33 => Self::AuxControlDesig,
            34 => Self::WindowMask,
            35 => Self::KeyGroup,
            36 => Self::GraphicContext,
            37 => Self::OutputList,
            38 => Self::ExtendedInputAttributes,
            39 => Self::ColourMap,
            40 => Self::ObjectLabelRef,
            41 => Self::ExternalObjectDefinition,
            42 => Self::ExternalReferenceName,
            43 => Self::ExternalObjectPointer,
            44 => Self::Animation,
            45 => Self::ColourPalette,
            46 => Self::GraphicData,
            47 => Self::WorkingSetSpecialControls,
            48 => Self::ScaledGraphic,
            49 => Self::ScaledBitmap,
            50 => Self::GraphicsContext,
            _ => Self::WorkingSet,
        }
    }

    /// Decode a wire byte, rejecting values that are not assigned to an
    /// implemented ISO 11783-6 VT object type.
    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::WorkingSet),
            1 => Some(Self::DataMask),
            2 => Some(Self::AlarmMask),
            3 => Some(Self::Container),
            4 => Some(Self::SoftKeyMask),
            5 => Some(Self::Key),
            6 => Some(Self::Button),
            7 => Some(Self::InputBoolean),
            8 => Some(Self::InputString),
            9 => Some(Self::InputNumber),
            10 => Some(Self::InputList),
            11 => Some(Self::OutputString),
            12 => Some(Self::OutputNumber),
            13 => Some(Self::Line),
            14 => Some(Self::Rectangle),
            15 => Some(Self::Ellipse),
            16 => Some(Self::Polygon),
            17 => Some(Self::Meter),
            18 => Some(Self::LinearBarGraph),
            19 => Some(Self::ArchedBarGraph),
            20 => Some(Self::PictureGraphic),
            21 => Some(Self::NumberVariable),
            22 => Some(Self::StringVariable),
            23 => Some(Self::FontAttributes),
            24 => Some(Self::LineAttributes),
            25 => Some(Self::FillAttributes),
            26 => Some(Self::InputAttributes),
            27 => Some(Self::ObjectPointer),
            28 => Some(Self::Macro),
            29 => Some(Self::AuxFunction),
            30 => Some(Self::AuxInput),
            31 => Some(Self::AuxFunction2),
            32 => Some(Self::AuxInput2),
            33 => Some(Self::AuxControlDesig),
            34 => Some(Self::WindowMask),
            35 => Some(Self::KeyGroup),
            36 => Some(Self::GraphicContext),
            37 => Some(Self::OutputList),
            38 => Some(Self::ExtendedInputAttributes),
            39 => Some(Self::ColourMap),
            40 => Some(Self::ObjectLabelRef),
            41 => Some(Self::ExternalObjectDefinition),
            42 => Some(Self::ExternalReferenceName),
            43 => Some(Self::ExternalObjectPointer),
            44 => Some(Self::Animation),
            45 => Some(Self::ColourPalette),
            46 => Some(Self::GraphicData),
            47 => Some(Self::WorkingSetSpecialControls),
            48 => Some(Self::ScaledGraphic),
            49 => Some(Self::ScaledBitmap),
            50 => Some(Self::GraphicsContext),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

#[must_use]
pub(crate) const fn window_mask_required_object_types(
    window_type: u8,
) -> Option<&'static [ObjectType]> {
    match window_type {
        0 => Some(&[]),
        1 | 10 => Some(&[ObjectType::OutputNumber, ObjectType::OutputString]),
        2 | 11 => Some(&[ObjectType::OutputNumber]),
        3 | 12 => Some(&[ObjectType::OutputString]),
        4 | 13 => Some(&[ObjectType::InputNumber, ObjectType::OutputString]),
        5 | 14 => Some(&[ObjectType::InputNumber]),
        6 | 15 => Some(&[ObjectType::InputString]),
        7 | 16 => Some(&[ObjectType::LinearBarGraph]),
        8 | 17 => Some(&[ObjectType::Button]),
        9 | 18 => Some(&[ObjectType::Button, ObjectType::Button]),
        _ => None,
    }
}

/// `true` when an object type is admissible as the Object Label graphic
/// representation/designator field. ISO 11783-6 defines this as an output
/// object subset used in proprietary screens/editor popups; reference data
/// objects such as String Variables are deliberately excluded.
#[must_use]
pub(crate) const fn is_object_label_graphic_representation_type(r#type: ObjectType) -> bool {
    matches!(
        r#type,
        ObjectType::Container
            | ObjectType::OutputString
            | ObjectType::OutputNumber
            | ObjectType::OutputList
            | ObjectType::Line
            | ObjectType::Rectangle
            | ObjectType::Ellipse
            | ObjectType::Polygon
            | ObjectType::Meter
            | ObjectType::LinearBarGraph
            | ObjectType::ArchedBarGraph
            | ObjectType::PictureGraphic
            | ObjectType::ObjectPointer
            | ObjectType::Animation
            | ObjectType::ScaledGraphic
            | ObjectType::GraphicContext
            | ObjectType::GraphicsContext
            | ObjectType::ScaledBitmap
    )
}

/// `true` when a Key Group Name reference is standard-conforming.
///
/// ISO 11783-6 Table B.63 defines the Key Group name as an Output String
/// object or an Object Pointer that directly points to an Output String. The
/// object-pool upload path and generic-attribute replay paths share this check
/// so a malformed proprietary-mapping label cannot be retained in one path and
/// silently ignored by another.
#[must_use]
pub(crate) fn key_group_name_reference_is_valid(pool: &ObjectPool, reference: ObjectID) -> bool {
    if reference == ObjectID::NULL {
        return true;
    }
    let Some(target) = pool.find(reference) else {
        return false;
    };
    match target.r#type {
        ObjectType::OutputString => true,
        ObjectType::ObjectPointer => target.get_object_pointer_body().is_ok_and(|body| {
            pool.find(body.value)
                .is_some_and(|obj| obj.r#type == ObjectType::OutputString)
        }),
        _ => false,
    }
}

/// `true` when a Key Group Icon reference is standard-conforming.
///
/// The icon is optional (`NULL`) or one of the output object families admitted
/// by the Object Label Graphic Representation table.
#[must_use]
pub(crate) fn key_group_icon_reference_is_valid(pool: &ObjectPool, reference: ObjectID) -> bool {
    reference == ObjectID::NULL
        || pool
            .find(reference)
            .is_some_and(|object| is_object_label_graphic_representation_type(object.r#type))
}

/// `true` when a Window Mask Name or Window Title reference is standard
/// conforming. These fields use the same text-designator shape as Key Group
/// Name: Output String directly, or Object Pointer directly to Output String.
#[must_use]
pub(crate) fn window_mask_text_reference_is_valid(pool: &ObjectPool, reference: ObjectID) -> bool {
    key_group_name_reference_is_valid(pool, reference)
}

/// `true` when a Window Mask Icon reference is standard conforming. The icon
/// is optional in the hosted core for compatibility with existing free-form
/// pools, but any non-NULL reference must be a graphic representation object.
#[must_use]
pub(crate) fn window_mask_icon_reference_is_valid(pool: &ObjectPool, reference: ObjectID) -> bool {
    key_group_icon_reference_is_valid(pool, reference)
}

/// `true` when an Output List item slot references an object family that can
/// be presented as the selected item by the hosted renderer. NULL remains the
/// standard no-display placeholder; Object Pointers are checked against their
/// current direct target so retargeting cannot turn a visible list item into
/// inert style/reference metadata.
#[must_use]
pub(crate) fn output_list_item_reference_is_valid(pool: &ObjectPool, reference: ObjectID) -> bool {
    output_list_item_reference_is_valid_inner(pool, reference, &mut Vec::new())
}

fn output_list_item_reference_is_valid_inner(
    pool: &ObjectPool,
    reference: ObjectID,
    path: &mut Vec<ObjectID>,
) -> bool {
    if reference == ObjectID::NULL {
        return true;
    }
    if path.contains(&reference) {
        return false;
    }
    let Some(object) = pool.find(reference) else {
        return false;
    };
    match object.r#type {
        ObjectType::ObjectPointer => {
            let Ok(body) = object.get_object_pointer_body() else {
                return false;
            };
            if body.value == ObjectID::NULL {
                return true;
            }
            path.push(reference);
            let valid = output_list_item_reference_is_valid_inner(pool, body.value, path);
            path.pop();
            valid
        }
        ObjectType::ExternalObjectPointer
        | ObjectType::DataMask
        | ObjectType::AlarmMask
        | ObjectType::WindowMask
        | ObjectType::Button
        | ObjectType::InputBoolean
        | ObjectType::InputString
        | ObjectType::InputNumber
        | ObjectType::InputList
        | ObjectType::OutputString
        | ObjectType::OutputNumber
        | ObjectType::OutputList
        | ObjectType::StringVariable
        | ObjectType::NumberVariable
        | ObjectType::Line
        | ObjectType::Rectangle
        | ObjectType::Ellipse
        | ObjectType::Polygon
        | ObjectType::Meter
        | ObjectType::LinearBarGraph
        | ObjectType::ArchedBarGraph
        | ObjectType::PictureGraphic
        | ObjectType::ScaledGraphic
        | ObjectType::GraphicContext
        | ObjectType::Container
        | ObjectType::Key
        | ObjectType::KeyGroup
        | ObjectType::Animation => true,
        _ => false,
    }
}

/// `true` when an object type may be targeted by the standard Enable/Disable
/// Object command. ISO 11783-6 limits this command to input field objects,
/// Buttons, and Animation objects; other object families must not accumulate
/// inert runtime enable state.
#[must_use]
pub(crate) const fn is_enable_disable_object_type(r#type: ObjectType) -> bool {
    matches!(
        r#type,
        ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
            | ObjectType::Button
            | ObjectType::Animation
    )
}

/// `true` when an object type may be selected by the standard Select Input
/// Object command for the advertised VT version. Button and Key selection are
/// VT4+ features; VT3 supports only input field objects.
#[must_use]
pub(crate) const fn is_select_input_object_type(r#type: ObjectType, vt_version: u16) -> bool {
    matches!(
        r#type,
        ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
    ) || (vt_version >= 4 && matches!(r#type, ObjectType::Button | ObjectType::Key))
}

/// `true` when Select Input Object option `0` may open the target for data
/// input. Button and Key objects can be focused but never opened for edit.
#[must_use]
pub(crate) const fn is_select_input_open_target_type(r#type: ObjectType) -> bool {
    matches!(
        r#type,
        ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
    )
}

/// `true` when the Change Soft Key Mask command's Mask Type byte names the
/// actual referenced mask object family. The standard uses `1` for Data Mask
/// and `2` for Alarm Mask; no other values are assigned.
#[must_use]
pub(crate) const fn change_soft_key_mask_type_matches(
    mask_type: u8,
    object_type: ObjectType,
) -> bool {
    matches!(
        (mask_type, object_type),
        (1, ObjectType::DataMask) | (2, ObjectType::AlarmMask)
    )
}

#[must_use]
pub(crate) const fn text_justification_is_valid(justification: u8) -> bool {
    justification <= 0x0F && (justification & 0x03) != 0x03 && ((justification >> 2) & 0x03) != 0x03
}

/// `true` when the standard Change Attribute command defines `attribute_id`
/// for `object_type`.
///
/// Server admission, hosted runtime replay, and macro helpers all use this
/// single table so an unsupported AID cannot be retained in one path while
/// another path silently drops it as inert state.
#[must_use]
pub(crate) const fn vt_change_attribute_id_is_supported(
    object_type: ObjectType,
    attribute_id: u8,
) -> bool {
    match object_type {
        ObjectType::DataMask => matches!(attribute_id, 1..=2),
        ObjectType::AlarmMask => matches!(attribute_id, 1..=4),
        ObjectType::WindowMask => matches!(attribute_id, 1..=8),
        ObjectType::Container => false,
        ObjectType::SoftKeyMask => attribute_id == 1,
        ObjectType::Key => matches!(attribute_id, 1..=2),
        ObjectType::KeyGroup => matches!(attribute_id, 1..=3),
        ObjectType::ExternalObjectDefinition
        | ObjectType::ExternalReferenceName
        | ObjectType::ExternalObjectPointer => matches!(attribute_id, 1..=3),
        ObjectType::OutputString => matches!(attribute_id, 1..=7),
        ObjectType::OutputNumber => matches!(attribute_id, 1..=11),
        ObjectType::OutputList => matches!(attribute_id, 1..=4),
        ObjectType::InputBoolean => matches!(attribute_id, 1..=6),
        ObjectType::InputString => matches!(attribute_id, 1..=9),
        ObjectType::InputNumber => matches!(attribute_id, 1..=13),
        ObjectType::InputList => matches!(attribute_id, 1..=5),
        ObjectType::FontAttributes => matches!(attribute_id, 1..=4),
        ObjectType::LineAttributes => matches!(attribute_id, 1..=3),
        ObjectType::FillAttributes => matches!(attribute_id, 1..=3),
        ObjectType::Line => matches!(attribute_id, 1..=4),
        ObjectType::Rectangle => matches!(attribute_id, 1..=5),
        ObjectType::Ellipse => matches!(attribute_id, 1..=7),
        ObjectType::Polygon => matches!(attribute_id, 1..=5),
        ObjectType::Meter => matches!(attribute_id, 1..=12),
        ObjectType::LinearBarGraph => matches!(attribute_id, 1..=12),
        ObjectType::ArchedBarGraph => matches!(attribute_id, 1..=13),
        ObjectType::Button => matches!(attribute_id, 1..=6),
        ObjectType::PictureGraphic => matches!(attribute_id, 1..=3),
        ObjectType::ScaledGraphic => matches!(attribute_id, 1..=5),
        ObjectType::GraphicData => false,
        ObjectType::Animation => matches!(attribute_id, 1..=9),
        ObjectType::GraphicContext => matches!(attribute_id, 1..=4 | 7..=17),
        ObjectType::ColourPalette => attribute_id == 1,
        ObjectType::WorkingSetSpecialControls => matches!(attribute_id, 2 | 3),
        _ => false,
    }
}

/// `true` when a Change Attribute value targets a standard one-byte attribute
/// slot. The ECU-to-VT Change Attribute payload is four bytes wide, but these
/// attributes are one-byte object fields; accepting non-zero upper bytes would
/// silently retain a value that later renders as a different low byte.
#[must_use]
pub(crate) const fn change_attribute_targets_one_byte_field(
    r#type: ObjectType,
    attribute_id: u8,
) -> bool {
    matches!(
        (r#type, attribute_id),
        (ObjectType::DataMask, 1)
            | (ObjectType::AlarmMask, 1 | 3 | 4)
            | (ObjectType::WindowMask, 1..=5)
            | (ObjectType::SoftKeyMask, 1)
            | (ObjectType::Key, 1 | 2)
            | (ObjectType::KeyGroup, 1)
            | (ObjectType::ExternalObjectDefinition, 1)
            | (ObjectType::ExternalReferenceName, 1)
            | (ObjectType::OutputString, 3 | 5 | 7)
            | (ObjectType::OutputNumber, 3 | 5 | 9 | 10 | 11)
            | (ObjectType::InputBoolean, 1)
            | (ObjectType::InputString, 3 | 6 | 8)
            | (ObjectType::InputNumber, 3 | 5 | 11 | 12 | 13)
            | (ObjectType::FontAttributes, 1..=4)
            | (ObjectType::LineAttributes, 1..=2)
            | (ObjectType::FillAttributes, 1 | 2)
            | (ObjectType::Line, 4)
            | (ObjectType::Rectangle, 4)
            | (ObjectType::Ellipse, 4..=6)
            | (ObjectType::Polygon, 5)
            | (ObjectType::Meter, 2..=8)
            | (ObjectType::LinearBarGraph, 3..=6)
            | (ObjectType::ArchedBarGraph, 3..=8)
            | (ObjectType::Button, 3..=6)
            | (ObjectType::PictureGraphic, 2 | 3)
            | (ObjectType::ScaledGraphic, 3 | 4)
            | (ObjectType::Animation, 4..=9)
            | (ObjectType::GraphicContext, 10 | 11 | 15 | 16 | 17)
            | (ObjectType::ColourPalette, 1)
    )
}

/// `true` when a Change Attribute value targets a standard two-byte attribute
/// slot. The command's wire payload is four bytes wide, but these fields are
/// stored as u16/i16/object-id values; non-zero upper bytes would otherwise be
/// retained by the server and truncated differently by render/runtime replay.
#[must_use]
pub(crate) const fn change_attribute_targets_two_byte_field(
    r#type: ObjectType,
    attribute_id: u8,
) -> bool {
    matches!(
        (r#type, attribute_id),
        (ObjectType::DataMask | ObjectType::AlarmMask, 2)
            | (ObjectType::WindowMask, 6..=8)
            | (ObjectType::KeyGroup, 2 | 3)
            | (ObjectType::ExternalObjectPointer, 1..=3)
            | (ObjectType::OutputString, 1 | 2 | 4 | 6)
            | (ObjectType::OutputNumber, 1 | 2 | 4 | 6)
            | (ObjectType::OutputList, 1..=3)
            | (ObjectType::InputBoolean, 2..=4)
            | (ObjectType::InputString, 1 | 2 | 4 | 5 | 7)
            | (ObjectType::InputNumber, 1 | 2 | 4 | 6)
            | (ObjectType::InputList, 1..=3)
            | (ObjectType::LineAttributes, 3)
            | (ObjectType::FillAttributes, 3)
            | (ObjectType::Line, 1..=3)
            | (ObjectType::Rectangle, 1 | 2 | 3 | 5)
            | (ObjectType::Ellipse, 1 | 2 | 3 | 7)
            | (ObjectType::Polygon, 1..=4)
            | (ObjectType::Meter, 1 | 9 | 10 | 11)
            | (ObjectType::LinearBarGraph, 1 | 2 | 7..=11)
            | (ObjectType::ArchedBarGraph, 1 | 2 | 8..=13)
            | (ObjectType::Button, 1 | 2)
            | (ObjectType::PictureGraphic, 1)
            | (ObjectType::ScaledGraphic, 1 | 2 | 5)
            | (ObjectType::Animation, 1..=3)
            | (ObjectType::GraphicContext, 1..=4 | 8 | 9 | 12..=14)
            | (ObjectType::WorkingSetSpecialControls, 2 | 3)
    )
}

// ─── Type-specific bodies ─────────────────────────────────────────────

/// Window Mask body (Type 34, §4.6.21).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowMaskBody {
    /// Width in User-Layout Data Mask columns. Used by free-form windows.
    pub width_cells: u8,
    /// Height in User-Layout Data Mask rows. Used by free-form windows.
    pub height_cells: u8,
    /// 0 = free form; non-zero values are VT-formatted window types.
    pub window_type: u8,
    pub background_color: u8,
    /// bit 0 = available for adjustment, bit 1 = transparent.
    pub options: u8,
    pub name: ObjectID,
    pub window_title: ObjectID,
    pub window_icon: ObjectID,
    /// Window-type-specific component references. Free-form windows keep this
    /// empty and use normal positional children instead.
    pub required_objects: Vec<ObjectID>,
}

impl Default for WindowMaskBody {
    fn default() -> Self {
        Self {
            width_cells: 1,
            height_cells: 1,
            window_type: 0,
            background_color: 0,
            options: 0,
            name: ObjectID::NULL,
            window_title: ObjectID::NULL,
            window_icon: ObjectID::NULL,
            required_objects: Vec::new(),
        }
    }
}

impl WindowMaskBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if !(1..=2).contains(&self.width_cells) {
            return Err(Error::invalid_data(
                "Window Mask width must be 1..=2 user-layout columns",
            ));
        }
        if !(1..=6).contains(&self.height_cells) {
            return Err(Error::invalid_data(
                "Window Mask height must be 1..=6 user-layout rows",
            ));
        }
        if self.options & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Window Mask options contain reserved bits",
            ));
        }
        if self.required_objects.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Window Mask required-object count exceeds u8 count field",
            ));
        }
        let mut data = Vec::with_capacity(12 + self.required_objects.len().saturating_mul(2));
        data.push(self.width_cells);
        data.push(self.height_cells);
        data.push(self.window_type);
        data.push(self.background_color);
        data.push(self.options);
        push_u16_le(&mut data, self.name);
        push_u16_le(&mut data, self.window_title);
        push_u16_le(&mut data, self.window_icon);
        data.push(self.required_objects.len() as u8);
        for object in &self.required_objects {
            push_u16_le(&mut data, *object);
        }
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 12 {
            return Err(Error::invalid_data("Window Mask body too short"));
        }
        let required_count = body[11] as usize;
        let required_end = 12usize
            .checked_add(required_count.saturating_mul(2))
            .ok_or_else(|| Error::invalid_data("Window Mask required-object list overflows"))?;
        if body.len() != required_end {
            return Err(Error::invalid_data(
                "Window Mask required-object count does not match body length",
            ));
        }
        if body[4] & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Window Mask body contains reserved option bits",
            ));
        }
        if !(1..=2).contains(&body[0]) {
            return Err(Error::invalid_data(
                "Window Mask body width is outside the 1..=2 user-layout column range",
            ));
        }
        if !(1..=6).contains(&body[1]) {
            return Err(Error::invalid_data(
                "Window Mask body height is outside the 1..=6 user-layout row range",
            ));
        }
        let mut required_objects = Vec::with_capacity(required_count);
        let mut offset = 12;
        for _ in 0..required_count {
            required_objects.push(u16_le_as(&body[offset..]));
            offset += 2;
        }
        Ok(Self {
            width_cells: body[0],
            height_cells: body[1],
            window_type: body[2],
            background_color: body[3],
            options: body[4],
            name: u16_le_as(&body[5..]),
            window_title: u16_le_as(&body[7..]),
            window_icon: u16_le_as(&body[9..]),
            required_objects,
        })
    }
}

/// Key Group body (Type 35, §4.6.22).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyGroupBody {
    /// bit 0 = available, bit 1 = transparent.
    pub options: u8,
    pub name: ObjectID,
    pub key_group_icon: ObjectID,
}

impl Default for KeyGroupBody {
    fn default() -> Self {
        Self {
            options: 0,
            name: ObjectID::NULL,
            key_group_icon: ObjectID::NULL,
        }
    }
}

impl KeyGroupBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(5);
        data.push(self.options);
        push_u16_le(&mut data, self.name);
        push_u16_le(&mut data, self.key_group_icon);
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 5 {
            return Err(Error::invalid_data("Key Group body too short"));
        }
        if body[0] & !0x03 != 0 {
            return Err(Error::invalid_data(
                "Key Group body contains reserved option bits",
            ));
        }
        Ok(Self {
            options: body[0],
            name: u16_le_as(&body[1..]),
            key_group_icon: u16_le_as(&body[3..]),
        })
    }
}

/// Key body (Type 5, §4.6.7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyBody {
    pub background_color: u8,
    pub key_code: u8,
}

impl KeyBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        // ISO 11783-6 Key body: background colour + key code only. (There is
        // no Key "options"/latchable field — that belongs to Button.)
        vec![self.background_color, self.key_code]
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 2 {
            return Err(Error::invalid_data("Key body too short"));
        }
        Ok(Self {
            background_color: body[0],
            key_code: body[1],
        })
    }
}

/// Single command inside a Macro body.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroCommand {
    pub command_type: u8,
    pub parameters: Vec<u8>,
}

impl MacroCommand {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(1 + self.parameters.len());
        data.push(self.command_type);
        data.extend_from_slice(&self.parameters);
        data
    }

    /// Total length of a VT command packet inside a Macro, including the
    /// command byte, per ISO 11783-6 Annex F "Data length".
    ///
    /// Most supported ECU→VT object commands are 8-byte frames. Change Child
    /// Position (`0xB4`) carries 16-bit x/y positions and is 9 bytes, while
    /// Change String Value (`0xB3`) is variable-length (it carries a string
    /// and is handled specially in [`MacroBody::decode`]).
    /// Returns `0` for unknown or variable-length commands.
    #[must_use]
    pub const fn get_command_length(cmd: u8) -> u16 {
        match cmd {
            // Change String Value: variable (string payload).
            0xB3 => 0,
            // Fixed 8-byte ECU→VT commands allowed in a Macro (Annex F).
            0xB4 => 9,
            0xA0..=0xAC | 0xAD..=0xB2 | 0xB5 | 0xB6 | 0xB7 | 0xBA | 0xBD | 0xBE => 8,
            _ => 0,
        }
    }
}

/// Macro body (Type 28, §4.6.30).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MacroBody {
    pub commands: Vec<MacroCommand>,
}

impl MacroBody {
    /// Encode to the ISO 11783-6 wire layout `[num_bytes:u16][commands…]`.
    /// The leading byte count makes the macro object self-delimiting in an
    /// object pool (the command stream itself carries no terminator).
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut commands = Vec::new();
        for cmd in &self.commands {
            commands.extend(cmd.encode());
        }
        let num_bytes = commands.len().min(u16::MAX as usize) as u16;
        let mut data = Vec::with_capacity(2 + num_bytes as usize);
        data.extend_from_slice(&num_bytes.to_le_bytes());
        data.extend_from_slice(&commands[..num_bytes as usize]);
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 2 {
            return Err(Error::invalid_data("Macro body too short"));
        }
        let num_bytes = u16_le(&body[0..]) as usize;
        if body.len() != 2 + num_bytes {
            return Err(Error::invalid_data(
                "Macro byte count does not match body length",
            ));
        }
        let body = &body[2..];
        let mut mb = Self::default();
        let mut offset = 0usize;
        while offset < body.len() {
            let command_type = body[offset];
            offset += 1;
            let cmd_len = MacroCommand::get_command_length(command_type);
            let mut cmd = MacroCommand {
                command_type,
                ..Default::default()
            };
            if cmd_len == 0 {
                if command_type == 0xB3 {
                    // Change String Value: [cmd][id_lo][id_hi][len_lo][len_hi][string...]
                    if offset + 4 > body.len() {
                        return Err(Error::invalid_data(
                            "incomplete Change String Value in macro",
                        ));
                    }
                    let str_len = u16_le(&body[offset + 2..]) as usize;
                    let total_len = 4 + str_len;
                    if offset + total_len > body.len() {
                        return Err(Error::invalid_data("string data exceeds macro body"));
                    }
                    cmd.parameters
                        .extend_from_slice(&body[offset..offset + total_len]);
                    offset += total_len;
                } else {
                    return Err(Error::invalid_data(format!(
                        "unknown or variable-length macro command 0x{command_type:02X}"
                    )));
                }
            } else {
                let param_len = (cmd_len - 1) as usize;
                if offset + param_len > body.len() {
                    return Err(Error::invalid_data("macro command parameters exceed body"));
                }
                cmd.parameters
                    .extend_from_slice(&body[offset..offset + param_len]);
                offset += param_len;
            }
            mb.commands.push(cmd);
        }
        Ok(mb)
    }
}

/// Alarm Mask body (Type 2, §4.6.3) with priority extension.
///
/// ISO 11783-6 fixed fields: background colour, soft-key mask, priority,
/// acoustic signal. (An earlier revision carried a non-standard `options`
/// byte here; it has been removed for conformance.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlarmMaskBody {
    pub background_color: u8,
    pub soft_key_mask: ObjectID,
    /// 0 = High (operator danger / urgent malfunction),
    /// 1 = Medium (normal alarm), 2 = Low (information only).
    pub priority: u8,
    /// 0 = highest priority, 1 = medium priority, 2 = lowest priority,
    /// 3 = none/silent.
    pub acoustic_signal: u8,
}

impl Default for AlarmMaskBody {
    fn default() -> Self {
        Self {
            background_color: 0,
            soft_key_mask: ObjectID::NULL,
            priority: 0,
            acoustic_signal: 0,
        }
    }
}

impl AlarmMaskBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.priority > 2 {
            return Err(Error::invalid_data(
                "Alarm Mask priority must be 0, 1, or 2",
            ));
        }
        if self.acoustic_signal > 3 {
            return Err(Error::invalid_data(
                "Alarm Mask acoustic signal must be 0, 1, 2, or 3",
            ));
        }
        let mut data = Vec::with_capacity(5);
        data.push(self.background_color);
        push_u16_le(&mut data, self.soft_key_mask);
        data.push(self.priority);
        data.push(self.acoustic_signal);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 5 {
            return Err(Error::invalid_data("Alarm Mask body too short"));
        }
        if body[3] > 2 {
            return Err(Error::invalid_data(
                "Alarm Mask body contains invalid priority",
            ));
        }
        if body[4] > 3 {
            return Err(Error::invalid_data(
                "Alarm Mask body contains invalid acoustic signal",
            ));
        }
        Ok(Self {
            background_color: body[0],
            soft_key_mask: u16_le_as(&body[1..]),
            priority: body[3],
            acoustic_signal: body[4],
        })
    }
}

/// Data Mask body (Type 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DataMaskBody {
    pub background_color: u8,
    pub soft_key_mask: ObjectID,
}

impl Default for DataMaskBody {
    fn default() -> Self {
        Self {
            background_color: 0,
            soft_key_mask: ObjectID::NULL,
        }
    }
}

impl DataMaskBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(3);
        data.push(self.background_color);
        push_u16_le(&mut data, self.soft_key_mask);
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 3 {
            return Err(Error::invalid_data("Data Mask body too short"));
        }
        Ok(Self {
            background_color: body[0],
            soft_key_mask: u16_le_as(&body[1..]),
        })
    }
}

/// Container body (Type 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ContainerBody {
    pub width: u16,
    pub height: u16,
    pub hidden: bool,
}

impl ContainerBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(5);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(u8::from(self.hidden));
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 5 {
            return Err(Error::invalid_data("Container body too short"));
        }
        if body[4] > 1 {
            return Err(Error::invalid_data(
                "Container body contains reserved hidden-state value",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            hidden: body[4] != 0,
        })
    }
}

/// Soft Key Mask body (Type 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SoftKeyMaskBody {
    pub background_color: u8,
}

impl SoftKeyMaskBody {
    #[must_use]
    pub const fn encode(&self) -> [u8; 1] {
        [self.background_color]
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.is_empty() {
            return Err(Error::invalid_data("Soft Key Mask body too short"));
        }
        Ok(Self {
            background_color: body[0],
        })
    }
}

/// Button body (Type 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ButtonBody {
    pub width: u16,
    pub height: u16,
    pub background_color: u8,
    pub border_color: u8,
    pub key_code: u8,
    pub options: u8,
}

impl ButtonBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(8);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.background_color);
        data.push(self.border_color);
        data.push(self.key_code);
        data.push(self.options);
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 8 {
            return Err(Error::invalid_data("Button body too short"));
        }
        if body[7] & 0xC0 != 0 {
            return Err(Error::invalid_data(
                "Button body contains reserved option bits",
            ));
        }
        Ok(Self {
            width: u16_le(&body[0..]),
            height: u16_le(&body[2..]),
            background_color: body[4],
            border_color: body[5],
            key_code: body[6],
            options: body[7],
        })
    }
}

/// Number Variable body (Type 21).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NumberVariableBody {
    pub value: u32,
}

impl NumberVariableBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        self.value.to_le_bytes().to_vec()
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 4 {
            return Err(Error::invalid_data("Number Variable body too short"));
        }
        Ok(Self {
            value: u32_le(&body[0..]),
        })
    }
}

/// String Variable body (Type 22). ISO 11783-6 layout: a u16 maximum
/// fixed length followed by the string value bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StringVariableBody {
    /// Maximum fixed length of the string value.
    pub length: u16,
    pub value: Vec<u8>,
}

impl StringVariableBody {
    /// Encode to the ISO 11783-6 wire layout `[length:u16][value bytes]`.
    /// The on-wire length is the actual `value` byte count so the pool
    /// walker can recover the body without a separate length prefix; the
    /// struct's `length` field is kept in step on decode.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let wire_len = self.value.len().min(u16::MAX as usize) as u16;
        let mut data = Vec::with_capacity(2 + wire_len as usize);
        data.extend_from_slice(&wire_len.to_le_bytes());
        data.extend_from_slice(&self.value[..wire_len as usize]);
        data
    }

    /// Decode a string-variable body. Falls back to treating the whole
    /// buffer as the value (length 0) when there are fewer than 2 bytes,
    /// so callers that hold legacy buffers without a length prefix do not
    /// panic.
    #[must_use]
    pub fn decode(body: &[u8]) -> Self {
        if body.len() < 2 {
            return Self {
                length: 0,
                value: body.to_vec(),
            };
        }
        let length = u16_le(&body[0..]);
        Self {
            length,
            value: body[2..].to_vec(),
        }
    }
}

/// Font Attributes body (Type 23).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FontAttributesBody {
    pub font_color: u8,
    pub font_size: u8,
    pub font_type: u8,
    pub font_style: u8,
}

#[inline]
pub(crate) const fn is_standard_font_type(font_type: u8) -> bool {
    matches!(font_type, 0..=2 | 4 | 5 | 7 | 240..=255)
}

#[inline]
pub(crate) const fn is_proportional_font_style(font_style: u8) -> bool {
    font_style & 0x80 != 0
}

#[inline]
pub(crate) const fn is_standard_font_size_for_style(font_size: u8, font_style: u8) -> bool {
    if is_proportional_font_style(font_style) {
        font_size >= 8
    } else {
        font_size <= 14
    }
}

impl FontAttributesBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        vec![
            self.font_color,
            self.font_size,
            self.font_type,
            self.font_style,
        ]
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 4 {
            return Err(Error::invalid_data("Font Attributes body too short"));
        }
        if !is_standard_font_size_for_style(body[1], body[3]) {
            return Err(Error::invalid_data(
                "Font Attributes body contains reserved font size for style",
            ));
        }
        if !is_standard_font_type(body[2]) {
            return Err(Error::invalid_data(
                "Font Attributes body contains reserved font type",
            ));
        }
        Ok(Self {
            font_color: body[0],
            font_size: body[1],
            font_type: body[2],
            font_style: body[3],
        })
    }
}

/// Line Attributes body (Type 24).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LineAttributesBody {
    pub line_color: u8,
    pub line_width: u8,
    pub line_art: u16,
}

impl LineAttributesBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(4);
        data.push(self.line_color);
        data.push(self.line_width);
        push_u16_le(&mut data, self.line_art);
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 4 {
            return Err(Error::invalid_data("Line Attributes body too short"));
        }
        Ok(Self {
            line_color: body[0],
            line_width: body[1],
            line_art: u16_le(&body[2..]),
        })
    }
}

/// Fill Attributes body (Type 25).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FillAttributesBody {
    /// 0 = none, 1 = line colour, 2 = fill colour, 3 = pattern.
    pub fill_type: u8,
    pub fill_color: u8,
    pub fill_pattern: ObjectID,
}

impl Default for FillAttributesBody {
    fn default() -> Self {
        Self {
            fill_type: 0,
            fill_color: 0,
            fill_pattern: ObjectID::NULL,
        }
    }
}

impl FillAttributesBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.fill_type > 3 {
            return Err(Error::invalid_data(
                "Fill Attributes fill type must be in 0..=3",
            ));
        }
        let mut data = Vec::with_capacity(4);
        data.push(self.fill_type);
        data.push(self.fill_color);
        push_u16_le(&mut data, self.fill_pattern);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 4 {
            return Err(Error::invalid_data("Fill Attributes body too short"));
        }
        if body[0] > 3 {
            return Err(Error::invalid_data(
                "Fill Attributes body contains reserved fill type",
            ));
        }
        Ok(Self {
            fill_type: body[0],
            fill_color: body[1],
            fill_pattern: u16_le_as(&body[2..]),
        })
    }
}

/// Input Attributes body (Type 26).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputAttributesBody {
    /// 0 = listed characters are valid, 1 = listed characters are invalid.
    pub validation_type: u8,
    pub validation_string: Vec<u8>,
}

impl InputAttributesBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.validation_type > 1 {
            return Err(Error::invalid_data(
                "Input Attributes validation type must be 0 or 1",
            ));
        }
        if self.validation_string.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "Input Attributes validation string exceeds u8 length field",
            ));
        }
        // ISO 11783-6: validation type, validation string length (u8), string.
        let mut data = Vec::with_capacity(2 + self.validation_string.len());
        data.push(self.validation_type);
        data.push(self.validation_string.len() as u8);
        data.extend_from_slice(&self.validation_string);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 2 {
            return Err(Error::invalid_data("Input Attributes body too short"));
        }
        if body[0] > 1 {
            return Err(Error::invalid_data(
                "Input Attributes body contains reserved validation type",
            ));
        }
        let len = body[1] as usize;
        if body.len() != 2 + len {
            return Err(Error::invalid_data(
                "Input Attributes string length does not match body length",
            ));
        }
        Ok(Self {
            validation_type: body[0],
            validation_string: body[2..].to_vec(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WideCharRange {
    pub first: u16,
    pub last: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedInputCodePlane {
    pub plane: u8,
    pub ranges: Vec<WideCharRange>,
}

/// Extended Input Attributes body (Type 38, VT version 4+).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExtendedInputAttributesBody {
    /// 0 = listed ranges are valid, 1 = listed ranges are invalid.
    pub validation_type: u8,
    pub code_planes: Vec<ExtendedInputCodePlane>,
}

impl ExtendedInputAttributesBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.validation_type > 1 {
            return Err(Error::invalid_data(
                "Extended Input Attributes validation type must be 0 or 1",
            ));
        }
        if self.code_planes.is_empty() || self.code_planes.len() > 17 {
            return Err(Error::invalid_data(
                "Extended Input Attributes code-plane count must be in 1..=17",
            ));
        }
        let mut data = Vec::new();
        data.push(self.validation_type);
        data.push(self.code_planes.len() as u8);
        let mut seen_planes = [false; 17];
        for code_plane in &self.code_planes {
            if code_plane.plane > 16 {
                return Err(Error::invalid_data(
                    "Extended Input Attributes code plane must be in 0..=16",
                ));
            }
            let plane_index = usize::from(code_plane.plane);
            if seen_planes[plane_index] {
                return Err(Error::invalid_data(
                    "Extended Input Attributes contains a duplicate code plane",
                ));
            }
            seen_planes[plane_index] = true;
            if code_plane.ranges.is_empty() || code_plane.ranges.len() > u8::MAX as usize {
                return Err(Error::invalid_data(
                    "Extended Input Attributes range count must be in 1..=255",
                ));
            }
            data.push(code_plane.plane);
            data.push(code_plane.ranges.len() as u8);
            for range in &code_plane.ranges {
                if range.first > range.last {
                    return Err(Error::invalid_data(
                        "Extended Input Attributes range first must be <= last",
                    ));
                }
                push_u16_le(&mut data, range.first);
                push_u16_le(&mut data, range.last);
            }
        }
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 2 {
            return Err(Error::invalid_data(
                "Extended Input Attributes body too short",
            ));
        }
        if body[0] > 1 {
            return Err(Error::invalid_data(
                "Extended Input Attributes body contains reserved validation type",
            ));
        }
        let plane_count = body[1] as usize;
        if plane_count == 0 || plane_count > 17 {
            return Err(Error::invalid_data(
                "Extended Input Attributes body contains invalid code-plane count",
            ));
        }
        let mut offset = 2;
        let mut code_planes = Vec::with_capacity(plane_count);
        let mut seen_planes = [false; 17];
        for _ in 0..plane_count {
            if offset + 2 > body.len() {
                return Err(Error::invalid_data(
                    "Extended Input Attributes code-plane record is truncated",
                ));
            }
            let plane = body[offset];
            let range_count = body[offset + 1] as usize;
            offset += 2;
            if plane > 16 || range_count == 0 {
                return Err(Error::invalid_data(
                    "Extended Input Attributes code-plane record is invalid",
                ));
            }
            let plane_index = usize::from(plane);
            if seen_planes[plane_index] {
                return Err(Error::invalid_data(
                    "Extended Input Attributes contains a duplicate code plane",
                ));
            }
            seen_planes[plane_index] = true;
            let range_bytes = range_count.checked_mul(4).ok_or_else(|| {
                Error::invalid_data("Extended Input Attributes range length overflows")
            })?;
            if offset + range_bytes > body.len() {
                return Err(Error::invalid_data(
                    "Extended Input Attributes range list is truncated",
                ));
            }
            let mut ranges = Vec::with_capacity(range_count);
            for _ in 0..range_count {
                let first = u16_le(&body[offset..]);
                let last = u16_le(&body[offset + 2..]);
                offset += 4;
                if first > last {
                    return Err(Error::invalid_data(
                        "Extended Input Attributes range first exceeds last",
                    ));
                }
                ranges.push(WideCharRange { first, last });
            }
            code_planes.push(ExtendedInputCodePlane { plane, ranges });
        }
        if offset != body.len() {
            return Err(Error::invalid_data(
                "Extended Input Attributes body has trailing bytes",
            ));
        }
        Ok(Self {
            validation_type: body[0],
            code_planes,
        })
    }
}

/// Object Pointer body (Type 27).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectPointerBody {
    pub value: ObjectID,
}

impl Default for ObjectPointerBody {
    fn default() -> Self {
        Self {
            value: ObjectID::NULL,
        }
    }
}

impl ObjectPointerBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(2);
        push_u16_le(&mut data, self.value);
        data
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 2 {
            return Err(Error::invalid_data("Object Pointer body too short"));
        }
        Ok(Self {
            value: u16_le_as(&body[0..]),
        })
    }
}

/// Output Line body (Type 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputLineBody {
    pub width: u16,
    pub height: u16,
    pub line_attributes: ObjectID,
    /// 0 = top-left to bottom-right, 1 = bottom-left to top-right.
    pub line_direction: u8,
}

impl Default for OutputLineBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            line_attributes: ObjectID::NULL,
            line_direction: 0,
        }
    }
}

impl OutputLineBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.line_direction > 1 {
            return Err(Error::invalid_data("Output Line direction must be 0 or 1"));
        }
        // ISO 11783-6 order: line attributes, width, height, direction.
        let mut data = Vec::with_capacity(7);
        push_u16_le(&mut data, self.line_attributes);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.line_direction);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 7 {
            return Err(Error::invalid_data("Output Line body too short"));
        }
        if body[6] > 1 {
            return Err(Error::invalid_data(
                "Output Line body contains reserved direction value",
            ));
        }
        Ok(Self {
            line_attributes: u16_le_as(&body[0..]),
            width: u16_le(&body[2..]),
            height: u16_le(&body[4..]),
            line_direction: body[6],
        })
    }
}

/// Output Rectangle body (Type 14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputRectangleBody {
    pub width: u16,
    pub height: u16,
    pub line_attributes: ObjectID,
    /// Bits 0..=3 suppress top/right/bottom/left lines.
    pub line_suppression: u8,
    pub fill_attributes: ObjectID,
}

impl Default for OutputRectangleBody {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            line_attributes: ObjectID::NULL,
            line_suppression: 0,
            fill_attributes: ObjectID::NULL,
        }
    }
}

impl OutputRectangleBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.line_suppression & !0x0F != 0 {
            return Err(Error::invalid_data(
                "Output Rectangle line suppression contains reserved bits",
            ));
        }
        // ISO 11783-6 order: line attributes, width, height, suppression, fill.
        let mut data = Vec::with_capacity(9);
        push_u16_le(&mut data, self.line_attributes);
        push_u16_le(&mut data, self.width);
        push_u16_le(&mut data, self.height);
        data.push(self.line_suppression);
        push_u16_le(&mut data, self.fill_attributes);
        Ok(data)
    }

    pub fn decode(body: &[u8]) -> Result<Self> {
        if body.len() < 9 {
            return Err(Error::invalid_data("Output Rectangle body too short"));
        }
        if body[6] & !0x0F != 0 {
            return Err(Error::invalid_data(
                "Output Rectangle body contains reserved line-suppression bits",
            ));
        }
        Ok(Self {
            line_attributes: u16_le_as(&body[0..]),
            width: u16_le(&body[2..]),
            height: u16_le(&body[4..]),
            line_suppression: body[6],
            fill_attributes: u16_le_as(&body[7..]),
        })
    }
}

#[cfg(test)]
mod change_attribute_parity_tests {
    use super::*;

    #[test]
    fn value_enabled_options_attributes_are_settable_via_change_attribute() {
        // The value / enabled / options attribute IDs are settable via Change
        // Attribute, matching the reference VT stack (in addition to Change
        // Numeric Value / Enable-Disable Object).
        assert!(vt_change_attribute_id_is_supported(ObjectType::InputBoolean, 5)); // value
        assert!(vt_change_attribute_id_is_supported(ObjectType::InputBoolean, 6)); // enabled
        assert!(vt_change_attribute_id_is_supported(ObjectType::InputString, 9)); // enabled
        assert!(vt_change_attribute_id_is_supported(ObjectType::InputList, 4)); // value
        assert!(vt_change_attribute_id_is_supported(ObjectType::InputList, 5)); // options
        assert!(vt_change_attribute_id_is_supported(ObjectType::OutputList, 4)); // value
        assert!(vt_change_attribute_id_is_supported(ObjectType::Meter, 12)); // value
        assert!(vt_change_attribute_id_is_supported(
            ObjectType::LinearBarGraph,
            12
        )); // value

        // Out-of-range AIDs remain rejected.
        assert!(!vt_change_attribute_id_is_supported(ObjectType::InputBoolean, 7));
        assert!(!vt_change_attribute_id_is_supported(ObjectType::OutputList, 5));
        assert!(!vt_change_attribute_id_is_supported(ObjectType::Meter, 13));
        // The Arched Bar Graph has no separate value attribute (ends at AID 13).
        assert!(!vt_change_attribute_id_is_supported(
            ObjectType::ArchedBarGraph,
            14
        ));
    }
}
