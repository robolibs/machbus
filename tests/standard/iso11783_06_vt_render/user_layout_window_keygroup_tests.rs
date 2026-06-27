#[test]
fn render_runtime_object_pointer_retargets_preserve_key_slot_context() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID(3),
                    ..Default::default()
                },
            )
            .with_children([30u16]),
        )
        .with_object(
            create_soft_key_mask(3, &SoftKeyMaskBody::default()).with_children([41u16, 42u16]),
        )
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([40u16]),
        )
        .with_object(create_object_pointer(
            40,
            &ObjectPointerBody {
                value: ObjectID(31),
            },
        ))
        .with_object(create_object_pointer(
            41,
            &ObjectPointerBody {
                value: ObjectID(31),
            },
        ))
        .with_object(create_external_object_pointer(
            42,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID(31),
                external_reference_name: ObjectID::NULL,
                external_object_id: ObjectID::NULL,
            },
        ))
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
        ))
        .with_object(create_output_string(50, &OutputStringBody::default()).unwrap());
    pool.validate().unwrap();

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        DocConfig {
            physical_soft_key_count: 4,
            ..Default::default()
        },
    )
    .unwrap();

    let key_group_labels = |runtime: &VtRenderRuntime| {
        runtime
            .scene()
            .find(ObjectID(30))
            .and_then(|node| match &node.kind {
                NodeKind::KeyGroup { labels, .. } => Some(labels.clone()),
                _ => None,
            })
            .unwrap()
    };
    let soft_key_labels = |runtime: &VtRenderRuntime| {
        runtime
            .scene()
            .soft_keys
            .iter()
            .map(|key| key.label.clone())
            .collect::<Vec<_>>()
    };

    assert_eq!(key_group_labels(&runtime), vec!["31".to_string()]);
    assert_eq!(
        soft_key_labels(&runtime),
        vec!["31".to_string(), "31".to_string()]
    );

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID(40),
            value: 32,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID(2)
        })
    );
    assert_eq!(
        key_group_labels(&runtime),
        vec!["32".to_string()],
        "Key Group ObjectPointer retargets to Key objects must rebuild labels"
    );

    for (target, reason) in [(ObjectID::NULL.raw(), "NULL"), (50, "non-Key object")] {
        assert_eq!(
            runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                id: ObjectID(40),
                value: u32::from(target),
            }),
            Ok(RenderUpdate::Unchanged),
            "Key Group ObjectPointer retarget to {reason} must be rejected"
        );
        assert_eq!(key_group_labels(&runtime), vec!["32".to_string()]);
    }

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID(41),
            value: u32::from(ObjectID::NULL.raw()),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID(2)
        })
    );
    assert_eq!(
        soft_key_labels(&runtime),
        vec!["31".to_string()],
        "Soft Key Mask ObjectPointer retarget to NULL must become a reserved/empty slot"
    );

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID(41),
            value: 50,
        }),
        Ok(RenderUpdate::Unchanged),
        "Soft Key Mask ObjectPointer retarget to non-Key must be rejected"
    );
    assert_eq!(soft_key_labels(&runtime), vec!["31".to_string()]);

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID(41),
            value: 32,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID(2)
        })
    );
    assert_eq!(
        soft_key_labels(&runtime),
        vec!["32".to_string(), "31".to_string()]
    );

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID(42),
            attribute_id: 1,
            value: u32::from(ObjectID::NULL.raw()),
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID(2)
        })
    );
    assert_eq!(
        soft_key_labels(&runtime),
        vec!["32".to_string()],
        "Soft Key Mask External Object Pointer NULL default must become a reserved/empty slot"
    );

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID(42),
            attribute_id: 1,
            value: 50,
        }),
        Ok(RenderUpdate::Unchanged),
        "Soft Key Mask External Object Pointer default retarget to non-Key must be rejected"
    );
    assert_eq!(soft_key_labels(&runtime), vec!["32".to_string()]);

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID(42),
            attribute_id: 1,
            value: 32,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID(2)
        })
    );
    assert_eq!(
        soft_key_labels(&runtime),
        vec!["32".to_string(), "32".to_string()]
    );
}

