#[test]
fn render_runtime_allows_null_soft_key_mask_change_attribute() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(
            2,
            &DataMaskBody {
                soft_key_mask: ObjectID::new(3),
                ..Default::default()
            },
        ))
        .with_object(
            create_soft_key_mask(3, &SoftKeyMaskBody::default()).with_children([4u16]),
        )
        .with_object(create_key(
            4,
            &KeyBody {
                key_code: 4,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(runtime.scene().soft_keys.len(), 1);
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(2),
                attribute_id: 2,
                value: u32::from(ObjectID::NULL.raw()),
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert!(
        runtime.scene().soft_keys.is_empty(),
        "Data Mask AID 2 uses NULL to clear the associated Soft Key Mask and clear the soft-key designators"
    );
}

#[test]
fn render_runtime_rejects_null_required_font_attribute_references() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 5u16]))
        .with_object(create_font_attributes(
            4,
            &FontAttributesBody {
                font_color: 1,
                ..Default::default()
            },
        ))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 20,
                    height: 10,
                    font_attributes: ObjectID::new(4),
                    value: b"ok".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_boolean(
                5,
                &InputBooleanBody {
                    foreground: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(3),
                attribute_id: 4,
                value: u32::from(ObjectID::NULL.raw()),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "Output String Font Attributes AID 4 has standard range 0..=65534, so NULL must not be retained"
    );
    let body = runtime
        .pool()
        .find(ObjectID::new(3))
        .unwrap()
        .get_output_string_body()
        .unwrap();
    assert_eq!(body.font_attributes, ObjectID::new(4));

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(5),
                attribute_id: 3,
                value: u32::from(ObjectID::NULL.raw()),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "Input Boolean foreground Font Attributes AID 3 has standard range 0..=65534, so NULL must not be retained"
    );
    let body = runtime
        .pool()
        .find(ObjectID::new(5))
        .unwrap()
        .get_input_boolean_body()
        .unwrap();
    assert_eq!(body.foreground, ObjectID::new(4));
}

