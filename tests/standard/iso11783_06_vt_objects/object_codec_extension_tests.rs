use machbus::isobus::vt::{
    AlarmMaskBody, AnimationBody, ArchedBarGraphBody, AuxControlDesignatorBody, AuxFunction2Body,
    AuxFunctionBody, AuxInput2Body, ButtonBody, ColourMapBody, DataMaskBody,
    ExtendedInputAttributesBody, ExtendedInputCodePlane, FillAttributesBody, FontAttributesBody,
    GestureType, GraphicContextBody, GraphicDataBody, InputBooleanBody, InputListBody,
    InputNumberBody, KeyBody, KeyGroupBody, LanguageCountryPair, LineAttributesBody,
    LinearBarGraphBody, MeterBody, NumberVariableBody, ObjectID, ObjectLabelRefBody,
    ObjectLabelRefEntry, ObjectPointerBody, ObjectPool, ObjectType, OutputListBody,
    OutputNumberBody, OutputRectangleBody, OutputStringBody, PictureGraphicBody, ScaledGraphicBody,
    StringVariableBody, VTObject, WideCharRange, WindowMaskBody, WorkingSetBody,
    WorkingSetSpecialControlsBody, create_alarm_mask, create_animation, create_arched_bar_graph,
    create_aux_control_designator, create_aux_function, create_aux_function2, create_aux_input2,
    create_button, create_colour_map, create_data_mask, create_extended_input_attributes,
    create_fill_attributes, create_font_attributes, create_graphic_context, create_graphic_data,
    create_input_boolean, create_input_list, create_input_number, create_key, create_key_group,
    create_line_attributes, create_linear_bar_graph, create_meter, create_number_variable,
    create_object_label_ref, create_object_pointer, create_output_list, create_output_number,
    create_output_rectangle, create_output_string, create_picture_graphic, create_scaled_graphic,
    create_soft_key_mask, create_string_variable, create_window_mask, create_working_set,
    create_working_set_special_controls,
};

#[test]
fn vt_object_public_type_decoders_reject_noncanonical_bytes() {
    for raw in 0u8..=50 {
        let object_type = ObjectType::try_from_u8(raw).expect("defined VT object type");
        assert_eq!(object_type.as_u8(), raw);
    }
    assert_eq!(
        ObjectType::try_from_u8(36),
        Some(ObjectType::GraphicContext)
    );
    assert_eq!(ObjectType::try_from_u8(37), Some(ObjectType::OutputList));
    assert_eq!(
        ObjectType::try_from_u8(38),
        Some(ObjectType::ExtendedInputAttributes)
    );
    assert_eq!(
        ObjectType::try_from_u8(40),
        Some(ObjectType::ObjectLabelRef)
    );
    assert_eq!(ObjectType::try_from_u8(44), Some(ObjectType::Animation));
    assert_eq!(ObjectType::try_from_u8(45), Some(ObjectType::ColourPalette));
    assert_eq!(ObjectType::try_from_u8(46), Some(ObjectType::GraphicData));
    assert_eq!(
        ObjectType::try_from_u8(47),
        Some(ObjectType::WorkingSetSpecialControls)
    );
    assert_eq!(ObjectType::try_from_u8(48), Some(ObjectType::ScaledGraphic));
    for raw in [51, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(ObjectType::try_from_u8(raw), None);
    }

    for raw in 0u8..=12 {
        let gesture = GestureType::try_from_u8(raw).expect("defined VT gesture type");
        assert_eq!(gesture.as_u8(), raw);
    }
    for raw in [13, 14, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(GestureType::try_from_u8(raw), None);
    }
}

