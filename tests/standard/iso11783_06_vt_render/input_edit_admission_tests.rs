#[test]
fn rejected_input_string_character_does_not_open_edit_transaction() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    width: 80,
                    height: 20,
                    input_attributes: ObjectID::new(4),
                    variable_reference: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_attributes(
                4,
                &InputAttributesBody {
                    validation_type: 0,
                    validation_string: b"A".to_vec(),
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            5,
            &StringVariableBody {
                length: 0,
                value: b"".to_vec(),
            },
        ));
    let scene = render(&pool, ObjectID::NULL);
    let mut input = InputRuntime::new();
    input.bind(&scene);

    assert_eq!(
        input.handle(&scene, &OperatorEvent::FocusNext),
        vec![VtEvent::FocusChanged {
            id: ObjectID::new(3),
        }]
    );

    assert_eq!(
        input.handle(&scene, &OperatorEvent::Char('B')),
        vec![VtEvent::Ignored {
            reason: "character rejected by input validation",
        }]
    );
    assert_eq!(input.open_input(), None);
    assert_eq!(input.edit_state(), &EditState::Idle);

    assert_eq!(
        input.handle(&scene, &OperatorEvent::Char('A')),
        vec![VtEvent::StringEditPreview {
            id: ObjectID::new(3),
            text: "A".to_owned(),
        }]
    );
    assert_eq!(input.open_input(), Some(ObjectID::new(3)));
    assert!(matches!(input.edit_state(), EditState::String { buffer } if buffer == "A"));
}

#[test]
fn hardware_character_on_non_editable_focused_object_does_not_open_input() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_button(
                3,
                &ButtonBody {
                    width: 40,
                    height: 20,
                    key_code: 7,
                    ..Default::default()
                },
            ),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut input = InputRuntime::new();
    input.bind(&scene);

    assert_eq!(
        input.handle(&scene, &OperatorEvent::FocusNext),
        vec![VtEvent::FocusChanged {
            id: ObjectID::new(3),
        }]
    );
    assert_eq!(
        input.handle(&scene, &OperatorEvent::HardwareKey(b'Q')),
        vec![VtEvent::Ignored {
            reason: "focused node is not an editable text/number field",
        }]
    );
    assert_eq!(input.selected_input(), Some(ObjectID::new(3)));
    assert_eq!(input.open_input(), None);
    assert_eq!(input.edit_state(), &EditState::Idle);
}

#[test]
fn disabled_or_hidden_focused_input_clears_open_edit_before_commit() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            4,
            &StringVariableBody {
                length: 0,
                value: b"".to_vec(),
            },
        ));
    let mut scene = render(&pool, ObjectID::NULL);
    let mut input = InputRuntime::new();
    input.bind(&scene);
    let _ = input.handle(&scene, &OperatorEvent::FocusNext);
    assert_eq!(
        input.handle(&scene, &OperatorEvent::Char('A')),
        vec![VtEvent::StringEditPreview {
            id: ObjectID::new(3),
            text: "A".to_owned(),
        }]
    );
    assert_eq!(input.open_input(), Some(ObjectID::new(3)));

    let node = scene
        .nodes
        .iter_mut()
        .find(|node| node.id == ObjectID::new(3))
        .unwrap();
    node.enabled = false;

    assert_eq!(
        input.handle(&scene, &OperatorEvent::Commit),
        vec![VtEvent::Ignored {
            reason: "focused node is disabled or hidden",
        }]
    );
    assert_eq!(input.selected_input(), None);
    assert_eq!(input.open_input(), None);
    assert_eq!(input.edit_state(), &EditState::Idle);
}

#[test]
fn vanished_focused_input_clears_open_edit_before_more_typing() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(4),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            4,
            &StringVariableBody {
                length: 0,
                value: b"".to_vec(),
            },
        ));
    let mut scene = render(&pool, ObjectID::NULL);
    let mut input = InputRuntime::new();
    input.bind(&scene);
    let _ = input.handle(&scene, &OperatorEvent::FocusNext);
    assert_eq!(
        input.handle(&scene, &OperatorEvent::Char('A')),
        vec![VtEvent::StringEditPreview {
            id: ObjectID::new(3),
            text: "A".to_owned(),
        }]
    );
    assert_eq!(input.open_input(), Some(ObjectID::new(3)));

    scene.nodes.retain(|node| node.id != ObjectID::new(3));

    assert_eq!(
        input.handle(&scene, &OperatorEvent::Char('B')),
        vec![VtEvent::Ignored {
            reason: "focused node vanished",
        }]
    );
    assert_eq!(input.selected_input(), None);
    assert_eq!(input.open_input(), None);
    assert_eq!(input.edit_state(), &EditState::Idle);
}
