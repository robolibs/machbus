fn window_mask_body_available() -> WindowMaskBody {
    WindowMaskBody {
        options: 0x01,
        ..Default::default()
    }
}

#[test]
fn nested_window_mask_clips_children_to_its_region() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(
            1,
            &WorkingSetBody {
                active_mask: ObjectID(2),
                ..Default::default()
            },
        ))
        .with_object(
            create_data_mask(2, &DataMaskBody::default())
                .with_children_pos([ChildRef::new(ObjectID(3), 10, 10)]),
        )
        .with_object(
            create_window_mask(3, &window_mask_body_available())
                .unwrap()
                .with_children_pos([ChildRef::new(ObjectID(10), 230, 35)]),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"CLIPPED".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    pool.validate().unwrap();

    let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID(2));
    let window_clip = Rect::new(10, 10, 240, 40);
    assert_eq!(scene.find(ObjectID(10)).unwrap().clip, Some(window_clip));

    let commands = GtuiRenderer::default().render(&scene);
    let text_index = commands
        .iter()
        .position(|command| {
            matches!(command, RenderCommand::DrawText { text, .. } if text == "CLIPPED")
        })
        .expect("window child text is rendered");
    let clip_index = commands
        .iter()
        .position(|command| matches!(command, RenderCommand::Clip(rect) if *rect == window_clip))
        .expect("window child clip is emitted");
    assert!(
        clip_index < text_index,
        "Window Mask child drawing must be clipped before backend text/image commands"
    );
}

#[test]
fn active_window_mask_scene_nodes_carry_the_active_window_clip() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(
            1,
            &WorkingSetBody {
                active_mask: ObjectID(3),
                ..Default::default()
            },
        ))
        .with_object(
            create_window_mask(3, &window_mask_body_available())
                .unwrap()
                .with_children_pos([ChildRef::new(ObjectID(10), 230, 35)]),
        )
        .with_object(
            create_output_string(
                10,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"ACTIVE".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    pool.validate().unwrap();

    let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID(3));
    let active_window_clip = Rect::new(0, 0, 240, 40);
    assert_eq!(scene.mask_rect, active_window_clip);
    assert_eq!(scene.find(ObjectID(10)).unwrap().clip, Some(active_window_clip));
}

#[test]
fn window_mask_clip_intersects_output_list_selected_item_clip() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(
            1,
            &WorkingSetBody {
                active_mask: ObjectID(3),
                ..Default::default()
            },
        ))
        .with_object(
            create_window_mask(3, &window_mask_body_available())
                .unwrap()
                .with_children_pos([ChildRef::new(ObjectID(10), 230, 35)]),
        )
        .with_object(
            create_output_list(
                10,
                &OutputListBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    items: vec![ObjectID(20)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                20,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"ITEM".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    pool.validate().unwrap();

    let scene = LayoutEngine::new(LayoutConfig::default()).build(&pool, ObjectID(3));

    assert_eq!(
        scene.find(ObjectID(20)).unwrap().clip,
        Some(Rect::new(230, 35, 10, 5)),
        "materialised OutputList items must keep both the Window Mask clip and the list viewport clip"
    );
}
