#[test]
fn render_runtime_rejects_output_list_item_retargets_to_metadata_objects() {
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
                    items: vec![ObjectID::new(12)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Visible".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(11, &FontAttributesBody::default()))
        .with_object(create_object_pointer(
            12,
            &ObjectPointerBody {
                value: ObjectID::new(10),
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeListItem {
                list: ObjectID::new(3),
                index: 0,
                item: ObjectID::new(11),
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "Output List items must not retarget to style metadata that cannot be displayed"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(3))
            .unwrap()
            .get_output_list_body()
            .unwrap()
            .items,
        vec![ObjectID::new(12)]
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
                id: ObjectID::new(12),
                value: 11,
            })
            .unwrap(),
        RenderUpdate::Unchanged,
        "Object Pointer retargets must preserve Output List item displayability"
    );
    assert_eq!(
        runtime
            .pool()
            .find(ObjectID::new(12))
            .unwrap()
            .get_object_pointer_body()
            .unwrap()
            .value,
        ObjectID::new(10)
    );
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Visible")
    ));
}

#[test]
fn render_output_list_follows_standard_no_display_item_rules() {
    for (name, item, extra) in [
        (
            "null placeholder",
            ObjectID::NULL,
            Vec::<machbus::isobus::vt::VTObject>::new(),
        ),
        (
            "null object pointer",
            ObjectID::new(10),
            vec![create_object_pointer(
                10,
                &ObjectPointerBody {
                    value: ObjectID::NULL,
                },
            )],
        ),
        (
            "hidden container",
            ObjectID::new(10),
            vec![create_container(
                10,
                &ContainerBody {
                    width: 20,
                    height: 10,
                    hidden: true,
                },
            )],
        ),
    ] {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
            .with_object(
                create_output_list(
                    3,
                    &OutputListBody {
                        width: 80,
                        height: 20,
                        value: 0,
                        items: vec![item],
                        ..Default::default()
                    },
                )
                .unwrap(),
            );
        for object in extra {
            pool = pool.with_object(object);
        }

        let scene = render(&pool, ObjectID::NULL);
        match &scene.find(ObjectID::new(3)).unwrap().kind {
            NodeKind::OutputList {
                selected_text,
                selected_item_materialized,
                ..
            } => {
                assert_eq!(
                    selected_text.as_deref(),
                    Some(""),
                    "OutputList should display no selected item for {name}"
                );
                assert!(!selected_item_materialized);
            }
            other => panic!("expected output-list node, got {other:?}"),
        }
        assert!(GtuiRenderer::default().render(&scene).iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text.is_empty())
        ));
    }
}

#[test]
fn render_output_list_resolves_object_pointer_item_text() {
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
                    items: vec![10.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            10,
            &ObjectPointerBody {
                value: ObjectID::new(11),
            },
        ))
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    value: b"Pointed".to_vec(),
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
            assert_eq!(selected_text.as_deref(), Some("Pointed"));
            assert!(*selected_item_materialized);
        }
        other => panic!("expected output-list node, got {other:?}"),
    }
    let selected_node = scene.find(ObjectID::new(11)).unwrap();
    assert_eq!(selected_node.parent, ObjectID::new(3));
    assert_eq!(selected_node.clip, Some(Rect::new(0, 0, 80, 20)));
    assert!(GtuiRenderer::default().render(&scene).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Pointed")
    ));
}

