#[test]
fn input_list_selected_drawable_item_materializes_inside_field() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 30,
                    height: 16,
                    value: 0,
                    options: 0x03,
                    items: vec![10.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_rectangle(
                10,
                &OutputRectangleBody {
                    width: 60,
                    height: 40,
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    assert!(
        scene.unsupported.is_empty(),
        "a selected drawable InputList item should be displayed, not downgraded"
    );

    let list = scene.find(ObjectID::new(3)).expect("input-list node");
    match &list.kind {
        NodeKind::InputList {
            selected,
            selected_text,
            selected_item_materialized,
            ..
        } => {
            assert_eq!(*selected, 0);
            assert!(selected_text.is_none());
            assert!(*selected_item_materialized);
        }
        other => panic!("expected InputList node, got {other:?}"),
    }

    let selected_rect = scene.find(ObjectID::new(10)).expect("selected rectangle node");
    assert_eq!(selected_rect.rect, Rect::new(0, 0, 60, 40));
    assert_eq!(selected_rect.clip, Some(Rect::new(0, 0, 30, 16)));
    assert!(matches!(
        selected_rect.kind,
        NodeKind::OutputRectangle { .. }
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, RenderCommand::Clip(rect) if *rect == Rect::new(0, 0, 30, 16))),
        "materialized item drawing must be clipped to the InputList field"
    );
    assert!(
        commands.iter().any(|command| {
            matches!(
                command,
                RenderCommand::StrokeRect {
                    rect,
                    ..
                } if *rect == Rect::new(0, 0, 60, 40)
            )
        }),
        "the selected drawable item should emit its own drawing commands"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, RenderCommand::DrawText { .. })),
        "materialized selected items suppress the compact InputList text fallback"
    );

    let mut input = InputRuntime::new();
    input.bind(&scene);
    let events = input.handle(&scene, &OperatorEvent::Tap(4, 4));
    assert!(
        matches!(
            events.as_slice(),
            [VtEvent::ListSelectionChanged {
                id,
                index: 0
            }] if *id == ObjectID::new(3)
        ),
        "the parent InputList remains the interactive hit target"
    );
}
