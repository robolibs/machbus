#[test]
fn render_runtime_updates_working_set_special_controls_colour_attributes() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_fill_attributes(
                4,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 20,
                    height: 20,
                    fill_attributes: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_palette(
                8,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_FF_00_00, 0xFF_00_00_FF],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_map(
                9,
                &ColourMapBody {
                    entries: vec![1, 0],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_working_set_special_controls(
                10,
                &WorkingSetSpecialControlsBody {
                    colour_palette: ObjectID::new(8),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(0xFF, 0x00, 0x00));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(10),
            attribute_id: 2,
            value: 9,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(0x00, 0x00, 0xFF));
    let body = runtime
        .pool()
        .find(ObjectID::new(10))
        .unwrap()
        .get_working_set_special_controls_body()
        .unwrap();
    assert_eq!(body.colour_map, ObjectID::new(9));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(10),
            attribute_id: 3,
            value: ObjectID::NULL.raw() as u32,
        })
        .unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(0, 0, 0));
}

#[test]
fn render_runtime_null_special_controls_palette_forces_default_palette() {
    let mut without_special_controls = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_fill_attributes(
                4,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 20,
                    height: 20,
                    fill_attributes: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_palette(
                8,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_FF_00_00],
                },
            )
            .unwrap(),
        );

    let runtime =
        VtRenderRuntime::from_pool(without_special_controls.clone(), DocConfig::default()).unwrap();
    assert_eq!(
        runtime.scene().find(ObjectID::new(3)).unwrap().style.fill_colour,
        Colour::rgb(0xFF, 0x00, 0x00),
        "without Working Set Special Controls, legacy pool palette discovery still applies"
    );

    without_special_controls = without_special_controls.with_object(
        create_working_set_special_controls(
            10,
            &WorkingSetSpecialControlsBody {
                colour_palette: ObjectID::NULL,
                ..Default::default()
            },
        )
        .unwrap(),
    );

    let runtime = VtRenderRuntime::from_pool(without_special_controls, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.scene().find(ObjectID::new(3)).unwrap().style.fill_colour,
        Colour::rgb(0xFF, 0xFF, 0xFF),
        "NULL Working Set Special Controls Colour Palette selects the VT default palette, not the first pool ColourPalette"
    );
}

#[test]
fn render_runtime_select_colour_map_persists_on_working_set_special_controls() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 20,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_map(
                9,
                &ColourMapBody {
                    entries: vec![1, 0],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_working_set_special_controls(10, &WorkingSetSpecialControlsBody::default())
                .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectColourMap {
            id: ObjectID::new(9),
        })
        .unwrap();
    let body = runtime
        .pool()
        .find(ObjectID::new(10))
        .unwrap()
        .get_working_set_special_controls_body()
        .unwrap();
    assert_eq!(body.colour_map, ObjectID::new(9));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectColourMap { id: ObjectID::NULL })
        .unwrap();
    let body = runtime
        .pool()
        .find(ObjectID::new(10))
        .unwrap()
        .get_working_set_special_controls_body()
        .unwrap();
    assert_eq!(body.colour_map, ObjectID::NULL);
    assert_eq!(body.colour_palette, ObjectID::NULL);
}