#[test]
fn render_output_list_clips_materialized_selected_item() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_list(
                3,
                &OutputListBody {
                    width: 10,
                    height: 10,
                    value: 0,
                    items: vec![10.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_fill_attributes(
                9,
                &FillAttributesBody {
                    fill_type: 2,
                    fill_color: 1,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                10,
                &OutputRectangleBody {
                    width: 20,
                    height: 10,
                    fill_attributes: ObjectID::new(9),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert_eq!(
        scene.find(ObjectID::new(10)).unwrap().clip,
        Some(Rect::new(0, 0, 10, 10))
    );
    let fb = FramebufferRenderer::default()
        .render_scene(&scene)
        .expect("scene renders to framebuffer");
    assert_eq!(fb.pixel(5, 5), Some(Colour::rgb(0, 0, 0)));
    assert_eq!(fb.pixel(15, 5), Some(Colour::rgb(255, 255, 255)));
}

#[test]
fn render_external_object_pointer_uses_registered_referenced_pool_when_enabled() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"External".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = LayoutEngine::new(LayoutConfig::default())
        .with_working_set_name(local_name.0, local_name.1)
        .with_external_object_pool(external_name.0, external_name.1, external_pool)
        .build(&local_pool, ObjectID::NULL);

    assert!(
        scene.find(ObjectID::new(50)).is_some(),
        "valid External Object Pointer should materialise the referenced pool object"
    );
    assert!(
        scene.find(ObjectID::new(10)).is_none(),
        "default object should not be drawn when the external reference is valid"
    );
    assert!(GtuiRenderer::default().render(&scene).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));
}

#[test]
fn render_external_button_activation_preserves_referenced_key_number() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
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
        .with_object(create_button(
            50,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 77,
                ..Default::default()
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(local_pool, LayoutConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool);

    assert!(matches!(
        runtime
            .scene()
            .find(ObjectID::new(50))
            .map(|node| &node.kind),
        Some(NodeKind::Button { key_number: 77, .. })
    ));
    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerDown(5, 5))
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(50)
    ));
    assert_eq!(
        messages[0].as_bytes(),
        &[
            cmd::BUTTON_ACTIVATION,
            ActivationCode::Pressed.as_u8(),
            50,
            0,
            3,
            0,
            77,
            0xFF,
        ],
        "external Button activation must preserve the referenced Button Key Number"
    );
}

#[test]
fn render_external_object_pointer_attribute_null_reference_falls_back_to_default() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"External".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(local_pool, LayoutConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool);
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(3),
            attribute_id: 2,
            value: ObjectID::NULL.raw() as u32,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let body = runtime
        .pool()
        .find(ObjectID::new(3))
        .unwrap()
        .get_external_object_pointer_body()
        .unwrap();
    assert_eq!(body.external_reference_name, ObjectID::NULL);
    assert!(
        runtime.scene().find(ObjectID::new(50)).is_none(),
        "NULL External Reference NAME must disable external resolution"
    );
    assert!(
        runtime.scene().find(ObjectID::new(10)).is_some(),
        "NULL External Reference NAME must fall back to the local default object"
    );
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Default")
    ));
}

