fn runtime_draws_text(runtime: &VtRenderRuntime, expected: &str) -> bool {
    runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == expected),
    )
}

#[test]
fn render_runtime_change_list_item_updates_input_list_selected_text() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_number_variable(9, &NumberVariableBody { value: 1 }))
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    value: b"OLD".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"NEW".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(9),
                    value: 255,
                    options: 0x01,
                    items: vec![ObjectID::NULL, ObjectID::new(4)],
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(runtime_draws_text(&runtime, "OLD"));
    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeListItem {
                list: ObjectID::new(3),
                index: 1,
                item: ObjectID::new(5),
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::InputList {
            selected: 1,
            selected_text,
            ..
        } if selected_text.as_deref() == Some("NEW")
    ));
    assert!(runtime_draws_text(&runtime, "NEW"));
}

#[test]
fn render_runtime_rejects_invalid_numeric_scalar_and_pointer_values() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"text".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(6, &FontAttributesBody::default()))
        .with_object(
            create_picture_graphic(
                23,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            19,
            &ObjectPointerBody {
                value: ObjectID::new(5),
            },
        ))
        .with_object(create_object_pointer(
            44,
            &ObjectPointerBody {
                value: ObjectID::new(23),
            },
        ))
        .with_object(
            create_input_boolean(
                39,
                &InputBooleanBody {
                    foreground: ObjectID::new(6),
                    value: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                42,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0x04,
                    transparency: 0xFF,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                38,
                &ScaledGraphicBody {
                    value: ObjectID::new(23),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                45,
                &ScaledGraphicBody {
                    value: ObjectID::new(44),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_animation(32, &AnimationBody::default())
                .unwrap()
                .with_children([5u16, 23u16]),
        )
        .with_object(create_external_reference_name(
            36,
            &ExternalReferenceNameBody {
                options: 1,
                name0: 0x1122_3344,
                name1: 0x5566_7788,
            },
        ))
        .with_object(create_external_object_pointer(
            37,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(5),
                external_reference_name: ObjectID::new(36),
                external_object_id: ObjectID::new(77),
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for (id, value) in [
        (39, 2),
        (19, 0x7777),
        (38, 5),
        (38, 19),
        (44, 5),
        (37, (88_u32 << 16) | u32::from(5_u16)),
        (32, 2),
    ] {
        assert_eq!(
            runtime
                .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                    id: ObjectID::new(id),
                    value,
                })
                .unwrap(),
            RenderUpdate::Unchanged,
            "invalid direct runtime Change Numeric Value for object {id} must be ignored"
        );
    }

    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(2))
            .unwrap()
            .get_data_mask_body()
            .unwrap()
            .background_color,
        0,
        "one-byte Change Attribute values must reject non-zero upper bytes instead of truncating"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(39))
            .unwrap()
            .get_input_boolean_body()
            .unwrap()
            .value,
        0
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(19))
            .unwrap()
            .get_object_pointer_body()
            .unwrap()
            .value,
        ObjectID::new(5)
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(38))
            .unwrap()
            .get_scaled_graphic_body()
            .unwrap()
            .value,
        ObjectID::new(23)
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(44))
            .unwrap()
            .get_object_pointer_body()
            .unwrap()
            .value,
        ObjectID::new(23),
        "ObjectPointer values reached by ScaledGraphic must not retarget to non-graphic objects"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(32))
            .unwrap()
            .get_animation_body()
            .unwrap()
            .value,
        0,
        "Animation Change Numeric Value must reject child indices outside the positional child list"
    );
    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                id: ObjectID::new(32),
                value: u32::from(u8::MAX),
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(32))
            .unwrap()
            .get_animation_body()
            .unwrap()
            .value,
        u8::MAX,
        "Animation Value 255 remains the standard no-selected-item numeric value"
    );
    let external_pointer = runtime
        .pool()
        .find(ObjectID::new(37))
        .unwrap()
        .get_external_object_pointer_body()
        .unwrap();
    assert_eq!(external_pointer.external_reference_name, ObjectID::new(36));
    assert_eq!(external_pointer.external_object_id, ObjectID::new(77));
}