#[test]
fn render_transparent_key_group_keeps_underlying_user_layout_area() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x03,
                    ..Default::default()
                },
            )
            .with_children([31u16]),
        )
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 21,
                ..Default::default()
            },
        ));
    pool.validate().unwrap();

    let engine = LayoutEngine::new(LayoutConfig::default())
        .with_placements(PlacementMap::new().set(30, 20, 30));
    let scene = render_with(&pool, &engine, ObjectID::NULL);
    let node = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(30))
        .expect("transparent key group is rendered");
    assert_eq!(node.rect, Rect::new(20, 30, 64, 40));
    assert!(matches!(
        &node.kind,
        NodeKind::KeyGroup {
            available: true,
            transparent: true,
            key_ids,
            key_numbers,
            labels,
        } if key_ids == &vec![ObjectID::new(31)]
            && key_numbers == &vec![21]
            && labels == &vec!["21".to_string()]
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(!commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::FillRect {
                rect,
                ..
            } if *rect == node.rect
        )
    }));
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::SoftKey {
                rect,
                label,
                ..
            } if *rect == node.rect && label == "21"
        )
    }));

    let mut input = InputRuntime::new();
    input.bind(&scene);
    assert_eq!(
        input.handle(&scene, &OperatorEvent::Tap(25, 35)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(31)
        }]
    );
}

#[test]
fn render_window_mask_uses_user_layout_cell_dimensions() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([ChildRef::new(
                ObjectID::new(30),
                240,
                40,
            )]),
        )
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    width_cells: 1,
                    height_cells: 2,
                    window_type: 0,
                    background_color: 7,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_children_pos([ChildRef::new(ObjectID::new(31), 0, 0)]),
        )
        .with_object(
            create_output_string(
                31,
                &OutputStringBody {
                    width: 30,
                    height: 12,
                    value: b"WIN".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    pool.validate().unwrap();

    let scene = render(&pool, ObjectID::NULL);
    let window = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(30))
        .expect("window mask is rendered");
    assert_eq!(window.rect, Rect::new(240, 40, 240, 80));
    assert!(matches!(
        &window.kind,
        NodeKind::Group {
            background: 7,
            transparent_bg: false,
            children,
        } if children.len() == 1 && children[0].id == ObjectID::new(31)
    ));

    let child = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(31))
        .expect("free-form window child is materialised");
    assert_eq!(child.rect, Rect::new(240, 40, 30, 12));
}