#[test]
fn vt_output_graphic_min_greater_than_max_is_a_render_clamp_not_pool_error() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4, 5]))
        .with_object(
            create_meter(
                3,
                &MeterBody {
                    width: 20,
                    min_value: 100,
                    max_value: 10,
                    value: 200,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_linear_bar_graph(
                4,
                &LinearBarGraphBody {
                    width: 20,
                    height: 10,
                    min_value: 100,
                    max_value: 10,
                    value: 200,
                    target_value: 200,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_arched_bar_graph(
                5,
                &ArchedBarGraphBody {
                    width: 20,
                    height: 10,
                    min_value: 100,
                    max_value: 10,
                    value: 200,
                    target_value: 200,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    pool.validate()
        .expect("Output Meter/Bar Graph min>=max is a defined render clamp case");
    let round_trip = ObjectPool::deserialize(&pool.serialize().unwrap()).unwrap();
    round_trip
        .validate()
        .expect("min>=max output graphic objects must survive pool decode");
    assert_eq!(
        round_trip
            .find(ObjectID::new(3))
            .unwrap()
            .get_meter_body()
            .unwrap()
            .min_value,
        100
    );
    assert_eq!(
        round_trip
            .find(ObjectID::new(4))
            .unwrap()
            .get_linear_bar_graph_body()
            .unwrap()
            .max_value,
        10
    );
    assert_eq!(
        round_trip
            .find(ObjectID::new(5))
            .unwrap()
            .get_arched_bar_graph_body()
            .unwrap()
            .min_value,
        100
    );
}

#[test]
fn vt_object_pool_models_output_list_extended_input_and_special_controls() {
    let output_list = create_output_list(
        10,
        &OutputListBody {
            width: 64,
            height: 16,
            value: 1,
            items: vec![11.into(), 12.into()],
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(output_list.r#type.as_u8(), 37);
    assert_eq!(output_list.get_output_list_body().unwrap().items.len(), 2);

    let extended_input = create_extended_input_attributes(
        20,
        &ExtendedInputAttributesBody {
            validation_type: 0,
            code_planes: vec![ExtendedInputCodePlane {
                plane: 0,
                ranges: vec![WideCharRange {
                    first: b'A' as u16,
                    last: b'Z' as u16,
                }],
            }],
        },
    )
    .unwrap();
    assert_eq!(extended_input.r#type.as_u8(), 38);
    let decoded = extended_input.get_extended_input_attributes_body().unwrap();
    assert_eq!(decoded.code_planes[0].ranges[0].first, b'A' as u16);

    let special_controls = create_working_set_special_controls(
        30,
        &WorkingSetSpecialControlsBody {
            colour_map: ObjectID::new(31),
            colour_palette: ObjectID::new(32),
            languages: vec![LanguageCountryPair {
                language: *b"en",
                country: *b"US",
            }],
            extra_bytes: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(special_controls.r#type.as_u8(), 47);
    let decoded = special_controls
        .get_working_set_special_controls_body()
        .unwrap();
    assert_eq!(decoded.colour_map, ObjectID::new(31));
    assert_eq!(decoded.languages[0].country, *b"US");

    let label_ref = create_object_label_ref(
        40,
        &ObjectLabelRefBody {
            labels: vec![ObjectLabelRefEntry {
                labelled_object: ObjectID::new(41),
                string_variable: ObjectID::new(42),
                font_type: 3,
                graphic_designator: ObjectID::NULL,
            }],
        },
    );
    assert_eq!(label_ref.r#type.as_u8(), 40);
    let decoded = label_ref.get_object_label_ref_body().unwrap();
    assert_eq!(decoded.labels[0].labelled_object, ObjectID::new(41));
    assert_eq!(decoded.labels[0].font_type, 3);
}

#[test]
fn extended_input_attributes_reject_duplicate_code_planes() {
    let duplicate = ExtendedInputAttributesBody {
        validation_type: 0,
        code_planes: vec![
            ExtendedInputCodePlane {
                plane: 1,
                ranges: vec![WideCharRange {
                    first: 0x0001,
                    last: 0x0002,
                }],
            },
            ExtendedInputCodePlane {
                plane: 1,
                ranges: vec![WideCharRange {
                    first: 0x0003,
                    last: 0x0004,
                }],
            },
        ],
    };
    assert!(
        duplicate.encode().is_err(),
        "17 maximum code planes means each Unicode code plane is represented at most once"
    );

    let raw_duplicate = [
        0, // validation type: listed ranges are valid
        2, // two code-plane records
        1, // plane 1
        1, // one range
        1, 0, 2, 0, // 0x0001..=0x0002
        1, // duplicate plane 1
        1, // one range
        3, 0, 4, 0, // 0x0003..=0x0004
    ];
    assert!(
        ExtendedInputAttributesBody::decode(&raw_duplicate).is_err(),
        "wire pools with duplicate ExtendedInputAttributes code-plane records are invalid"
    );
}

#[test]
fn colour_map_uses_standard_two_byte_count_and_standard_entry_lengths() {
    let body = ColourMapBody {
        entries: vec![1, 0],
    };
    let encoded = body.encode().unwrap();
    assert_eq!(
        &encoded[..2],
        &[2, 0],
        "Colour Map number-of-colour-indexes field is a two-byte integer"
    );
    assert_eq!(ColourMapBody::decode(&encoded).unwrap(), body);
    assert!(create_colour_map(50, &body).is_ok());

    assert!(
        ColourMapBody { entries: vec![0] }.encode().is_err(),
        "Colour Map entry count must be one of the standard VT graphics-depth sizes"
    );
    assert!(
        ColourMapBody::decode(&[1, 0, 0]).is_err(),
        "one-entry Colour Map payloads must not decode as standard object-pool records"
    );
    assert!(
        ColourMapBody {
            entries: vec![0; 256]
        }
        .encode()
        .is_ok(),
        "the standard 256-colour map must fit the two-byte count field"
    );
}

#[test]
fn working_set_special_controls_skips_forward_extension_bytes() {
    let body = [
        9, 0, // Number of bytes to follow.
        31, 0, // Colour Map object.
        32, 0, // Colour Palette object.
        0, // No language/country pairs.
        0xAA, 0xBB, 0xCC, 0xDD, // Future extension bytes.
    ];
    let decoded = WorkingSetSpecialControlsBody::decode(&body).unwrap();
    assert_eq!(decoded.colour_map, ObjectID::new(31));
    assert_eq!(decoded.colour_palette, ObjectID::new(32));
    assert!(decoded.languages.is_empty());
    assert_eq!(decoded.extra_bytes, vec![0xAA, 0xBB, 0xCC, 0xDD]);
    assert_eq!(decoded.bytes_to_follow().unwrap(), 9);
    assert_eq!(
        decoded.encode().unwrap(),
        body.to_vec(),
        "unknown forward-extension bytes must round-trip instead of being dropped"
    );

    let mut raw_pool = Vec::new();
    raw_pool.extend_from_slice(&[30, 0, ObjectType::WorkingSetSpecialControls.as_u8()]);
    raw_pool.extend_from_slice(&body);
    raw_pool.extend(
        create_number_variable(40, &NumberVariableBody { value: 0x4433_2211 })
            .serialize()
            .unwrap(),
    );

    let pool = ObjectPool::deserialize(&raw_pool).unwrap();
    assert_eq!(pool.size(), 2);
    assert_eq!(
        pool.find(30)
            .unwrap()
            .get_working_set_special_controls_body()
            .unwrap()
            .colour_palette,
        ObjectID::new(32)
    );
    assert_eq!(
        pool.find(40)
            .unwrap()
            .get_number_variable_body()
            .unwrap()
            .value,
        0x4433_2211
    );
}

#[test]
fn working_set_special_controls_rejects_truncated_language_pairs() {
    let body = [
        7, 0, // Number of bytes to follow.
        31, 0, // Colour Map object.
        32, 0, // Colour Palette object.
        1, // Claims one language/country pair, but only two bytes follow.
        b'e', b'n',
    ];
    assert!(WorkingSetSpecialControlsBody::decode(&body).is_err());
}

#[test]
fn scaled_graphic_uses_standard_scale_type_options_and_value_record() {
    let body = ScaledGraphicBody {
        width: 0x1234,
        height: 0x0056,
        scale_type: 0b0100_1011, // scale to width+height, middle/bottom justified.
        options: 0x01,
        value: ObjectID::new(0x3344),
    };
    let encoded = body.encode().unwrap();
    assert_eq!(
        encoded,
        vec![0x34, 0x12, 0x56, 0x00, 0b0100_1011, 0x01, 0x44, 0x33]
    );
    assert_eq!(ScaledGraphicBody::decode(&encoded).unwrap(), body);
    assert_eq!(
        encoded.len(),
        8,
        "Scaled Graphic Table B.76 has Width, Height, ScaleType, Options, and Value only"
    );

    assert!(
        ScaledGraphicBody {
            scale_type: 0x05,
            ..Default::default()
        }
        .encode()
        .is_err(),
        "scaling values 5..=7 are reserved"
    );
    assert!(
        ScaledGraphicBody {
            scale_type: 0x18,
            ..Default::default()
        }
        .encode()
        .is_err(),
        "horizontal justification value 3 is reserved"
    );
    assert!(
        ScaledGraphicBody {
            options: 0x02,
            ..Default::default()
        }
        .encode()
        .is_err(),
        "only the flashing option bit is standard"
    );
}

#[test]
fn vt_object_pool_validation_accepts_standard_scaled_graphic_value_sources() {
    let base = || {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
    };

    let graphic_data_source = base()
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_graphic_data(4, &GraphicDataBody::default()).unwrap());
    graphic_data_source.validate().unwrap();

    let picture_source = base()
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_picture_graphic(4, &PictureGraphicBody::default()).unwrap());
    picture_source.validate().unwrap();

    let pointer_source = base()
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            4,
            &ObjectPointerBody {
                value: ObjectID::new(5),
            },
        ))
        .with_object(create_picture_graphic(5, &PictureGraphicBody::default()).unwrap());
    pointer_source.validate().unwrap();

    let null_source = base().with_object(
        create_scaled_graphic(
            3,
            &ScaledGraphicBody {
                value: ObjectID::NULL,
                ..Default::default()
            },
        )
        .unwrap(),
    );
    null_source.validate().unwrap();

    let wrong_source = base()
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    assert!(
        wrong_source.validate().is_err(),
        "ScaledGraphic value sources are limited to graphic objects or ObjectPointer indirection"
    );

    let wrong_pointer_target = base()
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            4,
            &ObjectPointerBody {
                value: ObjectID::new(5),
            },
        ))
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());
    assert!(
        wrong_pointer_target.validate().is_err(),
        "ScaledGraphic ObjectPointer value sources must themselves resolve to graphic objects or NULL"
    );

    let pointer_cycle = base()
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    value: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            4,
            &ObjectPointerBody {
                value: ObjectID::new(4),
            },
        ));
    assert!(
        pointer_cycle.validate().is_err(),
        "ScaledGraphic ObjectPointer chains must not cycle"
    );
}

#[test]
fn vt_object_pool_requires_one_working_set_with_a_mask_child() {
    let pool_without_working_set =
        ObjectPool::default().with_object(create_data_mask(2, &DataMaskBody::default()));
    assert!(pool_without_working_set.validate().is_err());

    let pool_without_mask = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([99u16]));
    assert!(pool_without_mask.validate().is_err());
}

#[test]
fn vt_object_pool_validation_rejects_reserved_font_attribute_type() {
    let base = || {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([10u16]))
    };

    for font_type in [0, 1, 2, 4, 5, 7, 240, 255] {
        base()
            .with_object(create_font_attributes(
                10,
                &FontAttributesBody {
                    font_type,
                    ..Default::default()
                },
            ))
            .validate()
            .unwrap();
    }

    for font_type in [3, 6, 8, 239] {
        assert!(
            base()
                .with_object(create_font_attributes(
                    10,
                    &FontAttributesBody {
                        font_type,
                        ..Default::default()
                    },
                ))
                .validate()
                .is_err(),
            "FontAttributes font type {font_type} is reserved by the standard"
        );
    }

    base()
        .with_object(create_font_attributes(
            10,
            &FontAttributesBody {
                font_size: 64,
                font_type: 7,
                font_style: 0x80,
                ..Default::default()
            },
        ))
        .validate()
        .unwrap();

    for (font_size, font_style) in [(15, 0), (7, 0x80)] {
        assert!(
            base()
                .with_object(create_font_attributes(
                    10,
                    &FontAttributesBody {
                        font_size,
                        font_style,
                        ..Default::default()
                    },
                ))
                .validate()
                .is_err(),
            "FontAttributes font size {font_size} must match proportional style bit {font_style:#04x}"
        );
    }
}

#[test]
fn vt_output_number_rejects_non_finite_scale() {
    assert!(
        create_output_number(
            5,
            &OutputNumberBody {
                scale: f32::NAN,
                ..Default::default()
            },
        )
        .is_err(),
        "Output Number scale must reject NaN before object-pool upload"
    );

    let mut encoded = OutputNumberBody::default().encode().unwrap();
    encoded[18..22].copy_from_slice(&f32::INFINITY.to_le_bytes());
    assert!(
        OutputNumberBody::decode(&encoded).is_err(),
        "Output Number decode must reject infinite scale values before render use"
    );
}

#[test]
fn vt_number_objects_reject_reserved_hex_format() {
    assert!(
        create_output_number(
            5,
            &OutputNumberBody {
                format: 2,
                ..Default::default()
            },
        )
        .is_err(),
        "Output Number format is the standard fixed/exponential selector, not a hexadecimal mode"
    );
    assert!(
        create_input_number(
            6,
            &InputNumberBody {
                format: 2,
                ..Default::default()
            },
        )
        .is_err(),
        "Input Number format is the standard fixed/exponential selector, not a hexadecimal mode"
    );

    let mut encoded_output = OutputNumberBody::default().encode().unwrap();
    encoded_output[23] = 2;
    assert!(
        OutputNumberBody::decode(&encoded_output).is_err(),
        "Output Number decode must reject reserved format values before rendering"
    );
    let mut encoded_input = InputNumberBody::default().encode().unwrap();
    encoded_input[31] = 2;
    assert!(
        InputNumberBody::decode(&encoded_input).is_err(),
        "Input Number decode must reject reserved format values before rendering"
    );
}

#[test]
fn vt_number_objects_reject_reserved_decimal_counts() {
    assert!(
        create_output_number(
            5,
            &OutputNumberBody {
                number_of_decimals: 8,
                ..Default::default()
            },
        )
        .is_err(),
        "Output Number number-of-decimals is a standard 0..=7 selector"
    );
    assert!(
        create_input_number(
            6,
            &InputNumberBody {
                number_of_decimals: 8,
                ..Default::default()
            },
        )
        .is_err(),
        "Input Number number-of-decimals is a standard 0..=7 selector"
    );

    let mut encoded_output = OutputNumberBody::default().encode().unwrap();
    encoded_output[22] = 8;
    assert!(
        OutputNumberBody::decode(&encoded_output).is_err(),
        "Output Number decode must reject reserved decimal counts before rendering"
    );
    let mut encoded_input = InputNumberBody::default().encode().unwrap();
    encoded_input[30] = 8;
    assert!(
        InputNumberBody::decode(&encoded_input).is_err(),
        "Input Number decode must reject reserved decimal counts before rendering"
    );
}

#[test]
fn vt_object_pool_deserializes_standard_working_set_language_tail() {
    let bytes = vec![
        1,
        0,
        ObjectType::WorkingSet.as_u8(),
        7,
        1,
        2,
        0, // fixed fields: background/selectable/active mask
        1,
        1,
        1, // object, macro, language counts
        2,
        0,
        3,
        0,
        4,
        0, // child 2 at (3,4)
        5,
        6, // macro event/id
        b'e',
        b'n', // language
        2,
        0,
        ObjectType::DataMask.as_u8(),
        0,
        0xFF,
        0xFF,
        0,
        0, // data mask with no children/macros
    ];

    let pool = ObjectPool::deserialize(&bytes).unwrap();
    pool.validate().unwrap();
    let ws = pool.objects().first().unwrap();
    assert_eq!(ws.children, vec![ObjectID(2)]);
    assert_eq!(ws.children_pos[0].x, 3);
    assert_eq!(ws.children_pos[0].y, 4);
    assert_eq!(ws.macros.len(), 1);
    assert_eq!(ws.get_working_set_body().unwrap().languages, vec![*b"en"]);
    assert_eq!(pool.serialize().unwrap(), bytes);

    let invalid_language = ObjectPool::default()
        .with_object(
            create_working_set(
                1,
                &WorkingSetBody {
                    languages: vec![*b"e1"],
                    ..Default::default()
                },
            )
            .with_children([2u16]),
        )
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    assert!(invalid_language.validate().is_err());
}

#[test]
fn vt_object_pool_rejects_multiple_working_sets_before_upload_acceptance() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(create_working_set(3, &WorkingSetBody::default()).with_children([4u16]))
        .with_object(create_data_mask(4, &DataMaskBody::default()));

    assert!(
        pool.validate().is_err(),
        "an object pool must contain exactly one Working Set even when every Working Set has a valid mask child"
    );
}

#[test]
fn vt_object_pool_working_set_children_accept_designators_and_masks() {
    let valid_data_and_alarm_masks = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 3u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(create_alarm_mask(3, &AlarmMaskBody::default()).unwrap());
    valid_data_and_alarm_masks.validate().unwrap();

    // An Output String child is a valid Working Set *designator* (the icon/label
    // shown in the VT's working-set selector), per ISO 11783-6 — not an error.
    let designator_child = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 4u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    designator_child.validate().unwrap();

    let missing_mask_child = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 99u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    assert!(
        missing_mask_child.validate().is_err(),
        "missing Working Set children must still be rejected by graph validation"
    );
}

#[test]
fn vt_object_pool_soft_key_and_key_group_children_are_keys() {
    let valid = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID(3),
                    ..Default::default()
                },
            )
            .with_children([6u16]),
        )
        .with_object(create_soft_key_mask(3, &Default::default()).with_children([4u16, 8u16, 9u16]))
        .with_object(create_key_group(6, &KeyGroupBody::default()).with_children([5u16, 10u16]))
        .with_object(create_key(4, &KeyBody::default()))
        .with_object(create_key(5, &KeyBody::default()))
        .with_object(create_key(7, &KeyBody::default()))
        .with_object(create_key(11, &KeyBody::default()))
        .with_object(create_object_pointer(
            8,
            &ObjectPointerBody { value: ObjectID(7) },
        ))
        .with_object(create_object_pointer(
            9,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ))
        .with_object(create_object_pointer(
            10,
            &ObjectPointerBody {
                value: ObjectID(11),
            },
        ));
    valid.validate().unwrap();

    let valid_key_group_designators = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([6u16]))
        .with_object(
            create_key_group(
                6,
                &KeyGroupBody {
                    name: ObjectID(12),
                    key_group_icon: ObjectID(13),
                    ..Default::default()
                },
            )
            .with_children([5u16]),
        )
        .with_object(create_key(5, &KeyBody::default()))
        .with_object(create_object_pointer(
            12,
            &ObjectPointerBody {
                value: ObjectID(14),
            },
        ))
        .with_object(create_output_string(14, &OutputStringBody::default()).unwrap())
        .with_object(create_output_rectangle(13, &OutputRectangleBody::default()).unwrap());
    valid_key_group_designators.validate().unwrap();

    let key_group_wrong_name = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([6u16]))
        .with_object(
            create_key_group(
                6,
                &KeyGroupBody {
                    name: ObjectID(12),
                    ..Default::default()
                },
            )
            .with_children([5u16]),
        )
        .with_object(create_key(5, &KeyBody::default()))
        .with_object(create_number_variable(12, &NumberVariableBody { value: 0 }));
    assert!(
        key_group_wrong_name.validate().is_err(),
        "KeyGroup Name must reference OutputString or an ObjectPointer to OutputString"
    );

    let key_group_wrong_icon = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([6u16]))
        .with_object(
            create_key_group(
                6,
                &KeyGroupBody {
                    key_group_icon: ObjectID(12),
                    ..Default::default()
                },
            )
            .with_children([5u16]),
        )
        .with_object(create_key(5, &KeyBody::default()))
        .with_object(create_number_variable(12, &NumberVariableBody { value: 0 }));
    assert!(
        key_group_wrong_icon.validate().is_err(),
        "KeyGroup Icon must reference an Object Label graphic representation object"
    );

    let soft_key_wrong_child = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID(3),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_soft_key_mask(3, &Default::default()).with_children([4u16]))
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    assert!(
        soft_key_wrong_child.validate().is_err(),
        "SoftKeyMask children must be Key/ObjectPointer/ExternalObjectPointer objects"
    );

    let soft_key_pointer_wrong_target = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(create_soft_key_mask(3, &Default::default()).with_children([4u16]))
        .with_object(create_object_pointer(
            4,
            &ObjectPointerBody { value: ObjectID(5) },
        ))
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());
    assert!(
        soft_key_pointer_wrong_target.validate().is_err(),
        "SoftKeyMask ObjectPointer children may reserve NULL slots or resolve to Key objects only"
    );

    let key_group_wrong_child = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([6u16]))
        .with_object(create_key_group(6, &KeyGroupBody::default()).with_children([7u16]))
        .with_object(create_output_string(7, &OutputStringBody::default()).unwrap());
    assert!(
        key_group_wrong_child.validate().is_err(),
        "KeyGroup children must be Key or ObjectPointer objects"
    );

    let key_group_pointer_wrong_target = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([6u16]))
        .with_object(create_key_group(6, &KeyGroupBody::default()).with_children([7u16]))
        .with_object(create_object_pointer(
            7,
            &ObjectPointerBody { value: ObjectID(8) },
        ))
        .with_object(create_output_string(8, &OutputStringBody::default()).unwrap());
    assert!(
        key_group_pointer_wrong_target.validate().is_err(),
        "KeyGroup ObjectPointer children must resolve to Key objects"
    );

    let mut too_many_soft_keys = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID(3),
                    ..Default::default()
                },
            )
            .with_children([4u16]),
        )
        .with_object(create_soft_key_mask(3, &Default::default()).with_children(10u16..75u16))
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    for id in 10u16..75u16 {
        too_many_soft_keys = too_many_soft_keys.with_object(create_key(id, &KeyBody::default()));
    }
    assert!(
        too_many_soft_keys.validate().is_err(),
        "SoftKeyMask rejects more than 64 virtual keys"
    );

    let oversized_key_group = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([6u16]))
        .with_object(create_key_group(6, &KeyGroupBody::default()).with_children(10u16..15u16))
        .with_object(create_key(10, &KeyBody::default()))
        .with_object(create_key(11, &KeyBody::default()))
        .with_object(create_key(12, &KeyBody::default()))
        .with_object(create_key(13, &KeyBody::default()))
        .with_object(create_key(14, &KeyBody::default()));
    assert!(
        oversized_key_group.validate().is_err(),
        "KeyGroup rejects more than four key children"
    );

    let key_group_reserved_options = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([6u16]))
        .with_object(
            create_key_group(
                6,
                &KeyGroupBody {
                    options: 0x04,
                    ..Default::default()
                },
            )
            .with_children([10u16]),
        )
        .with_object(create_key(10, &KeyBody::default()));
    assert!(
        key_group_reserved_options.validate().is_err(),
        "KeyGroup option bits 2..=7 are reserved"
    );
}

