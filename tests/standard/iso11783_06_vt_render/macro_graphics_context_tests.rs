#[test]
fn render_runtime_expands_graphics_context_polygon_command() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
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
        )
        .with_object(
            create_fill_attributes(
                12,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 18,
                    fill_pattern: ObjectID::NULL,
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x00,
            payload: vec![2, 0, 3, 0],
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x05,
            payload: 12u16.to_le_bytes().to_vec(),
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x0C,
            payload: vec![
                3, // number of points
                10, 0, 0, 0, // first point: +10, +0 from original cursor
                10, 0, 10, 0, // second point: +10, +10
                0, 0, 0, 0, // closed back at original cursor
            ],
        })
        .unwrap();

    let commands = runtime.render(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::Polygon {
            origin: (6, 8),
            points,
            filled: true,
            fill_colour,
            width: 1,
            ..
        } if points.as_slice() == [(6, 8), (16, 8), (16, 18), (6, 8)]
            && *fill_colour == Palette::default_isobus().resolve(18)
    )));
}

#[test]
fn render_runtime_executes_macro_effects_against_scene_state() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([5u16]))
        .with_object(create_container(5, &ContainerBody::default()).with_children([3u16]))
        .with_object(
            create_output_number(
                3,
                &OutputNumberBody {
                    width: 40,
                    height: 12,
                    variable_reference: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_number_variable(10, &NumberVariableBody { value: 1 }))
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![
                    MacroCommand {
                        command_type: cmd::CHANGE_NUMERIC_VALUE,
                        parameters: vec![10, 0, 0xFF, 42, 0, 0, 0],
                    },
                    MacroCommand {
                        command_type: cmd::HIDE_SHOW,
                        parameters: vec![5, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                ],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert!(!node.visible);
    assert!(matches!(
        &node.kind,
        NodeKind::OutputNumber { text, .. } if text == "42"
    ));
}

#[test]
fn render_runtime_executes_macro_geometry_and_background_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16]),
        )
        .with_object(
            create_fill_attributes(
                6,
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
                    width: 4,
                    height: 4,
                    fill_attributes: ObjectID::new(6),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                4,
                &OutputRectangleBody {
                    width: 4,
                    height: 4,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_polygon(
                5,
                &OutputPolygonBody {
                    width: 20,
                    height: 20,
                    polygon_type: 3,
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 10, y: 10 },
                        PolygonPoint { x: 20, y: 20 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_map(
                7,
                &ColourMapBody {
                    entries: vec![1, 0],
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![
                    MacroCommand {
                        command_type: cmd::SELECT_COLOUR_MAP,
                        parameters: vec![7, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_BACKGROUND_COLOUR,
                        parameters: vec![2, 0, 7, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_CHILD_LOCATION,
                        parameters: vec![2, 0, 3, 0, 5, 6, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_CHILD_POSITION,
                        parameters: vec![2, 0, 4, 0, 0xFE, 0xFF, 9, 0],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_SIZE,
                        parameters: vec![3, 0, 20, 0, 10, 0, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_POLYGON_SCALE,
                        parameters: vec![5, 0, 30, 0, 40, 0, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_POLYGON_POINT,
                        parameters: vec![5, 0, 1, 15, 0, 16, 0],
                    },
                ],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    assert_eq!(runtime.scene().background, 7);
    assert_eq!(
        runtime.scene().find(ObjectID::new(3)).unwrap().rect,
        Rect::new(5, 6, 20, 10)
    );
    assert_eq!(
        runtime
            .scene()
            .find(ObjectID::new(3))
            .unwrap()
            .style
            .fill_colour,
        Colour::rgb(0, 0, 0)
    );
    assert_eq!(
        runtime.scene().find(ObjectID::new(4)).unwrap().rect,
        Rect::new(-2, 9, 4, 4)
    );
    let polygon = runtime.scene().find(ObjectID::new(5)).unwrap();
    assert_eq!(polygon.rect, Rect::new(0, 13, 30, 40));
    assert!(matches!(
        &polygon.kind,
        NodeKind::OutputPolygon { points, .. }
            if points == &vec![(0, 0), (15, 16), (30, 40)]
    ));
}

#[test]
fn render_runtime_macro_updates_graphic_context_background_colour() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 12,
                    viewport_height: 8,
                    canvas_width: 12,
                    canvas_height: 8,
                    options: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            12,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_BACKGROUND_COLOUR,
                    parameters: vec![11, 0, 9, 0xFF, 0xFF, 0xFF, 0xFF],
                }],
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(12),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    let expected = Palette::default_isobus().resolve(9);
    assert_eq!(
        runtime
            .scene()
            .find(ObjectID::new(11))
            .unwrap()
            .style
            .background,
        expected
    );
    let commands = runtime.render_commands(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextCanvas {
            object_id,
            background: 9,
            ..
        } if *object_id == ObjectID::new(11)
    )));
}

#[test]
fn render_runtime_executes_macro_change_end_point_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_line(
                3,
                &OutputLineBody {
                    width: 8,
                    height: 9,
                    line_direction: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_END_POINT,
                    parameters: vec![3, 0, 16, 0, 17, 0, 1],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    let line = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!((line.rect.w, line.rect.h), (16, 17));
    assert!(matches!(
        &line.kind,
        NodeKind::OutputLine { direction } if *direction == 1
    ));
}

#[test]
fn render_runtime_executes_macro_soft_key_mask_and_list_item_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_list(
                3,
                &OutputListBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    items: vec![ObjectID::NULL],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_soft_key_mask(4, &SoftKeyMaskBody::default()).with_children([10u16]))
        .with_object(create_key(10, &KeyBody::default()))
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    width: 30,
                    height: 12,
                    value: b"Macro".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![
                    MacroCommand {
                        command_type: cmd::CHANGE_SOFT_KEY_MASK,
                        parameters: vec![1, 2, 0, 4, 0, 0xFF, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_LIST_ITEM,
                        parameters: vec![3, 0, 0, 11, 0, 0xFF, 0xFF],
                    },
                ],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(runtime.scene().soft_keys.is_empty());
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputList { selected_text, .. } if selected_text.as_deref() == Some("")
    ));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    assert_eq!(runtime.scene().soft_keys.len(), 1);
    assert_eq!(runtime.scene().soft_keys[0].id, ObjectID::new(10));
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputList {
            selected_text,
            selected_item_materialized,
            ..
        } if selected_text.as_deref() == Some("Macro") && *selected_item_materialized
    ));
    assert_eq!(
        runtime.scene().find(ObjectID::new(11)).unwrap().parent,
        ObjectID::new(3)
    );
}

#[test]
fn render_runtime_macro_soft_key_mask_requires_matching_mask_type() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(create_soft_key_mask(4, &SoftKeyMaskBody::default()).with_children([10u16]))
        .with_object(create_key(10, &KeyBody::default()))
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_SOFT_KEY_MASK,
                    // Mask Type 2 names Alarm Mask, so it must not mutate a Data Mask.
                    parameters: vec![2, 2, 0, 4, 0, 0xFF, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(runtime.scene().soft_keys.is_empty());
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::Unchanged)
    );
    assert!(
        runtime.scene().soft_keys.is_empty(),
        "wrong mask-type macro replay must not attach a soft-key mask"
    );
}

#[test]
fn render_runtime_executes_macro_priority_and_mask_lock_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 3u16]))
        .with_object(create_data_mask(
            2,
            &DataMaskBody {
                background_color: 1,
                ..Default::default()
            },
        ))
        .with_object(
            create_alarm_mask(
                3,
                &AlarmMaskBody {
                    priority: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![
                    MacroCommand {
                        command_type: cmd::LOCK_UNLOCK_MASK,
                        parameters: vec![1, 2, 0, 100, 0, 0xFF, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_BACKGROUND_COLOUR,
                        parameters: vec![2, 0, 9, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                    MacroCommand {
                        command_type: cmd::CHANGE_PRIORITY,
                        parameters: vec![3, 0, 2, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                ],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(runtime.scene().background, 1);

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(2))
            .unwrap()
            .get_data_mask_body()
            .unwrap()
            .background_color,
        9,
        "macro commands still mutate the backing pool while the mask is locked"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(3))
            .unwrap()
            .get_alarm_mask_body()
            .unwrap()
            .priority,
        2
    );
    assert_eq!(
        runtime.scene().background,
        1,
        "visible active mask stays frozen until the macro lock expires or unlocks"
    );

    assert_eq!(
        runtime.advance_mask_lock_time(100),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(runtime.scene().background, 9);
}

#[test]
fn render_runtime_executes_macro_select_input_object_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 80,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_string(
                4,
                &InputStringBody {
                    options: 0x01,
                    width: 80,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::SELECT_INPUT_OBJECT_COMMAND,
                    parameters: vec![4, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_ne!(runtime.input().selected_input(), Some(ObjectID::new(4)));

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert_eq!(runtime.input().selected_input(), Some(ObjectID::new(4)));
    assert_eq!(runtime.input().open_input(), None);
}

#[test]
fn render_runtime_executes_macro_audio_effects_as_terminal_side_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    background_color: 1,
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
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
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![
                    MacroCommand {
                        command_type: cmd::CONTROL_AUDIO_SIGNAL,
                        parameters: vec![2, 0x34, 0x12, 0x78, 0x56, 0xBC, 0x9A],
                    },
                    MacroCommand {
                        command_type: cmd::SET_AUDIO_VOLUME,
                        parameters: vec![80, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                    },
                ],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert_eq!(runtime.scene().background, 1);
}

#[test]
fn render_runtime_executes_macro_change_object_label_effects() {
    let initial_label = ObjectLabelRefEntry {
        labelled_object: ObjectID::new(3),
        string_variable: ObjectID::new(11),
        font_type: 1,
        graphic_designator: ObjectID::NULL,
    };
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
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
                length: 4,
                value: b"Old ".to_vec(),
            },
        ))
        .with_object(create_string_variable(
            12,
            &StringVariableBody {
                length: 4,
                value: b"New ".to_vec(),
            },
        ))
        .with_object(create_object_label_ref(
            13,
            &ObjectLabelRefBody {
                labels: vec![initial_label],
            },
        ))
        .with_object(create_macro(
            20,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_OBJECT_LABEL,
                    parameters: vec![3, 0, 12, 0, 2, 0xFF, 0xFF],
                }],
            },
        ))
        .with_object(create_macro(
            21,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_OBJECT_LABEL,
                    parameters: vec![3, 0, 11, 0, 3, 0xFF, 0xFF],
                }],
            },
        ))
        .with_object(create_macro(
            22,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_OBJECT_LABEL,
                    parameters: vec![3, 0, 0xFF, 0xFF, 3, 0xFF, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.object_label(ObjectID::new(3)),
        Some(ObjectLabelState {
            string_variable: ObjectID::new(11),
            font_type: 1,
            graphic_designator: ObjectID::NULL,
        })
    );

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(20),
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert_eq!(
        runtime.object_label(ObjectID::new(3)),
        Some(ObjectLabelState {
            string_variable: ObjectID::new(12),
            font_type: 2,
            graphic_designator: ObjectID::NULL,
        })
    );
    assert_eq!(
        runtime.object_label_text(ObjectID::new(3)).as_deref(),
        Some("New ")
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
                id: ObjectID::new(21),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "macro Change Object Label effects must reject reserved Annex K font-type values"
    );
    assert_eq!(
        runtime.object_label(ObjectID::new(3)),
        Some(ObjectLabelState {
            string_variable: ObjectID::new(12),
            font_type: 2,
            graphic_designator: ObjectID::NULL,
        })
    );
    assert_eq!(
        runtime.object_label_text(ObjectID::new(3)).as_deref(),
        Some("New ")
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
                id: ObjectID::new(22),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "macro Change Object Label effects must reject reserved Annex K font-type values even with a NULL string reference"
    );
    assert_eq!(
        runtime.object_label(ObjectID::new(3)),
        Some(ObjectLabelState {
            string_variable: ObjectID::new(12),
            font_type: 2,
            graphic_designator: ObjectID::NULL,
        })
    );
}

#[test]
fn render_runtime_executes_nested_macro_effects_with_recursion_guard() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    background_color: 1,
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
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
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::EXECUTE_MACRO,
                    parameters: vec![10, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                }],
            },
        ))
        .with_object(create_macro(
            10,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_BACKGROUND_COLOUR,
                    parameters: vec![2, 0, 9, 0xFF, 0xFF, 0xFF, 0xFF],
                }],
            },
        ))
        .with_object(create_macro(
            11,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::EXECUTE_MACRO,
                    parameters: vec![11, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(runtime.scene().background, 1);

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert_eq!(runtime.scene().background, 9);

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(11),
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert_eq!(runtime.scene().background, 9);
}

#[test]
fn render_runtime_executes_macro_delete_object_pool_lifecycle_effect() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    background_color: 3,
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
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
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::DELETE_OBJECT_POOL,
                    parameters: vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(!runtime.pool().is_empty());
    assert_eq!(runtime.scene().background, 3);

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::NULL
        })
    ));
    assert!(runtime.pool().is_empty());
    assert_eq!(runtime.active_mask(), ObjectID::NULL);
    assert!(
        runtime
            .scene()
            .unsupported
            .iter()
            .any(|record| record.reason.contains("active mask not present"))
    );
}