#[test]
fn render_transparent_window_mask_preserves_underlying_user_layout_area() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([ChildRef::new(
                ObjectID::new(30),
                240,
                40,
            )]),
        )
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    width_cells: 1,
                    height_cells: 1,
                    window_type: 0,
                    background_color: 7,
                    options: 0x03,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_children_pos([ChildRef::new(ObjectID::new(31), 0, 0)]),
        )
        .with_object(
            create_output_string(
                31,
                &OutputStringBody {
                    width: 30,
                    height: 12,
                    value: b"WIN".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    pool.validate().unwrap();

    let scene = render(&pool, ObjectID::NULL);
    let window = scene.find(ObjectID::new(30)).unwrap();
    assert!(matches!(
        &window.kind,
        NodeKind::Group {
            background: 7,
            transparent_bg: true,
            children,
        } if children.len() == 1
    ));
    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        !commands.iter().any(|command| matches!(
            command,
            RenderCommand::FillRect {
                rect,
                ..
            } if *rect == window.rect
        )),
        "transparent Window Mask must not paint its own background over the underlying user-layout area"
    );
    assert!(commands.iter().any(|command| {
        matches!(command, RenderCommand::DrawText { text, .. } if text == "WIN")
    }));
}

#[test]
fn render_unavailable_window_mask_blanks_its_cell_without_children() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    width_cells: 2,
                    height_cells: 1,
                    window_type: 0,
                    background_color: 4,
                    options: 0x00,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_children([31u16]),
        )
        .with_object(
            create_output_string(
                31,
                &OutputStringBody {
                    width: 30,
                    height: 12,
                    value: b"HIDE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    let window = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(30))
        .expect("window mask cell is still blanked");
    assert_eq!(window.rect, Rect::new(0, 0, 480, 40));
    assert!(!window.enabled);
    assert!(matches!(
        &window.kind,
        NodeKind::Group {
            background: 4,
            transparent_bg: false,
            children,
        } if children.is_empty()
    ));
    assert!(scene.find(ObjectID::new(31)).is_none());
}

#[test]
fn render_active_typed_window_mask_materialises_required_object_slots() {
    let pool = ObjectPool::default()
        .with_object(
            create_working_set(
                1,
                &WorkingSetBody {
                    active_mask: ObjectID::new(2),
                    ..Default::default()
                },
            )
            .with_children([100u16]),
        )
        .with_object(create_output_string(100, &OutputStringBody::default()).unwrap())
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_type: 10,
                    background_color: 5,
                    options: 0x01,
                    required_objects: vec![ObjectID::new(31), ObjectID::new(32)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_number(
                31,
                &OutputNumberBody {
                    width: 40,
                    height: 12,
                    value: 7,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                32,
                &OutputStringBody {
                    width: 40,
                    height: 12,
                    value: b"RIGHT".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    pool.validate().unwrap();

    let scene = render(&pool, ObjectID::new(30));
    assert_eq!(scene.active_mask, ObjectID::new(30));
    assert_eq!(scene.background, 5);
    assert_eq!(scene.mask_rect, Rect::new(0, 0, 480, 40));
    assert!(scene.unsupported.is_empty());
    assert!(matches!(
        &scene.find(ObjectID::new(31)).unwrap().kind,
        NodeKind::OutputNumber { text, .. } if text == "7"
    ));
    assert_eq!(
        scene.find(ObjectID::new(31)).unwrap().rect,
        Rect::new(0, 0, 40, 12)
    );
    assert!(matches!(
        &scene.find(ObjectID::new(32)).unwrap().kind,
        NodeKind::OutputString { text, .. } if text == "RIGHT"
    ));
    assert_eq!(
        scene.find(ObjectID::new(32)).unwrap().rect,
        Rect::new(240, 0, 40, 12)
    );
}

#[test]
fn render_runtime_key_group_pointer_events_emit_soft_key_activation_codes() {
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
                key_code: 21,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            32,
            &KeyBody {
                key_code: 22,
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

    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerDown(5, 5))
        .unwrap();
    assert_eq!(
        events,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Pressed,
        }]
    );
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].kind, VtBusMessageKind::SoftKeyActivation);
    assert_eq!(
        messages[0].as_bytes(),
        &[
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Pressed.as_u8(),
            31,
            0,
            30,
            0,
            21,
            0xFF,
        ]
    );

    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerUp(5, 45))
        .unwrap();
    assert_eq!(
        events,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Aborted,
        }]
    );
    assert_eq!(
        messages[0].as_bytes(),
        &[
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Aborted.as_u8(),
            31,
            0,
            30,
            0,
            21,
            0xFF,
        ]
    );

    let _ = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 45));
    let events = runtime.handle_operator_event(OperatorEvent::PointerUp(5, 45));
    assert_eq!(
        events,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(32),
            code: ActivationCode::Released,
        }]
    );
}

#[test]
fn render_runtime_physical_soft_keys_activate_user_layout_key_group_cells() {
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
                key_code: 21,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            32,
            &KeyBody {
                key_code: 22,
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

    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 2),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(1)),
        vec![VtEvent::Ignored {
            reason: "physical soft-key cell is not available"
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(2)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(31),
        }]
    );

    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PhysicalSoftKey(3))
        .unwrap();
    assert_eq!(
        events,
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(32),
        }]
    );
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages[0].as_bytes(),
        &[
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Pressed.as_u8(),
            32,
            0,
            30,
            0,
            22,
            0xFF,
        ]
    );
    assert_eq!(
        messages[1].as_bytes(),
        &[
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Released.as_u8(),
            32,
            0,
            30,
            0,
            22,
            0xFF,
        ]
    );
}

#[test]
fn render_unavailable_key_group_as_non_activating_user_layout_area() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(create_key_group(30, &KeyGroupBody::default()).with_children([31u16]))
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 21,
                ..Default::default()
            },
        ));
    pool.validate().unwrap();

    let engine = LayoutEngine::new(LayoutConfig::default())
        .with_placements(PlacementMap::new().set(30, 20, 30));
    let scene = render_with(&pool, &engine, ObjectID::NULL);
    let node = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(30))
        .expect("unavailable key group is still placed");
    assert!(!node.enabled);
    assert!(matches!(
        &node.kind,
        NodeKind::KeyGroup {
            available: false,
            ..
        }
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, RenderCommand::SoftKey { .. }))
    );

    let mut input = InputRuntime::new();
    input.bind(&scene);
    assert_eq!(
        input.handle(&scene, &OperatorEvent::Tap(25, 35)),
        vec![VtEvent::Ignored {
            reason: "tap missed all interactive nodes"
        }]
    );
}