#[test]
fn render_scene_exposes_working_set_special_controls_languages() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_working_set_special_controls(
                10,
                &WorkingSetSpecialControlsBody {
                    languages: vec![
                        LanguageCountryPair {
                            language: *b"en",
                            country: *b"US",
                        },
                        LanguageCountryPair {
                            language: *b"de",
                            country: *b"DE",
                        },
                        LanguageCountryPair {
                            language: *b"en",
                            country: *b"  ",
                        },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert_eq!(scene.supported_languages.len(), 3);
    assert!(scene.supports_language(*b"en", *b"US"));
    assert!(scene.supports_language(*b"de", *b"DE"));
    assert!(scene.supports_language(*b"en", *b"  "));
    assert!(
        scene.supports_language(*b"EN", *b"us"),
        "language and country matching is ASCII-case-insensitive"
    );
    assert!(
        scene.supports_language(*b"en", *b"GB"),
        "two-space country sentinel advertises language-only support"
    );
    assert!(
        scene.supports_language(*b"de", *b"  "),
        "language-only requests match any exact country for the language"
    );
    assert!(!scene.supports_language(*b"de", *b"AT"));
    assert!(!scene.supports_language(*b"fr", *b"FR"));

    assert_eq!(
        scene.select_language(&[
            SceneLanguage {
                language: *b"fr",
                country: *b"FR",
            },
            SceneLanguage {
                language: *b"en",
                country: *b"GB",
            },
        ]),
        Some(SceneLanguage {
            language: *b"en",
            country: *b"  ",
        }),
        "selection returns the advertised language-only fallback pair"
    );
    assert_eq!(
        scene.select_language(&[SceneLanguage {
            language: *b"de",
            country: *b"  ",
        }]),
        Some(SceneLanguage {
            language: *b"de",
            country: *b"DE",
        }),
        "language-only host preference returns the first advertised country"
    );
    assert_eq!(
        scene.select_language(&[SceneLanguage {
            language: *b"DE",
            country: *b"de",
        }]),
        Some(SceneLanguage {
            language: *b"de",
            country: *b"DE",
        }),
        "exact-country preference matching ignores ASCII case but returns the advertised pair"
    );
}

#[test]
fn render_scene_uses_working_set_languages_until_special_controls_supersede_them() {
    let pool = ObjectPool::default()
        .with_object(
            create_working_set(
                1,
                &WorkingSetBody {
                    languages: vec![*b"en", *b"fr"],
                    ..Default::default()
                },
            )
            .with_children([2u16]),
        )
        .with_object(create_data_mask(2, &DataMaskBody::default()));

    let scene = render(&pool, ObjectID::NULL);
    assert_eq!(
        scene.supported_languages,
        vec![
            SceneLanguage {
                language: *b"en",
                country: *b"  ",
            },
            SceneLanguage {
                language: *b"fr",
                country: *b"  ",
            },
        ]
    );

    let superseded = pool.with_object(
        create_working_set_special_controls(
            10,
            &WorkingSetSpecialControlsBody {
                languages: vec![LanguageCountryPair {
                    language: *b"de",
                    country: *b"DE",
                }],
                ..Default::default()
            },
        )
        .unwrap(),
    );
    let scene = render(&superseded, ObjectID::NULL);
    assert_eq!(
        scene.supported_languages,
        vec![SceneLanguage {
            language: *b"de",
            country: *b"DE",
        }]
    );
}

// ─── Visibility / enabled state ────────────────────────────────────

#[test]
fn render_input_fields_carry_standard_enabled_state_without_confusing_input_string_options() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(
            create_input_boolean(
                3,
                &InputBooleanBody {
                    enabled: 1, // enabled
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_string(
                4,
                &InputStringBody {
                    // Input String options are transparency/wrapping bits, not
                    // the enable flag. Runtime Enable/Disable commands layer
                    // through `SceneNode::enabled` overrides instead.
                    options: 0x00,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let bool_node = scene.find(ObjectID::new(3)).expect("input boolean");
    let str_node = scene.find(ObjectID::new(4)).expect("input string");
    assert!(bool_node.enabled);
    assert!(str_node.enabled);
}

#[test]
fn render_runtime_enable_disable_controls_input_string_enabled_state() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x00,
                    width: 80,
                    height: 16,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(runtime.scene().find(ObjectID::new(3)).unwrap().enabled);

    runtime
        .apply_ecu_command(&VtRuntimeCommand::EnableDisable {
            id: ObjectID::new(3),
            enabled: false,
        })
        .unwrap();
    assert!(!runtime.scene().find(ObjectID::new(3)).unwrap().enabled);

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 6,
            value: 0x05,
        })
        .unwrap();
    assert!(
        !runtime.scene().find(ObjectID::new(3)).unwrap().enabled,
        "Input String options are transparency/wrapping bits and must not undo runtime Disable"
    );
}

// ─── Numeric / string value updates ────────────────────────────────

#[test]
fn render_output_string_resolves_variable_reference_value() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_string_variable(
            4,
            &StringVariableBody {
                length: 5,
                value: b"HELLO".to_vec(),
            },
        ))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 100,
                    height: 20,
                    variable_reference: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let node = scene.find(ObjectID::new(3)).expect("output string");
    match &node.kind {
        NodeKind::OutputString { text, .. } => assert_eq!(text, "HELLO"),
        other => panic!("expected OutputString, got {other:?}"),
    }
}

#[test]
fn render_output_number_applies_offset_scale_and_decimals() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_number_variable(4, &NumberVariableBody { value: 42 }))
        .with_object(
            create_output_number(
                3,
                &OutputNumberBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(4),
                    offset: 8,
                    // displayed = (value + offset) * scale (ISO 11783-6).
                    // (42 + 8) * 0.5 = 25
                    scale: 0.5,
                    number_of_decimals: 0,
                    format: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let node = scene.find(ObjectID::new(3)).expect("output number");
    match &node.kind {
        NodeKind::OutputNumber { text, .. } => assert_eq!(text, "25"), // (42+8)*0.5
        other => panic!("expected OutputNumber, got {other:?}"),
    }
}

#[test]
fn render_number_fields_honour_zero_blank_and_truncate_options() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16]),
        )
        .with_object(
            create_output_number(
                3,
                &OutputNumberBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    options: 0x04,
                    number_of_decimals: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_number(
                4,
                &InputNumberBody {
                    width: 80,
                    height: 20,
                    value: 239,
                    options: 0x08,
                    scale: 0.01,
                    number_of_decimals: 1,
                    options2: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_number(
                5,
                &OutputNumberBody {
                    width: 50,
                    height: 20,
                    value: 42,
                    options: 0x02,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);

    match &scene.find(ObjectID::new(3)).unwrap().kind {
        NodeKind::OutputNumber { text, .. } => {
            assert_eq!(text, "", "zero-as-blank option suppresses numeric text")
        }
        other => panic!("expected OutputNumber, got {other:?}"),
    }
    match &scene.find(ObjectID::new(4)).unwrap().kind {
        NodeKind::InputNumber { text, .. } => {
            assert_eq!(text, "2.3", "truncate option avoids rounding 2.39 to 2.4")
        }
        other => panic!("expected InputNumber, got {other:?}"),
    }
    match &scene.find(ObjectID::new(5)).unwrap().kind {
        NodeKind::OutputNumber { text, .. } => {
            assert_eq!(text, "00042", "leading-zero option pads to field cells")
        }
        other => panic!("expected OutputNumber, got {other:?}"),
    }
}

#[test]
fn render_output_string_falls_back_to_inline_value_without_variable() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    value: b"INLINE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    match &scene.find(ObjectID::new(3)).unwrap().kind {
        NodeKind::OutputString { text, .. } => assert_eq!(text, "INLINE"),
        _ => unreachable!(),
    }
}

// ─── Soft keys ─────────────────────────────────────────────────────

#[test]
fn render_soft_key_mask_is_modelled_in_soft_key_area() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(6, &KeyBody::default()));
    let scene = render(&pool, ObjectID::NULL);
    assert_eq!(scene.soft_keys.len(), 1);
    assert_eq!(scene.soft_keys[0].id, ObjectID::new(6));
    // The soft key sits in the configured soft-key area.
    assert!(scene.soft_keys[0].rect.x >= 480);
}

