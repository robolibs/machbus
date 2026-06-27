//! ISOBUS Object Pool (`.iop`) binary file parser.
//!
//! This is a thin compatibility surface over the conformant ISO 11783-6
//! object-pool codec in [`crate::isobus::vt::ObjectPool`]. It is the
//! single source of truth: [`parse_iop_data`] deserialises the real
//! object-pool wire format (`[id:2][type:1][body…]`, no per-object length
//! prefix, parse-by-type boundaries) and projects each object to a flat
//! [`RawIopObject`] for callers that only want id/type/raw-body triples
//! (FNV hashing, object counting, the C++-mirror demo).
//!
//! New code that needs the typed object graph, validation, or rendering
//! should use [`crate::isobus::vt::ObjectPool`] /
//! [`crate::isobus::vt::render::IopDocument`] directly.

use alloc::{collections::BTreeSet, format, string::String, vec::Vec};

#[cfg(feature = "default")]
use std::{fs, path::Path};

use super::error::{Error, ErrorCode, Result};

/// Parsed IOP object: id, type byte, and the raw post-header body bytes
/// (object-specific fields plus, for parent objects, the child/macro
/// tail) exactly as they appear on the wire after `[id:2][type:1]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawIopObject {
    pub id: u16,
    pub type_byte: u8,
    pub body: Vec<u8>,
}

/// Read an IOP file from disk. Returns the raw bytes.
#[cfg(feature = "default")]
pub fn read_iop_file(path: impl AsRef<Path>) -> Result<Vec<u8>> {
    let path = path.as_ref();
    let data = fs::read(path).map_err(|e| {
        Error::with_message(
            super::error::ErrorCode::DriverError,
            format!("failed to open IOP file {}: {e}", path.display()),
        )
    })?;
    if data.is_empty() {
        return Err(Error::with_message(
            super::error::ErrorCode::DriverError,
            "empty IOP file",
        ));
    }
    tracing::info!(
        target: "machbus.util.iop",
        path = %path.display(),
        bytes = data.len(),
        "IOP read",
    );
    Ok(data)
}

/// Parse an IOP buffer into flat id/type/body triples.
///
/// Walks the conformant ISO 11783-6 object-pool layout directly and returns
/// each object's raw on-wire body. Malformed buffers (empty, truncated,
/// unknown object type, duplicate/NULL id) are rejected rather than
/// prefix-decoded, because IOP files are object-pool artifacts, not streams.
pub fn parse_iop_data(data: &[u8]) -> Result<Vec<RawIopObject>> {
    if data.is_empty() {
        return Err(Error::invalid_data("IOP data is empty"));
    }
    let mut objects = Vec::new();
    let mut ids = BTreeSet::new();
    let mut offset = 0usize;

    while offset < data.len() {
        if data.len() - offset < 3 {
            return Err(pool_validation("object header extends past pool data"));
        }

        let id = u16::from_le_bytes([data[offset], data[offset + 1]]);
        if id == u16::MAX {
            return Err(pool_validation("object id 0xFFFF is reserved"));
        }
        if !ids.insert(id) {
            return Err(pool_validation(format!("duplicate VT object id {id}")));
        }

        let type_byte = data[offset + 2];
        if !is_known_object_type(type_byte) {
            return Err(pool_validation(format!(
                "unknown VT object type 0x{type_byte:02X}"
            )));
        }

        let body_off = offset + 3;
        let body_len = object_body_total_len(type_byte, data, body_off)?;
        let Some(body_end) = body_off.checked_add(body_len) else {
            return Err(pool_validation("object body length overflows pool data"));
        };
        if body_end > data.len() {
            return Err(pool_validation("object body extends past pool data"));
        }

        objects.push(RawIopObject {
            id,
            type_byte,
            body: data[body_off..body_end].to_vec(),
        });
        offset = body_end;
    }

    tracing::info!(
        target: "machbus.util.iop",
        count = objects.len(),
        "IOP parsed",
    );
    Ok(objects)
}

#[inline]
fn pool_validation(message: impl Into<String>) -> Error {
    Error::with_message(ErrorCode::PoolValidation, message)
}

#[inline]
#[must_use]
const fn is_known_object_type(type_byte: u8) -> bool {
    type_byte <= 50
}