#[test]
fn render_runtime_executes_macro_generic_attribute_effects() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 12,
                    height: 12,
                    line_suppression: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_ATTRIBUTE,
                    parameters: vec![3, 0, 4, 0x05, 0, 0, 0],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputRectangle {
            line_suppression: 0,
            ..
        }
    ));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputRectangle {
            line_suppression: 0x05,
            ..
        }
    ));
}

#[test]
fn render_runtime_macro_change_string_value_rejects_invalid_utf8() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    value: b"OK".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_STRING_VALUE,
                    parameters: vec![3, 0, 1, 0, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
                id: ObjectID::new(9),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "macro Change String Value must reject invalid UTF-8 instead of lossy-decoding into retained text"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(3))
            .unwrap()
            .get_output_string_body()
            .unwrap()
            .value,
        b"OK"
    );
}

#[test]
fn render_runtime_executes_macro_attribute_object_value_effects() {
    let macro_body = MacroBody {
        commands: vec![
            MacroCommand {
                command_type: cmd::CHANGE_FONT_ATTRIBUTES,
                parameters: vec![6, 0, 3, 4, 1, 0x11, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_LINE_ATTRIBUTES,
                parameters: vec![7, 0, 5, 6, 0xAA, 0x55, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_FILL_ATTRIBUTES,
                parameters: vec![8, 0, 2, 7, 0xFF, 0xFF, 0xFF],
            },
        ],
    };
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([
                ChildRef::new(ObjectID::new(3), 0, 0),
                ChildRef::new(ObjectID::new(4), 0, 20),
            ]),
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
        .with_object(create_line_attributes(7, &LineAttributesBody::default()))
        .with_object(create_fill_attributes(8, &FillAttributesBody::default()).unwrap())
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 40,
                    height: 20,
                    font_attributes: ObjectID::new(6),
                    value: b"txt".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                4,
                &OutputRectangleBody {
                    width: 20,
                    height: 20,
                    line_attributes: ObjectID::new(7),
                    fill_attributes: ObjectID::new(8),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(20, &macro_body));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(20),
        })
        .unwrap();

    assert!(matches!(update, RenderUpdate::SceneRebuilt { .. }));
    let text = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(text.style.font, FontMetrics::for_size(4));
    assert!(text.style.decoration.bold);
    assert!(text.style.decoration.inverted);
    let rect = runtime.scene().find(ObjectID::new(4)).unwrap();
    assert_eq!(rect.style.line_width, 6);
    assert_eq!(rect.style.line_art, 0x55AA);
    assert_eq!(rect.style.fill_type, FillType::FillColour);
}