#[test]
fn render_runtime_rejects_numeric_value_width_overflow_before_truncation() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"text".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                23,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_animation(32, &AnimationBody::default())
                .unwrap()
                .with_children([5u16, 23u16]),
        )
        .with_object(
            create_picture_graphic(
                42,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            19,
            &ObjectPointerBody {
                value: ObjectID::new(23),
            },
        ))
        .with_object(
            create_output_list(
                20,
                &OutputListBody {
                    items: vec![ObjectID::new(5)],
                    value: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_meter(
                29,
                &MeterBody {
                    value: 5,
                    min_value: 0,
                    max_value: 100,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                38,
                &ScaledGraphicBody {
                    value: ObjectID::new(23),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                46,
                &InputListBody {
                    items: vec![ObjectID::new(5)],
                    variable_reference: ObjectID::new(47),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_number_variable(47, &NumberVariableBody { value: 0 }));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for (id, value) in [(19, 42), (20, 0), (29, 6), (38, 42), (46, 1)] {
        assert!(matches!(
            runtime
                .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                    id: ObjectID::new(id),
                    value,
                })
                .unwrap(),
            RenderUpdate::SceneRebuilt { .. }
        ));
    }

    for (id, value) in [
        (19, 0x0001_0017), // Object Pointer is a two-byte numeric target.
        (20, 0x0000_0101), // Output List selected index is a one-byte numeric target.
        (29, 0x0001_0007), // Meter value is a two-byte numeric target.
        (38, 0x0001_0017), // Scaled Graphic value source is a two-byte numeric target.
        (46, 0x0000_0102), // Input List indirect numeric value is still one byte.
    ] {
        assert_eq!(
            runtime
                .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                    id: ObjectID::new(id),
                    value,
                })
                .unwrap(),
            RenderUpdate::Unchanged,
            "direct runtime Change Numeric Value for object {id} must reject non-zero upper bytes before truncation"
        );
    }

    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(19))
            .unwrap()
            .get_object_pointer_body()
            .unwrap()
            .value,
        ObjectID::new(42)
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(20))
            .unwrap()
            .get_output_list_body()
            .unwrap()
            .value,
        0
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(29))
            .unwrap()
            .get_meter_body()
            .unwrap()
            .value,
        6
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(38))
            .unwrap()
            .get_scaled_graphic_body()
            .unwrap()
            .value,
        ObjectID::new(42)
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(47))
            .unwrap()
            .get_number_variable_body()
            .unwrap()
            .value,
        1,
        "Input List variable-reference updates must also reject high bytes before mutating the backing NumberVariable"
    );
}