#[test]
fn render_external_object_pointer_numeric_value_retargets_reference_name_and_object_id() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let first_external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let second_external_name = (0x1111_AAAA, 0x2222_BBBB);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: first_external_name.0,
                name1: first_external_name.1,
            },
        ))
        .with_object(create_external_reference_name(
            6,
            &ExternalReferenceNameBody {
                options: 1,
                name0: second_external_name.0,
                name1: second_external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ));
    let first_external_pool = ObjectPool::default()
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"First".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let second_external_pool = ObjectPool::default()
        .with_object(
            create_external_object_definition(
                41,
                &ExternalObjectDefinitionBody {
                    options: 1,
                    name0: local_name.0,
                    name1: local_name.1,
                    object_ids: vec![ObjectID::new(60)],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                60,
                &OutputStringBody {
                    value: b"Second".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(local_pool, LayoutConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(
        first_external_name.0,
        first_external_name.1,
        first_external_pool,
    );
    runtime.register_external_object_pool(
        second_external_name.0,
        second_external_name.1,
        second_external_pool,
    );
    assert!(runtime.scene().find(ObjectID::new(50)).is_some());
    assert!(runtime.scene().find(ObjectID::new(60)).is_none());

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(3),
            value: (60_u32 << 16) | u32::from(6_u16),
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert!(runtime.scene().find(ObjectID::new(50)).is_none());
    assert!(
        runtime.scene().find(ObjectID::new(60)).is_some(),
        "External Object Pointer Change Numeric Value updates both the reference NAME object and referenced external object ID"
    );
}

#[test]
fn render_output_list_materialises_object_pointer_to_external_object_pointer() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([7u16]))
        .with_object(
            create_output_list(
                7,
                &OutputListBody {
                    width: 80,
                    height: 16,
                    value: 0,
                    items: vec![ObjectID::new(20)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            20,
            &ObjectPointerBody {
                value: ObjectID::new(3),
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
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"External".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = LayoutEngine::new(LayoutConfig::default())
        .with_working_set_name(local_name.0, local_name.1)
        .with_external_object_pool(external_name.0, external_name.1, external_pool)
        .build(&local_pool, ObjectID::NULL);

    assert!(matches!(
        scene.find(ObjectID::new(7)).map(|node| &node.kind),
        Some(NodeKind::OutputList {
            selected_item_materialized: true,
            ..
        })
    ));
    assert!(
        scene.find(ObjectID::new(50)).is_some(),
        "Output List item ObjectPointer should be able to target an External Object Pointer"
    );
    assert!(GtuiRenderer::default().render(&scene).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));
}

#[test]
fn render_external_object_pointer_falls_back_when_reference_is_not_enabled() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 0,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"External".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = LayoutEngine::new(LayoutConfig::default())
        .with_working_set_name(local_name.0, local_name.1)
        .with_external_object_pool(external_name.0, external_name.1, external_pool)
        .build(&local_pool, ObjectID::NULL);

    assert!(
        scene.find(ObjectID::new(10)).is_some(),
        "disabled External Reference NAME must draw the local default object"
    );
    assert!(
        scene.find(ObjectID::new(50)).is_none(),
        "disabled External Reference NAME must not materialise external objects"
    );
    assert!(GtuiRenderer::default().render(&scene).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Default")
    ));
}

#[test]
fn render_runtime_external_object_pointer_rebuilds_after_registering_referenced_pool() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"External".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(local_pool, DocConfig::default()).unwrap();
    assert!(
        runtime.scene().find(ObjectID::new(10)).is_some(),
        "runtime without host external context falls back to the local default"
    );

    runtime.set_working_set_name(local_name.0, local_name.1);
    let update =
        runtime.register_external_object_pool(external_name.0, external_name.1, external_pool);

    assert!(matches!(update, RenderUpdate::SceneRebuilt { .. }));
    assert!(runtime.scene().find(ObjectID::new(50)).is_some());
    assert!(
        runtime.scene().find(ObjectID::new(10)).is_none(),
        "valid host-registered external pool replaces the local default"
    );
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));
}

#[test]
fn render_runtime_external_pool_registration_replaces_and_unregisters_by_name() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let external_pool = |text: &'static [u8]| {
        ObjectPool::default()
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
            .with_object(
                create_output_string(
                    50,
                    &OutputStringBody {
                        value: text.to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
    };

    let mut runtime = VtRenderRuntime::from_pool(local_pool, DocConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool(b"Old"));
    assert!(
        runtime.render(&GtuiRenderer::default()).iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Old")
        )
    );

    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool(b"New"));
    let commands = runtime.render(&GtuiRenderer::default());
    assert!(
        commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "New")
        )
    );
    assert!(
        !commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Old")
        )
    );

    runtime.unregister_external_object_pool(external_name.0, external_name.1);
    assert!(runtime.scene().find(ObjectID::new(10)).is_some());
    assert!(runtime.scene().find(ObjectID::new(50)).is_none());
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Default")
    ));
}

#[test]
fn render_runtime_external_context_noops_do_not_rebuild() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"External".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(local_pool, DocConfig::default()).unwrap();
    assert!(matches!(
        runtime.set_working_set_name(local_name.0, local_name.1),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime.set_working_set_name(local_name.0, local_name.1),
        RenderUpdate::Unchanged,
        "repeated host NAME announcements must not dirty/rebuild the renderer"
    );

    assert!(matches!(
        runtime.register_external_object_pool(
            external_name.0,
            external_name.1,
            external_pool.clone()
        ),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_eq!(
        runtime.register_external_object_pool(
            external_name.0,
            external_name.1,
            external_pool.clone()
        ),
        RenderUpdate::Unchanged,
        "re-registering the identical referenced pool is a host-loop no-op"
    );
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));

    assert_eq!(
        runtime.unregister_external_object_pool(0xDEAD_BEEF, 0xFEED_CAFE),
        RenderUpdate::Unchanged,
        "unregistering a missing referenced pool is a host-loop no-op"
    );
}

