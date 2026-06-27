/// Resolve an InputString's `input_attributes` reference into a runtime
/// character-set rule. A NULL reference, a missing object, a wrong object
/// type, or an undecodable body all yield `None` (no constraint).
///
/// ISO 11783-6 applies classic Input Attributes only to 8-bit strings and
/// Extended Input Attributes only to WideStrings. This renderer can only prove
/// the current encoding when the Input String references a String Variable; if
/// there is no referenced value in the object model, the attribute object is
/// retained as the intended rule instead of silently discarding it.
fn resolve_input_validation(
    pool: &ObjectPool,
    attr_ref: ObjectID,
    value_ref: ObjectID,
) -> Option<InputValidation> {
    if attr_ref == ObjectID::NULL {
        return None;
    }
    let value_is_wide = referenced_string_variable_is_wide(pool, value_ref);
    let obj = pool.find(attr_ref)?;
    match obj.r#type {
        ObjectType::InputAttributes => {
            if value_is_wide == Some(true) {
                return None;
            }
            let body = obj.get_input_attributes_body().ok()?;
            Some(InputValidation {
                // Validation type 0 = listed characters are valid (whitelist).
                allow_listed: body.validation_type == 0,
                byte_oriented: true,
                chars: body.validation_string,
                ranges: Vec::new(),
            })
        }
        ObjectType::ExtendedInputAttributes => {
            if value_is_wide == Some(false) {
                return None;
            }
            let body = obj.get_extended_input_attributes_body().ok()?;
            let mut ranges = Vec::new();
            for code_plane in body.code_planes {
                ranges.extend(
                    code_plane
                        .ranges
                        .into_iter()
                        .map(|range| InputValidationRange {
                            plane: code_plane.plane,
                            first: range.first,
                            last: range.last,
                        }),
                );
            }
            Some(InputValidation {
                // Validation type 0 = listed ranges are valid (whitelist).
                allow_listed: body.validation_type == 0,
                byte_oriented: false,
                chars: Vec::new(),
                ranges,
            })
        }
        _ => None,
    }
}

fn referenced_string_variable_is_wide(pool: &ObjectPool, value_ref: ObjectID) -> Option<bool> {
    if value_ref == ObjectID::NULL {
        return None;
    }
    let obj = pool.find(value_ref)?;
    if obj.r#type != ObjectType::StringVariable {
        return None;
    }
    obj.get_string_variable_body()
        .ok()
        .map(|body| text::is_wide_string(&body.value))
}

/// Compact carrier for the fields a node inherits from its placement.
struct BaseNode {
    id: ObjectID,
    object_type: ObjectType,
    parent: ObjectID,
    x: i32,
    y: i32,
}

/// Mutable recursion state shared while building child scene nodes.
struct BuildState<'a> {
    scene: &'a mut Scene,
    path: &'a mut Vec<ObjectID>,
    clip: Option<Rect>,
    visible: bool,
}

/// Return a parent object's children as positional refs, preferring the
/// real ISO 11783-6 positions stored on [`VTObject::children_pos`] and
/// falling back to the legacy position-less list at the origin.
fn positional_children(obj: &VTObject) -> Vec<crate::isobus::vt::ChildRef> {
    if !obj.children_pos.is_empty() {
        obj.children_pos.clone()
    } else {
        obj.children
            .iter()
            .map(|&id| crate::isobus::vt::ChildRef::at_origin(id))
            .collect()
    }
}

fn i32_to_i16_saturating(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// Decode just the background colour of a mask object (used when a mask
/// appears as a nested child). Returns `(background, soft_key_mask)`.
fn decode_mask_background(obj: &VTObject) -> (u8, ObjectID, bool) {
    match obj.r#type {
        ObjectType::DataMask => obj
            .get_data_mask_body()
            .map(|b| (b.background_color, b.soft_key_mask, false))
            .unwrap_or((0, ObjectID::NULL, false)),
        ObjectType::AlarmMask => obj
            .get_alarm_mask_body()
            .map(|b| (b.background_color, b.soft_key_mask, false))
            .unwrap_or((0, ObjectID::NULL, false)),
        ObjectType::WindowMask => obj
            .get_window_mask_body()
            .map(|b| (b.background_color, ObjectID::NULL, b.options & 0x02 != 0))
            .unwrap_or((0, ObjectID::NULL, false)),
        _ => (0, ObjectID::NULL, false),
    }
}