#[test]
fn render_runtime_rejects_null_required_polygon_line_attribute_reference() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_line_attributes(4, &LineAttributesBody::default()))
        .with_object(
            create_output_polygon(
                3,
                &OutputPolygonBody {
                    width: 20,
                    height: 10,
                    line_attributes: ObjectID::new(4),
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 20, y: 0 },
                        PolygonPoint { x: 0, y: 10 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(3),
                attribute_id: 3,
                value: u32::from(ObjectID::NULL.raw()),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "Output Polygon Line Attributes AID 3 has standard range 0..=65534, so NULL must not be retained"
    );
    let body = runtime
        .pool()
        .find(ObjectID::new(3))
        .unwrap()
        .get_output_polygon_body()
        .unwrap();
    assert_eq!(body.line_attributes, ObjectID::new(4));
}

#[test]
fn render_runtime_applies_generic_input_attributes_to_scene() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default())
                .with_children([3u16, 4u16, 5u16, 6u16, 7u16, 8u16]),
        )
        .with_object(create_number_variable(20, &NumberVariableBody { value: 1 }))
        .with_object(create_number_variable(21, &NumberVariableBody { value: 1 }))
        .with_object(create_string_variable(
            22,
            &StringVariableBody {
                length: 3,
                value: b"ecu".to_vec(),
            },
        ))
        .with_object(
            create_input_boolean(
                3,
                &InputBooleanBody {
                    width: 8,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_boolean(
                7,
                &InputBooleanBody {
                    width: 8,
                    value: 0,
                    enabled: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_string(
                4,
                &InputStringBody {
                    width: 30,
                    height: 10,
                    options: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_number(
                5,
                &InputNumberBody {
                    width: 30,
                    height: 10,
                    value: 12,
                    options: 1,
                    options2: 3,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                6,
                &InputListBody {
                    width: 30,
                    height: 10,
                    value: 1,
                    options: 3,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                8,
                &InputListBody {
                    width: 30,
                    height: 10,
                    value: 1,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 2,
            value: 16,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 4,
            value: 20,
        })
        .unwrap();
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(7),
                attribute_id: 5,
                value: 1,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(7),
                attribute_id: 6,
                value: 0,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(5),
                attribute_id: 14,
                value: 8,
            })
            .unwrap(),
        RenderUpdate::Unchanged
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(5),
                attribute_id: 15,
                value: 1,
            })
            .unwrap(),
        RenderUpdate::Unchanged
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(6),
                attribute_id: 5,
                value: 1,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(8),
                attribute_id: 4,
                value: 0,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 7,
            value: 22,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(5),
            attribute_id: 10,
            value: 0.5f32.to_bits(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(5),
            attribute_id: 11,
            value: 1,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(6),
            attribute_id: 3,
            value: 21,
        })
        .unwrap();
    let boolean = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(boolean.rect.w, 16);
    assert_eq!(boolean.rect.h, 16);
    assert!(matches!(
        &boolean.kind,
        NodeKind::InputBoolean { value: true, .. }
    ));

    let direct_boolean = runtime.scene().find(ObjectID::new(7)).unwrap();
    assert!(
        !direct_boolean.enabled,
        "Input Boolean enabled (AID 6) is settable via Change Attribute, matching the reference VT stack"
    );
    assert!(matches!(
        &direct_boolean.kind,
        NodeKind::InputBoolean {
            enabled: false,
            value: true
        }
    ));

    let input_text = runtime.scene().find(ObjectID::new(4)).unwrap();
    assert!(matches!(
        &input_text.kind,
        NodeKind::InputString { text, .. } if text == "ecu"
    ));

    let input_number = runtime.scene().find(ObjectID::new(5)).unwrap();
    assert!(matches!(
        &input_number.kind,
        NodeKind::InputNumber {
            real_time_editing: true,
            text,
            ..
        } if text == "6.0"
    ));

    let input_list = runtime.scene().find(ObjectID::new(6)).unwrap();
    assert!(matches!(
        &input_list.kind,
        NodeKind::InputList {
            real_time_editing: false,
            selected: 1,
            item_count: 2,
            ..
        }
    ));
    let direct_input_list = runtime.scene().find(ObjectID::new(8)).unwrap();
    assert!(matches!(
        &direct_input_list.kind,
        NodeKind::InputList {
            selected: 0,
            item_count: 2,
            ..
        }
    ));
}

#[test]
fn render_runtime_applies_generic_style_attribute_objects_to_scene() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 7u16, 8u16]),
        )
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 30,
                    height: 10,
                    font_attributes: ObjectID::new(4),
                    value: b"abc".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(
            4,
            &FontAttributesBody {
                font_color: 1,
                font_size: 1,
                font_type: 0,
                font_style: 0,
            },
        ))
        .with_object(create_line_attributes(
            5,
            &LineAttributesBody {
                line_color: 1,
                line_width: 1,
                line_art: 0xFFFF,
            },
        ))
        .with_object(
            create_fill_attributes(
                6,
                &FillAttributesBody {
                    fill_type: 0,
                    fill_color: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_line(
                7,
                &OutputLineBody {
                    width: 12,
                    height: 1,
                    line_attributes: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                8,
                &OutputRectangleBody {
                    width: 12,
                    height: 12,
                    line_attributes: ObjectID::new(5),
                    fill_attributes: ObjectID::new(6),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 1,
            value: 2,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 2,
            value: 3,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(5),
            attribute_id: 2,
            value: 7,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(6),
            attribute_id: 1,
            value: 2,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(6),
            attribute_id: 2,
            value: 3,
        })
        .unwrap();

    let palette = Palette::default_isobus();
    let text = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(text.style.foreground, palette.resolve(2));
    assert_eq!(text.style.font, FontMetrics::for_size(3));

    let line = runtime.scene().find(ObjectID::new(7)).unwrap();
    assert_eq!(line.style.line_width, 7);

    let rect = runtime.scene().find(ObjectID::new(8)).unwrap();
    assert_eq!(rect.style.line_width, 7);
    assert_eq!(rect.style.fill_type, FillType::FillColour);
    assert_eq!(rect.style.fill_colour, palette.resolve(3));
}

#[test]
fn render_runtime_applies_generic_graphic_attributes_to_scene() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 5u16, 8u16, 10u16]),
        )
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 4,
                    actual_width: 4,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1; 12],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                4,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![2; 4],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                6,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![9; 4],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                5,
                &ScaledGraphicBody {
                    width: 2,
                    height: 2,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    options: 0,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                7,
                &OutputRectangleBody {
                    width: 3,
                    height: 3,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_animation(
                8,
                &AnimationBody {
                    width: 3,
                    height: 3,
                    refresh_interval_ms: 100,
                    value: 0,
                    enabled: 1,
                    first_child_index: 0,
                    default_child_index: 0,
                    last_child_index: 0,
                    options: 1,
                },
            )
            .unwrap()
            .with_children_pos([ChildRef::at_origin(ObjectID::new(7))]),
        )
        .with_object(
            create_graphic_context(
                10,
                &GraphicContextBody {
                    viewport_width: 10,
                    viewport_height: 10,
                    viewport_x: 0,
                    viewport_y: 0,
                    canvas_width: 10,
                    canvas_height: 10,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for read_only_canvas_aid in [5, 6] {
        assert_eq!(
            runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(10),
                attribute_id: read_only_canvas_aid,
                value: 20,
            }),
            Ok(RenderUpdate::Unchanged),
            "Graphics Context Canvas Width/Height are read-only AIDs"
        );
    }
    for (id, attribute_id, value) in [
        (3, 1, 6),
        (3, 2, 1),
        (3, 3, 1),
        (5, 1, 8),
        (5, 2, 4),
        (8, 3, 250),
        (10, 1, 12),
        (10, 3, 5),
        (10, 16, 1),
    ] {
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(id),
                attribute_id,
                value,
            })
            .unwrap();
    }

    assert_eq!(runtime.animation_refresh_interval_ms(), Some(250));
    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::IndexedImage {
            rect: Rect { w: 6, h: 2, .. },
            width: 4,
            height: 2,
            transparency: 1,
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::IndexedImage {
            rect: Rect { w: 8, h: 4, .. },
            width: 2,
            height: 2,
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextCanvas {
            rect: Rect {
                x: 5,
                w: 12,
                h: 10,
                ..
            },
            canvas_width: 10,
            canvas_height: 10,
            background: 0,
            transparent: true,
            ..
        }
    )));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(5),
            value: 6,
        })
        .unwrap();
    let commands = runtime.render(&GtuiRenderer::default());
    assert!(
        commands.iter().any(|command| matches!(
            command,
            RenderCommand::IndexedImage {
                object_id,
                data,
                ..
            } if *object_id == ObjectID::new(5) && data.iter().all(|byte| *byte == 9)
        )),
        "ScaledGraphic Change Numeric Value retargets the standard graphic Value object reference"
    );

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(5),
            value: ObjectID::NULL.raw() as u32,
        })
        .unwrap();
    assert!(
        runtime.scene().find(ObjectID::new(5)).is_none(),
        "ScaledGraphic NULL value is the standard no-graphic case, not an unsupported placeholder"
    );
    assert!(
        runtime
            .scene()
            .unsupported
            .iter()
            .all(|record| record.id != ObjectID::new(5)),
        "ScaledGraphic NULL value should not be reported as a missing GraphicData error"
    );
}

