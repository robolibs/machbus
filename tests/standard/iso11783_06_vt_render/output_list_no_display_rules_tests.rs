#[test]
fn render_output_list_does_not_synthesize_index_count_when_no_item_is_drawn() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_list(
                3,
                &OutputListBody {
                    width: 80,
                    height: 20,
                    value: 1,
                    items: vec![10.into(), 11.into(), 12.into()],
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    match &scene.find(ObjectID::new(3)).unwrap().kind {
        NodeKind::OutputList {
            selected,
            item_count,
            selected_text,
            selected_item_materialized,
        } => {
            assert_eq!(*selected, 1);
            assert_eq!(*item_count, 3);
            assert!(selected_text.is_none());
            assert!(!selected_item_materialized);
        }
        other => panic!("expected output-list node, got {other:?}"),
    }

    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text.is_empty())
        ),
        "an OutputList with no displayable selected item renders a blank field"
    );
    assert!(
        !commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "2 / 3")
        ),
        "OutputList must not fabricate a diagnostic index/count label"
    );
}