/// Format a raw `u32` value using the ISO 11783-6 output/input number
/// transform: `displayed = (value + offset) * scale`.
///
/// `format`: 0 = fixed decimal, 1 = exponential. Non-standard values are
/// treated as fixed decimal defensively; standard object-pool and Change
/// Attribute admission reject them before normal render use.
///
/// `options`: bit 2 = display zero as blank; bit 3 = truncate instead of
/// round to the requested number of decimals. Bit 1 pads fixed decimal output
/// with leading zeroes up to the visible text-cell width.
fn format_number(
    raw: u32,
    offset: i32,
    scale: f32,
    decimals: u8,
    format: u8,
    options: u8,
    max_chars: Option<usize>,
) -> String {
    let displayed = (f64::from(raw) + f64::from(offset)) * f64::from(scale);
    if options & 0x04 != 0 && displayed == 0.0 {
        return String::new();
    }
    match format {
        1 => {
            format!("{displayed:.precision$e}", precision = decimals as usize)
        }
        _ => {
            let decimals = decimals.min(7);
            let factor = 10i64.pow(decimals as u32);
            let scaled = if options & 0x08 != 0 {
                (displayed * factor as f64).trunc()
            } else {
                (displayed * factor as f64).round()
            } as i64;
            let text = if decimals == 0 {
                format!("{scaled}")
            } else {
                let sign = if scaled < 0 { "-" } else { "" };
                let absolute = scaled.unsigned_abs();
                let factor = factor as u64;
                let whole = absolute / factor;
                let frac = absolute % factor;
                format!("{sign}{whole}.{frac:0width$}", width = decimals as usize)
            };
            if options & 0x02 != 0 {
                pad_numeric_with_leading_zeroes(text, max_chars)
            } else {
                text
            }
        }
    }
}