#[test]
fn render_runtime_applies_generic_shape_and_gauge_attributes_to_scene() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default())
                .with_children([3u16, 4u16, 5u16, 6u16, 7u16, 8u16, 9u16, 10u16]),
        )
        .with_object(
            create_output_line(
                3,
                &OutputLineBody {
                    width: 10,
                    height: 10,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                4,
                &OutputRectangleBody {
                    width: 10,
                    height: 10,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_ellipse(
                5,
                &OutputEllipseBody {
                    width: 10,
                    height: 10,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_polygon(
                6,
                &OutputPolygonBody {
                    width: 10,
                    height: 10,
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 10, y: 0 },
                        PolygonPoint { x: 0, y: 10 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_meter(
                7,
                &MeterBody {
                    width: 20,
                    max_value: 50,
                    value: 25,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_linear_bar_graph(
                8,
                &LinearBarGraphBody {
                    width: 20,
                    height: 10,
                    max_value: 50,
                    value: 25,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_arched_bar_graph(
                9,
                &ArchedBarGraphBody {
                    width: 20,
                    height: 10,
                    max_value: 50,
                    value: 25,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_button(
            10,
            &ButtonBody {
                width: 20,
                height: 10,
                ..Default::default()
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    for command in [
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 4,
            value: 1,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 4,
            value: 0x05,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(5),
            attribute_id: 4,
            value: 1,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(6),
            attribute_id: 1,
            value: 80,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(7),
            attribute_id: 2,
            value: 7,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(7),
            attribute_id: 9,
            value: 10,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(8),
            attribute_id: 8,
            value: 60,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(9),
            attribute_id: 2,
            value: 88,
        },
        VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(10),
            attribute_id: 1,
            value: 90,
        },
    ] {
        assert!(runtime.apply_ecu_command(&command).is_ok());
    }

    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputLine { direction } if *direction == 1
    ));
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(4)).unwrap().kind,
        NodeKind::OutputRectangle {
            line_suppression,
            ..
        } if *line_suppression == 0x05
    ));
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(5)).unwrap().kind,
        NodeKind::OutputEllipse {
            closed: true,
            ellipse_type: 1,
            ..
        }
    ));
    assert_eq!(runtime.scene().find(ObjectID::new(6)).unwrap().rect.w, 80);
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(7)).unwrap().kind,
        NodeKind::Meter {
            needle_colour,
            min_value,
            ..
        } if *needle_colour == 7 && *min_value == 10
    ));
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(8)).unwrap().kind,
        NodeKind::LinearBarGraph { max_value, .. } if *max_value == 60
    ));
    assert_eq!(runtime.scene().find(ObjectID::new(9)).unwrap().rect.h, 88);
    assert_eq!(runtime.scene().find(ObjectID::new(10)).unwrap().rect.w, 90);
}