#[test]
fn render_runtime_rejects_invalid_generic_attribute_scalar_and_reference_values() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    font_attributes: ObjectID::new(6),
                    value: b"text".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(6, &FontAttributesBody::default()))
        .with_object(
            create_output_number(
                44,
                &OutputNumberBody {
                    scale: 1.0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_number(
                45,
                &InputNumberBody {
                    min_value: 0,
                    max_value: 100,
                    scale: 1.0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                49,
                &InputListBody {
                    value: 1,
                    items: vec![ObjectID::new(5)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_list(
                50,
                &OutputListBody {
                    value: 1,
                    items: vec![ObjectID::new(5)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_line_attributes(7, &LineAttributesBody::default()))
        .with_object(create_fill_attributes(8, &FillAttributesBody::default()).unwrap())
        .with_object(create_container(26, &ContainerBody::default()))
        .with_object(
            create_window_mask(
                41,
                &WindowMaskBody {
                    width_cells: 2,
                    height_cells: 6,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_palette(
                13,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_00_00_00],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                23,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                38,
                &ScaledGraphicBody {
                    value: ObjectID::new(23),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                42,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0x04,
                    transparency: 0xFF,
                    data: vec![1],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_context(
                40,
                &GraphicContextBody {
                    viewport_width: 10,
                    canvas_width: 10,
                    viewport_zoom_raw: 1.0_f32.to_bits(),
                    font_attributes: ObjectID::new(6),
                    line_attributes: ObjectID::new(7),
                    fill_attributes: ObjectID::new(8),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_boolean(
                39,
                &InputBooleanBody {
                    value: 1,
                    enabled: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_alarm_mask(
                46,
                &AlarmMaskBody {
                    acoustic_signal: 3,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_key_group(
                47,
                &KeyGroupBody {
                    options: 1,
                    name: ObjectID::new(5),
                    key_group_icon: ObjectID::new(23),
                },
            )
            .with_children([48u16]),
        )
        .with_object(create_key(48, &KeyBody::default()))
        .with_object(create_external_reference_name(
            36,
            &ExternalReferenceNameBody {
                options: 1,
                name0: 0x1122_3344,
                name1: 0x5566_7788,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(7),
                attribute_id: 3,
                value: 0x55AA,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(7))
            .unwrap()
            .get_line_attributes_body()
            .unwrap()
            .line_art,
        0x55AA,
        "Line Attributes line-art AID is a two-byte pattern, not a one-byte scalar"
    );
    for (id, attribute_id, value) in [
        (2, 1, 0x0100),              // one-byte Data Mask background colour must not truncate.
        (5, 4, 5),                   // Output String font reference must point to FontAttributes.
        (44, 8, f32::NAN.to_bits()), // Output Number scale must be finite.
        (44, 9, 8),                  // Output Number decimals admit only the standard 0..=7 range.
        (45, 11, 8),                 // Input Number decimals admit only the standard 0..=7 range.
        (26, 3, 2), // Container hidden state is read-only for Change Attribute.
        (46, 4, 4),                  // Alarm Mask acoustic signal admits only 0..=3.
        (47, 2, 36), // Key Group name must be OutputString/ObjectPointer-to-OutputString.
        (47, 3, 36), // Key Group icon must be an Object Label graphic representation.
        (6, 2, 15),  // Font Attributes non-proportional size is limited to standard enum 0..=14.
        (6, 3, 3),   // Font Attributes font type 3 is reserved.
        (6, 4, 0x80), // Proportional style requires a proportional font-size value.
        (7, 3, 0x0001_55AA), // Line Attributes line art is two bytes, not a truncating u32.
        (8, 3, 5),   // Fill Attributes pattern must point to PictureGraphic.
        (8, 3, 0x0001_0017), // Fill Attributes pattern is two bytes, not a truncating u32.
        (41, 1, 0),  // Window Mask width is 1..=2 user-layout columns.
        (41, 2, 7),  // Window Mask height is 1..=6 user-layout rows.
        (41, 3, 19), // Window Mask type admits only the standard 0..=18 range.
        (41, 3, 1),  // Window Mask type changes must match the retained required-object list.
        (41, 5, 0x04), // Window Mask options reserves bits 2..=7.
        (41, 6, 36), // Window Mask name must be OutputString/ObjectPointer-to-OutputString.
        (41, 7, 36), // Window Mask title must be OutputString/ObjectPointer-to-OutputString.
        (41, 8, 36), // Window Mask icon must be an Object Label graphic representation.
        (13, 1, 1),  // Colour Palette options is reserved to zero.
        (23, 2, 0x08), // Picture Graphic options reserves bits 3..=7.
        (38, 3, 5),  // Scaled Graphic value source must be graphic-like.
        (32, 4, 2),  // Animation selected child index must exist or be 255.
        (32, 6, 2),  // Animation first child index must exist and not exceed current last.
        (32, 7, 2),  // Animation default child index must exist.
        (32, 8, 2),  // Animation last child index must exist and follow current first.
        (36, 1, 2),  // External Reference NAME options only admits enabled bit.
        (40, 1, 32768), // Graphic Context dimensions are limited to signed 15-bit.
        (40, 3, 0x0001_0000), // Graphic Context viewport position is a two-byte signed field.
        (40, 8, 0x0001_0000), // Graphic Context cursor position is a two-byte signed field.
        (40, 7, f32::NAN.to_bits()), // Graphic Context zoom must be finite/in-range.
        (40, 14, 5), // Graphic Context fill attributes must point to FillAttributes.
    ] {
        assert_eq!(
            runtime
                .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                    id: ObjectID::new(id),
                    attribute_id,
                    value,
                })
                .unwrap(),
            RenderUpdate::Unchanged,
            "invalid direct runtime Change Generic Attribute for object {id} AID {attribute_id} must be ignored"
        );
    }

    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(5))
            .unwrap()
            .get_output_string_body()
            .unwrap()
            .font_attributes,
        ObjectID::new(6)
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(44))
            .unwrap()
            .get_output_number_body()
            .unwrap()
            .scale,
        1.0,
        "non-finite Output Number scale must not mutate hosted runtime state"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(44))
            .unwrap()
            .get_output_number_body()
            .unwrap()
            .number_of_decimals,
        0,
        "reserved Output Number decimal counts must not mutate hosted runtime state"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(45))
            .unwrap()
            .get_input_number_body()
            .unwrap()
            .number_of_decimals,
        0,
        "reserved Input Number decimal counts must not mutate hosted runtime state"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(49))
            .unwrap()
            .get_input_list_body()
            .unwrap()
            .value,
        1,
        "Input List selected-index AID 4 is read-only for Change Attribute"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(50))
            .unwrap()
            .get_output_list_body()
            .unwrap()
            .value,
        1,
        "Output List selected-index AID 4 is read-only for Change Attribute"
    );
    assert!(
        !runtime
            .pool()
            .find(ObjectID::new(26))
            .unwrap()
            .get_container_body()
            .unwrap()
            .hidden
    );
    let input_boolean = runtime
        .pool()
        .find(ObjectID::new(39))
        .unwrap()
        .get_input_boolean_body()
        .unwrap();
    assert_eq!(
        input_boolean.value, 1,
        "reserved Input Boolean value bytes must not mutate hosted runtime state"
    );
    assert_eq!(
        input_boolean.enabled, 1,
        "reserved Input Boolean enabled bytes must not mutate hosted runtime state"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(46))
            .unwrap()
            .get_alarm_mask_body()
            .unwrap()
            .acoustic_signal,
        3,
        "reserved Alarm Mask acoustic signal values must not mutate hosted runtime state"
    );
    let key_group = runtime
        .pool()
        .find(ObjectID::new(47))
        .unwrap()
        .get_key_group_body()
        .unwrap();
    assert_eq!(
        key_group.name,
        ObjectID::new(5),
        "Key Group name must not mutate to a non-OutputString reference"
    );
    assert_eq!(
        key_group.key_group_icon,
        ObjectID::new(23),
        "Key Group icon must not mutate to a non-graphic-representation reference"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(41))
            .unwrap()
            .get_window_mask_body()
            .unwrap()
            .window_type,
        0,
        "Window Mask type AID must not mutate to a type whose required-object list is absent"
    );
    let font_attributes = runtime
        .pool()
        .find(ObjectID::new(6))
        .unwrap()
        .get_font_attributes_body()
        .unwrap();
    assert_eq!(font_attributes.font_size, 0);
    assert_eq!(font_attributes.font_type, 0);
    assert_eq!(font_attributes.font_style, 0);
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(8))
            .unwrap()
            .get_fill_attributes_body()
            .unwrap()
            .fill_pattern,
        ObjectID::NULL
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(7))
            .unwrap()
            .get_line_attributes_body()
            .unwrap()
            .line_art,
        0x55AA,
        "Line Attributes line-art AID must reject non-zero upper Change Attribute bytes without overwriting the retained two-byte pattern"
    );
    let window_mask = runtime
        .pool()
        .find(ObjectID::new(41))
        .unwrap()
        .get_window_mask_body()
        .unwrap();
    assert_eq!(window_mask.width_cells, 2);
    assert_eq!(window_mask.height_cells, 6);
    assert_eq!(window_mask.window_type, 0);
    assert_eq!(window_mask.options, 0x01);
    assert_eq!(
        window_mask.name,
        ObjectID::NULL,
        "Window Mask name must not mutate to a non-OutputString reference"
    );
    assert_eq!(
        window_mask.window_title,
        ObjectID::NULL,
        "Window Mask title must not mutate to a non-OutputString reference"
    );
    assert_eq!(
        window_mask.window_icon,
        ObjectID::NULL,
        "Window Mask icon must not mutate to a non-graphic-representation reference"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(13))
            .unwrap()
            .get_colour_palette_body()
            .unwrap()
            .options,
        0
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(23))
            .unwrap()
            .get_picture_graphic_body()
            .unwrap()
            .options,
        0
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(38))
            .unwrap()
            .get_scaled_graphic_body()
            .unwrap()
            .value,
        ObjectID::new(23)
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(36))
            .unwrap()
            .get_external_reference_name_body()
            .unwrap()
            .options,
        1
    );
    let graphic_context = runtime
        .pool()
        .find(ObjectID::new(40))
        .unwrap()
        .get_graphic_context_body()
        .unwrap();
    assert_eq!(graphic_context.viewport_width, 10);
    assert_eq!(graphic_context.viewport_x, 0);
    assert_eq!(graphic_context.cursor_x, 0);
    assert_eq!(graphic_context.viewport_zoom_raw, 1.0_f32.to_bits());
    assert_eq!(graphic_context.fill_attributes, ObjectID::new(8));

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(40),
                attribute_id: 13,
                value: ObjectID::NULL.0 as u32,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(40))
            .unwrap()
            .get_graphic_context_body()
            .unwrap()
            .line_attributes,
        ObjectID::NULL
    );

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(23),
            attribute_id: 2,
            value: 0x07,
        })
        .unwrap();
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(23))
            .unwrap()
            .get_picture_graphic_body()
            .unwrap()
            .options,
        0x03,
        "Picture Graphic Options Change Attribute may alter transparency/flashing but must ignore the static raw/RLE bit"
    );

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(42),
            attribute_id: 2,
            value: 0x00,
        })
        .unwrap();
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(42))
            .unwrap()
            .get_picture_graphic_body()
            .unwrap()
            .options,
        0x04,
        "Picture Graphic Options Change Attribute must preserve an uploaded RLE/static bit"
    );
}