/// `(child_list_offset, record_size)` for parent types. `record_size` is
/// 6 for positional parents and 2 for OID-only parents.
#[inline]
#[must_use]
const fn parent_layout(type_byte: u8) -> Option<(usize, usize)> {
    match type_byte {
        // Positional parents: DataMask, AlarmMask, Container, Key, Button,
        // Animation. WorkingSet and WindowMask have variable pre-child tails
        // and are handled before this table.
        1 => Some((3, 6)),
        2 => Some((5, 6)),
        3 => Some((5, 6)),
        5 => Some((2, 6)),
        6 => Some((8, 6)),
        44 => Some((12, 6)),
        // OID-only parents: SoftKeyMask, KeyGroup.
        4 => Some((1, 2)),
        35 => Some((5, 2)),
        _ => None,
    }
}

#[inline]
fn pool_walk_short() -> Error {
    pool_validation("object body extends past pool data")
}

#[inline]
fn peek_u8_at(data: &[u8], off: usize) -> Result<usize> {
    data.get(off)
        .map(|&b| b as usize)
        .ok_or_else(pool_walk_short)
}

#[inline]
fn peek_u16_at(data: &[u8], off: usize) -> Result<usize> {
    if off + 2 > data.len() {
        return Err(pool_walk_short());
    }
    Ok(u16::from_le_bytes([data[off], data[off + 1]]) as usize)
}

#[inline]
fn peek_u32_at(data: &[u8], off: usize) -> Result<usize> {
    if off + 4 > data.len() {
        return Err(pool_walk_short());
    }
    Ok(u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]) as usize)
}

#[inline]
fn checked_add_len(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or_else(|| pool_validation("object body length overflows pool data"))
}

#[inline]
fn checked_mul_len(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or_else(|| pool_validation("object body length overflows pool data"))
}

fn window_mask_child_list_offset(data: &[u8], off: usize) -> Result<usize> {
    let required_count = peek_u8_at(data, off + 11)?;
    checked_add_len(12, checked_mul_len(required_count, 2)?)
}

/// Total serialized body length of one object, given the raw pool `data` and
/// the absolute offset `off` of the first body byte.
fn object_body_total_len(type_byte: u8, data: &[u8], off: usize) -> Result<usize> {
    if type_byte == 0 {
        let num_objects = peek_u8_at(data, off + 4)?;
        let num_macros = peek_u8_at(data, off + 5)?;
        let num_languages = peek_u8_at(data, off + 6)?;
        return checked_add_len(
            checked_add_len(
                checked_add_len(7, checked_mul_len(num_objects, 6)?)?,
                checked_mul_len(num_macros, 2)?,
            )?,
            checked_mul_len(num_languages, 2)?,
        );
    }
    if type_byte == 34 {
        // ISO 11783-6 parent tail: both counts precede both lists.
        let fixed = window_mask_child_list_offset(data, off)?;
        let num_objects = peek_u8_at(data, off + fixed)?;
        let num_macros = peek_u8_at(data, off + fixed + 1)?;
        let child_bytes = checked_mul_len(num_objects, 6)?;
        let base = checked_add_len(fixed + 2, child_bytes)?;
        return checked_add_len(base, checked_mul_len(num_macros, 2)?);
    }
    if let Some((fixed, record_size)) = parent_layout(type_byte) {
        // ISO 11783-6 parent tail: both counts precede both lists.
        let num_objects = peek_u8_at(data, off + fixed)?;
        let num_macros = peek_u8_at(data, off + fixed + 1)?;
        let child_bytes = checked_mul_len(num_objects, record_size)?;
        let base = checked_add_len(fixed + 2, child_bytes)?;
        return checked_add_len(base, checked_mul_len(num_macros, 2)?);
    }

    if type_byte == 20 {
        // PictureGraphic: macro count precedes the pixel data, refs follow it.
        let data_len = peek_u32_at(data, off + 9)?;
        let num_macros = peek_u8_at(data, off + 13)?;
        return checked_add_len(
            checked_add_len(14, data_len)?,
            checked_mul_len(num_macros, 2)?,
        );
    }

    // Leaf objects: per-type body, then (for most) a trailing macro list.
    let body = leaf_body_only(type_byte, data, off)?;
    if leaf_has_macro_tail_byte(type_byte) {
        let num_macros = peek_u8_at(data, off + body)?;
        return checked_add_len(checked_add_len(body, 1)?, checked_mul_len(num_macros, 2)?);
    }
    Ok(body)
}

