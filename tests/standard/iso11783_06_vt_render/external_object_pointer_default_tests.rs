#[test]
fn render_external_object_pointer_uses_default_when_external_pointer_chain_is_null() {
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
                    width: 96,
                    height: 24,
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
        .with_object(create_object_pointer(
            50,
            &ObjectPointerBody {
                value: ObjectID::new(51),
            },
        ))
        .with_object(create_object_pointer(
            51,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ));

    let scene = LayoutEngine::new(LayoutConfig::default())
        .with_working_set_name(local_name.0, local_name.1)
        .with_external_object_pool(external_name.0, external_name.1, external_pool)
        .build(&local_pool, ObjectID::NULL);

    assert!(
        scene.find(ObjectID::new(10)).is_some(),
        "an external target ObjectPointer chain ending at NULL must draw the local default object"
    );
    assert!(
        scene
            .unsupported
            .iter()
            .all(|record| record.id != ObjectID::new(50) && record.id != ObjectID::new(51)),
        "the external NULL pointer chain is a standard default-object case, not an unsupported object"
    );
    assert!(GtuiRenderer::default().render(&scene).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "Default")
    ));
}

#[test]
fn render_object_pointer_null_targets_are_blank_without_unsupported_records() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_object_pointer(
            3,
            &ObjectPointerBody {
                value: ObjectID::NULL,
            },
        ));

    let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID::NULL);

    assert!(
        scene.find(ObjectID::new(3)).is_none(),
        "ObjectPointer is an indirection; a NULL value is the standard no-object case"
    );
    assert!(
        scene.unsupported.is_empty(),
        "a standard ObjectPointer NULL no-object case must not be reported as unsupported"
    );
}

#[test]
fn render_unresolved_external_object_pointer_with_null_default_is_blank() {
    let local_pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_object_pointer(
            3,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
                external_reference_name: ObjectID::NULL,
                external_object_id: ObjectID::new(50),
            },
        ));

    let scene = LayoutEngine::new(LayoutConfig::default()).build(&local_pool, ObjectID::NULL);

    assert!(
        scene.find(ObjectID::new(3)).is_none(),
        "ExternalObjectPointer with an unresolved external target and NULL local default draws nothing"
    );
    assert!(
        scene.unsupported.is_empty(),
        "NULL local default is a standard blank fallback, not an unsupported object"
    );
}