#[test]
fn render_runtime_imports_retained_state_relative_generic_attributes_without_map_order_drift() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_number(
                3,
                &InputNumberBody {
                    min_value: 0,
                    max_value: 100,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut working_set = ServerWorkingSet {
        pool,
        ..Default::default()
    };
    working_set
        .object_state
        .attributes
        .insert((ObjectID::new(3), 7), 150);
    working_set
        .object_state
        .attributes
        .insert((ObjectID::new(3), 8), 200);

    let runtime =
        VtRenderRuntime::from_server_working_set(&working_set, DocConfig::default()).unwrap();
    let body = runtime
        .pool()
        .find(ObjectID::new(3))
        .unwrap()
        .get_input_number_body()
        .unwrap();
    assert_eq!(
        (body.min_value, body.max_value),
        (150, 200),
        "server snapshots must import accepted min/max retained attributes as one final state, not lose the lower AID because map order applies it before the matching bound"
    );
}

#[test]
fn render_runtime_change_numeric_value_updates_input_list_selection() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    value: b"LOW".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"HIGH".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    options: 0x01,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::InputList {
            selected: 0,
            selected_text,
            ..
        } if selected_text.as_deref() == Some("LOW")
    ));
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                id: ObjectID::new(3),
                value: 1,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::InputList {
            selected: 1,
            selected_text,
            ..
        } if selected_text.as_deref() == Some("HIGH")
    ));
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                id: ObjectID::new(3),
                value: 1,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "same retained Input List value should not rebuild again"
    );
}