#[test]
fn render_soft_key_mask_resolves_pointers_and_preserves_null_slots() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default())
                .with_children([9u16, 10u16, 7u16, 11u16]),
        )
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 6,
                background_color: 5,
            },
        ))
        .with_object(create_key(
            7,
            &KeyBody {
                key_code: 7,
                background_color: 6,
            },
        ))
        .with_object(create_object_pointer(
            9,
            &ObjectPointerBody { value: ObjectID(6) },
        ))
        .with_object(create_object_pointer(
            10,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ))
        .with_object(create_object_pointer(
            11,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ));
    let scene = render(&pool, ObjectID::NULL);

    assert_eq!(
        scene.soft_keys.len(),
        2,
        "middle NULL ObjectPointer reserves a cell but trailing NULL slots are not displayed"
    );
    assert_eq!(scene.soft_keys[0].id, ObjectID::new(6));
    assert_eq!(scene.soft_keys[0].cell_index, 0);
    assert_eq!(scene.soft_keys[0].label, "6");
    assert_eq!(
        scene.soft_keys[0].style.background,
        Palette::default_isobus().resolve(5)
    );
    assert_eq!(scene.soft_keys[1].id, ObjectID::new(7));
    assert_eq!(scene.soft_keys[1].cell_index, 2);
    assert_eq!(scene.soft_keys[1].label, "7");
    assert!(
        scene.soft_keys[1].rect.y
            > scene.soft_keys[0].rect.y + i32::from(scene.soft_keys[0].rect.h),
        "the NULL pointer in the middle must reserve its physical soft-key slot"
    );
}