#[test]
fn render_runtime_maps_soft_key_navigation_events_to_page_changes() {
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
        navigation_soft_key_count: 2,
        ..Default::default()
    };
    let mut runtime = VtRenderRuntime::from_pool(pool, config).unwrap();
    assert_eq!(runtime.soft_key_page(), 0);
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

    let next = runtime.handle_operator_event(OperatorEvent::SoftKeyNavigation(
        SoftKeyKind::NavigationNext,
    ));
    assert_eq!(
        next,
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
    assert_eq!(runtime.soft_key_page(), 1);
    assert!(runtime.is_dirty());

    runtime.clear_dirty();
    let previous = runtime.handle_operator_event(OperatorEvent::SoftKeyNavigation(
        SoftKeyKind::NavigationPrevious,
    ));
    assert_eq!(
        previous,
        vec![VtEvent::SoftKeyPageChanged {
            page: 0,
            page_count: 3,
        }]
    );
    assert_eq!(runtime.soft_key_page(), 0);
    assert!(runtime.is_dirty());

    let unchanged = runtime.handle_operator_event(OperatorEvent::SoftKeyNavigation(
        SoftKeyKind::NavigationPrevious,
    ));
    assert!(matches!(
        unchanged.as_slice(),
        [VtEvent::Ignored {
            reason: "soft-key page unchanged"
        }]
    ));
}

#[test]
fn runtime_allows_key_group_cells_after_trimmed_soft_key_null_slots() {
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
            .with_children([30u16]),
        )
        .with_object(create_key_group(
            30,
            &KeyGroupBody {
                options: 0x01,
                ..Default::default()
            },
        ).with_children([40u16, 41u16]))
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([
                ObjectID::new(6),
                ObjectID::new(7),
                ObjectID::new(8),
                ObjectID::new(9),
                ObjectID::new(20),
                ObjectID::new(21),
                ObjectID::new(22),
            ]),
        )
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
        ))
        .with_object(create_object_pointer(
            22,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ));
    for id in [6u16, 7, 8, 9, 40, 41] {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 6,
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
            .filter(|key| key.kind != SoftKeyKind::Application)
            .count(),
        0,
        "trailing NULL Soft Key Mask slots are trimmed before paging/nav cells are reserved"
    );
    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 4),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }),
        "Key Group placement must use the actual rendered navigation cells, not the raw child-count"
    );
}

#[test]
fn runtime_removes_key_group_placement_when_new_soft_key_mask_claims_cells() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(20),
                    ..Default::default()
                },
            )
            .with_children([30u16]),
        )
        .with_object(
            create_soft_key_mask(20, &SoftKeyMaskBody::default())
                .with_children([101u16, 102, 103, 104]),
        )
        .with_object(
            create_soft_key_mask(21, &SoftKeyMaskBody::default()).with_children([
                101u16, 102, 103, 104, 105, 106, 107, 108,
            ]),
        )
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([31u16, 32u16]),
        );
    for id in [31u16, 32, 101, 102, 103, 104, 105, 106, 107, 108] {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 6,
            navigation_soft_key_count: 2,
            ..Default::default()
        },
    )
    .unwrap();

    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 4),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.user_layout_placements(),
        vec![UserLayoutPlacement::KeyGroup {
            id: ObjectID::new(30),
            first_cell: 4,
        }]
    );

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeSoftKeyMask {
                data_mask: ObjectID::new(2),
                soft_key_mask: ObjectID::new(21),
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert!(
        runtime.user_layout_placements().is_empty(),
        "stale Key Group mapping must be removed when a later Soft Key Mask reserves those cells"
    );
    assert!(
        runtime
            .place_key_group_in_user_layout(ObjectID::new(30), 4)
            .is_err()
    );
}