#[test]
fn render_runtime_working_set_name_change_revalidates_external_pointer_after_unlock() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let other_name = (0x0102_0304, 0x0506_0708);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
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
        .with_object(
            create_output_string(
                50,
                &OutputStringBody {
                    value: b"External".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(local_pool, DocConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool);
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: true,
            timeout_ms: 0,
        })
        .unwrap();
    assert!(matches!(
        runtime.set_working_set_name(other_name.0, other_name.1),
        RenderUpdate::NotRenderAffecting { .. }
    ));
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: false,
            timeout_ms: 0,
        }),
        Ok(RenderUpdate::SceneRebuilt { active_mask }) if active_mask == ObjectID::new(2)
    ));
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Default")
    ));
    assert!(runtime.scene().find(ObjectID::new(50)).is_none());
}

#[test]
fn render_runtime_external_pool_context_changes_defer_while_active_mask_locked() {
    let local_name = (0x1111_2222, 0x3333_4444);
    let external_name = (0xAAAA_BBBB, 0xCCCC_DDDD);
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: external_name.0,
                name1: external_name.1,
            },
        ))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::new(10),
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    value: b"Default".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let external_pool = || {
        ObjectPool::default()
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
            .with_object(
                create_output_string(
                    50,
                    &OutputStringBody {
                        value: b"External".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
    };

    let mut runtime = VtRenderRuntime::from_pool(local_pool, DocConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime
        .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: true,
            timeout_ms: 0,
        })
        .unwrap();

    assert!(matches!(
        runtime.register_external_object_pool(external_name.0, external_name.1, external_pool()),
        RenderUpdate::NotRenderAffecting { .. }
    ));
    let locked_commands = runtime.render(&GtuiRenderer::default());
    assert!(
        locked_commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Default")
        ),
        "active-mask lock must preserve the currently visible local fallback"
    );
    assert!(
        !locked_commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
        ),
        "external pool registration must not refresh a locked active mask immediately"
    );

    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: false,
            timeout_ms: 0,
        }),
        Ok(RenderUpdate::SceneRebuilt { active_mask }) if active_mask == ObjectID::new(2)
    ));
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: true,
            timeout_ms: 0,
        })
        .unwrap();
    assert!(matches!(
        runtime.unregister_external_object_pool(external_name.0, external_name.1),
        RenderUpdate::NotRenderAffecting { .. }
    ));
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "External")
    ));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
            id: ObjectID::new(2),
            locked: false,
            timeout_ms: 0,
        })
        .unwrap();
    assert!(runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Default")
    ));
}

#[test]
fn render_missing_child_is_recorded_not_panic() {
    // Mask references a child that does not exist in the pool.
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([99u16]));
    let scene = render(&pool, ObjectID::NULL);
    assert!(
        scene
            .unsupported
            .iter()
            .any(|r| r.reason.contains("not present"))
    );
}

#[test]
fn render_cycle_does_not_infinite_loop() {
    // Two containers referencing each other as children.
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_container(3, &ContainerBody::default()).with_children([4u16]))
        .with_object(create_container(4, &ContainerBody::default()).with_children([3u16]));
    // Should terminate and report a cycle.
    let scene = render(&pool, ObjectID::NULL);
    assert!(scene.unsupported.iter().any(|r| r.reason.contains("cycle")));
}

// ─── Explicit placements / auto-layout ─────────────────────────────

#[test]
fn render_explicit_placements_override_auto_layout() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap());
    let placements = PlacementMap::new().set(3u16, 100, 10).set(4u16, 200, 20);
    let engine = LayoutEngine::new(LayoutConfig::default()).with_placements(placements);
    let scene = render_with(&pool, &engine, ObjectID::NULL);
    let n3 = scene.find(ObjectID::new(3)).unwrap();
    let n4 = scene.find(ObjectID::new(4)).unwrap();
    assert_eq!((n3.rect.x, n3.rect.y), (100, 10));
    assert_eq!((n4.rect.x, n4.rect.y), (200, 20));
}

// ─── Style resolution: font / fill / line ───────────────────────────