#[test]
fn vt_server_accepts_input_list_change_numeric_value_as_one_byte_selection() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap())
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap())
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    options: 0x01,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    let source = 0x42;
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_vt_command(cmd::GET_MEMORY),
        source,
    ));
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(pool.serialize().unwrap());
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer, source))
            .is_empty()
    );
    let end_response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_vt_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(end_response[0].data[1], 0);

    let mut change = [0u8; 8];
    change[0] = cmd::CHANGE_NUMERIC_VALUE;
    change[1..3].copy_from_slice(&3u16.to_le_bytes());
    change[3] = 0xFF;
    change[4] = 1;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change.to_vec(), source))
            .is_empty()
    );

    let state = &server.clients()[0].object_state;
    assert_eq!(state.numeric_values.get(&ObjectID::new(3)), Some(&1));
    assert!(matches!(
        state.accepted_effects.last(),
        Some(ServerRenderEffect::ChangeNumericValue {
            id,
            value: 1,
        }) if *id == ObjectID::new(3)
    ));

    let mut bad_width = change;
    bad_width[5] = 1;
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_width.to_vec(), source));
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        1,
        "Input List selection uses the one-byte Change Numeric Value payload form"
    );
}

#[test]
fn runtime_non_real_time_input_list_previews_until_commit() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_number_variable(9, &NumberVariableBody { value: 1 }))
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap())
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap())
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(9),
                    value: 255,
                    options: 0x01,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);

    let preview = rt.handle(&scene, &OperatorEvent::Tap(10, 10));
    assert!(matches!(
        preview[0],
        VtEvent::ListSelectionPreview {
            id,
            index: 0
        } if id == ObjectID::new(3)
    ));
    assert_eq!(rt.open_input(), Some(ObjectID::new(3)));

    let cancelled = rt.handle(&scene, &OperatorEvent::Cancel);
    assert!(matches!(
        cancelled[0],
        VtEvent::InputEsc {
            id,
            error_code: 0,
            transfer_sequence_number: None,
        } if id == ObjectID::new(3)
    ));
    assert_eq!(rt.open_input(), None);

    let preview = rt.handle(&scene, &OperatorEvent::Tap(10, 10));
    assert!(matches!(preview[0], VtEvent::ListSelectionPreview { .. }));
    let committed = rt.handle(&scene, &OperatorEvent::Commit);
    assert!(matches!(
        committed[0],
        VtEvent::ListSelectionChanged {
            id,
            index: 0
        } if id == ObjectID::new(3)
    ));
}

