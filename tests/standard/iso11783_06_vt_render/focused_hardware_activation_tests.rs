#[test]
fn hardware_enter_activates_focused_button() {
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
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::FocusNext),
        vec![VtEvent::FocusChanged {
            id: ObjectID::new(3),
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::HardwareKey(b'\r')),
        vec![VtEvent::ButtonActivated {
            id: ObjectID::new(3),
        }]
    );
}

#[test]
fn hardware_enter_button_activation_lowers_to_bus_messages() {
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
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    let _ = runtime.handle_operator_event(OperatorEvent::FocusNext);
    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::HardwareKey(b'\r'))
        .unwrap();

    assert_eq!(
        events,
        vec![VtEvent::ButtonActivated {
            id: ObjectID::new(3),
        }]
    );
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].kind, VtBusMessageKind::ButtonActivation);
    assert_eq!(messages[1].kind, VtBusMessageKind::ButtonActivation);
    assert_eq!(
        messages[0].as_bytes(),
        &[cmd::BUTTON_ACTIVATION, 1, 3, 0, 2, 0, 7, 0xFF]
    );
    assert_eq!(
        messages[1].as_bytes(),
        &[cmd::BUTTON_ACTIVATION, 0, 3, 0, 2, 0, 7, 0xFF]
    );
}

#[test]
fn commit_activates_selected_button_focus_only_target() {
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
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
                id: ObjectID::new(3),
                open_for_input: false,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting { .. }
    ));
    assert_eq!(runtime.selected_input(), Some(ObjectID::new(3)));
    assert_eq!(runtime.open_input(), None);
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::Commit),
        vec![VtEvent::ButtonActivated {
            id: ObjectID::new(3),
        }]
    );
}

#[test]
fn commit_activates_selected_soft_key_focus_only_target() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            ),
        )
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 42,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
                id: ObjectID::new(6),
                open_for_input: false,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting { .. }
    ));
    assert_eq!(runtime.selected_input(), Some(ObjectID::new(6)));
    assert_eq!(runtime.open_input(), None);
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::Commit),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(6),
        }]
    );
}

#[test]
fn commit_soft_key_focus_only_activation_lowers_to_bus_messages() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            ),
        )
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 42,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
            id: ObjectID::new(6),
            open_for_input: false,
        })
        .unwrap();
    let (events, messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::Commit)
        .unwrap();

    assert_eq!(
        events,
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(6),
        }]
    );
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].kind, VtBusMessageKind::SoftKeyActivation);
    assert_eq!(messages[1].kind, VtBusMessageKind::SoftKeyActivation);
    assert_eq!(
        messages[0].as_bytes(),
        &[cmd::SOFT_KEY_ACTIVATION, 1, 6, 0, 5, 0, 42, 0xFF]
    );
    assert_eq!(
        messages[1].as_bytes(),
        &[cmd::SOFT_KEY_ACTIVATION, 0, 6, 0, 5, 0, 42, 0xFF]
    );
}

#[test]
fn commit_activates_selected_key_group_focus_only_target() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([31u16]),
        )
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 31,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
                id: ObjectID::new(31),
                open_for_input: false,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting { .. }
    ));
    assert_eq!(runtime.selected_input(), Some(ObjectID::new(31)));
    assert_eq!(runtime.open_input(), None);
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::Commit),
        vec![VtEvent::SoftKeyActivated {
            id: ObjectID::new(31),
        }]
    );
}
