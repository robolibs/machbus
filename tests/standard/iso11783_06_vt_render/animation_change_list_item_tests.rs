fn runtime_animation_draws_text(runtime: &VtRenderRuntime, expected: &str) -> bool {
    runtime.render(&GtuiRenderer::default()).iter().any(
        |command| matches!(command, RenderCommand::DrawText { text, .. } if text == expected),
    )
}

#[test]
fn render_runtime_change_list_item_updates_animation_frame_slot() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default())
                .with_children_pos([ChildRef::new(ObjectID::new(3), 0, 0)]),
        )
        .with_object(
            create_animation(
                3,
                &AnimationBody {
                    width: 80,
                    height: 20,
                    value: 0,
                    default_child_index: 0,
                    last_child_index: 0,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_children_pos([ChildRef::at_origin(ObjectID::new(4))]),
        )
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"OLD".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    width: 80,
                    height: 20,
                    value: b"NEW".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(runtime_animation_draws_text(&runtime, "OLD"));
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeListItem {
                list: ObjectID::new(3),
                index: 0,
                item: ObjectID::new(5),
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert!(runtime_animation_draws_text(&runtime, "NEW"));
    assert!(
        !runtime_animation_draws_text(&runtime, "OLD"),
        "Animation Change List Item must replace the frame object instead of leaving a stale frame"
    );
}