#[test]
fn runtime_tap_toggles_input_boolean() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_boolean(
                3,
                &InputBooleanBody {
                    enabled: 1,
                    width: 50,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);
    // The input boolean occupies (0,0,50,50).
    let ev = rt.handle(&scene, &OperatorEvent::Tap(10, 10));
    assert!(matches!(
        ev[0],
        VtEvent::BooleanValueChanged { id, value: true } if id == ObjectID::new(3)
    ));
}

#[test]
fn runtime_ignores_events_with_no_focus() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);
    let ev = rt.handle(&scene, &OperatorEvent::Char('A'));
    assert!(matches!(ev[0], VtEvent::Ignored { .. }));
}

// ─── Unsupported objects are reported, not dropped ─────────────────

#[test]
fn render_reports_unsupported_object_types_safely() {
    // An empty Animation object has no drawable active frame. It must
    // be recorded, not panic.
    let animation = create_animation(
        9,
        &AnimationBody {
            width: 16,
            height: 16,
            refresh_interval_ms: 100,
            options: 0,
            ..Default::default()
        },
    )
    .unwrap();
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([9u16]))
        .with_object(animation);
    let scene = render(&pool, ObjectID::NULL);
    assert!(
        scene
            .unsupported
            .iter()
            .any(|r| r.object_type == ObjectType::Animation)
    );
}

#[test]
fn render_animation_materialises_active_frame() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_animation(
                3,
                &AnimationBody {
                    width: 80,
                    height: 20,
                    refresh_interval_ms: 100,
                    value: 0,
                    enabled: 1,
                    first_child_index: 0,
                    default_child_index: 0,
                    last_child_index: 1,
                    options: 0,
                },
            )
            .unwrap()
            .with_children_pos([
                ChildRef::at_origin(ObjectID::new(10)),
                ChildRef::new(ObjectID::new(11), 4, 5),
            ]),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"ONE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"TWO".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let first = LayoutEngine::new(LayoutConfig::default())
        .with_animation_elapsed_ms(0)
        .build(&pool, ObjectID::NULL);
    let second = LayoutEngine::new(LayoutConfig::default())
        .with_animation_elapsed_ms(100)
        .build(&pool, ObjectID::NULL);

    let first_node = first.find(ObjectID::new(3)).expect("animation node");
    assert_eq!(first_node.object_type, ObjectType::Animation);
    assert!(matches!(
        &first_node.kind,
        NodeKind::OutputString { text, .. } if text == "ONE"
    ));
    assert!(
        GtuiRenderer::default()
            .render(&first)
            .iter()
            .any(|cmd| matches!(cmd, RenderCommand::DrawText { text, .. } if text == "ONE"))
    );

    let second_node = second.find(ObjectID::new(3)).expect("animation node");
    assert_eq!(second_node.object_type, ObjectType::Animation);
    assert!(matches!(
        &second_node.kind,
        NodeKind::OutputString { text, .. } if text == "TWO"
    ));
    assert!(
        GtuiRenderer::default()
            .render(&second)
            .iter()
            .any(|cmd| matches!(cmd, RenderCommand::DrawText { text, .. } if text == "TWO"))
    );
}