/// Whether a leaf object type carries the standard trailing
/// `[number of macros][macro refs]` list (mirrors the typed codec).
fn leaf_has_macro_tail_byte(type_byte: u8) -> bool {
    matches!(
        type_byte,
        7 | 8
            | 9
            | 10
            | 11
            | 12
            | 13
            | 14
            | 15
            | 16
            | 17
            | 18
            | 19
            | 23
            | 24
            | 25
            | 26
            | 33
            | 37
            | 48
    )
}

/// Length of a leaf object body excluding any trailing macro list.
fn leaf_body_only(type_byte: u8, data: &[u8], off: usize) -> Result<usize> {
    match type_byte {
        // Fixed-length leaves.
        7 => Ok(9),   // InputBoolean
        8 => Ok(14),  // InputString
        9 => Ok(34),  // InputNumber
        12 => Ok(25), // OutputNumber
        13 => Ok(7),  // Line
        14 => Ok(9),  // Rectangle
        15 => Ok(11), // Ellipse
        17 => Ok(17), // Meter
        18 => Ok(16), // LinearBarGraph
        19 => Ok(19), // ArchedBarGraph
        21 => Ok(4),  // NumberVariable
        23 => Ok(4),  // FontAttributes
        24 => Ok(4),  // LineAttributes
        25 => Ok(4),  // FillAttributes
        27 => Ok(2),  // ObjectPointer
        29 => Ok(4),  // AuxFunction
        30 => Ok(3),  // AuxInput
        31 => Ok(6),  // AuxFunction2
        32 => Ok(7),  // AuxInput2
        36 => Ok(31), // GraphicContext
        42 => Ok(9),  // ExternalReferenceName
        43 => Ok(6),  // ExternalObjectPointer
        48 => Ok(8),  // ScaledGraphic
        49 => Ok(16), // ScaledBitmap compatibility extension
        50 => Ok(12), // GraphicsContext compatibility extension

        // Variable-length leaves.
        10 => checked_add_len(9, checked_mul_len(peek_u8_at(data, off + 7)?, 2)?),
        11 => checked_add_len(13, peek_u16_at(data, off + 11)?),
        16 => checked_add_len(10, checked_mul_len(peek_u8_at(data, off + 9)?, 4)?),
        20 => checked_add_len(13, peek_u32_at(data, off + 9)?),
        22 => checked_add_len(2, peek_u16_at(data, off)?),
        26 => checked_add_len(2, peek_u8_at(data, off + 1)?),
        28 => checked_add_len(2, peek_u16_at(data, off)?),
        33 => checked_add_len(3, peek_u8_at(data, off + 2)?),
        37 => checked_add_len(8, checked_mul_len(peek_u8_at(data, off + 7)?, 2)?),
        38 => extended_input_attributes_len(data, off),
        39 => checked_add_len(2, peek_u16_at(data, off)?),
        40 => checked_add_len(2, checked_mul_len(peek_u16_at(data, off)?, 7)?),
        41 => checked_add_len(10, checked_mul_len(peek_u8_at(data, off + 9)?, 2)?),
        45 => checked_add_len(3, checked_mul_len(peek_u16_at(data, off + 1)?, 4)?),
        46 => checked_add_len(5, peek_u32_at(data, off + 1)?),
        47 => checked_add_len(2, peek_u16_at(data, off)?),

        // Parent types are handled above.
        0 | 1 | 2 | 3 | 4 | 5 | 6 | 34 | 35 | 44 => {
            unreachable!("parent types handled by parent_layout")
        }
        _ => Err(pool_validation(format!(
            "unknown VT object type 0x{type_byte:02X}"
        ))),
    }
}

fn extended_input_attributes_len(data: &[u8], off: usize) -> Result<usize> {
    let plane_count = peek_u8_at(data, off + 1)?;
    let mut rel = 2usize;
    for _ in 0..plane_count {
        let range_count = peek_u8_at(data, off + rel + 1)?;
        rel = checked_add_len(rel, 2)?;
        rel = checked_add_len(rel, checked_mul_len(range_count, 4)?)?;
        if off + rel > data.len() {
            return Err(pool_walk_short());
        }
    }
    Ok(rel)
}

/// `true` if `data` walks to a clean end without any truncated object.
/// An empty or short buffer (`< HEADER_SIZE` bytes) is considered
/// invalid (matches the C++ early return).
#[must_use]
pub fn validate(data: &[u8]) -> bool {
    parse_iop_data(data).is_ok()
}