#[test]
fn render_resolves_font_fill_and_line_attribute_references() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_font_attributes(
            10,
            &FontAttributesBody {
                font_color: 5,
                font_size: 8,
                ..Default::default()
            },
        ))
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    width: 100,
                    height: 20,
                    font_attributes: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let node = scene.find(ObjectID::new(3)).unwrap();
    // Font size 8 maps to a concrete metric strictly larger than size 0.
    let base = FontMetrics::for_size(0);
    assert!(node.style.font.cell_h >= base.cell_h);
    // Foreground resolved through the palette (index 5).
    let pal = Palette::default_isobus();
    assert_eq!(node.style.foreground, pal.resolve(5));
}

#[test]
fn render_fill_attributes_drive_fill_type() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            machbus::isobus::vt::VTObject::default()
                .with_id(11u16)
                .with_type(ObjectType::FillAttributes)
                .with_body(
                    FillAttributesBody {
                        fill_type: 2,
                        fill_color: 7,
                        ..Default::default()
                    }
                    .encode()
                    .unwrap(),
                ),
        )
        .with_object(
            create_output_rectangle(
                3,
                &OutputRectangleBody {
                    width: 40,
                    height: 40,
                    fill_attributes: ObjectID::new(11),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let node = scene.find(ObjectID::new(3)).unwrap();
    assert_eq!(node.style.fill_type, FillType::FillColour);
}

// ─── Coverage ledger ───────────────────────────────────────────────

#[test]
fn coverage_ledger_classifies_every_object_type_present() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(create_font_attributes(10, &FontAttributesBody::default()))
        .with_object(create_picture_graphic(11, &PictureGraphicBody::default()).unwrap());
    let doc = IopDocument::from_pool(&pool, DocConfig::default()).unwrap();
    let cov = doc.coverage();
    let statuses: Vec<_> = cov
        .rows
        .iter()
        .map(|r| (r.object_type, r.render_status))
        .collect();
    assert!(statuses.contains(&(ObjectType::DataMask, RenderStatus::Drawable)));
    assert!(statuses.contains(&(ObjectType::FontAttributes, RenderStatus::ReferenceResolved)));
}

#[test]
fn coverage_render_status_is_total_over_all_types() {
    // Every defined object type has a classification; no panic.
    for raw in 0u8..=50 {
        let Some(ty) = option_object_type(raw) else {
            continue;
        };
        let s = render_status_for(ty);
        assert!(matches!(
            s,
            RenderStatus::Drawable
                | RenderStatus::ReferenceResolved
                | RenderStatus::Interactive
                | RenderStatus::SoftKey
                | RenderStatus::ParsedButNotRendered
                | RenderStatus::MissingObjectModel
                | RenderStatus::OutOfScope
        ));
    }
}

#[test]
fn coverage_static_ledger_includes_recently_modelled_standard_families() {
    let ledger = coverage_ledger();
    for raw in 0u8..=50 {
        let ty = option_object_type(raw).expect("known VT object type");
        assert!(
            ledger.iter().any(|row| row.object_type == Some(ty)),
            "missing render coverage ledger row for {ty:?}"
        );
    }
    assert!(ledger.iter().any(|row| {
        row.name == "OutputList"
            && row.object_type == Some(ObjectType::OutputList)
            && row.render_status == RenderStatus::Drawable
    }));
    assert!(ledger.iter().any(|row| {
        row.name == "ExtendedInputAttributes"
            && row.object_type == Some(ObjectType::ExtendedInputAttributes)
            && row.render_status == RenderStatus::ReferenceResolved
    }));
    assert!(ledger.iter().any(|row| {
        row.name == "WorkingSetSpecialControls"
            && row.object_type == Some(ObjectType::WorkingSetSpecialControls)
            && row.render_status == RenderStatus::ReferenceResolved
    }));
    assert!(ledger.iter().any(|row| {
        row.name == "Animation"
            && row.object_type == Some(ObjectType::Animation)
            && row.render_status == RenderStatus::Drawable
    }));
    assert!(ledger.iter().any(|row| {
        row.name == "GraphicContext"
            && row.object_type == Some(ObjectType::GraphicContext)
            && row.render_status == RenderStatus::Drawable
    }));
    assert!(ledger.iter().any(|row| {
        row.name == "GraphicsContext"
            && row.object_type == Some(ObjectType::GraphicsContext)
            && row.render_status == RenderStatus::Drawable
    }));
    assert!(ledger.iter().any(|row| {
        row.name == "ScaledBitmap"
            && row.object_type == Some(ObjectType::ScaledBitmap)
            && row.render_status == RenderStatus::Drawable
    }));
}

#[test]
fn coverage_csv_has_header_and_rows() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let doc = IopDocument::from_pool(&pool, DocConfig::default()).unwrap();
    let csv = doc.coverage().to_csv();
    assert!(csv.starts_with("object_type,render_status"));
    assert!(csv.contains("WorkingSet,reference-resolved"));
}