#[test]
fn render_animation_uses_standard_value_range_locations_and_disabled_behaviour() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_animation(
                3,
                &AnimationBody {
                    width: 40,
                    height: 20,
                    refresh_interval_ms: 100,
                    value: 1,
                    enabled: 1,
                    first_child_index: 1,
                    default_child_index: 0,
                    last_child_index: 2,
                    options: 0,
                },
            )
            .unwrap()
            .with_children_pos([
                ChildRef::at_origin(ObjectID::new(10)),
                ChildRef::new(ObjectID::new(11), 4, 5),
                ChildRef::new(ObjectID::new(12), 6, 7),
            ]),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    width: 40,
                    height: 20,
                    value: b"DEFAULT".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    width: 40,
                    height: 20,
                    value: b"ONE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                12,
                &OutputStringBody {
                    width: 40,
                    height: 20,
                    value: b"TWO".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let first = LayoutEngine::new(LayoutConfig::default())
        .with_animation_elapsed_ms(0)
        .build(&pool, ObjectID::NULL);
    let first_node = first.find(ObjectID::new(3)).expect("animation node");
    assert_eq!(first_node.rect.x, 4);
    assert_eq!(first_node.rect.y, 5);
    assert!(matches!(
        &first_node.kind,
        NodeKind::OutputString { text, .. } if text == "ONE"
    ));

    let second = LayoutEngine::new(LayoutConfig::default())
        .with_animation_elapsed_ms(100)
        .build(&pool, ObjectID::NULL);
    assert!(matches!(
        &second.find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputString { text, .. } if text == "TWO"
    ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 9,
            value: 0b100,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::EnableDisable {
            id: ObjectID::new(3),
            enabled: false,
        })
        .unwrap();
    let default_commands = runtime.render(&GtuiRenderer::default());
    assert!(
        default_commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "DEFA")
        ),
        "disabled Default Object mode draws the configured default child: {default_commands:?}"
    );

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 9,
            value: 0b110,
        })
        .unwrap();
    assert!(
        !runtime
            .render(&GtuiRenderer::default())
            .iter()
            .any(|command| matches!(command, RenderCommand::DrawText { text: _, .. })),
        "disabled Blank mode draws no animation child"
    );
}

#[test]
fn render_runtime_advances_animation_clock_and_marks_dirty() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_animation(
                3,
                &AnimationBody {
                    width: 80,
                    height: 20,
                    refresh_interval_ms: 100,
                    value: 0,
                    enabled: 1,
                    first_child_index: 0,
                    default_child_index: 0,
                    last_child_index: 1,
                    options: 0,
                },
            )
            .unwrap()
            .with_children_pos([
                ChildRef::at_origin(ObjectID::new(10)),
                ChildRef::at_origin(ObjectID::new(11)),
            ]),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"ONE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"TWO".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(runtime.animation_elapsed_ms(), 0);
    assert_eq!(runtime.animation_refresh_interval_ms(), Some(100));
    assert!(runtime_draws_text(&runtime, "ONE"));

    runtime.clear_dirty();
    let tick = runtime.tick_animation(50);
    assert_eq!(tick.update, RenderUpdate::Unchanged);
    assert_eq!(tick.next_refresh_interval_ms, Some(100));
    assert_eq!(runtime.animation_elapsed_ms(), 50);
    assert!(!runtime.is_dirty());
    assert!(runtime_draws_text(&runtime, "ONE"));

    let tick = runtime.tick_animation(50);
    assert!(matches!(tick.update, RenderUpdate::SceneRebuilt { .. }));
    assert_eq!(tick.next_refresh_interval_ms, Some(100));
    assert_eq!(runtime.animation_elapsed_ms(), 100);
    assert!(runtime.is_dirty());
    assert!(runtime_draws_text(&runtime, "TWO"));
}

#[test]
fn render_runtime_suspends_animation_clock_while_not_visible() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 4u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_data_mask(4, &DataMaskBody::default()).with_children([20u16]))
        .with_object(
            create_animation(
                3,
                &AnimationBody {
                    width: 80,
                    height: 20,
                    refresh_interval_ms: 100,
                    value: 0,
                    enabled: 1,
                    first_child_index: 0,
                    default_child_index: 0,
                    last_child_index: 1,
                    options: 0x01,
                },
            )
            .unwrap()
            .with_children_pos([
                ChildRef::at_origin(ObjectID::new(10)),
                ChildRef::at_origin(ObjectID::new(11)),
            ]),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"ONE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"TWO".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                20,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"ALT".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(runtime_draws_text(&runtime, "ONE"));

    let tick = runtime.tick_animation(100);
    assert!(matches!(tick.update, RenderUpdate::SceneRebuilt { .. }));
    assert!(runtime_draws_text(&runtime, "TWO"));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeActiveMask {
            mask: ObjectID::new(4),
        })
        .unwrap();
    assert_eq!(runtime.animation_refresh_interval_ms(), None);
    assert!(runtime_draws_text(&runtime, "ALT"));
    assert_eq!(runtime.tick_animation(500).update, RenderUpdate::Unchanged);

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeActiveMask {
            mask: ObjectID::new(2),
        })
        .unwrap();
    assert_eq!(runtime.animation_refresh_interval_ms(), Some(100));
    assert!(
        runtime_draws_text(&runtime, "TWO"),
        "hidden animation must resume from the suspended frame, not from the global host clock"
    );

    let tick = runtime.tick_animation(100);
    assert!(matches!(tick.update, RenderUpdate::SceneRebuilt { .. }));
    assert!(
        runtime_draws_text(&runtime, "ONE"),
        "looping animation advances only after becoming visible again"
    );
}