#[test]
fn render_soft_key_mask_resolves_external_object_pointer_key_when_registered() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([20u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                background_color: 3,
                key_code: 6,
            },
        ))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            20,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(6),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ));
    let external_pool = ObjectPool::default()
        .with_object(
            create_external_object_definition(
                40,
                &ExternalObjectDefinitionBody {
                    options: 1,
                    name0: local_name.0,
                    name1: local_name.1,
                    object_ids: vec![ObjectID::new(50)],
                },
            )
            .unwrap(),
        )
        .with_object(create_key(
            50,
            &KeyBody {
                background_color: 7,
                key_code: 50,
            },
        ));

    let scene = LayoutEngine::new(LayoutConfig::default())
        .with_working_set_name(local_name.0, local_name.1)
        .with_external_object_pool(external_name.0, external_name.1, external_pool.clone())
        .build(&pool, ObjectID::NULL);

    assert_eq!(scene.soft_keys.len(), 1);
    assert_eq!(
        scene.soft_keys[0].id,
        ObjectID::new(50),
        "External Object Pointer soft-key entries should use the referenced Key when the host has registered the referenced Working Set"
    );
    assert_eq!(scene.soft_keys[0].key_number, 50);
    assert_eq!(scene.soft_keys[0].label, "50");
    assert_eq!(
        scene.soft_keys[0].style.background,
        Palette::default_isobus().resolve(7)
    );

    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool);
    let (_, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PhysicalSoftKey(0))
        .unwrap();
    assert_eq!(
        messages[0].as_bytes(),
        &[
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Pressed.as_u8(),
            50,
            0,
            5,
            0,
            50,
            0xFF,
        ],
        "external soft-key activation must preserve the referenced Key Number"
    );
}

#[test]
fn render_runtime_soft_key_page_count_trims_trailing_null_pointer_slots() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default())
                .with_children([6u16, 7u16, 8u16, 20u16, 21u16]),
        )
        .with_object(create_key(6, &KeyBody::default()))
        .with_object(create_key(7, &KeyBody::default()))
        .with_object(create_key(8, &KeyBody::default()))
        .with_object(create_object_pointer(
            20,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ))
        .with_object(create_object_pointer(
            21,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 4,
            navigation_soft_key_count: 1,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        runtime.soft_key_page_count(),
        1,
        "runtime page count must use the same trailing-NULL trimming as the rendered soft-key cells"
    );
    assert_eq!(
        runtime.next_soft_key_page(),
        RenderUpdate::Unchanged,
        "trimmed trailing NULL slots must not create an empty second soft-key page"
    );
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        vec![ObjectID::new(6), ObjectID::new(7), ObjectID::new(8)]
    );
}