#[test]
fn render_graphic_context_canvas_surface() {
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
        );
    let scene = render(&pool, ObjectID::NULL);
    assert!(scene.unsupported.is_empty());
    assert!(matches!(
        scene.find(ObjectID::new(11)).map(|node| &node.kind),
        Some(NodeKind::GraphicContext {
            canvas_width: 80,
            canvas_height: 30,
            background: 0,
            transparency_colour: 0,
            transparent: false
        })
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextCanvas {
            object_id,
            rect: Rect {
                x: 4,
                y: 5,
                w: 60,
                h: 16,
            },
            canvas_width: 80,
            canvas_height: 30,
            background: 0,
            transparency_colour: 0,
            transparent: false,
        } if *object_id == ObjectID::new(11)
    )));
}

#[test]
fn render_runtime_applies_graphic_context_background_colour() {
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
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(matches!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeBackgroundColour {
            id: ObjectID::new(11),
            colour: 9,
        }),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));

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
            transparent: false,
            ..
        } if *object_id == ObjectID::new(11)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, colour }
            if *rect == Rect::new(0, 0, 12, 8) && *colour == expected
    )));
}

#[test]
fn render_runtime_preserves_graphic_context_transparency_colour_in_canvas_copy() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([11u16]))
        .with_object(
            create_graphic_context(
                11,
                &GraphicContextBody {
                    viewport_width: 2,
                    viewport_height: 2,
                    canvas_width: 2,
                    canvas_height: 2,
                    options: 0x01,
                    transparency_colour: 7,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                20,
                &PictureGraphicBody {
                    width: 2,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 7,
                    data: vec![0; 4],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    runtime
        .apply_ecu_command(&VtRuntimeCommand::GraphicsContext {
            id: ObjectID::new(11),
            subcommand: 0x13,
            payload: 20u16.to_le_bytes().to_vec(),
        })
        .unwrap();

    let commands = runtime.render_commands(&GtuiRenderer::default());
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextCanvas {
            object_id,
            transparency_colour: 7,
            transparent: true,
            ..
        } if *object_id == ObjectID::new(11)
    )));
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::GraphicsContextPictureData {
            object_id,
            picture_id,
            width: 2,
            height: 2,
            data,
            ..
        } if *object_id == ObjectID::new(11)
            && *picture_id == ObjectID::new(20)
            && data == &[7, 7, 7, 7]
    )));
}

#[test]
fn render_scaled_graphic_applies_standard_scale_type_to_destination_rect() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 10,
                    height: 6,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    options: 0,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_graphic_data(
                4,
                &GraphicDataBody {
                    format: 0,
                    options: 0,
                    data: minimal_png_rgba(2, 2),
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert!(matches!(
        scene.find(ObjectID::new(3)).map(|node| node.rect),
        Some(Rect { w: 10, h: 6, .. })
    ));
}

#[test]
fn render_scaled_graphic_accepts_picture_graphic_and_object_pointer_values() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 5u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 8,
                    height: 8,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    options: 0,
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
                    transparency: 7,
                    data: vec![1; 4],
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_graphic(
                5,
                &ScaledGraphicBody {
                    width: 8,
                    height: 8,
                    scale_type: 3,
                    value: ObjectID::new(6),
                    options: 0,
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            6,
            &ObjectPointerBody {
                value: ObjectID::new(4),
            },
        ));

    let scene = render(&pool, ObjectID::NULL);
    assert!(
        scene.unsupported.is_empty(),
        "PictureGraphic and ObjectPointer values are standard ScaledGraphic references"
    );
    let commands = GtuiRenderer::default().render(&scene);
    let image_count = commands
        .iter()
        .filter(|command| {
            matches!(
                command,
                RenderCommand::IndexedImage {
                    width: 2,
                    height: 2,
                    transparency: 7,
                    ..
                }
            )
        })
        .count();
    assert_eq!(image_count, 2);
}

