#[test]
fn tap_ignores_disabled_input_number_and_input_list_without_focus_or_preview() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([
                ChildRef::new(ObjectID::new(3), 0, 0),
                ChildRef::new(ObjectID::new(4), 30, 0),
            ]),
        )
        .with_object(
            create_input_number(
                3,
                &InputNumberBody {
                    options2: 0x00,
                    width: 20,
                    height: 20,
                    min_value: 0,
                    max_value: 99,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                4,
                &InputListBody {
                    width: 20,
                    height: 20,
                    value: 0,
                    options: 0x00,
                    items: vec![ObjectID::new(5)],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap());
    let scene = render(&pool, ObjectID::NULL);
    let mut input = InputRuntime::new();
    input.bind(&scene);

    let disabled_number = input.handle(&scene, &OperatorEvent::Tap(1, 1));
    let disabled_list = input.handle(&scene, &OperatorEvent::Tap(31, 1));

    assert!(matches!(
        disabled_number[0],
        VtEvent::Ignored {
            reason: "input field is disabled"
        }
    ));
    assert!(matches!(
        disabled_list[0],
        VtEvent::Ignored {
            reason: "input field is disabled"
        }
    ));
    assert_eq!(input.selected_input(), None);
    assert_eq!(input.open_input(), None);
    assert!(matches!(input.edit_state(), EditState::Idle));
}

#[test]
fn tap_ignores_runtime_disabled_input_string_after_scene_rebind() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    width: 40,
                    height: 20,
                    max_length: 4,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    let selected = runtime.handle_operator_event(OperatorEvent::Tap(1, 1));
    assert!(matches!(
        selected[0],
        VtEvent::FocusChanged { id } if id == ObjectID::new(3)
    ));
    assert_eq!(runtime.selected_input(), Some(ObjectID::new(3)));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::EnableDisable {
            id: ObjectID::new(3),
            enabled: false,
        })
        .unwrap();
    assert_eq!(runtime.selected_input(), None);
    assert_eq!(runtime.open_input(), None);
    assert!(
        runtime
            .scene()
            .find(ObjectID::new(3))
            .is_some_and(|node| !node.enabled)
    );

    let disabled = runtime.handle_operator_event(OperatorEvent::Tap(1, 1));
    assert!(matches!(
        disabled[0],
        VtEvent::Ignored {
            reason: "input field is disabled"
        }
    ));
    assert_eq!(runtime.selected_input(), None);
    assert_eq!(runtime.open_input(), None);
}