#[test]
fn render_runtime_applies_key_background_colour_to_soft_key_cell() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(6, &KeyBody::default()));
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeBackgroundColour {
            id: ObjectID::new(6),
            colour: 9,
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert_eq!(
        runtime.scene().soft_keys[0].style.background,
        Palette::default_isobus().resolve(9)
    );
}

#[test]
fn render_soft_key_and_key_group_labels_resolve_key_children() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([30u16]),
        )
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(6, &KeyBody::default()).with_children([60u16]))
        .with_object(
            create_output_string(
                60,
                &OutputStringBody {
                    value: b"Go".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([31u16]),
        )
        .with_object(create_key(31, &KeyBody::default()).with_children([61u16]))
        .with_object(create_object_pointer(
            61,
            &ObjectPointerBody {
                value: ObjectID::new(62),
            },
        ))
        .with_object(
            create_output_string(
                62,
                &OutputStringBody {
                    value: b"KG".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert_eq!(scene.soft_keys[0].label, "Go");
    assert!(matches!(
        &scene.find(ObjectID::new(30)).unwrap().kind,
        NodeKind::KeyGroup { labels, .. } if labels.as_slice() == ["KG"]
    ));
}

#[test]
fn render_soft_key_mask_pages_application_keys_and_reserves_navigation_cells() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default())
                .with_children([6u16, 7u16, 8u16, 9u16, 10u16, 11u16]),
        );
    for id in 6u16..=11 {
        pool = pool.with_object(create_key(
            id,
            &KeyBody {
                key_code: id as u8,
                ..Default::default()
            },
        ));
    }

    let engine = LayoutEngine::new(LayoutConfig::default())
        .with_soft_key_counts(4, 1)
        .with_soft_key_page(1);
    let scene = render_with(&pool, &engine, ObjectID::NULL);

    // Four physical keys with one navigation reservation leave three
    // application keys per page. Page one contains the second group.
    assert_eq!(
        scene
            .soft_keys
            .iter()
            .filter(|key| key.kind == SoftKeyKind::Application)
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        vec![ObjectID::new(9), ObjectID::new(10), ObjectID::new(11)]
    );
    assert_eq!(
        scene.soft_keys[0].rect.h,
        LayoutConfig::default().soft_key_area.h / 4
    );
    assert_eq!(scene.soft_keys[0].label, "9");
    assert_eq!(
        scene
            .soft_keys
            .iter()
            .map(|key| (key.cell_index, key.kind))
            .collect::<Vec<_>>(),
        vec![
            (0, SoftKeyKind::Application),
            (1, SoftKeyKind::Application),
            (2, SoftKeyKind::Application),
            (3, SoftKeyKind::NavigationPrevious),
        ]
    );
    assert_eq!(
        scene
            .soft_keys
            .iter()
            .filter(|key| key.kind != SoftKeyKind::Application)
            .map(|key| key.kind)
            .collect::<Vec<_>>(),
        vec![SoftKeyKind::NavigationPrevious]
    );
}

#[test]
fn render_runtime_tracks_soft_key_pages_for_active_mask() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default())
                .with_children([6u16, 7u16, 8u16, 9u16, 10u16, 11u16]),
        );
    for id in 6u16..=11 {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let config = LayoutConfig {
        physical_soft_key_count: 4,
        navigation_soft_key_count: 1,
        ..Default::default()
    };
    let mut runtime = VtRenderRuntime::from_pool(pool, config).unwrap();
    assert_eq!(runtime.soft_key_page_count(), 2);
    assert_eq!(runtime.soft_key_page(), 0);
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .filter(|key| key.kind == SoftKeyKind::Application)
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        vec![ObjectID::new(6), ObjectID::new(7), ObjectID::new(8)]
    );

    assert!(matches!(
        runtime.next_soft_key_page(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(runtime.soft_key_page(), 1);
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .filter(|key| key.kind == SoftKeyKind::Application)
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        vec![ObjectID::new(9), ObjectID::new(10), ObjectID::new(11)]
    );
    assert_eq!(runtime.next_soft_key_page(), RenderUpdate::Unchanged);
    assert!(matches!(
        runtime.previous_soft_key_page(),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(runtime.soft_key_page(), 0);
}

#[test]
fn render_soft_key_mask_uses_all_physical_keys_without_navigation_when_it_fits() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default())
                .with_children([6u16, 7u16, 8u16, 9u16, 10u16, 11u16]),
        );
    for id in 6u16..=11 {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let config = LayoutConfig {
        physical_soft_key_count: 6,
        navigation_soft_key_count: 2,
        ..Default::default()
    };
    let mut runtime = VtRenderRuntime::from_pool(pool, config).unwrap();
    assert_eq!(runtime.soft_key_page_count(), 1);
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .map(|key| key.kind)
            .collect::<Vec<_>>(),
        vec![
            SoftKeyKind::Application,
            SoftKeyKind::Application,
            SoftKeyKind::Application,
            SoftKeyKind::Application,
            SoftKeyKind::Application,
            SoftKeyKind::Application,
        ]
    );
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        (6u16..=11).map(ObjectID::new).collect::<Vec<_>>()
    );
    assert_eq!(runtime.next_soft_key_page(), RenderUpdate::Unchanged);
}