#[test]
fn render_typed_window_mask_materialises_required_object_slot() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([ChildRef::new(
                ObjectID::new(30),
                120,
                40,
            )]),
        )
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_type: 3,
                    background_color: 6,
                    options: 0x01,
                    required_objects: vec![ObjectID::new(31)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                31,
                &OutputStringBody {
                    width: 50,
                    height: 12,
                    value: b"SLOT".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    pool.validate().unwrap();

    let scene = render(&pool, ObjectID::NULL);
    let window = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(30))
        .expect("typed window mask is rendered");
    assert_eq!(window.rect, Rect::new(120, 40, 240, 40));
    assert!(matches!(
        &window.kind,
        NodeKind::Group {
            background: 6,
            transparent_bg: false,
            children,
        } if children.len() == 1
            && children[0].id == ObjectID::new(31)
            && children[0].x == 0
            && children[0].y == 0
    ));

    let slot = scene
        .nodes
        .iter()
        .find(|node| node.id == ObjectID::new(31))
        .expect("required window object is materialised");
    assert_eq!(slot.rect, Rect::new(120, 40, 50, 12));
}

#[test]
fn render_runtime_routes_pointer_hits_on_soft_key_cells() {
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
        navigation_soft_key_count: 2,
        ..Default::default()
    };
    let mut runtime = VtRenderRuntime::from_pool(pool, config).unwrap();

    let stray_release = runtime.handle_operator_event(OperatorEvent::PointerUp(490, 190));
    assert!(matches!(
        stray_release.as_slice(),
        [VtEvent::Ignored {
            reason: "soft-key release did not match pressed cell"
        }]
    ));

    let app_down = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 10));
    assert!(matches!(
        app_down.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Pressed
        }] if *id == ObjectID::new(6)
    ));
    let mismatched_release = runtime.handle_operator_event(OperatorEvent::PointerUp(490, 190));
    assert!(matches!(
        mismatched_release.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Aborted
        }] if *id == ObjectID::new(6)
    ));
    assert_eq!(runtime.soft_key_page(), 0);

    let down = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 190));
    assert!(matches!(
        down.as_slice(),
        [VtEvent::Ignored {
            reason: "soft-key activation waits for release"
        }]
    ));
    assert_eq!(runtime.soft_key_page(), 0);

    let next = runtime.handle_operator_event(OperatorEvent::PointerUp(490, 190));
    assert_eq!(
        next,
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
    assert_eq!(runtime.soft_key_page(), 1);

    let application = runtime.handle_operator_event(OperatorEvent::Tap(490, 10));
    assert_eq!(
        application,
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(8),
        }]
    );
}

#[test]
fn render_runtime_routes_physical_soft_key_cells() {
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
        navigation_soft_key_count: 2,
        ..Default::default()
    };
    let mut runtime = VtRenderRuntime::from_pool(pool, config).unwrap();

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(0)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(6),
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(3)),
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(0)),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(8),
        }]
    );
    assert!(matches!(
        runtime
            .handle_operator_event(OperatorEvent::PhysicalSoftKey(9))
            .as_slice(),
        [VtEvent::Ignored {
            reason: "physical soft-key cell is not available"
        }]
    ));
}

#[test]
fn render_runtime_direct_soft_key_activation_requires_visible_enabled_target() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(
            2,
            &DataMaskBody {
                soft_key_mask: ObjectID::new(5),
                ..Default::default()
            },
        ))
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 6,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            7,
            &KeyBody {
                key_code: 7,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::SoftKeyActivate(ObjectID::new(6))),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(6),
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::SoftKeyActivate(ObjectID::new(7))),
        vec![VtEvent::Ignored {
            reason: "soft-key activation target is not visible"
        }],
        "host-provided direct soft-key IDs must be checked against the active scene"
    );
}

// ─── Operator input → VT events ────────────────────────────────────