#[test]
fn render_runtime_executes_macros_bound_to_object_event_refs_in_order() {
    let mut number = create_output_number(
        3,
        &OutputNumberBody {
            width: 40,
            height: 12,
            variable_reference: ObjectID::new(10),
            ..Default::default()
        },
    )
    .unwrap();
    number.add_macro(0x05, 9);
    number.add_macro(0x05, 11);

    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(number)
        .with_object(create_number_variable(10, &NumberVariableBody { value: 1 }))
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_NUMERIC_VALUE,
                    parameters: vec![10, 0, 0xFF, 42, 0, 0, 0],
                }],
            },
        ))
        .with_object(create_macro(
            11,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_NUMERIC_VALUE,
                    parameters: vec![10, 0, 0xFF, 99, 0, 0, 0],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(
        runtime
            .execute_macro_event(ObjectID::new(3), 0x06)
            .unwrap()
            .is_empty()
    );

    let updates = runtime.execute_macro_event(ObjectID::new(3), 0x05).unwrap();
    assert_eq!(updates.len(), 2);
    assert!(updates.iter().all(|update| matches!(
        update,
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID(2)
        }
    )));
    assert!(matches!(
        runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::OutputNumber { ref text, .. } if text == "99"
    ));
}

#[test]
fn render_runtime_executes_macro_change_active_mask_for_current_working_set() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 4u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_data_mask(4, &DataMaskBody::default()).with_children([5u16]))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    value: b"first".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"second".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_ACTIVE_MASK,
                    parameters: vec![1, 0, 4, 0, 0xFF, 0xFF, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(runtime.scene().active_mask, ObjectID::new(2));
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(4)
        })
    );
    assert_eq!(runtime.scene().active_mask, ObjectID::new(4));
    assert!(runtime.scene().find(ObjectID::new(3)).is_none());
    assert!(runtime.scene().find(ObjectID::new(5)).is_some());
}

