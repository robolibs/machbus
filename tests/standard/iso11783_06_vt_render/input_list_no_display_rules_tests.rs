#[test]
fn render_input_list_does_not_synthesize_index_count_when_no_item_text_is_drawn() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_font_attributes(10, &FontAttributesBody::default()))
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    options: 0x01,
                    items: vec![10.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    match &scene.find(ObjectID::new(3)).unwrap().kind {
        NodeKind::InputList {
            selected,
            item_count,
            selected_text,
            ..
        } => {
            assert_eq!(*selected, 0);
            assert_eq!(*item_count, 1);
            assert!(selected_text.is_none());
        }
        other => panic!("expected input-list node, got {other:?}"),
    }

    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text.is_empty())
        ),
        "an InputList with no displayable selected item renders a blank field"
    );
    assert!(
        !commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "1/1")
        ),
        "InputList must not fabricate a diagnostic index/count label"
    );
}

#[test]
fn input_list_operator_selection_keeps_standard_empty_items_selectable() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_container(
            10,
            &ContainerBody {
                width: 40,
                height: 20,
                hidden: true,
            },
        ))
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    width: 40,
                    height: 20,
                    value: b"OK".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            12,
            &ObjectPointerBody {
                value: ObjectID::NULL,
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
                    items: vec![ObjectID::NULL, ObjectID::new(10), ObjectID::new(12), ObjectID::new(11)],
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
        panic!("expected input-list node");
    };
    assert_eq!(*selected, 0);
    assert_eq!(selected_text.as_deref(), Some(""));
    assert_eq!(selectable_indices.as_slice(), [1, 2, 3]);

    let mut input = InputRuntime::new();
    input.bind(&scene);
    let events = input.handle(&scene, &OperatorEvent::Tap(10, 10));
    assert!(matches!(
        events.as_slice(),
        [VtEvent::ListSelectionChanged {
            id,
            index: 1
        }] if *id == ObjectID::new(3)
    ));

    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_container(
            10,
            &ContainerBody {
                width: 40,
                height: 20,
                hidden: true,
            },
        ))
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    value: 1,
                    options: 0x03,
                    items: vec![ObjectID::NULL, ObjectID::new(10)],
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut input = InputRuntime::new();
    input.bind(&scene);
    let events = input.handle(&scene, &OperatorEvent::Tap(10, 10));
    assert!(matches!(
        events.as_slice(),
        [VtEvent::ListSelectionChanged {
            id,
            index: 1
        }] if *id == ObjectID::new(3)
    ));
}
