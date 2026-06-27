#[test]
fn input_list_selection_skips_unavailable_external_object_pointer_items() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_external_reference_name(
            4,
            &ExternalReferenceNameBody {
                options: 1,
                name0: 0xAAAA_BBBB,
                name1: 0xCCCC_DDDD,
            },
        ))
        .with_object(create_external_object_pointer(
            10,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    width: 40,
                    height: 20,
                    value: b"LOCAL".to_vec(),
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
                    options: 0x03,
                    items: vec![ObjectID::new(10), ObjectID::new(11)],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    let NodeKind::InputList {
        selected,
        selected_text,
        selectable_indices,
        ..
    } = &scene.find(ObjectID::new(3)).unwrap().kind
    else {
        panic!("expected InputList node");
    };
    assert_eq!(*selected, 0);
    assert_eq!(
        selected_text.as_deref(),
        Some(""),
        "unavailable external item remains a blank selected field"
    );
    assert_eq!(
        selectable_indices.as_slice(),
        [1],
        "operator selection must skip unresolved external items with no local default"
    );

    let mut input = InputRuntime::new();
    input.bind(&scene);
    assert!(matches!(
        input.handle(&scene, &OperatorEvent::Tap(10, 10)).as_slice(),
        [VtEvent::ListSelectionChanged {
            id,
            index: 1
        }] if *id == ObjectID::new(3)
    ));
}

#[test]
fn input_list_selection_resolves_registered_external_object_pointer_text() {
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
            10,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID::NULL,
                external_reference_name: ObjectID::new(4),
                external_object_id: ObjectID::new(50),
            },
        ))
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    options: 0x03,
                    items: vec![ObjectID::new(10)],
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
                    width: 40,
                    height: 20,
                    value: b"EXT".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(local_pool, LayoutConfig::default()).unwrap();
    runtime.set_working_set_name(local_name.0, local_name.1);
    runtime.register_external_object_pool(external_name.0, external_name.1, external_pool);
    let scene = runtime.scene();
    let NodeKind::InputList {
        selected_text,
        selectable_indices,
        ..
    } = &scene.find(ObjectID::new(3)).unwrap().kind
    else {
        panic!("expected InputList node");
    };
    assert_eq!(selected_text.as_deref(), Some("EXT"));
    assert_eq!(selectable_indices.as_slice(), [0]);
}
