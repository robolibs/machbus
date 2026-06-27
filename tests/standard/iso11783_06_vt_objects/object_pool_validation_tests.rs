#[test]
fn vt_object_pool_validation_checks_window_mask_required_objects() {
    let base = || {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
    };

    let valid_typed_window = base()
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_type: 3,
                    options: 0x01,
                    required_objects: vec![ObjectID::new(31)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(31, &OutputStringBody::default()).unwrap());
    valid_typed_window.validate().unwrap();

    assert!(
        create_window_mask(
            30,
            &WindowMaskBody {
                options: 0x04,
                ..Default::default()
            },
        )
        .is_err(),
        "WindowMask option bits 2..=7 are reserved"
    );
    assert!(
        create_window_mask(
            30,
            &WindowMaskBody {
                width_cells: 0,
                ..Default::default()
            },
        )
        .is_err(),
        "WindowMask width is restricted to the 1..=2 user-layout column range"
    );
    assert!(
        create_window_mask(
            30,
            &WindowMaskBody {
                height_cells: 7,
                ..Default::default()
            },
        )
        .is_err(),
        "WindowMask height is restricted to the 1..=6 user-layout row range"
    );

    let wrong_required_type = base()
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_type: 3,
                    options: 0x01,
                    required_objects: vec![ObjectID::new(31)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_number(31, &OutputNumberBody::default()).unwrap());
    assert!(
        wrong_required_type.validate().is_err(),
        "typed WindowMask required-object slots must reference the standard object type"
    );

    let valid_window_designators = base()
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    name: ObjectID::new(32),
                    window_title: ObjectID::new(33),
                    window_icon: ObjectID::new(34),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(31, &OutputStringBody::default()).unwrap())
        .with_object(create_object_pointer(
            32,
            &ObjectPointerBody {
                value: ObjectID::new(31),
            },
        ))
        .with_object(create_output_string(33, &OutputStringBody::default()).unwrap())
        .with_object(create_output_rectangle(34, &OutputRectangleBody::default()).unwrap());
    valid_window_designators.validate().unwrap();

    let wrong_window_name = base()
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    name: ObjectID::new(31),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_number(31, &OutputNumberBody::default()).unwrap());
    assert!(
        wrong_window_name.validate().is_err(),
        "WindowMask Name must reference OutputString or an ObjectPointer to OutputString"
    );

    let wrong_window_title = base()
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_title: ObjectID::new(31),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_number(31, &OutputNumberBody::default()).unwrap());
    assert!(
        wrong_window_title.validate().is_err(),
        "WindowMask Title must reference OutputString or an ObjectPointer to OutputString"
    );

    let wrong_window_icon = base()
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_icon: ObjectID::new(31),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_number_variable(31, &NumberVariableBody { value: 0 }));
    assert!(
        wrong_window_icon.validate().is_err(),
        "WindowMask Icon must reference an Object Label graphic representation object"
    );

    let missing_required_slot = base().with_object(
        create_window_mask(
            30,
            &WindowMaskBody {
                window_type: 3,
                options: 0x01,
                required_objects: Vec::new(),
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        missing_required_slot.validate().is_err(),
        "typed WindowMask bodies must supply the standard required-object count"
    );

    let free_form_with_required_objects = base()
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_type: 0,
                    options: 0x01,
                    required_objects: vec![ObjectID::new(31)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(31, &OutputStringBody::default()).unwrap());
    assert!(
        free_form_with_required_objects.validate().is_err(),
        "free-form WindowMask bodies must use positional children, not required-object slots"
    );

    let unsupported_typed_window_is_accepted_but_ignored = base().with_object(
        create_window_mask(
            30,
            &WindowMaskBody {
                window_type: 99,
                options: 0x01,
                required_objects: vec![ObjectID::new(404)],
                ..Default::default()
            },
        )
        .unwrap(),
    );
    unsupported_typed_window_is_accepted_but_ignored
        .validate()
        .unwrap();
}

#[test]
fn vt_object_pool_validation_checks_object_label_reference_list() {
    let base = || {
        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
            .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
            .with_object(create_string_variable(
                4,
                &StringVariableBody {
                    value: b"Speed".to_vec(),
                    ..Default::default()
                },
            ))
    };

    let valid = base().with_object(create_object_label_ref(
        5,
        &ObjectLabelRefBody {
            labels: vec![ObjectLabelRefEntry {
                labelled_object: ObjectID(3),
                string_variable: ObjectID(4),
                font_type: 1,
                graphic_designator: ObjectID::NULL,
            }],
        },
    ));
    valid.validate().unwrap();

    let valid_graphic_designator = base()
        .with_object(create_output_number(6, &OutputNumberBody::default()).unwrap())
        .with_object(create_object_label_ref(
            5,
            &ObjectLabelRefBody {
                labels: vec![ObjectLabelRefEntry {
                    labelled_object: ObjectID(3),
                    string_variable: ObjectID(4),
                    font_type: 1,
                    graphic_designator: ObjectID(6),
                }],
            },
        ));
    valid_graphic_designator.validate().unwrap();

    let duplicate_targets = base().with_object(create_object_label_ref(
        5,
        &ObjectLabelRefBody {
            labels: vec![
                ObjectLabelRefEntry {
                    labelled_object: ObjectID(3),
                    string_variable: ObjectID(4),
                    font_type: 1,
                    graphic_designator: ObjectID::NULL,
                },
                ObjectLabelRefEntry {
                    labelled_object: ObjectID(3),
                    string_variable: ObjectID::NULL,
                    font_type: 2,
                    graphic_designator: ObjectID::NULL,
                },
            ],
        },
    ));
    assert!(
        duplicate_targets.validate().is_err(),
        "an object label reference list must not label the same object twice"
    );

    let missing_string = base().with_object(create_object_label_ref(
        5,
        &ObjectLabelRefBody {
            labels: vec![ObjectLabelRefEntry {
                labelled_object: ObjectID(3),
                string_variable: ObjectID(99),
                font_type: 1,
                graphic_designator: ObjectID::NULL,
            }],
        },
    ));
    assert!(
        missing_string.validate().is_err(),
        "object label string references must resolve to StringVariable objects"
    );

    let wrong_graphic_designator_type = base().with_object(create_object_label_ref(
        5,
        &ObjectLabelRefBody {
            labels: vec![ObjectLabelRefEntry {
                labelled_object: ObjectID(3),
                string_variable: ObjectID(4),
                font_type: 1,
                graphic_designator: ObjectID(4),
            }],
        },
    ));
    assert!(
        wrong_graphic_designator_type.validate().is_err(),
        "object label graphic designators must be output/drawable representation objects, not StringVariable references"
    );

    let reserved_font_type = base().with_object(create_object_label_ref(
        5,
        &ObjectLabelRefBody {
            labels: vec![ObjectLabelRefEntry {
                labelled_object: ObjectID(3),
                string_variable: ObjectID(4),
                font_type: 3,
                graphic_designator: ObjectID::NULL,
            }],
        },
    ));
    assert!(
        reserved_font_type.validate().is_err(),
        "object label font types must stay on standard Annex K font-type codes when a label string is supplied"
    );

    let two_label_lists = base()
        .with_object(create_object_label_ref(5, &ObjectLabelRefBody::default()))
        .with_object(create_object_label_ref(6, &ObjectLabelRefBody::default()));
    assert!(
        two_label_lists.validate().is_err(),
        "an object pool must contain at most one object label reference list"
    );

    let two_special_controls = base()
        .with_object(
            create_working_set_special_controls(5, &WorkingSetSpecialControlsBody::default())
                .unwrap(),
        )
        .with_object(
            create_working_set_special_controls(6, &WorkingSetSpecialControlsBody::default())
                .unwrap(),
        );
    assert!(
        two_special_controls.validate().is_err(),
        "an object pool must contain at most one working set special controls object"
    );

    let invalid_language = base().with_object(
        create_working_set_special_controls(
            5,
            &WorkingSetSpecialControlsBody {
                languages: vec![LanguageCountryPair {
                    language: *b"e1",
                    country: *b"US",
                }],
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        invalid_language.validate().is_err(),
        "working set special controls language codes must be two ASCII letters"
    );

    let invalid_country = base().with_object(
        create_working_set_special_controls(
            5,
            &WorkingSetSpecialControlsBody {
                languages: vec![LanguageCountryPair {
                    language: *b"en",
                    country: *b"U1",
                }],
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        invalid_country.validate().is_err(),
        "working set special controls country codes must be two ASCII letters"
    );

    let not_applicable_country = base().with_object(
        create_working_set_special_controls(
            5,
            &WorkingSetSpecialControlsBody {
                languages: vec![LanguageCountryPair {
                    language: *b"en",
                    country: *b"  ",
                }],
                ..Default::default()
            },
        )
        .unwrap(),
    );
    assert!(
        not_applicable_country.validate().is_ok(),
        "working set special controls accepts the standard two-space country-code sentinel"
    );
}

#[test]
fn vt_object_pool_validation_checks_auxiliary_object_references() {
    let working_set = create_working_set(1, &WorkingSetBody::default()).with_children([2u16]);
    let data_mask = create_data_mask(2, &DataMaskBody::default());

    let valid_label = create_output_string(
        30,
        &OutputStringBody {
            value: b"aux".to_vec(),
            ..Default::default()
        },
    )
    .unwrap();
    let valid_pool = ObjectPool::default()
        .with_object(working_set.clone())
        .with_object(data_mask.clone())
        .with_object(
            create_aux_function(
                20,
                &AuxFunctionBody {
                    function_type: 1,
                    designator: ObjectID(21),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_aux_control_designator(
                21,
                &AuxControlDesignatorBody {
                    aux_object: ObjectID(20),
                    designator: b"F1".to_vec(),
                },
            )
            .unwrap(),
        )
        .with_object(
            create_aux_function2(
                22,
                &AuxFunction2Body {
                    function_type: 2,
                    name: ObjectID(30),
                    icon: ObjectID::NULL,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_aux_input2(
                23,
                &AuxInput2Body {
                    input_type: 1,
                    input_status: 1,
                    input_value: 123,
                    name: ObjectID(30),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(valid_label);
    valid_pool.validate().unwrap();

    let missing_designator = ObjectPool::default()
        .with_object(working_set.clone())
        .with_object(data_mask.clone())
        .with_object(
            create_aux_function(
                20,
                &AuxFunctionBody {
                    designator: ObjectID(99),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    assert!(
        missing_designator.validate().is_err(),
        "AUX function designators must point at uploaded designator objects"
    );

    let wrong_designator_type = ObjectPool::default()
        .with_object(working_set.clone())
        .with_object(data_mask.clone())
        .with_object(
            create_aux_function(
                20,
                &AuxFunctionBody {
                    designator: ObjectID(30),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                30,
                &OutputStringBody {
                    value: b"not-designator".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    assert!(
        wrong_designator_type.validate().is_err(),
        "AUX function designators must point at the expected object type"
    );

    let designator_to_non_aux_object = ObjectPool::default()
        .with_object(working_set.clone())
        .with_object(data_mask.clone())
        .with_object(
            create_aux_control_designator(
                21,
                &AuxControlDesignatorBody {
                    aux_object: ObjectID(2),
                    designator: b"bad".to_vec(),
                },
            )
            .unwrap(),
        );
    assert!(
        designator_to_non_aux_object.validate().is_err(),
        "AUX control designators must reference AUX function or input objects"
    );

    let aux_function2_missing_name = ObjectPool::default()
        .with_object(working_set)
        .with_object(data_mask)
        .with_object(
            create_aux_function2(
                22,
                &AuxFunction2Body {
                    name: ObjectID(99),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    assert!(
        aux_function2_missing_name.validate().is_err(),
        "AUX-N name/icon references must exist when they are not NULL"
    );
}