#[test]
fn render_output_list_as_selected_index_display() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_list(
                3,
                &OutputListBody {
                    width: 80,
                    height: 20,
                    value: 1,
                    items: vec![10.into(), 11.into(), 12.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    match &scene.find(ObjectID::new(3)).unwrap().kind {
        NodeKind::OutputList {
            selected,
            item_count,
            selected_text,
            selected_item_materialized,
        } => {
            assert_eq!(*selected, 1);
            assert_eq!(*item_count, 3);
            assert!(selected_text.is_none());
            assert!(!selected_item_materialized);
        }
        other => panic!("expected output-list node, got {other:?}"),
    }
}

#[test]
fn render_output_list_resolves_selected_item_text() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_list(
                3,
                &OutputListBody {
                    width: 80,
                    height: 20,
                    value: 1,
                    items: vec![10.into(), 11.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"First".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    value: b"Second".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    match &scene.find(ObjectID::new(3)).unwrap().kind {
        NodeKind::OutputList {
            selected_text,
            selected_item_materialized,
            ..
        } => {
            assert_eq!(selected_text.as_deref(), Some("Second"));
            assert!(*selected_item_materialized);
        }
        other => panic!("expected output-list node, got {other:?}"),
    }
    let selected_node = scene.find(ObjectID::new(11)).unwrap();
    assert_eq!(selected_node.parent, ObjectID::new(3));
    assert_eq!(selected_node.rect.x, 0);
    assert_eq!(selected_node.rect.y, 0);
    assert_eq!(selected_node.clip, Some(Rect::new(0, 0, 80, 20)));
    let commands = GtuiRenderer::default().render(&scene);
    let clip_index = commands
        .iter()
        .position(|command| {
            matches!(
                command,
                RenderCommand::Clip(Rect {
                    x: 0,
                    y: 0,
                    w: 80,
                    h: 20
                })
            )
        })
        .expect("selected OutputList item installs its clip before drawing");
    let text_index = commands
        .iter()
        .position(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Second"),
        )
        .expect("selected OutputList item emits its text");
    assert!(clip_index < text_index);
    assert!(commands.iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Second")
    ));
}

#[test]
fn render_soft_keys_honour_horizontal_soft_key_area_profiles() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(20),
                    ..Default::default()
                },
            ),
        )
        .with_object(create_soft_key_mask(20, &SoftKeyMaskBody::default()).with_children([
            101u16, 102, 103,
        ]))
        .with_object(create_key(
            101,
            &KeyBody {
                key_code: 1,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            102,
            &KeyBody {
                key_code: 2,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            103,
            &KeyBody {
                key_code: 3,
                ..Default::default()
            },
        ));
    let runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            soft_key_area: Rect::new(0, 240, 480, 64),
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(runtime.scene().soft_keys[0].rect, Rect::new(0, 240, 80, 64));
    assert_eq!(
        runtime.scene().soft_keys[1].rect,
        Rect::new(80, 240, 80, 64)
    );
    assert_eq!(
        runtime.scene().soft_keys[2].rect,
        Rect::new(160, 240, 80, 64)
    );
}

#[test]
fn runtime_places_and_activates_key_groups_in_horizontal_soft_key_area_profiles() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([31u16, 32u16]),
        )
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 31,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            32,
            &KeyBody {
                key_code: 32,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            soft_key_area: Rect::new(0, 240, 480, 64),
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 2),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.scene().find(ObjectID::new(30)).unwrap().rect,
        Rect::new(160, 240, 160, 64)
    );
    let commands = GtuiRenderer::default().render(runtime.scene());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::SoftKey {
            rect,
            label,
            ..
        } if *rect == Rect::new(160, 240, 80, 64) && label == "31"
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::SoftKey {
            rect,
            label,
            ..
        } if *rect == Rect::new(240, 240, 80, 64) && label == "32"
    )));
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(3)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(32)
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::Tap(250, 260)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(32)
        }]
    );
}
