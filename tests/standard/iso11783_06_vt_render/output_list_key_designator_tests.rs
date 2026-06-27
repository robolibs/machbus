#[test]
fn render_output_list_selected_key_as_display_only_designator() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_list(
                3,
                &OutputListBody {
                    width: 48,
                    height: 20,
                    value: 0,
                    items: vec![10.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_key(
                10,
                &KeyBody {
                    background_color: 4,
                    key_code: 7,
                },
            )
            .with_children([11u16]),
        )
        .with_object(
            create_output_string(
                11,
                &OutputStringBody {
                    value: b"GO".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert!(
        scene.unsupported.is_empty(),
        "a Key selected by an OutputList is displayable and must not become a placeholder"
    );

    let list = scene.find(ObjectID::new(3)).expect("output list node");
    match &list.kind {
        NodeKind::OutputList {
            selected_item_materialized,
            selected_text,
            ..
        } => {
            assert!(*selected_item_materialized);
            assert!(
                selected_text.is_none(),
                "materialized Key designators should suppress compact fallback text"
            );
        }
        other => panic!("expected OutputList node, got {other:?}"),
    }

    let key = scene.find(ObjectID::new(10)).expect("selected Key node");
    assert_eq!(key.clip, Some(Rect::new(0, 0, 48, 20)));
    match &key.kind {
        NodeKind::KeyDesignator { label, key_number } => {
            assert_eq!(label, "GO");
            assert_eq!(*key_number, 7);
        }
        other => panic!("expected KeyDesignator node, got {other:?}"),
    }

    let commands = GtuiRenderer::default().render(&scene);
    assert!(commands.iter().any(|command| {
        matches!(
            command,
            RenderCommand::SoftKey {
                rect,
                kind: SoftKeyKind::Application,
                label,
                ..
            } if *rect == Rect::new(0, 0, 64, 40) && label == "GO"
        )
    }));
    assert!(!commands.iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "GO")
    ));

    let mut input = InputRuntime::default();
    input.bind(&scene);
    let event = input.handle(&scene, &OperatorEvent::Tap(4, 4));
    assert!(
        matches!(event.as_slice(), [VtEvent::Ignored { .. }]),
        "display-only OutputList Key designators must not emit soft-key or button activations"
    );
}