#[test]
fn render_runtime_applies_change_end_point_to_output_line() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_line(
                3,
                &OutputLineBody {
                    width: 10,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeEndPoint {
            id: ObjectID::new(3),
            width: 30,
            height: 40,
            line_direction: 1,
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));

    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.rect.w, 30);
    assert_eq!(node.rect.h, 40);
    assert!(matches!(
        node.kind,
        NodeKind::OutputLine { direction } if direction == 1
    ));
    let commands = runtime.render_commands(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Line {
            x0: 29,
            y0: 0,
            x1: 0,
            y1: 39,
            ..
        }
    )));

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeEndPoint {
                id: ObjectID::new(3),
                width: 1,
                height: 1,
                line_direction: 2,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "reserved line directions must not mutate render state"
    );
    assert_eq!(runtime.scene().find(ObjectID::new(3)).unwrap().rect.w, 30);
}

#[test]
fn render_runtime_tracks_object_label_metadata_without_redraw() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16]))
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 40,
                    height: 12,
                    value: b"abc".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            11,
            &StringVariableBody {
                length: 5,
                value: b"Speed".to_vec(),
            },
        ))
        .with_object(create_object_label_ref(
            13,
            &ObjectLabelRefBody {
                labels: vec![ObjectLabelRefEntry {
                    labelled_object: ObjectID::new(3),
                    string_variable: ObjectID::new(11),
                    font_type: 1,
                    graphic_designator: ObjectID::NULL,
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let label = ObjectLabelState {
        string_variable: ObjectID::new(11),
        font_type: 2,
        graphic_designator: ObjectID::NULL,
    };
    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeObjectLabel {
                id: ObjectID::new(3),
                label,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting { .. }
    ));
    assert_eq!(runtime.object_label(ObjectID::new(3)), Some(label));
    assert_eq!(
        runtime.object_label_text(ObjectID::new(3)).as_deref(),
        Some("Speed")
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeObjectLabel {
                id: ObjectID::new(3),
                label: ObjectLabelState {
                    string_variable: ObjectID::new(11),
                    font_type: 3,
                    graphic_designator: ObjectID::NULL,
                },
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "direct runtime Change Object Label must reject reserved Annex K font-type values"
    );
    assert_eq!(runtime.object_label(ObjectID::new(3)), Some(label));
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeObjectLabel {
                id: ObjectID::new(5),
                label,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "direct runtime Change Object Label must target an object declared by the Object Label Reference List"
    );
    assert_eq!(runtime.object_label(ObjectID::new(5)), None);
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeObjectLabel {
                id: ObjectID::new(3),
                label: ObjectLabelState {
                    string_variable: ObjectID::NULL,
                    font_type: 3,
                    graphic_designator: ObjectID::NULL,
                },
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "direct runtime Change Object Label must reject reserved Annex K font-type values even with a NULL string reference"
    );
    assert_eq!(runtime.object_label(ObjectID::new(3)), Some(label));
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeObjectLabel {
                id: ObjectID::new(3),
                label,
            })
            .unwrap(),
        RenderUpdate::Unchanged
    );
}

#[test]
fn render_runtime_external_object_pointer_uses_default_and_updates_standard_attributes() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([20u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 48,
                    height: 12,
                    value: b"old".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    width: 48,
                    height: 12,
                    value: b"new".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_external_reference_name(
            30,
            &ExternalReferenceNameBody {
                options: 1,
                name0: 0x1122_3344,
                name1: 0x5566_7788,
            },
        ))
        .with_object(create_external_object_pointer(
            20,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(3),
                external_reference_name: ObjectID::new(30),
                external_object_id: ObjectID::NULL,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputString { text, .. } if text == "old"
    ));
    assert!(runtime.scene().find(ObjectID::new(20)).is_none());

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(20),
            attribute_id: 1,
            value: 4,
        })
        .unwrap();
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(4)).unwrap().kind,
        NodeKind::OutputString { text, .. } if text == "new"
    ));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(20),
            value: (77_u32 << 16) | u32::from(30_u16),
        })
        .unwrap();
    let body = runtime
        .pool()
        .find(ObjectID::new(20))
        .unwrap()
        .get_external_object_pointer_body()
        .unwrap();
    assert_eq!(body.external_reference_name, ObjectID::new(30));
    assert_eq!(body.external_object_id, ObjectID::new(77));
    assert!(
        runtime.scene().find(ObjectID::new(4)).is_some(),
        "without an external-pool resolver, the renderer keeps drawing the local default object"
    );
}