#[test]
fn render_soft_key_mask_pages_ten_key_terminal_with_reserved_navigation() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children(10u16..27u16),
        );
    for id in 10u16..27u16 {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let config = LayoutConfig {
        physical_soft_key_count: 10,
        navigation_soft_key_count: 2,
        ..Default::default()
    };
    let mut runtime = VtRenderRuntime::from_pool(pool, config).unwrap();
    assert_eq!(runtime.soft_key_page_count(), 3);
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .filter(|key| key.kind == SoftKeyKind::Application)
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        (10u16..18u16).map(ObjectID::new).collect::<Vec<_>>()
    );
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .filter(|key| key.kind != SoftKeyKind::Application)
            .map(|key| key.kind)
            .collect::<Vec<_>>(),
        vec![SoftKeyKind::NavigationPrevious, SoftKeyKind::NavigationNext]
    );

    assert!(matches!(
        runtime.set_soft_key_page(2),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .filter(|key| key.kind == SoftKeyKind::Application)
            .map(|key| key.id)
            .collect::<Vec<_>>(),
        vec![ObjectID::new(26)]
    );
}

#[test]
fn render_soft_key_mask_clamps_over_reserved_navigation_profile() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children(10u16..13u16),
        );
    for id in 10u16..13 {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 2,
            navigation_soft_key_count: 2,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .map(|key| key.cell_index)
            .collect::<Vec<_>>(),
        vec![0, 1],
        "over-reserved navigation profiles must not overlap an application cell"
    );
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .map(|key| (key.kind, key.enabled))
            .collect::<Vec<_>>(),
        vec![
            (SoftKeyKind::Application, true),
            (SoftKeyKind::NavigationNext, true),
        ]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(0)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(10),
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(1)),
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
}

#[test]
fn render_runtime_disables_boundary_soft_key_navigation_cells() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children(10u16..27u16),
        );
    for id in 10u16..27u16 {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let config = LayoutConfig {
        physical_soft_key_count: 10,
        navigation_soft_key_count: 2,
        ..Default::default()
    };
    let mut runtime = VtRenderRuntime::from_pool(pool, config).unwrap();
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .filter(|key| key.kind != SoftKeyKind::Application)
            .map(|key| (key.cell_index, key.kind, key.enabled))
            .collect::<Vec<_>>(),
        vec![
            (8, SoftKeyKind::NavigationPrevious, false),
            (9, SoftKeyKind::NavigationNext, true),
        ],
        "the first page must keep the previous navigation cell visible but inactive"
    );
    assert!(matches!(
        runtime
            .handle_operator_event(OperatorEvent::PhysicalSoftKey(8))
            .as_slice(),
        [VtEvent::Ignored {
            reason: "physical soft-key cell is not available"
        }]
    ));
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(9)),
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );

    assert!(matches!(
        runtime.set_soft_key_page(2),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime
            .scene()
            .soft_keys
            .iter()
            .filter(|key| key.kind != SoftKeyKind::Application)
            .map(|key| (key.cell_index, key.kind, key.enabled))
            .collect::<Vec<_>>(),
        vec![
            (8, SoftKeyKind::NavigationPrevious, true),
            (9, SoftKeyKind::NavigationNext, false),
        ],
        "the final page must keep the next navigation cell visible but inactive"
    );
    assert!(matches!(
        runtime
            .handle_operator_event(OperatorEvent::PhysicalSoftKey(9))
            .as_slice(),
        [VtEvent::Ignored {
            reason: "physical soft-key cell is not available"
        }]
    ));
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(8)),
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
}