#[test]
fn runtime_lowers_completed_vt_events_to_bus_payloads() {
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
            .with_children([3u16, 8u16]),
        )
        .with_object(
            create_input_boolean(
                3,
                &InputBooleanBody {
                    width: 20,
                    enabled: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 42,
                ..Default::default()
            },
        ))
        .with_object(create_button(
            8,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 9,
                ..Default::default()
            },
        ));

    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    let messages = runtime
        .bus_messages_for_events(&[
            VtEvent::FocusChanged {
                id: ObjectID::new(3),
            },
            VtEvent::BooleanValueChanged {
                id: ObjectID::new(3),
                value: true,
            },
            VtEvent::StringValueChanged {
                id: ObjectID::new(4),
                text: "OK".to_string(),
            },
            VtEvent::InputEsc {
                id: ObjectID::new(4),
                error_code: 0,
                transfer_sequence_number: None,
            },
            VtEvent::ListSelectionChanged {
                id: ObjectID::new(9),
                index: 2,
            },
            VtEvent::SoftKeyActivated {
                id: ObjectID::new(6),
            },
            VtEvent::ButtonActivated {
                id: ObjectID::new(8),
            },
            VtEvent::StringEditPreview {
                id: ObjectID::new(4),
                text: "preview".to_string(),
            },
            VtEvent::SoftKeyPageChanged {
                page: 1,
                page_count: 2,
            },
        ])
        .unwrap();

    let payloads = messages
        .iter()
        .map(|message| message.as_bytes().to_vec())
        .collect::<Vec<_>>();

    assert_eq!(
        payloads[0],
        vec![cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 0, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        payloads[1],
        vec![cmd::NUMERIC_VALUE_CHANGE, 3, 0, 0xFF, 1, 0, 0, 0]
    );
    assert_eq!(
        payloads[2],
        vec![cmd::STRING_VALUE_CHANGE, 4, 0, 2, 0, b'O', b'K']
    );
    assert_eq!(
        payloads[3],
        vec![cmd::VT_ESC, 4, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF]
    );
    assert_eq!(
        payloads[4],
        vec![cmd::NUMERIC_VALUE_CHANGE, 9, 0, 0xFF, 2, 0, 0, 0]
    );
    assert_eq!(
        payloads[5],
        vec![
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Pressed.as_u8(),
            6,
            0,
            5,
            0,
            42,
            0xFF
        ]
    );
    assert_eq!(
        payloads[6],
        vec![
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Released.as_u8(),
            6,
            0,
            5,
            0,
            42,
            0xFF
        ]
    );
    assert_eq!(
        payloads[7],
        vec![
            cmd::BUTTON_ACTIVATION,
            ActivationCode::Pressed.as_u8(),
            8,
            0,
            2,
            0,
            9,
            0xFF
        ]
    );
    assert_eq!(
        payloads[8],
        vec![
            cmd::BUTTON_ACTIVATION,
            ActivationCode::Released.as_u8(),
            8,
            0,
            2,
            0,
            9,
            0xFF
        ]
    );
    assert_eq!(payloads.len(), 9);
}

#[test]
fn runtime_lowers_activation_code_events_to_bus_payloads() {
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
            .with_children([8u16]),
        )
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 42,
                ..Default::default()
            },
        ))
        .with_object(create_button(
            8,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 9,
                ..Default::default()
            },
        ));

    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    let messages = runtime
        .bus_messages_for_events(&[
            VtEvent::SoftKeyActivation {
                id: ObjectID::new(6),
                code: ActivationCode::Held,
            },
            VtEvent::ButtonActivation {
                id: ObjectID::new(8),
                code: ActivationCode::Aborted,
            },
        ])
        .unwrap();

    let payloads = messages
        .iter()
        .map(|message| message.as_bytes().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(
        payloads,
        vec![
            vec![
                cmd::SOFT_KEY_ACTIVATION,
                ActivationCode::Held.as_u8(),
                6,
                0,
                5,
                0,
                42,
                0xFF
            ],
            vec![
                cmd::BUTTON_ACTIVATION,
                ActivationCode::Aborted.as_u8(),
                8,
                0,
                2,
                0,
                9,
                0xFF
            ],
        ]
    );
}

#[test]
fn runtime_rejects_activation_bus_events_for_disabled_scene_targets() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([8u16]))
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 42,
                ..Default::default()
            },
        ))
        .with_object(create_button(
            8,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 9,
                options: 0x10,
                ..Default::default()
            },
        ));
    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(
        runtime
            .bus_messages_for_events(&[VtEvent::ButtonActivated {
                id: ObjectID::new(8),
            }])
            .is_err(),
        "semantic activation lowering must not bypass disabled Button scene state"
    );
    assert!(
        runtime
            .bus_messages_for_events(&[VtEvent::ButtonActivation {
                id: ObjectID::new(8),
                code: ActivationCode::Pressed,
            }])
            .is_err(),
        "activation-code lowering must also reject disabled Button scene state"
    );
    assert!(
        runtime
            .bus_messages_for_events(&[VtEvent::SoftKeyActivated {
                id: ObjectID::new(6),
            }])
            .is_err(),
        "semantic soft-key lowering must not activate keys outside the active scene"
    );
    assert!(
        runtime
            .bus_messages_for_events(&[VtEvent::SoftKeyActivation {
                id: ObjectID::new(6),
                code: ActivationCode::Pressed,
            }])
            .is_err(),
        "activation-code soft-key lowering must also require an active visible cell"
    );
}