#[test]
fn render_runtime_updates_external_definition_and_reference_metadata() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    value: b"a".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    value: b"b".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_external_object_definition(
                31,
                &ExternalObjectDefinitionBody {
                    options: 0,
                    name0: 0,
                    name1: 0,
                    object_ids: vec![ObjectID::new(3)],
                },
            )
            .unwrap(),
        )
        .with_object(create_external_reference_name(
            30,
            &ExternalReferenceNameBody {
                options: 0,
                name0: 0,
                name1: 0,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(31),
            attribute_id: 2,
            value: 0xAABB_CCDD,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(30),
            attribute_id: 1,
            value: 1,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeListItem {
            list: ObjectID::new(31),
            index: 0,
            item: ObjectID::new(4),
        })
        .unwrap();

    let definition = runtime
        .pool()
        .find(ObjectID::new(31))
        .unwrap()
        .get_external_object_definition_body()
        .unwrap();
    assert_eq!(definition.name0, 0xAABB_CCDD);
    assert_eq!(definition.object_ids, vec![ObjectID::new(4)]);
    let reference = runtime
        .pool()
        .find(ObjectID::new(30))
        .unwrap()
        .get_external_reference_name_body()
        .unwrap();
    assert_eq!(reference.options & 0x01, 1);
}

#[test]
fn render_runtime_materialises_initial_object_label_reference_list() {
    let label = ObjectLabelRefEntry {
        labelled_object: ObjectID::new(3),
        string_variable: ObjectID::new(11),
        font_type: 4,
        graphic_designator: ObjectID::NULL,
    };
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16]))
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 40,
                    height: 12,
                    value: b"abc".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            11,
            &StringVariableBody {
                value: b"Speed".to_vec(),
                ..Default::default()
            },
        ))
        .with_object(create_object_label_ref(
            12,
            &ObjectLabelRefBody {
                labels: vec![label],
            },
        ));

    let runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime.object_label(ObjectID::new(3)),
        Some(ObjectLabelState {
            string_variable: ObjectID::new(11),
            font_type: 4,
            graphic_designator: ObjectID::NULL,
        })
    );
    assert_eq!(
        runtime.object_label_text(ObjectID::new(3)).as_deref(),
        Some("Speed")
    );
}