#[test]
fn button_options_use_standard_disabled_bit_and_reject_reserved_bits() {
    let disabled = create_button(
        1,
        &ButtonBody {
            options: 0x10,
            ..Default::default()
        },
    );
    assert_eq!(
        disabled.get_button_body().unwrap().options & 0x10,
        0x10,
        "Button option bit 4 is the standard Disabled bit"
    );

    let reserved = create_button(
        2,
        &ButtonBody {
            options: 0x40,
            ..Default::default()
        },
    );
    assert!(
        reserved.get_button_body().is_err(),
        "Button option bit 6 remains reserved and must not be treated as Disabled"
    );
}

#[test]
fn vt_object_pool_serializes_deserializes_and_validates_minimal_graph() {
    let working_set = create_working_set(1, &WorkingSetBody::default()).with_children([2u16]);
    let data_mask = create_data_mask(
        2,
        &DataMaskBody {
            background_color: 3,
            soft_key_mask: 0xFFFF.into(),
        },
    );
    let pool = ObjectPool::default()
        .with_object(working_set)
        .with_object(data_mask);
    pool.validate().unwrap();

    let bytes = pool.serialize().unwrap();
    let decoded = ObjectPool::deserialize(&bytes).unwrap();
    decoded.validate().unwrap();

    assert_eq!(decoded.size(), 2);
    assert_eq!(decoded.find(1).unwrap().r#type, ObjectType::WorkingSet);
    assert_eq!(decoded.find(2).unwrap().r#type, ObjectType::DataMask);
}

#[test]
fn vt_object_pool_rejects_unknown_object_type_and_duplicate_ids() {
    let unknown_object = [1, 0, 0xFE, 0, 0];
    assert!(ObjectPool::deserialize(&unknown_object).is_err());

    let mut pool = ObjectPool::default();
    pool.add(VTObject::default().with_id(1)).unwrap();
    assert!(pool.add(VTObject::default().with_id(1)).is_err());
}

#[test]
fn vt_object_pool_rejects_null_object_id_as_real_object_identifier() {
    let mut pool = ObjectPool::default();
    let err = pool
        .add(VTObject::default().with_id(ObjectID::NULL))
        .expect_err("NULL object ID is a reference sentinel, not a valid object identity");
    assert_eq!(err.code, machbus::net::ErrorCode::InvalidState);
    assert!(err.message.contains("NULL object ID"));

    let serialized_null_working_set = [0xFF, 0xFF, ObjectType::WorkingSet.as_u8(), 0, 0];
    assert!(
        ObjectPool::deserialize(&serialized_null_working_set).is_err(),
        "deserializing a pool must reject the sentinel before it can become a real object"
    );
}

#[test]
fn vt_object_pool_validation_rejects_malformed_typed_bodies() {
    let working_set = create_working_set(1, &WorkingSetBody::default()).with_children([2u16]);
    let malformed_data_mask = VTObject::default()
        .with_id(2)
        .with_type(ObjectType::DataMask)
        .with_body(vec![3]);
    let pool = ObjectPool::default()
        .with_object(working_set)
        .with_object(malformed_data_mask);

    assert!(
        pool.validate().is_err(),
        "pool validation must reject objects whose typed body cannot decode"
    );

    let picture_with_reserved_options = VTObject::default()
        .with_id(3)
        .with_type(ObjectType::PictureGraphic)
        .with_body(vec![1, 0, 1, 0, 1, 0, 2, 0x08, 0xFF, 1, 0, 0, 0, 0]);
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(picture_with_reserved_options);

    assert!(
        pool.validate().is_err(),
        "pool validation must reject Picture Graphic reserved option bits before render use"
    );
}

#[test]
fn vt_object_pool_validation_checks_typed_body_references() {
    let working_set = create_working_set(1, &WorkingSetBody::default()).with_children([2u16]);
    let data_mask_with_missing_soft_keys = create_data_mask(
        2,
        &DataMaskBody {
            background_color: 0,
            soft_key_mask: 99.into(),
        },
    );
    let missing_soft_key_ref = ObjectPool::default()
        .with_object(working_set.clone())
        .with_object(data_mask_with_missing_soft_keys);
    assert!(
        missing_soft_key_ref.validate().is_err(),
        "mask-level body references must point at existing objects"
    );

    let data_mask_with_wrong_soft_key_type = create_data_mask(
        2,
        &DataMaskBody {
            background_color: 0,
            soft_key_mask: 3.into(),
        },
    );
    let wrong_soft_key_type = ObjectPool::default()
        .with_object(working_set.clone())
        .with_object(data_mask_with_wrong_soft_key_type)
        .with_object(create_string_variable(
            3,
            &StringVariableBody {
                length: 20,
                value: b"not-a-soft-key-mask".to_vec(),
            },
        ));
    assert!(
        wrong_soft_key_type.validate().is_err(),
        "mask-level body references must point at the expected object type"
    );

    let valid_mask = create_data_mask(
        2,
        &DataMaskBody {
            background_color: 0,
            soft_key_mask: 3.into(),
        },
    )
    .with_children([4u16]);
    let valid_string_output = create_output_string(
        4,
        &OutputStringBody {
            font_attributes: 5.into(),
            variable_reference: 6.into(),
            value: b"ok".to_vec(),
            ..OutputStringBody::default()
        },
    )
    .unwrap();
    let valid_refs = ObjectPool::default()
        .with_object(working_set.clone())
        .with_object(valid_mask)
        .with_object(create_soft_key_mask(3, &Default::default()))
        .with_object(valid_string_output)
        .with_object(create_font_attributes(5, &FontAttributesBody::default()))
        .with_object(create_string_variable(
            6,
            &StringVariableBody {
                length: 2,
                value: b"ok".to_vec(),
            },
        ));
    valid_refs.validate().unwrap();

    let valid_boolean_refs = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_input_boolean(
                4,
                &InputBooleanBody {
                    foreground: ObjectID(5),
                    variable_reference: ObjectID(6),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(5, &FontAttributesBody::default()))
        .with_object(create_number_variable(6, &NumberVariableBody { value: 1 }));
    valid_boolean_refs.validate().unwrap();

    assert!(
        create_alarm_mask(
            7,
            &AlarmMaskBody {
                priority: 3,
                ..Default::default()
            },
        )
        .is_err(),
        "Alarm Mask fixed Priority field only admits high/medium/low"
    );
    assert!(
        create_alarm_mask(
            7,
            &AlarmMaskBody {
                acoustic_signal: 4,
                ..Default::default()
            },
        )
        .is_err(),
        "Alarm Mask fixed Acoustic Signal field only admits high/medium/low/none"
    );
    let mut invalid_alarm_body = AlarmMaskBody {
        priority: 2,
        acoustic_signal: 3,
        ..Default::default()
    }
    .encode()
    .unwrap();
    invalid_alarm_body[3] = 3;
    assert!(
        AlarmMaskBody::decode(&invalid_alarm_body).is_err(),
        "Alarm Mask object-pool decode must reject reserved Priority bytes"
    );
    invalid_alarm_body[3] = 2;
    invalid_alarm_body[4] = 4;
    assert!(
        AlarmMaskBody::decode(&invalid_alarm_body).is_err(),
        "Alarm Mask object-pool decode must reject reserved Acoustic Signal bytes"
    );

    assert!(
        create_input_boolean(
            4,
            &InputBooleanBody {
                value: 2,
                ..Default::default()
            },
        )
        .is_err(),
        "Input Boolean fixed Value field only admits standard FALSE/TRUE values"
    );
    assert!(
        create_input_boolean(
            4,
            &InputBooleanBody {
                enabled: 2,
                ..Default::default()
            },
        )
        .is_err(),
        "Input Boolean fixed Enabled field only admits standard FALSE/TRUE values"
    );
    let mut invalid_boolean_body = InputBooleanBody {
        value: 1,
        enabled: 1,
        ..Default::default()
    }
    .encode()
    .unwrap();
    invalid_boolean_body[7] = 2;
    assert!(
        InputBooleanBody::decode(&invalid_boolean_body).is_err(),
        "Input Boolean object-pool decode must reject non-boolean Value bytes"
    );
    invalid_boolean_body[7] = 1;
    invalid_boolean_body[8] = 2;
    assert!(
        InputBooleanBody::decode(&invalid_boolean_body).is_err(),
        "Input Boolean object-pool decode must reject non-boolean Enabled bytes"
    );

    let wrong_boolean_foreground_type = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_input_boolean(
                4,
                &InputBooleanBody {
                    foreground: ObjectID(5),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());
    assert!(
        wrong_boolean_foreground_type.validate().is_err(),
        "Input Boolean foreground references must point at FontAttributes objects"
    );

    let wrong_boolean_variable_type = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_input_boolean(
                4,
                &InputBooleanBody {
                    variable_reference: ObjectID(5),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            5,
            &StringVariableBody {
                length: 1,
                value: b"x".to_vec(),
            },
        ));
    assert!(
        wrong_boolean_variable_type.validate().is_err(),
        "Input Boolean variable references must point at NumberVariable objects"
    );

    let valid_input_list_refs = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_input_list(
                4,
                &InputListBody {
                    variable_reference: ObjectID(5),
                    items: vec![ObjectID::NULL, 6.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_number_variable(5, &NumberVariableBody { value: 0 }))
        .with_object(create_output_string(6, &OutputStringBody::default()).unwrap());
    valid_input_list_refs
        .validate()
        .expect("Input List item NULL is a standard invisible no-item placeholder");

    let valid_output_list_null_item = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_output_list(
                4,
                &OutputListBody {
                    value: 0,
                    items: vec![ObjectID::NULL, 6.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(6, &OutputStringBody::default()).unwrap());
    valid_output_list_null_item
        .validate()
        .expect("Output List item NULL is a standard no-display placeholder");

    let wrong_output_list_item_type = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_output_list(
                4,
                &OutputListBody {
                    value: 0,
                    items: vec![6.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(6, &FontAttributesBody::default()));
    assert!(
        wrong_output_list_item_type.validate().is_err(),
        "Output List items must reference displayable objects, not style metadata"
    );

    let wrong_input_list_variable_type = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_input_list(
                4,
                &InputListBody {
                    variable_reference: ObjectID(5),
                    items: vec![6.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            5,
            &StringVariableBody {
                length: 1,
                value: b"x".to_vec(),
            },
        ))
        .with_object(create_output_string(6, &OutputStringBody::default()).unwrap());
    assert!(
        wrong_input_list_variable_type.validate().is_err(),
        "Input List variable references must point at NumberVariable objects"
    );

    let missing_input_list_item = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_input_list(
                4,
                &InputListBody {
                    items: vec![99.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    assert!(
        missing_input_list_item.validate().is_err(),
        "non-NULL Input List item references must point at uploaded objects"
    );

    let wrong_variable_type = ObjectPool::default()
        .with_object(working_set)
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([4u16]))
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    variable_reference: 5.into(),
                    value: b"bad".to_vec(),
                    ..OutputStringBody::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(5, &FontAttributesBody::default()));
    assert!(
        wrong_variable_type.validate().is_err(),
        "text output variable references must point at StringVariable objects"
    );
}

#[test]
fn vt_object_pool_validation_checks_animation_child_indices() {
    let valid = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_animation(
                3,
                &AnimationBody {
                    value: 1,
                    enabled: 1,
                    first_child_index: 0,
                    default_child_index: 0,
                    last_child_index: 1,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_children([4u16, 5u16]),
        )
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap())
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());
    valid.validate().unwrap();

    for (body, reason) in [
        (
            AnimationBody {
                value: 2,
                last_child_index: 1,
                ..Default::default()
            },
            "selected value index",
        ),
        (
            AnimationBody {
                first_child_index: 2,
                last_child_index: 1,
                ..Default::default()
            },
            "first index after last index",
        ),
        (
            AnimationBody {
                default_child_index: 2,
                last_child_index: 1,
                ..Default::default()
            },
            "default child index",
        ),
        (
            AnimationBody {
                last_child_index: 2,
                ..Default::default()
            },
            "last child index",
        ),
    ] {
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
            .with_object(
                create_animation(3, &body)
                    .unwrap()
                    .with_children([4u16, 5u16]),
            )
            .with_object(create_output_string(4, &OutputStringBody::default()).unwrap())
            .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());
        assert!(
            pool.validate().is_err(),
            "Animation validation must reject invalid {reason}"
        );
    }
}

#[test]
fn vt_object_pool_validation_checks_graphic_context_attribute_references() {
    let base = || {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([10u16]))
    };
    let graphic_context = || {
        create_graphic_context(
            10,
            &GraphicContextBody {
                viewport_width: 8,
                viewport_height: 8,
                canvas_width: 8,
                canvas_height: 8,
                font_attributes: ObjectID::new(20),
                line_attributes: ObjectID::new(21),
                fill_attributes: ObjectID::new(22),
                ..Default::default()
            },
        )
        .unwrap()
    };

    let valid = base()
        .with_object(graphic_context())
        .with_object(create_font_attributes(20, &FontAttributesBody::default()))
        .with_object(create_line_attributes(21, &LineAttributesBody::default()))
        .with_object(create_fill_attributes(22, &FillAttributesBody::default()).unwrap());
    valid.validate().unwrap();

    let missing_line_attributes = base()
        .with_object(graphic_context())
        .with_object(create_font_attributes(20, &FontAttributesBody::default()))
        .with_object(create_fill_attributes(22, &FillAttributesBody::default()).unwrap());
    assert!(
        missing_line_attributes.validate().is_err(),
        "GraphicContext line attribute references must point at uploaded objects"
    );

    let wrong_fill_type = base()
        .with_object(graphic_context())
        .with_object(create_font_attributes(20, &FontAttributesBody::default()))
        .with_object(create_line_attributes(21, &LineAttributesBody::default()))
        .with_object(create_font_attributes(22, &FontAttributesBody::default()));
    assert!(
        wrong_fill_type.validate().is_err(),
        "GraphicContext fill attribute references must point at FillAttributes objects"
    );
}

#[test]
fn vt_object_pool_validation_checks_fill_attribute_pattern_reference_type_and_row_alignment() {
    let base = || {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([10u16]))
    };
    let fill_attributes = |pattern: ObjectID| {
        create_fill_attributes(
            10,
            &FillAttributesBody {
                fill_type: 3,
                fill_color: 7,
                fill_pattern: pattern,
            },
        )
        .unwrap()
    };
    let picture = |actual_width: u16, format: u8, data: Vec<u8>| {
        create_picture_graphic(
            20,
            &PictureGraphicBody {
                width: actual_width,
                actual_width,
                actual_height: 1,
                format,
                options: 0,
                transparency: 0xFF,
                data,
            },
        )
        .unwrap()
    };

    let valid = base()
        .with_object(fill_attributes(ObjectID::new(20)))
        .with_object(picture(1, 2, vec![1]));
    valid.validate().unwrap();

    let null_pattern = base().with_object(fill_attributes(ObjectID::NULL));
    null_pattern.validate().unwrap();

    let aligned_mono_pattern = base()
        .with_object(fill_attributes(ObjectID::new(20)))
        .with_object(picture(8, 0, vec![0x80]));
    aligned_mono_pattern.validate().unwrap();

    let aligned_16_colour_pattern = base()
        .with_object(fill_attributes(ObjectID::new(20)))
        .with_object(picture(2, 1, vec![0x12]));
    aligned_16_colour_pattern.validate().unwrap();

    let unused_mono_bits = base()
        .with_object(fill_attributes(ObjectID::new(20)))
        .with_object(picture(7, 0, vec![0x80]));
    assert!(
        unused_mono_bits.validate().is_err(),
        "FillAttributes pattern PictureGraphic format 0 must not leave unused row bits"
    );

    let unused_16_colour_bits = base()
        .with_object(fill_attributes(ObjectID::new(20)))
        .with_object(picture(3, 1, vec![0x12, 0x30]));
    assert!(
        unused_16_colour_bits.validate().is_err(),
        "FillAttributes pattern PictureGraphic format 1 must not leave unused row bits"
    );

    let wrong_type = base()
        .with_object(fill_attributes(ObjectID::new(20)))
        .with_object(create_font_attributes(20, &FontAttributesBody::default()));
    assert!(
        wrong_type.validate().is_err(),
        "FillAttributes fill pattern references must point at PictureGraphic objects or NULL"
    );
}