#[test]
fn runtime_lowers_vt_esc_transfer_sequence_number() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    let messages = runtime
        .bus_messages_for_events(&[VtEvent::InputEsc {
            id: ObjectID::new(3),
            error_code: 0x10,
            transfer_sequence_number: Some(0x0A),
        }])
        .unwrap();

    assert_eq!(
        messages[0].as_bytes(),
        &[cmd::VT_ESC, 3, 0, 0x10, 0xFF, 0xFF, 0xFF, 0xAF]
    );

    let err = runtime.bus_messages_for_events(&[VtEvent::InputEsc {
        id: ObjectID::new(3),
        error_code: 0,
        transfer_sequence_number: Some(0x10),
    }]);
    assert!(err.is_err());
    let err = runtime.messages_for_events(
        &[VtEvent::InputEsc {
            id: ObjectID::new(3),
            error_code: 0,
            transfer_sequence_number: Some(0x10),
        }],
        0x80,
        0x42,
    );
    assert!(
        err.is_err(),
        "full-message VT ESC emission must reject out-of-range VT6 TAN before CAN wrapping"
    );
}

#[test]
fn runtime_lowers_pointing_events_to_payload_and_full_messages() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    let messages = runtime
        .bus_messages_for_events(&[VtEvent::PointingEvent {
            x: 10,
            y: 20,
            touch_state: ActivationCode::Pressed,
            parent_mask: ObjectID::new(2),
            transfer_sequence_number: None,
        }])
        .unwrap();
    assert_eq!(messages[0].kind, VtBusMessageKind::PointingEvent);
    assert_eq!(
        messages[0].as_bytes(),
        &[cmd::POINTING_EVENT, 10, 0, 20, 0, 1, 0xFF, 0xFF]
    );

    let full = runtime
        .messages_for_events(
            &[VtEvent::PointingEvent {
                x: 10,
                y: 20,
                touch_state: ActivationCode::Held,
                parent_mask: ObjectID::new(2),
                transfer_sequence_number: Some(0x0A),
            }],
            0x80,
            0x42,
        )
        .unwrap();
    assert_eq!(
        full[0].data,
        vec![cmd::POINTING_EVENT, 10, 0, 20, 0, 0xA2, 2, 0]
    );

    assert!(
        runtime
            .bus_messages_for_events(&[VtEvent::PointingEvent {
                x: 0,
                y: 0,
                touch_state: ActivationCode::Aborted,
                parent_mask: ObjectID::new(2),
                transfer_sequence_number: None,
            }])
            .is_err()
    );
    assert!(
        runtime
            .bus_messages_for_events(&[VtEvent::PointingEvent {
                x: 0,
                y: 0,
                touch_state: ActivationCode::Pressed,
                parent_mask: ObjectID::new(2),
                transfer_sequence_number: Some(0x10),
            }])
            .is_err()
    );
}

#[test]
fn runtime_pointer_events_emit_pointing_for_non_interactive_mask_area() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let mut runtime = VtRenderRuntime::from_pool(pool.clone(), LayoutConfig::default()).unwrap();

    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerDown(10, 20))
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [VtEvent::PointingEvent {
            x: 10,
            y: 20,
            touch_state: ActivationCode::Pressed,
            parent_mask,
            transfer_sequence_number: None,
        }] if *parent_mask == ObjectID::new(2)
    ));
    assert_eq!(
        messages[0].as_bytes(),
        &[cmd::POINTING_EVENT, 10, 0, 20, 0, 1, 0xFF, 0xFF]
    );

    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerMove(12, 25))
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [VtEvent::PointingEvent {
            x: 12,
            y: 25,
            touch_state: ActivationCode::Held,
            parent_mask,
            transfer_sequence_number: None,
        }] if *parent_mask == ObjectID::new(2)
    ));
    assert_eq!(messages[0].as_bytes()[5], ActivationCode::Held.as_u8());

    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerUp(13, 26))
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [VtEvent::PointingEvent {
            x: 13,
            y: 26,
            touch_state: ActivationCode::Released,
            parent_mask,
            transfer_sequence_number: None,
        }] if *parent_mask == ObjectID::new(2)
    ));
    assert_eq!(messages[0].as_bytes()[5], ActivationCode::Released.as_u8());

    let button_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([8u16]))
        .with_object(create_button(
            8,
            &ButtonBody {
                width: 40,
                height: 20,
                ..Default::default()
            },
        ));
    let mut button_runtime =
        VtRenderRuntime::from_pool(button_pool, LayoutConfig::default()).unwrap();
    let (events, messages) = button_runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerDown(5, 5))
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));
    assert_eq!(messages[0].kind, VtBusMessageKind::ButtonActivation);
}