#[test]
fn render_runtime_from_server_working_set_materialises_render_state() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([
                ChildRef::new(ObjectID::new(5), 1, 2),
                ChildRef::new(ObjectID::new(12), 4, 5),
            ]),
        )
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 80,
                    height: 12,
                    variable_reference: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            10,
            &StringVariableBody {
                length: 6,
                value: b"old   ".to_vec(),
            },
        ))
        .with_object(
            create_input_string(
                12,
                &InputStringBody {
                    width: 40,
                    height: 12,
                    options: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_graphic_context(11, &GraphicContextBody::default()).unwrap())
        .with_object(
            create_alarm_mask(
                13,
                &AlarmMaskBody {
                    priority: 2,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut ws = machbus::isobus::vt::ServerWorkingSet {
        pool,
        pool_uploaded: true,
        pool_activated: true,
        ..Default::default()
    };
    ws.object_state
        .string_values
        .insert(ObjectID::new(10), "server".to_owned());
    ws.object_state.visibility.insert(ObjectID::new(5), false);
    ws.object_state
        .child_positions
        .insert((ObjectID::new(2), ObjectID::new(5)), (30, 40));
    ws.object_state
        .attributes
        .insert((ObjectID::new(3), 1), 120);
    ws.object_state.object_labels.insert(
        ObjectID::new(3),
        ObjectLabelState {
            string_variable: ObjectID::new(10),
            font_type: 1,
            graphic_designator: ObjectID::NULL,
        },
    );
    ws.object_state.selected_input_object = ObjectID::new(12);
    ws.object_state.priorities.insert(ObjectID::new(13), 1);
    ws.object_state
        .graphics_contexts
        .push(machbus::isobus::vt::GraphicsContextCommand {
            object_id: ObjectID::new(11),
            subcommand: 0,
            payload: vec![1, 0, 2, 0],
        });

    let runtime = VtRenderRuntime::from_server_working_set(&ws, DocConfig::default()).unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.rect.x, 30);
    assert_eq!(node.rect.y, 40);
    assert_eq!(node.rect.w, 120);
    assert!(!node.visible);
    assert!(!runtime.scene().find(ObjectID::new(5)).unwrap().visible);
    assert!(matches!(
        &node.kind,
        NodeKind::OutputString { text, .. } if text == "server"
    ));
    assert_eq!(
        runtime.object_label_text(ObjectID::new(3)).as_deref(),
        Some("server")
    );
    assert_eq!(runtime.input().selected_input(), Some(ObjectID::new(12)));
    assert_eq!(runtime.graphics_context_commands().len(), 1);
    assert_eq!(
        runtime.graphics_context_commands()[0].object_id,
        ObjectID::new(11)
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(13))
            .unwrap()
            .get_alarm_mask_body()
            .unwrap()
            .priority,
        1,
        "server-retained Change Priority must materialise into the hosted runtime pool"
    );
    assert!(
        runtime
            .render_commands(&GtuiRenderer::default())
            .iter()
            .any(|command| matches!(
                command,
                RenderCommand::GraphicsContextReplay {
                    object_id,
                    subcommand: 0,
                    payload,
                } if *object_id == ObjectID::new(11) && payload.as_slice() == [1, 0, 2, 0]
            ))
    );
}

#[test]
fn render_runtime_change_priority_updates_alarm_mask_metadata_only() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_alarm_mask(
                3,
                &AlarmMaskBody {
                    priority: 2,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangePriority {
            id: ObjectID::new(3),
            priority: 0,
        })
        .unwrap();
    assert!(matches!(update, RenderUpdate::NotRenderAffecting { .. }));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(3))
            .unwrap()
            .get_alarm_mask_body()
            .unwrap()
            .priority,
        0
    );

    let invalid = runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangePriority {
            id: ObjectID::new(3),
            priority: 3,
        })
        .unwrap();
    assert_eq!(invalid, RenderUpdate::Unchanged);
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(3))
            .unwrap()
            .get_alarm_mask_body()
            .unwrap()
            .priority,
        0,
        "invalid priority values must not change Alarm Mask metadata"
    );
}