#[test]
fn render_runtime_ignores_macro_change_active_mask_for_other_working_set() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 4u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(create_data_mask(4, &DataMaskBody::default()))
        .with_object(create_macro(
            9,
            &MacroBody {
                commands: vec![MacroCommand {
                    command_type: cmd::CHANGE_ACTIVE_MASK,
                    parameters: vec![99, 0, 4, 0, 0xFF, 0xFF, 0xFF],
                }],
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ExecuteMacro {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::Unchanged)
    );
    assert_eq!(runtime.scene().active_mask, ObjectID::new(2));
}

#[test]
fn render_runtime_lock_unlock_mask_defers_visible_active_mask_refresh() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(
            2,
            &DataMaskBody {
                background_color: 1,
                ..Default::default()
            },
        ))
        .with_object(create_soft_key_mask(3, &SoftKeyMaskBody::default()));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(runtime.scene().background, 1);
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(3),
            locked: true,
            timeout_ms: 100,
        }),
        Ok(RenderUpdate::Unchanged),
        "Lock/Unlock Mask does not freeze Soft Key Masks"
    );

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: true,
            timeout_ms: 100,
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeBackgroundColour {
            id: ObjectID::new(2),
            colour: 9,
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(2))
            .unwrap()
            .get_data_mask_body()
            .unwrap()
            .background_color,
        9,
        "commands still mutate the backing pool while the visible mask is locked"
    );
    assert_eq!(
        runtime.scene().background,
        1,
        "the retained visible scene must remain frozen until unlock"
    );

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: false,
            timeout_ms: 0,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(runtime.scene().background, 9);
}