#[test]
fn render_key_group_as_user_layout_cells() {
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
            .with_children([31u16, 33u16]),
        )
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 21,
                ..Default::default()
            },
        ))
        .with_object(create_object_pointer(
            33,
            &ObjectPointerBody {
                value: ObjectID::new(32),
            },
        ))
        .with_object(create_key(
            32,
            &KeyBody {
                key_code: 22,
                ..Default::default()
            },
        ));
    pool.validate().unwrap();

    let engine = LayoutEngine::new(LayoutConfig {
        physical_soft_key_count: 6,
        ..Default::default()
    })
    .with_placements(PlacementMap::new().set(30, 20, 30));
    let scene = render_with(&pool, &engine, ObjectID::NULL);
    let node = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(30))
        .expect("key group is rendered");
    assert_eq!(node.rect, Rect::new(20, 30, 64, 80));
    assert!(matches!(
        &node.kind,
        NodeKind::KeyGroup {
            available: true,
            transparent: false,
            key_ids,
            key_numbers,
            labels,
        } if key_ids == &vec![ObjectID::new(31), ObjectID::new(32)]
            && key_numbers == &vec![21, 22]
            && labels == &vec!["21".to_string(), "22".to_string()]
    ));

    let commands = GtuiRenderer::default().render(&scene);
    let rendered_keys = commands
        .iter()
        .filter_map(|command| match command {
            RenderCommand::SoftKey {
                rect, kind, label, ..
            } => Some((*rect, *kind, label.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rendered_keys,
        vec![
            (Rect::new(20, 30, 64, 40), SoftKeyKind::Application, "21"),
            (Rect::new(20, 70, 64, 40), SoftKeyKind::Application, "22"),
        ]
    );

    let mut input = InputRuntime::new();
    input.bind(&scene);
    assert_eq!(
        input.handle(&scene, &OperatorEvent::Tap(25, 35)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(31)
        }]
    );
    assert_eq!(input.soft_key_latched(), Some(ObjectID::new(31)));
    assert_eq!(
        input.handle(&scene, &OperatorEvent::Tap(25, 75)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(32)
        }]
    );
    assert_eq!(input.soft_key_latched(), Some(ObjectID::new(32)));
    assert_eq!(
        input.handle(&scene, &OperatorEvent::FocusNext),
        vec![VtEvent::Ignored {
            reason: "no interactive fields available"
        }]
    );
}