fn pad_numeric_with_leading_zeroes(text: String, max_chars: Option<usize>) -> String {
    let Some(max_chars) = max_chars else {
        return text;
    };
    let width = text.chars().count();
    if width >= max_chars {
        return text;
    }
    let zero_count = max_chars - width;
    let mut out = String::with_capacity(text.len().saturating_add(zero_count));
    if let Some(rest) = text.strip_prefix('-') {
        out.push('-');
        for _ in 0..zero_count {
            out.push('0');
        }
        out.push_str(rest);
    } else {
        for _ in 0..zero_count {
            out.push('0');
        }
        out.push_str(&text);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::vt::{
        AlarmMaskBody, ContainerBody, DataMaskBody, ObjectPool, OutputStringBody, WorkingSetBody,
        create_alarm_mask, create_container, create_data_mask, create_output_string,
        create_working_set,
    };

    fn pool_with_data_mask() -> ObjectPool {
        let os = create_output_string(2, &OutputStringBody::default()).unwrap();
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
            .with_object(os.with_id(3).with_type(ObjectType::OutputString))
    }

    #[test]
    fn null_active_mask_resolves_to_first_working_set_child() {
        let pool = pool_with_data_mask();
        let engine = LayoutEngine::new(LayoutConfig::default());
        let scene = engine.build(&pool, ObjectID::NULL);
        assert_eq!(scene.active_mask, ObjectID::new(2));
        assert!(!scene.is_empty());
    }

    #[test]
    fn explicit_active_mask_is_used_when_it_is_a_mask() {
        let pool = ObjectPool::default()
            .with_object(
                create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 3u16]),
            )
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_alarm_mask(3, &AlarmMaskBody::default()).unwrap());
        let engine = LayoutEngine::new(LayoutConfig::default());
        let scene = engine.build(&pool, ObjectID::new(3));
        assert_eq!(scene.active_mask, ObjectID::new(3));
    }

    #[test]
    fn input_string_resolves_input_attributes_validation_from_pool() {
        use crate::isobus::vt::{
            InputAttributesBody, InputStringBody, create_input_attributes, create_input_string,
        };
        let input = create_input_string(
            4,
            &InputStringBody {
                width: 80,
                height: 16,
                input_attributes: 5.into(),
                ..Default::default()
            },
        )
        .unwrap();
        let attrs = create_input_attributes(
            5,
            &InputAttributesBody {
                validation_type: 0, // whitelist
                validation_string: b"ABC".to_vec(),
            },
        )
        .unwrap();
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
            .with_object(input)
            .with_object(attrs);

        let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID::NULL);
        let node = scene
            .find(ObjectID::new(4))
            .expect("input string is placed");
        match &node.kind {
            NodeKind::InputString { validation, .. } => {
                let rule = validation.as_ref().expect("validation resolved from pool");
                assert!(rule.allow_listed);
                assert!(rule.byte_oriented);
                assert_eq!(rule.chars, b"ABC");
                assert!(rule.accepts('A'));
                assert!(!rule.accepts('Z'));
                assert!(!rule.accepts('😀'));
            }
            other => panic!("expected InputString node, got {other:?}"),
        }
    }

    #[test]
    fn input_string_classic_blacklist_rejects_listed_and_non_byte_characters() {
        use crate::isobus::vt::{
            InputAttributesBody, InputStringBody, create_input_attributes, create_input_string,
        };
        let input = create_input_string(
            4,
            &InputStringBody {
                width: 80,
                height: 16,
                input_attributes: 5.into(),
                ..Default::default()
            },
        )
        .unwrap();
        let attrs = create_input_attributes(
            5,
            &InputAttributesBody {
                validation_type: 1, // blacklist
                validation_string: b"X".to_vec(),
            },
        )
        .unwrap();
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
            .with_object(input)
            .with_object(attrs);

        let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID::NULL);
        let node = scene
            .find(ObjectID::new(4))
            .expect("input string is placed");
        match &node.kind {
            NodeKind::InputString { validation, .. } => {
                let rule = validation.as_ref().expect("validation resolved from pool");
                assert!(!rule.allow_listed);
                assert!(rule.byte_oriented);
                assert!(rule.accepts('A'));
                assert!(!rule.accepts('X'));
                assert!(
                    !rule.accepts('😀'),
                    "classic 8-bit Input Attributes must not admit wide characters"
                );
            }
            other => panic!("expected InputString node, got {other:?}"),
        }
    }

    #[test]
    fn input_string_resolves_extended_input_attributes_validation_from_pool() {
        use crate::isobus::vt::{
            ExtendedInputAttributesBody, ExtendedInputCodePlane, InputStringBody, WideCharRange,
            create_extended_input_attributes, create_input_string,
        };
        let input = create_input_string(
            4,
            &InputStringBody {
                width: 80,
                height: 16,
                input_attributes: 5.into(),
                ..Default::default()
            },
        )
        .unwrap();
        let attrs = create_extended_input_attributes(
            5,
            &ExtendedInputAttributesBody {
                validation_type: 0, // whitelist
                code_planes: vec![
                    ExtendedInputCodePlane {
                        plane: 0,
                        ranges: vec![WideCharRange {
                            first: 0x00E0,
                            last: 0x00FF,
                        }],
                    },
                    ExtendedInputCodePlane {
                        plane: 1,
                        ranges: vec![WideCharRange {
                            first: 0xF600,
                            last: 0xF64F,
                        }],
                    },
                ],
            },
        )
        .unwrap();
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
            .with_object(input)
            .with_object(attrs);

        let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID::NULL);
        let node = scene
            .find(ObjectID::new(4))
            .expect("input string is placed");
        match &node.kind {
            NodeKind::InputString { validation, .. } => {
                let rule = validation
                    .as_ref()
                    .expect("extended validation resolved from pool");
                assert!(rule.allow_listed);
                assert!(!rule.byte_oriented);
                assert!(rule.chars.is_empty());
                assert_eq!(rule.ranges.len(), 2);
                assert!(rule.accepts('é'));
                assert!(rule.accepts('😀'));
                assert!(!rule.accepts('A'));
            }
            other => panic!("expected InputString node, got {other:?}"),
        }
    }

    #[test]
    fn input_string_resolves_extended_input_attributes_blacklist_from_pool() {
        use crate::isobus::vt::{
            ExtendedInputAttributesBody, ExtendedInputCodePlane, InputStringBody, WideCharRange,
            create_extended_input_attributes, create_input_string,
        };
        let input = create_input_string(
            4,
            &InputStringBody {
                width: 80,
                height: 16,
                input_attributes: 5.into(),
                ..Default::default()
            },
        )
        .unwrap();
        let attrs = create_extended_input_attributes(
            5,
            &ExtendedInputAttributesBody {
                validation_type: 1, // blacklist
                code_planes: vec![ExtendedInputCodePlane {
                    plane: 0,
                    ranges: vec![WideCharRange {
                        first: 0x00E9,
                        last: 0x00E9,
                    }],
                }],
            },
        )
        .unwrap();
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
            .with_object(input)
            .with_object(attrs);

        let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID::NULL);
        let node = scene
            .find(ObjectID::new(4))
            .expect("input string is placed");
        match &node.kind {
            NodeKind::InputString { validation, .. } => {
                let rule = validation
                    .as_ref()
                    .expect("extended validation resolved from pool");
                assert!(!rule.allow_listed);
                assert!(!rule.byte_oriented);
                assert!(rule.accepts('A'));
                assert!(rule.accepts('😀'));
                assert!(!rule.accepts('é'));
            }
            other => panic!("expected InputString node, got {other:?}"),
        }
    }

    #[test]
    fn input_string_validation_respects_string_encoding_kind() {
        use crate::isobus::vt::{
            ExtendedInputAttributesBody, ExtendedInputCodePlane, InputAttributesBody,
            InputStringBody, StringVariableBody, WideCharRange, create_extended_input_attributes,
            create_input_attributes, create_input_string, create_string_variable,
        };

        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(
                create_data_mask(2, &DataMaskBody::default()).with_children([4u16, 14u16, 24u16]),
            )
            .with_object(
                create_input_string(
                    4,
                    &InputStringBody {
                        input_attributes: 5.into(),
                        variable_reference: 6.into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_extended_input_attributes(
                    5,
                    &ExtendedInputAttributesBody {
                        validation_type: 0,
                        code_planes: vec![ExtendedInputCodePlane {
                            plane: 0,
                            ranges: vec![WideCharRange {
                                first: 0x00E0,
                                last: 0x00FF,
                            }],
                        }],
                    },
                )
                .unwrap(),
            )
            .with_object(create_string_variable(
                6,
                &StringVariableBody {
                    length: 3,
                    value: b"abc".to_vec(),
                },
            ))
            .with_object(
                create_input_string(
                    14,
                    &InputStringBody {
                        input_attributes: 15.into(),
                        variable_reference: 16.into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_attributes(
                    15,
                    &InputAttributesBody {
                        validation_type: 0,
                        validation_string: b"A".to_vec(),
                    },
                )
                .unwrap(),
            )
            .with_object(create_string_variable(
                16,
                &StringVariableBody {
                    length: 4,
                    value: vec![0xFF, 0xFE, 0x41, 0x00],
                },
            ))
            .with_object(
                create_input_string(
                    24,
                    &InputStringBody {
                        input_attributes: 5.into(),
                        variable_reference: 16.into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            );

        let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID::NULL);
        assert!(
            matches!(
                scene.find(ObjectID::new(4)).map(|node| &node.kind),
                Some(NodeKind::InputString {
                    validation: None,
                    ..
                })
            ),
            "Extended Input Attributes must not validate an 8-bit String Variable"
        );
        assert!(
            matches!(
                scene.find(ObjectID::new(14)).map(|node| &node.kind),
                Some(NodeKind::InputString {
                    validation: None,
                    ..
                })
            ),
            "classic Input Attributes must not validate a WideString Variable"
        );
        match scene.find(ObjectID::new(24)).map(|node| &node.kind) {
            Some(NodeKind::InputString {
                text, validation, ..
            }) => {
                assert_eq!(text, "A");
                let rule = validation
                    .as_ref()
                    .expect("wide string uses extended attrs");
                assert!(rule.accepts('é'));
                assert!(!rule.accepts('A'));
            }
            other => panic!("expected wide InputString, got {other:?}"),
        }
    }

    #[test]
    fn runtime_overrides_apply_and_survive_rebuild_from_macro_report() {
        use crate::isobus::vt::render::macros::{MacroEffect, apply_macro_effects};
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
            .with_object(create_container(4, &ContainerBody::default()).with_children([3u16]))
            .with_object(create_output_string(3, &OutputStringBody::default()).unwrap());

        // Baseline: the output string node is visible.
        let base = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID::NULL);
        assert!(base.find(ObjectID::new(3)).unwrap().visible);

        // A macro hides container 4; fold its report into runtime overrides.
        let mut pool_mut = pool.clone();
        let report = apply_macro_effects(
            &mut pool_mut,
            &[MacroEffect::HideShow {
                object: ObjectID::new(4),
                show: false,
            }],
        );
        let mut overrides = RuntimeOverrides::new();
        overrides.apply_report(&report);
        assert!(!overrides.is_empty());

        // Rebuilding with the overrides keeps child object 3 hidden through its parent.
        let scene = LayoutEngine::new(LayoutConfig::default())
            .with_overrides(overrides)
            .build(&pool, ObjectID::NULL);
        assert!(!scene.find(ObjectID::new(3)).unwrap().visible);
    }

    #[test]
    fn colour_palette_object_overrides_the_render_palette() {
        use crate::isobus::vt::{ColourPaletteBody, create_colour_palette};
        let palette_obj = create_colour_palette(
            5,
            &ColourPaletteBody {
                options: 0,
                entries_argb: vec![0xFF_10_20_30, 0x80_40_50_60],
            },
        )
        .unwrap();
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(palette_obj);

        let palette = LayoutEngine::new(LayoutConfig::default()).effective_palette(&pool);
        assert_eq!(
            palette.resolve(0),
            crate::isobus::vt::render::style::Colour::rgb(0x10, 0x20, 0x30)
        );
        assert_eq!(
            palette.resolve(1),
            crate::isobus::vt::render::style::Colour::rgb(0x40, 0x50, 0x60)
        );
    }

    #[test]
    fn format_number_decimal_and_exp() {
        // scale=1.0 is identity; decimals add fractional places.
        assert_eq!(format_number(5, 0, 1.0, 0, 0, 0, None), "5");
        assert_eq!(format_number(5, 0, 1.0, 1, 0, 0, None), "5.0");
        assert_eq!(format_number(5, 0, 1.0, 2, 0, 0, None), "5.00");
        assert!(format_number(12345, 0, 1.0, 0, 1, 0, None).contains('e'));
    }

    #[test]
    fn format_number_applies_offset_and_scale() {
        // displayed = (value + offset) * scale (ISO 11783-6).
        // raw=10, offset=2, scale=2.0 -> (10+2)*2 = 24
        assert_eq!(format_number(10, 2, 2.0, 0, 0, 0, None), "24");
        // scale=0.25 with decimals: (8+0)*0.25 = 2.00
        assert_eq!(format_number(8, 0, 0.25, 2, 0, 0, None), "2.00");
        // Fractional scale: raw=3, scale=0.1 -> 0.3 -> "0.3"
        assert_eq!(format_number(3, 0, 0.1, 1, 0, 0, None), "0.3");
    }

    #[test]
    fn format_number_honours_zero_blank_and_truncate_option_bits() {
        assert_eq!(format_number(0, 0, 1.0, 2, 0, 0x04, None), "");
        assert_eq!(format_number(234, 0, 0.01, 1, 0, 0, None), "2.3");
        assert_eq!(format_number(239, 0, 0.01, 1, 0, 0, None), "2.4");
        assert_eq!(format_number(239, 0, 0.01, 1, 0, 0x08, None), "2.3");
        assert_eq!(format_number(42, 0, 1.0, 0, 0, 0x02, Some(5)), "00042");
        assert_eq!(format_number(42, -100, 1.0, 0, 0, 0x02, Some(5)), "-0058");
    }

    #[test]
    fn placement_map_get_set() {
        let m = PlacementMap::new().set(7u16, 10, 20);
        assert_eq!(m.get(ObjectID::new(7)), Some((10, 20)));
        assert_eq!(m.get(ObjectID::new(8)), None);
    }

    #[test]
    fn placement_map_supports_negative_coords() {
        // ISO 11783-6 child locations are signed; negative offsets are legal.
        let m = PlacementMap::new().set(7u16, -5, -10);
        assert_eq!(m.get(ObjectID::new(7)), Some((-5, -10)));
    }

    #[test]
    fn layout_never_panics_on_empty_pool() {
        let pool = ObjectPool::default();
        let engine = LayoutEngine::new(LayoutConfig::default());
        let scene = engine.build(&pool, ObjectID::NULL);
        assert!(scene.nodes.is_empty());
        assert!(!scene.unsupported.is_empty());
    }

    #[test]
    fn layout_auto_stacks_children_vertically() {
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(
                create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16]),
            )
            .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
            .with_object(create_output_string(4, &OutputStringBody::default()).unwrap())
            .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());
        let engine = LayoutEngine::new(LayoutConfig {
            auto_layout_gap: 8,
            ..LayoutConfig::default()
        });
        let scene = engine.build(&pool, ObjectID::NULL);
        let ys: Vec<i32> = scene.nodes.iter().map(|n| n.rect.y).collect();
        assert_eq!(ys.len(), 3);
        // Auto-stack advances by the gap each time.
        assert!(ys[1] >= ys[0] + 8);
        assert!(ys[2] >= ys[1] + 8);
    }
}
