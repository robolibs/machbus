#[test]
fn render_unavailable_transparent_window_mask_blanks_user_layout_cell() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    background_color: 7,
                    ..Default::default()
                },
            )
            .with_children_pos([ChildRef::new(ObjectID::new(30), 0, 0)]),
        )
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    width_cells: 1,
                    height_cells: 1,
                    window_type: 0,
                    background_color: 2,
                    options: 0x02,
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
                    value: b"SHOULD_NOT_DRAW".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );

    let scene = render(&pool, ObjectID::NULL);
    let window = scene.find(ObjectID::new(30)).unwrap();
    assert!(!window.enabled);
    assert!(matches!(&window.kind, NodeKind::Group { .. }));
    assert!(scene.find(ObjectID::new(31)).is_none());

    let commands = GtuiRenderer::default().render(&scene);
    let expected_rect = Rect::new(0, 0, 240, 40);
    let expected_colour = Palette::default_isobus().resolve(7);
    assert!(commands.iter().any(|command| matches!(
        command,
        RenderCommand::FillRect { rect, colour } if *rect == expected_rect && *colour == expected_colour
    )));
    assert!(!commands.iter().any(|command| matches!(
        command,
        RenderCommand::DrawText { text, .. } if text == "SHOULD_NOT_DRAW"
    )));
}