#[test]
fn render_key_group_resolves_external_object_pointer_key_when_registered() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
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
            .with_children([33u16]),
        )
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 21,
                ..Default::default()
            },
        ))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            33,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(31),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ));
    pool.validate().unwrap();

    let external_pool = ObjectPool::default()
        .with_object(
            create_external_object_definition(
                40,
                &ExternalObjectDefinitionBody {
                    options: 1,
                    name0: local_name.0,
                    name1: local_name.1,
                    object_ids: vec![ObjectID::new(50)],
                },
            )
            .unwrap(),
        )
        .with_object(create_key(
            50,
            &KeyBody {
                key_code: 50,
                ..Default::default()
            },
        ));

    let engine = LayoutEngine::new(LayoutConfig {
        physical_soft_key_count: 6,
        ..Default::default()
    })
    .with_working_set_name(local_name.0, local_name.1)
    .with_external_object_pool(external_name.0, external_name.1, external_pool)
    .with_placements(PlacementMap::new().set(30, 20, 30));
    let scene = render_with(&pool, &engine, ObjectID::NULL);

    assert!(matches!(
        &scene.find(ObjectID::new(30)).unwrap().kind,
        NodeKind::KeyGroup {
            key_ids,
            labels,
            ..
        } if key_ids == &vec![ObjectID::new(50)] && labels == &vec!["50".to_string()]
    ));

    let mut input = InputRuntime::new();
    input.bind(&scene);
    assert_eq!(
        input.handle(&scene, &OperatorEvent::Tap(25, 35)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(50)
        }],
        "KeyGroup activation should report the referenced external Key id"
    );
}

#[test]
fn render_runtime_places_external_only_key_group_in_user_layout() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
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
            .with_children([33u16]),
        )
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            33,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ));
    pool.validate().unwrap();

    let external_pool = ObjectPool::default()
        .with_object(
            create_external_object_definition(
                40,
                &ExternalObjectDefinitionBody {
                    options: 1,
                    name0: local_name.0,
                    name1: local_name.1,
                    object_ids: vec![ObjectID::new(50)],
                },
            )
            .unwrap(),
        )
        .with_object(create_key(
            50,
            &KeyBody {
                key_code: 50,
                ..Default::default()
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool);

    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 2),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }),
        "operator placement must count externally resolved KeyGroup children"
    );
    let node = runtime.scene().find(ObjectID::new(30)).unwrap();
    assert_eq!(node.rect, Rect::new(480, 80, 64, 40));
    assert!(matches!(
        &node.kind,
        NodeKind::KeyGroup {
            key_ids,
            key_numbers,
            labels,
            ..
        } if key_ids == &vec![ObjectID::new(50)]
            && key_numbers == &vec![50]
            && labels == &vec!["50".to_string()]
    ));

    let (_, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PhysicalSoftKey(2))
        .unwrap();
    assert_eq!(
        messages[0].as_bytes(),
        &[
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Pressed.as_u8(),
            50,
            0,
            30,
            0,
            50,
            0xFF,
        ],
        "external KeyGroup activation must preserve the referenced Key Number"
    );
}

#[test]
fn render_runtime_preserves_blank_external_key_group_slots_without_referenced_pool() {
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
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
            .with_children([33u16, 32u16]),
        )
        .with_object(create_key(
            32,
            &KeyBody {
                key_code: 32,
                ..Default::default()
            },
        ))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            33,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ));
    pool.validate().unwrap();

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 1),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }),
        "a remembered/operator KeyGroup placement should reserve unresolved external pointer slots instead of collapsing or rejecting them"
    );
    let node = runtime.scene().find(ObjectID::new(30)).unwrap();
    assert_eq!(node.rect, Rect::new(480, 40, 64, 80));
    assert!(matches!(
        &node.kind,
        NodeKind::KeyGroup {
            key_ids,
            labels,
            ..
        } if key_ids == &vec![ObjectID::NULL, ObjectID::new(32)]
            && labels == &vec!["".to_string(), "32".to_string()]
    ));

    let commands = GtuiRenderer::default().render(runtime.scene());
    let rendered_keys = commands
        .iter()
        .filter_map(|command| match command {
            RenderCommand::SoftKey { rect, label, .. } => Some((*rect, label.as_str())),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(rendered_keys, vec![(Rect::new(480, 80, 64, 40), "32")]);

    let mut input = InputRuntime::new();
    input.bind(runtime.scene());
    assert_eq!(
        input.handle(runtime.scene(), &OperatorEvent::Tap(485, 45)),
        vec![VtEvent::Ignored {
            reason: "key group has no key under cursor"
        }]
    );
    assert_eq!(
        input.handle(runtime.scene(), &OperatorEvent::Tap(485, 85)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(32)
        }]
    );
}