#[test]
fn render_runtime_mask_lock_timeout_releases_deferred_refresh() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(
            2,
            &DataMaskBody {
                background_color: 1,
                ..Default::default()
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: true,
            timeout_ms: 100,
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeBackgroundColour {
            id: ObjectID::new(2),
            colour: 9,
        }),
        Ok(RenderUpdate::NotRenderAffecting { .. })
    ));
    assert_eq!(runtime.scene().background, 1);

    assert_eq!(runtime.advance_mask_lock_time(99), RenderUpdate::Unchanged);
    assert_eq!(runtime.scene().background, 1);
    assert_eq!(
        runtime.advance_mask_lock_time(1),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(runtime.scene().background, 9);
}

#[test]
fn render_runtime_zero_mask_lock_timeout_waits_for_explicit_unlock() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(
            2,
            &DataMaskBody {
                background_color: 1,
                ..Default::default()
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: true,
            timeout_ms: 0,
        })
        .unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeBackgroundColour {
            id: ObjectID::new(2),
            colour: 9,
        })
        .unwrap();

    assert_eq!(
        runtime.advance_mask_lock_time(10_000),
        RenderUpdate::Unchanged
    );
    assert_eq!(runtime.scene().background, 1);
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: false,
            timeout_ms: 0,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(runtime.scene().background, 9);
}