#[test]
fn render_runtime_rejects_malformed_graphics_context_payload_without_recording() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16, 11u16]))
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap())
        .with_object(create_string_variable(
            6,
            &StringVariableBody {
                length: 2,
                value: b"ok".to_vec(),
            },
        ))
        .with_object(
            create_picture_graphic(
                7,
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
        .with_object(create_graphic_context(11, &GraphicContextBody::default()).unwrap());

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x00,
                payload: vec![1, 0],
            })
            .is_err()
    );
    assert!(runtime.graphics_context_commands().is_empty());
    assert!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x15,
                payload: Vec::new(),
            })
            .is_err(),
        "unknown Graphics Context subcommands must not be retained as inert replay state"
    );
    assert!(runtime.graphics_context_commands().is_empty());
    assert!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x0F,
                payload: f32::NAN.to_bits().to_le_bytes().to_vec(),
            })
            .is_err()
    );
    assert!(runtime.graphics_context_commands().is_empty());
    let mut invalid_pan_zoom = vec![1, 0, 2, 0];
    invalid_pan_zoom.extend(33.0f32.to_bits().to_le_bytes());
    assert!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x10,
                payload: invalid_pan_zoom,
            })
            .is_err()
    );
    assert!(runtime.graphics_context_commands().is_empty());
    assert!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x12,
                payload: 6u16.to_le_bytes().to_vec(),
            })
            .is_err(),
        "Draw VT Object must reject non-drawable reference objects"
    );
    assert!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x13,
                payload: ObjectID::NULL.raw().to_le_bytes().to_vec(),
            })
            .is_err(),
        "Copy Canvas must reject NULL Picture Graphic targets"
    );
    assert!(runtime.graphics_context_commands().is_empty());

    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![1, 0, 2, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x12,
            payload: 5u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 7u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    assert_eq!(runtime.graphics_context_commands().len(), 3);
}

#[test]
fn render_runtime_applies_recorded_server_effects_in_order() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16]))
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 80,
                    height: 12,
                    variable_reference: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            10,
            &StringVariableBody {
                length: 3,
                value: b"old".to_vec(),
            },
        ))
        .with_object(create_object_label_ref(
            13,
            &ObjectLabelRefBody {
                labels: vec![ObjectLabelRefEntry {
                    labelled_object: ObjectID::new(3),
                    string_variable: ObjectID::new(10),
                    font_type: 0,
                    graphic_designator: ObjectID::NULL,
                }],
            },
        ))
        .with_object(create_graphic_context(11, &GraphicContextBody::default()).unwrap());

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let updates = runtime
        .apply_server_effects(&[
            ServerRenderEffect::ChangeStringValue {
                id: ObjectID::new(10),
                text: "bus".to_owned(),
            },
            ServerRenderEffect::HideShow {
                id: ObjectID::new(5),
                visible: false,
            },
            ServerRenderEffect::ChangeObjectLabel {
                id: ObjectID::new(3),
                label: ObjectLabelState {
                    string_variable: ObjectID::new(10),
                    font_type: 1,
                    graphic_designator: ObjectID::NULL,
                },
            },
            ServerRenderEffect::GraphicsContext {
                id: ObjectID::new(11),
                subcommand: 0x0D,
                payload: vec![1, 2, b'o', b'k'],
            },
            ServerRenderEffect::Esc,
        ])
        .unwrap();

    assert!(matches!(
        updates.as_slice(),
        [
            RenderUpdate::SceneRebuilt { .. },
            RenderUpdate::SceneRebuilt { .. },
            RenderUpdate::NotRenderAffecting { .. },
            RenderUpdate::CommandStreamChanged { .. },
            RenderUpdate::Unchanged,
        ]
    ));
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert!(!node.visible);
    assert!(matches!(
        &node.kind,
        NodeKind::OutputString { text, .. } if text == "bus"
    ));
    assert_eq!(
        runtime.object_label_text(ObjectID::new(3)).as_deref(),
        Some("bus")
    );
    assert_eq!(runtime.graphics_context_commands().len(), 1);
    assert_eq!(
        runtime
            .graphics_context_commands_for(ObjectID::new(11))
            .next(),
        Some(&machbus::isobus::vt::GraphicsContextCommand {
            object_id: ObjectID::new(11),
            subcommand: 0x0D,
            payload: vec![1, 2, b'o', b'k'],
        })
    );
    assert!(runtime.is_dirty());
    assert!(
        runtime
            .render(&GtuiRenderer::default())
            .iter()
            .any(|command| matches!(
                command,
                RenderCommand::GraphicsContextReplay {
                    object_id,
                    subcommand: 0x0D,
                    payload,
                } if *object_id == ObjectID::new(11)
                    && payload.as_slice() == [1, 2, b'o', b'k']
            ))
    );
}