#[test]
fn render_picture_graphic_uses_target_width_but_preserves_raw_bitmap_size() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_picture_graphic(
                3,
                &PictureGraphicBody {
                    width: 6,
                    actual_width: 2,
                    actual_height: 2,
                    format: 2,
                    options: 0,
                    transparency: 0xFF,
                    data: vec![1, 2, 3, 4],
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    let node = scene.find(ObjectID::new(3)).unwrap();
    assert_eq!(
        node.rect,
        Rect::new(0, 0, 6, 2),
        "PictureGraphic target width is the displayed width; actual_width remains the source bitmap width"
    );
    let commands = GtuiRenderer::default().render(&scene);
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::IndexedImage {
                object_id,
                rect,
                width: 2,
                height: 2,
                data,
                ..
            } if *object_id == ObjectID::new(3)
                && *rect == Rect::new(0, 0, 6, 2)
                && data == &[1, 2, 3, 4]
        )
    }));
}

#[test]
fn render_picture_graphic_transparency_option_controls_framebuffer_pixels() {
    let pool_for_options = |options| {
        ObjectPool::default()
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
                        options,
                        transparency: 1,
                        data: vec![1],
                    },
                )
                .unwrap(),
            )
    };

    let opaque = render(&pool_for_options(0), ObjectID::NULL);
    let opaque_fb = FramebufferRenderer::default()
        .render_scene(&opaque)
        .expect("opaque picture renders");
    assert_eq!(
        opaque_fb.pixel(0, 0),
        Some(Colour::rgb(0, 0, 0)),
        "opaque PictureGraphic must draw pixels even when they equal the transparency colour"
    );

    let transparent = render(&pool_for_options(0x01), ObjectID::NULL);
    let commands = GtuiRenderer::default().render(&transparent);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::IndexedImage {
            transparent: true,
            transparency: 1,
            ..
        }
    )));
    let transparent_fb = FramebufferRenderer::default()
        .render_scene(&transparent)
        .expect("transparent picture renders");
    assert_eq!(
        transparent_fb.pixel(0, 0),
        Some(Colour::rgb(255, 255, 255)),
        "transparent PictureGraphic should leave background visible for transparency-colour pixels"
    );
}

#[test]
fn render_scaled_graphic_preserves_picture_transparency_without_treating_it_as_rle() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_scaled_graphic(
                3,
                &ScaledGraphicBody {
                    width: 1,
                    height: 1,
                    scale_type: 3,
                    value: ObjectID::new(4),
                    options: 0,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_picture_graphic(
                4,
                &PictureGraphicBody {
                    width: 1,
                    actual_width: 1,
                    actual_height: 1,
                    format: 2,
                    options: 0x01,
                    transparency: 1,
                    data: vec![1],
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert!(scene.unsupported.is_empty());
    let commands = GtuiRenderer::default().render(&scene);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::IndexedImage {
            object_id,
            transparent: true,
            transparency: 1,
            data,
            ..
        } if *object_id == ObjectID::new(3) && data == &[1]
    )));
}

// ─── Text measurement & alignment unit behaviour ───────────────────

#[test]
fn text_measurement_clips_overflow_and_aligns() {
    let metrics = FontMetrics {
        cell_w: 8,
        cell_h: 12,
        ascent: 10,
        descent: 2,
    };
    let m = text::measure("HELLO", metrics, 16, 12);
    assert_eq!(m.visible_cols, 2); // 16px / 8px = 2 cols
    assert_eq!(m.clipped_cols, 3);
    assert_eq!(
        text::aligned_origin_x(HorizontalAlign::Middle, 2, metrics, 16),
        0
    );
}

#[test]
fn text_decode_lossy_handles_latin1_and_invalid() {
    assert_eq!(text::decode_lossy(b"OK"), "OK");
    assert_eq!(text::decode_lossy(&[0xFF]), "\u{00FF}");
    assert_eq!(text::decode_lossy(&[0x01]), "?");
}

// ─── GTUI renderer command matrix ──────────────────────────────────