/// FNV-1a hash of the IOP buffer, formatted as a 7-character version
/// string `A..=P` (one nibble per char). Bit-identical to the C++
/// implementation.
#[must_use]
pub fn hash_to_version(data: &[u8]) -> String {
    let mut hash: u32 = 2_166_136_261;
    for b in data {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    let mut s = String::with_capacity(7);
    for i in 0u8..7 {
        let nibble = ((hash >> (i * 4)) & 0x0F) as u8;
        s.push((b'A' + nibble) as char);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::isobus::vt::{
        AnimationBody, ColourPaletteBody, ContainerBody, ExtendedInputAttributesBody,
        ExtendedInputCodePlane, ExternalObjectDefinitionBody, ExternalObjectPointerBody,
        ExternalReferenceNameBody, GraphicContextBody, GraphicDataBody, MacroBody, MacroCommand,
        NumberVariableBody, ObjectID, ObjectPool, ObjectType, OutputStringBody, ScaledGraphicBody,
        WideCharRange, WindowMaskBody, WorkingSetBody, create_animation, create_colour_palette,
        create_container, create_extended_input_attributes, create_external_object_definition,
        create_external_object_pointer, create_external_reference_name, create_graphic_context,
        create_graphic_data, create_macro, create_number_variable, create_output_string,
        create_scaled_graphic, create_window_mask, create_working_set,
    };

    /// Build a conformant (ISO 11783-6, no per-object length prefix) IOP
    /// buffer carrying a Container and a NumberVariable.
    fn synthetic_iop() -> Vec<u8> {
        let container = create_container(
            1,
            &ContainerBody {
                width: 100,
                height: 200,
                hidden: false,
            },
        );
        let number = create_number_variable(2, &NumberVariableBody { value: 0xCAFE_F00D });
        ObjectPool::default()
            .with_object(container)
            .with_object(number)
            .serialize()
            .unwrap()
    }

    #[test]
    fn parses_two_objects() {
        let buf = synthetic_iop();
        let objs = parse_iop_data(&buf).unwrap();
        assert_eq!(objs.len(), 2);
        assert_eq!(objs[0].id, 1);
        assert_eq!(objs[0].type_byte, ObjectType::Container.as_u8());
        assert_eq!(objs[1].id, 2);
        assert_eq!(objs[1].type_byte, ObjectType::NumberVariable.as_u8());
        // NumberVariable body is the 4-byte value (no length prefix, no tail).
        assert_eq!(objs[1].body, vec![0x0D, 0xF0, 0xFE, 0xCA]);
    }

    #[test]
    fn parsed_bytes_reassemble_into_the_same_buffer() {
        // The flat projection is lossless: re-emitting `[id][type][body]`
        // for each object reproduces the original buffer.
        let buf = synthetic_iop();
        let objs = parse_iop_data(&buf).unwrap();
        let mut rebuilt = Vec::new();
        for o in &objs {
            rebuilt.extend_from_slice(&o.id.to_le_bytes());
            rebuilt.push(o.type_byte);
            rebuilt.extend_from_slice(&o.body);
        }
        assert_eq!(rebuilt, buf);
    }

    #[test]
    fn empty_buffer_errors() {
        assert!(parse_iop_data(&[]).is_err(), "empty IOP buffer must reject");
    }

    #[test]
    fn short_buffer_errors() {
        // 2 bytes is one short of the 3-byte object header.
        assert!(
            parse_iop_data(&[0u8; 2]).is_err(),
            "short header must reject"
        );
    }

    #[test]
    fn truncated_body_errors() {
        let mut buf = synthetic_iop();
        // Append a NumberVariable header that promises 4 value bytes but
        // delivers only 2.
        buf.extend_from_slice(&[0x03, 0x00]); // id
        buf.push(ObjectType::NumberVariable.as_u8());
        buf.extend_from_slice(&[0xAA, 0xBB]); // only 2 of 4 expected
        assert!(parse_iop_data(&buf).is_err(), "truncated body must reject");
    }

    #[test]
    fn trailing_partial_header_errors() {
        let mut buf = synthetic_iop();
        buf.extend_from_slice(&[0x03, 0x00]); // 2 bytes — no type
        assert!(
            parse_iop_data(&buf).is_err(),
            "trailing partial header must reject"
        );
    }

    #[test]
    fn unknown_object_type_errors() {
        let buf = [0x01, 0x00, 0xFE];
        assert!(parse_iop_data(&buf).is_err(), "unknown type must reject");
    }

    #[test]
    fn validate_accepts_well_formed() {
        assert!(validate(&synthetic_iop()));
    }

    #[test]
    fn validate_rejects_truncated_tail() {
        let mut buf = synthetic_iop();
        buf.extend_from_slice(&[0x03, 0x00]); // partial object
        assert!(!validate(&buf));
    }

    #[test]
    fn validate_rejects_too_short() {
        assert!(!validate(&[]));
        assert!(!validate(&[0u8; 2]));
    }

    #[test]
    fn output_string_handles_variable_length() {
        let obj = create_output_string(
            0x0010,
            &OutputStringBody {
                width: 50,
                height: 30,
                value: b"Hello".to_vec(),
                ..Default::default()
            },
        )
        .unwrap();
        let buf = ObjectPool::default().with_object(obj).serialize().unwrap();
        let objs = parse_iop_data(&buf).unwrap();
        assert_eq!(objs.len(), 1);
        assert_eq!(objs[0].id, 0x0010);
        assert_eq!(objs[0].type_byte, ObjectType::OutputString.as_u8());
        // OutputString body: fixed 13 bytes + 5 string bytes + 1 macro-count byte.
        assert_eq!(objs[0].body.len(), 19);
        assert_eq!(&objs[0].body[13..18], b"Hello");
    }

    #[test]
    fn macro_handles_variable_length() {
        let obj = create_macro(
            0x20,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: 0xA0,
                    parameters: vec![1, 2, 3, 4, 5],
                }],
            },
        );
        let buf = ObjectPool::default().with_object(obj).serialize().unwrap();
        let objs = parse_iop_data(&buf).unwrap();
        assert_eq!(objs.len(), 1);
        // Macro body: num_bytes(2) + one 6-byte command = 8 bytes.
        assert_eq!(objs[0].body.len(), 8);
    }

    #[test]
    fn parses_corrected_vt4_vt6_graphic_type_lengths() {
        let pool = ObjectPool::default()
            .with_object(
                create_extended_input_attributes(
                    0x30,
                    &ExtendedInputAttributesBody {
                        validation_type: 0,
                        code_planes: vec![ExtendedInputCodePlane {
                            plane: 1,
                            ranges: vec![WideCharRange {
                                first: 0xF600,
                                last: 0xF64F,
                            }],
                        }],
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_graphic_data(
                    0x31,
                    &GraphicDataBody {
                        format: 0,
                        data: vec![1, 2, 3],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_scaled_graphic(
                    0x32,
                    &ScaledGraphicBody {
                        width: 8,
                        height: 4,
                        scale_type: 3,
                        value: 0x31.into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            );
        let bytes = pool.serialize().unwrap();
        let objects = parse_iop_data(&bytes).unwrap();
        assert_eq!(
            objects
                .iter()
                .map(|o| (o.id, o.type_byte, o.body.len()))
                .collect::<Vec<_>>(),
            vec![
                (0x30, ObjectType::ExtendedInputAttributes.as_u8(), 8),
                (0x31, ObjectType::GraphicData.as_u8(), 8),
                (0x32, ObjectType::ScaledGraphic.as_u8(), 9),
            ]
        );
    }

    #[test]
    fn parser_lengths_match_standard_vt5_vt6_extended_objects() {
        let pool = ObjectPool::default()
            .with_object(
                create_working_set(
                    0x40,
                    &WorkingSetBody {
                        background_colour: 1,
                        selectable: 1,
                        active_mask: ObjectID(0x41),
                        languages: vec![*b"en"],
                    },
                )
                .with_children([0x41u16]),
            )
            .with_object(
                create_window_mask(
                    0x41,
                    &WindowMaskBody {
                        required_objects: vec![ObjectID(0x42), ObjectID(0x43)],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(create_graphic_context(0x42, &GraphicContextBody::default()).unwrap())
            .with_object(
                create_external_object_definition(
                    0x43,
                    &ExternalObjectDefinitionBody {
                        options: 1,
                        name0: 0x1122_3344,
                        name1: 0x5566_7788,
                        object_ids: vec![ObjectID(0x40), ObjectID(0x41)],
                    },
                )
                .unwrap(),
            )
            .with_object(create_external_reference_name(
                0x44,
                &ExternalReferenceNameBody {
                    options: 1,
                    name0: 0x0102_0304,
                    name1: 0x0506_0708,
                },
            ))
            .with_object(create_external_object_pointer(
                0x45,
                &ExternalObjectPointerBody {
                    default_object_id: ObjectID(0x41),
                    external_reference_name: ObjectID(0x44),
                    external_object_id: ObjectID(0x99),
                },
            ))
            .with_object(
                create_animation(
                    0x46,
                    &AnimationBody {
                        width: 8,
                        height: 8,
                        refresh_interval_ms: 100,
                        value: 0,
                        enabled: 0,
                        first_child_index: 0,
                        default_child_index: 0,
                        last_child_index: 0,
                        options: 0,
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_colour_palette(
                    0x47,
                    &ColourPaletteBody {
                        options: 0,
                        entries_argb: vec![0xFF_10_20_30, 0x80_40_50_60],
                    },
                )
                .unwrap(),
            );
        let bytes = pool.serialize().unwrap();
        let objects = parse_iop_data(&bytes).unwrap();
        assert_eq!(
            objects
                .iter()
                .map(|o| (o.id, o.type_byte, o.body.len()))
                .collect::<Vec<_>>(),
            vec![
                (0x40, ObjectType::WorkingSet.as_u8(), 15),
                (0x41, ObjectType::WindowMask.as_u8(), 18),
                (0x42, ObjectType::GraphicContext.as_u8(), 31),
                (0x43, ObjectType::ExternalObjectDefinition.as_u8(), 14),
                (0x44, ObjectType::ExternalReferenceName.as_u8(), 9),
                (0x45, ObjectType::ExternalObjectPointer.as_u8(), 6),
                (0x46, ObjectType::Animation.as_u8(), 14),
                (0x47, ObjectType::ColourPalette.as_u8(), 11),
            ]
        );
    }

    #[test]
    fn hash_to_version_is_seven_uppercase_chars_in_a_to_p() {
        let v = hash_to_version(b"hello world");
        assert_eq!(v.len(), 7);
        assert!(v.chars().all(|c| ('A'..='P').contains(&c)));
    }

    #[test]
    fn hash_to_version_is_deterministic() {
        let a = hash_to_version(b"sample IOP");
        let b = hash_to_version(b"sample IOP");
        assert_eq!(a, b);
    }

    #[test]
    fn hash_to_version_distinguishes_different_inputs() {
        let a = hash_to_version(b"foo");
        let b = hash_to_version(b"bar");
        assert_ne!(a, b);
    }

    #[test]
    fn hash_to_version_known_value_for_empty_buffer() {
        // FNV-1a offset basis = 0x811C9DC5. With no input the hash is
        // the offset basis verbatim. Nibbles emitted low-to-high are
        // 5, C, D, 9, C, 1, 1 → "FMNJMBB".
        assert_eq!(hash_to_version(b""), "FMNJMBB");
    }

    #[test]
    fn read_iop_file_round_trip_via_tempfile() {
        let path = std::env::temp_dir().join("machbus_iop_parser_test.bin");
        let want = synthetic_iop();
        std::fs::write(&path, &want).expect("write fixture");
        let got = read_iop_file(&path).expect("read");
        assert_eq!(got, want);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_iop_file_missing_path_errors() {
        let r = read_iop_file("/no/such/machbus/iop/file/xyz.bin");
        assert!(r.is_err());
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_iop_parser_accepts_or_rejects_arbitrary_bytes_without_panics(
            data in proptest::collection::vec(any::<u8>(), 0..=1024),
        ) {
            match parse_iop_data(&data) {
                Ok(objects) => {
                    prop_assert!(validate(&data));
                    // Each object is at least a 3-byte header plus body.
                    prop_assert!(objects.len() <= data.len() / 3);
                    let parsed_bytes: usize = objects.iter().map(|obj| 3 + obj.body.len()).sum();
                    prop_assert_eq!(parsed_bytes, data.len());
                    for obj in &objects {
                        prop_assert!(!obj.body.is_empty());
                    }
                }
                Err(_) => {
                    prop_assert!(!validate(&data));
                }
            }
        }

        #[test]
        fn proptest_hash_to_version_is_stable_shape(
            data in proptest::collection::vec(any::<u8>(), 0..=1024),
        ) {
            let version = hash_to_version(&data);
            prop_assert_eq!(version.len(), 7);
            prop_assert!(version.bytes().all(|b| (b'A'..=b'P').contains(&b)));
        }
    }
}