#[test]
fn render_runtime_replays_fixture_command_trace_from_server_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16]))
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            4,
            &StringVariableBody {
                length: 4,
                value: b"old ".to_vec(),
            },
        ));

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
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer, source));
    let end_response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_vt_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(end_response[0].data[1], 0);
    assert_eq!(
        server.clients()[0]
            .pool
            .find(ObjectID::new(5))
            .map(|object| object.r#type),
        Some(ObjectType::Container),
        "fixture command trace Hide/Show target must stay a standard Container"
    );

    for fixture in [
        "change_string_variable_4_live",
        "hide_container_5",
        "show_container_5",
    ] {
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            parse_named_hex_bytes(VT_RENDER_TRACE_HEX, fixture),
            source,
        ));
    }

    let ws = &server.clients()[0];
    assert_eq!(
        ws.object_state.accepted_effects,
        vec![
            ServerRenderEffect::ChangeStringValue {
                id: ObjectID::new(4),
                text: "live".to_owned(),
            },
            ServerRenderEffect::HideShow {
                id: ObjectID::new(5),
                visible: false,
            },
            ServerRenderEffect::HideShow {
                id: ObjectID::new(5),
                visible: true,
            },
        ]
    );

    let runtime = VtRenderRuntime::from_server_working_set(ws, DocConfig::default()).unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert!(node.visible);
    assert!(matches!(
        &node.kind,
        NodeKind::OutputString { text, .. } if text == "live"
    ));
    let framebuffer = FramebufferRenderer::default()
        .render_runtime(&runtime)
        .expect("fixture command trace renders to framebuffer");
    assert_eq!(framebuffer.width(), DocConfig::default().canvas.0);
    assert_eq!(framebuffer.height(), DocConfig::default().canvas.1);
}

#[test]
fn render_runtime_expands_graphics_context_draw_text_to_backend_text_command() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 60,
                    viewport_height: 16,
                    viewport_x: 4,
                    viewport_y: 5,
                    canvas_width: 80,
                    canvas_height: 30,
                    options: 0,
                    ..GraphicContextBody::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0D,
            payload: vec![0, 5, b'H', b'E', b'L', b'L', b'O'],
        })
        .unwrap();
    assert!(matches!(update, RenderUpdate::CommandStreamChanged { .. }));

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextReplay {
            object_id,
            subcommand: 0x0D,
            payload,
        } if *object_id == ObjectID::new(11)
            && payload.as_slice() == [0, 5, b'H', b'E', b'L', b'L', b'O']
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect {
            rect: Rect {
                x: 4,
                y: 5,
                w: 60,
                h: 16
            },
            ..
        }
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText {
            rect: Rect {
                x: 4,
                y: 5,
                w: 60,
                h: 16
            },
            text,
            ..
        } if text == "HELLO"
    )));

    let framebuffer = FramebufferRenderer::default()
        .render_runtime(&runtime)
        .expect("runtime graphics-context command stream renders to framebuffer");
    assert!(framebuffer.count_colour(Colour::rgb(0, 0, 0)) > 0);
}