#[test]
fn runtime_pointing_prefers_free_form_window_mask_parent() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    window_type: 0,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool.clone(), LayoutConfig::default()).unwrap();

    let events = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        events.as_slice(),
        [VtEvent::PointingEvent {
            x: 5,
            y: 5,
            touch_state: ActivationCode::Pressed,
            parent_mask,
            transfer_sequence_number: None,
        }] if *parent_mask == ObjectID::new(30)
    ));
}

#[test]
fn runtime_reports_visible_user_layout_objects_with_hide_show_messages() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16, 40u16]))
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_key_group(40, &KeyGroupBody::default()).with_children([41u16]))
        .with_object(create_key(
            41,
            &KeyBody {
                key_code: 41,
                ..Default::default()
            },
        ));
    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.user_layout_hide_show_events(Some(0x0A)),
        vec![VtEvent::UserLayoutHideShow {
            first: (ObjectID::new(30), true),
            second: Some((ObjectID::new(40), false)),
            transfer_sequence_number: Some(0x0A),
        }]
    );

    let messages = runtime.user_layout_hide_show_messages(Some(0x0A)).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].kind, VtBusMessageKind::UserLayoutHideShow);
    assert_eq!(
        messages[0].as_bytes(),
        &[cmd::USER_LAYOUT_HIDE_SHOW, 30, 0, 1, 40, 0, 0, 0xAF]
    );

    let full = runtime
        .user_layout_hide_show_full_messages(0x80, 0x42, None)
        .unwrap();
    assert_eq!(full.len(), 1);
    assert_eq!(
        full[0].data.as_slice(),
        &[cmd::USER_LAYOUT_HIDE_SHOW, 30, 0, 1, 40, 0, 0, 0xFF]
    );

    assert!(runtime.user_layout_hide_show_messages(Some(0x10)).is_err());
}

#[test]
fn runtime_reports_active_masks_with_hide_show_messages_for_inactive_visible_working_set() {
    let pool = ObjectPool::default()
        .with_object(
            create_working_set(
                1,
                &WorkingSetBody {
                    active_mask: ObjectID::new(2),
                    ..Default::default()
                },
            )
            .with_children([2u16]),
        )
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(20),
                    ..Default::default()
                },
            )
            .with_children([30u16]),
        )
        .with_object(create_soft_key_mask(20, &SoftKeyMaskBody::default()).with_children([21u16]))
        .with_object(create_key(21, &KeyBody::default()))
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.active_mask_hide_show_events(true, Some(0x03)),
        vec![VtEvent::UserLayoutHideShow {
            first: (ObjectID::new(2), true),
            second: Some((ObjectID::new(20), true)),
            transfer_sequence_number: Some(0x03),
        }]
    );

    let shown = runtime
        .active_mask_hide_show_messages(true, Some(0x03))
        .unwrap();
    assert_eq!(shown.len(), 1);
    assert_eq!(
        shown[0].as_bytes(),
        &[cmd::USER_LAYOUT_HIDE_SHOW, 2, 0, 1, 20, 0, 1, 0x3F]
    );

    let hidden = runtime
        .active_mask_hide_show_full_messages(false, 0x80, 0x42, None)
        .unwrap();
    assert_eq!(hidden.len(), 1);
    assert_eq!(
        hidden[0].data.as_slice(),
        &[cmd::USER_LAYOUT_HIDE_SHOW, 2, 0, 0, 20, 0, 0, 0xFF]
    );
}