#[test]
fn render_runtime_scales_polygon_points_with_size_change_algorithm() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_polygon(
                3,
                &OutputPolygonBody {
                    width: 100,
                    height: 100,
                    polygon_type: 3,
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 50, y: 50 },
                        PolygonPoint { x: 100, y: 100 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangePolygonScale {
            id: ObjectID::new(3),
            width: 200,
            height: 50,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.rect.w, 200);
    assert_eq!(node.rect.h, 50);
    assert!(matches!(
        &node.kind,
        NodeKind::OutputPolygon { points, .. }
            if points == &vec![(0, 0), (100, 25), (200, 50)]
    ));
}

#[test]
fn render_runtime_select_colour_map_remaps_resolved_styles() {
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
            create_colour_map(
                7,
                &ColourMapBody {
                    entries: vec![1, 0],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(255, 255, 255));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::SelectColourMap {
            id: ObjectID::new(7),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(0, 0, 0));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectColourMap { id: ObjectID::NULL })
        .unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(255, 255, 255));
}

#[test]
fn render_runtime_select_colour_palette_replaces_rgb_entries() {
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
                    entries_argb: vec![0xFF_FF_00_00],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_palette(
                9,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_00_00_FF],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(0xFF, 0x00, 0x00));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::SelectColourMap {
            id: ObjectID::new(9),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(0x00, 0x00, 0xFF));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectColourMap { id: ObjectID::NULL })
        .unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(255, 255, 255));
}

#[test]
fn render_runtime_colour_map_updates_framebuffer_indexed_images() {
    let red = Colour::rgb(0xFF, 0x00, 0x00);
    let green = Colour::rgb(0x00, 0xFF, 0x00);
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![0],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_palette(
                8,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_FF_00_00, 0xFF_00_FF_00],
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
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let fb = FramebufferRenderer::default()
        .render_runtime(&runtime)
        .unwrap();
    assert_eq!(fb.pixel(0, 0), Some(red));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectColourMap {
            id: ObjectID::new(9),
        })
        .unwrap();
    let fb = FramebufferRenderer::default()
        .render_runtime(&runtime)
        .unwrap();
    assert_eq!(fb.pixel(0, 0), Some(green));
}

#[test]
fn render_runtime_applies_working_set_special_controls_initial_colours() {
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
                    colour_map: ObjectID::new(9),
                    colour_palette: ObjectID::new(8),
                    languages: Vec::new(),
                    extra_bytes: Vec::new(),
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(0x00, 0x00, 0xFF));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectColourMap { id: ObjectID::NULL })
        .unwrap();
    let node = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_colour, Colour::rgb(255, 255, 255));
}
