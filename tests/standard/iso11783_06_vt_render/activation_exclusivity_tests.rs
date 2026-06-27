fn activation_exclusivity_pool() -> ObjectPool {
    ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 3u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children_pos([
                ChildRef::new(ObjectID::new(8), 0, 0),
                ChildRef::new(ObjectID::new(9), 60, 0),
            ]),
        )
        .with_object(create_data_mask(3, &DataMaskBody::default()))
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16, 7u16]),
        )
        .with_object(create_soft_key_mask(10, &SoftKeyMaskBody::default()))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 6,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            7,
            &KeyBody {
                key_code: 7,
                ..Default::default()
            },
        ))
        .with_object(create_button(
            8,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 8,
                ..Default::default()
            },
        ))
        .with_object(create_button(
            9,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 9,
                ..Default::default()
            },
        ))
}

fn container_button_activation_pool() -> ObjectPool {
    ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children_pos([ChildRef::new(
                ObjectID::new(10),
                0,
                0,
            )]),
        )
        .with_object(
            create_container(
                10,
                &ContainerBody {
                    width: 80,
                    height: 40,
                    hidden: false,
                },
            )
            .with_children_pos([ChildRef::new(ObjectID::new(8), 0, 0)]),
        )
        .with_object(create_button(
            8,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 8,
                ..Default::default()
            },
        ))
}

fn key_group_activation_pool() -> ObjectPool {
    ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16, 3u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16]))
        .with_object(create_data_mask(3, &DataMaskBody::default()))
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([31u16, 32u16]),
        )
        .with_object(create_key(
            31,
            &KeyBody {
                key_code: 21,
                ..Default::default()
            },
        ))
        .with_object(create_key(
            32,
            &KeyBody {
                key_code: 22,
                ..Default::default()
            },
        ))
}

fn paged_soft_key_navigation_pool() -> ObjectPool {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([3u16]),
        )
        .with_object(create_output_string(3, &OutputStringBody::default()).unwrap())
        .with_object(
            create_soft_key_mask(5, &SoftKeyMaskBody::default())
                .with_children([6u16, 7u16, 8u16, 9u16, 10u16, 11u16]),
        );
    for id in 6u16..=11 {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }
    pool
}

#[test]
fn runtime_change_soft_key_mask_releases_pressed_soft_key_when_erased() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 10));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(6)
    ));

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeSoftKeyMask {
                data_mask: ObjectID::new(2),
                soft_key_mask: ObjectID::new(10),
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { active_mask } if active_mask == ObjectID::new(2)
    ));
    assert_eq!(
        runtime.take_pending_activation_events(),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(6),
            code: ActivationCode::Released,
        }]
    );
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(!runtime
        .handle_operator_event(OperatorEvent::PointerUp(490, 10))
        .iter()
        .any(|event| matches!(event, VtEvent::SoftKeyActivation { id, .. } if *id == ObjectID::new(6))));
}

#[test]
fn runtime_hide_show_releases_pressed_button_when_parent_container_is_hidden() {
    let mut runtime =
        VtRenderRuntime::from_pool(container_button_activation_pool(), LayoutConfig::default())
            .unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::HideShow {
                id: ObjectID::new(10),
                visible: false,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { active_mask } if active_mask == ObjectID::new(2)
    ));
    assert_eq!(
        runtime.take_pending_activation_events(),
        vec![VtEvent::ButtonActivation {
            id: ObjectID::new(8),
            code: ActivationCode::Released,
        }]
    );
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(!runtime
        .handle_operator_event(OperatorEvent::PointerUp(5, 5))
        .iter()
        .any(|event| matches!(event, VtEvent::ButtonActivation { id, .. } if *id == ObjectID::new(8))));
}

#[test]
fn runtime_child_position_releases_touch_pressed_button_when_moved_off_touch_point() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeChildPosition {
                parent: ObjectID::new(2),
                child: ObjectID::new(8),
                x: 100,
                y: 0,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { active_mask } if active_mask == ObjectID::new(2)
    ));
    assert_eq!(
        runtime.take_pending_activation_events(),
        vec![VtEvent::ButtonActivation {
            id: ObjectID::new(8),
            code: ActivationCode::Released,
        }]
    );
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(!runtime
        .handle_operator_event(OperatorEvent::PointerUp(5, 5))
        .iter()
        .any(|event| matches!(event, VtEvent::ButtonActivation { id, .. } if *id == ObjectID::new(8))));
}

#[test]
fn runtime_child_position_preserves_touch_pressed_button_when_still_under_touch_point() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));

    assert!(matches!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeChildPosition {
                parent: ObjectID::new(2),
                child: ObjectID::new(8),
                x: 1,
                y: 1,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt { active_mask } if active_mask == ObjectID::new(2)
    ));
    assert!(runtime.take_pending_activation_events().is_empty());
    assert!(matches!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(8)
    ));
    assert!(matches!(
        runtime
            .handle_operator_event(OperatorEvent::PointerUp(5, 5))
            .as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Released,
        }] if *id == ObjectID::new(8)
    ));
}

#[test]
fn runtime_pointer_move_off_pressed_soft_key_aborts_and_ignores_later_release() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 10));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(6)
    ));

    assert!(matches!(
        runtime
            .handle_operator_event(OperatorEvent::PointerMove(5, 5))
            .as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Aborted,
        }] if *id == ObjectID::new(6)
    ));
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(!runtime
        .handle_operator_event(OperatorEvent::PointerUp(5, 5))
        .iter()
        .any(|event| matches!(event, VtEvent::SoftKeyActivation { id, .. } if *id == ObjectID::new(6))));
}

#[test]
fn runtime_pointer_move_off_pressed_button_aborts_and_ignores_later_release() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));

    assert!(matches!(
        runtime
            .handle_operator_event(OperatorEvent::PointerMove(100, 100))
            .as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Aborted,
        }] if *id == ObjectID::new(8)
    ));
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(!runtime
        .handle_operator_event(OperatorEvent::PointerUp(100, 100))
        .iter()
        .any(|event| matches!(event, VtEvent::ButtonActivation { id, .. } if *id == ObjectID::new(8))));
}

#[test]
fn runtime_pointer_move_off_pressed_key_group_key_aborts_and_ignores_later_release() {
    let mut runtime = VtRenderRuntime::from_pool(
        key_group_activation_pool(),
        LayoutConfig {
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();

    let (pressed, pressed_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerDown(5, 5))
        .unwrap();
    assert_eq!(
        pressed,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Pressed,
        }]
    );
    assert_eq!(pressed_messages.len(), 1);

    let (aborted, aborted_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerMove(5, 45))
        .unwrap();
    assert_eq!(
        aborted,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Aborted,
        }]
    );
    assert_eq!(aborted_messages.len(), 1);
    assert_eq!(
        aborted_messages[0].as_bytes(),
        &[
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Aborted.as_u8(),
            31,
            0,
            30,
            0,
            21,
            0xFF,
        ]
    );
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );

    let (release, release_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PointerUp(5, 45))
        .unwrap();
    assert!(release_messages.is_empty());
    assert!(
        !release
            .iter()
            .any(|event| matches!(event, VtEvent::SoftKeyActivation { .. })),
        "later physical release after slide-off abort must not emit a second key activation"
    );
}

#[test]
fn runtime_active_mask_change_releases_pressed_key_group_key_before_erasing_it() {
    let mut runtime = VtRenderRuntime::from_pool(
        key_group_activation_pool(),
        LayoutConfig {
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert_eq!(
        pressed,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Pressed,
        }]
    );

    assert!(matches!(
        runtime.set_active_mask(ObjectID::new(3)).unwrap(),
        RenderUpdate::SceneRebuilt { active_mask } if active_mask == ObjectID::new(3)
    ));
    assert_eq!(
        runtime.take_pending_activation_events(),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Released,
        }]
    );
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(
        !runtime
            .handle_operator_event(OperatorEvent::PointerUp(5, 5))
            .iter()
            .any(|event| matches!(event, VtEvent::SoftKeyActivation { id, .. } if *id == ObjectID::new(31))),
        "later physical release after display-change release must not emit a second key activation"
    );
}

#[test]
fn runtime_active_mask_change_releases_pressed_soft_key_before_erasing_it() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 10));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(6)
    ));

    assert!(matches!(
        runtime.set_active_mask(ObjectID::new(3)).unwrap(),
        RenderUpdate::SceneRebuilt { active_mask } if active_mask == ObjectID::new(3)
    ));
    assert_eq!(
        runtime.take_pending_activation_events(),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(6),
            code: ActivationCode::Released,
        }]
    );
    assert!(runtime.pending_activation_events().is_empty());
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(!runtime
        .handle_operator_event(OperatorEvent::PointerUp(490, 10))
        .iter()
        .any(|event| matches!(event, VtEvent::SoftKeyActivation { id, .. } if *id == ObjectID::new(6))));
}

#[test]
fn runtime_active_mask_change_releases_pressed_button_before_erasing_it() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));

    assert!(matches!(
        runtime.set_active_mask(ObjectID::new(3)).unwrap(),
        RenderUpdate::SceneRebuilt { active_mask } if active_mask == ObjectID::new(3)
    ));
    assert_eq!(
        runtime.take_pending_activation_events(),
        vec![VtEvent::ButtonActivation {
            id: ObjectID::new(8),
            code: ActivationCode::Released,
        }]
    );
    assert!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .is_empty()
    );
    assert!(!runtime
        .handle_operator_event(OperatorEvent::PointerUp(5, 5))
        .iter()
        .any(|event| matches!(event, VtEvent::ButtonActivation { id, .. } if *id == ObjectID::new(8))));
}

#[test]
fn runtime_rejects_second_soft_key_press_until_first_release() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let first_press = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 10));
    assert!(matches!(
        first_press.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(6)
    ));

    let second_press = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 130));
    assert_eq!(
        second_press,
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(1)),
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::SoftKeyActivate(ObjectID::new(7))),
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::Tap(490, 130)),
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert!(matches!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(6)
    ));

    let first_release = runtime.handle_operator_event(OperatorEvent::PointerUp(490, 10));
    assert!(matches!(
        first_release.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Released,
        }] if *id == ObjectID::new(6)
    ));

    let second_after_release = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 130));
    assert!(matches!(
        second_after_release.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(7)
    ));
}

#[test]
fn runtime_rejects_second_button_press_until_first_release() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let first_press = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        first_press.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));

    let second_press = runtime.handle_operator_event(OperatorEvent::PointerDown(65, 5));
    assert_eq!(
        second_press,
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert!(matches!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(8)
    ));

    let first_release = runtime.handle_operator_event(OperatorEvent::PointerUp(5, 5));
    assert!(matches!(
        first_release.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Released,
        }] if *id == ObjectID::new(8)
    ));

    let second_after_release = runtime.handle_operator_event(OperatorEvent::PointerDown(65, 5));
    assert!(matches!(
        second_after_release.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(9)
    ));
}

#[test]
fn runtime_physical_soft_key_down_up_emits_press_hold_and_release() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();

    let (pressed, pressed_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PhysicalSoftKeyDown(0))
        .unwrap();
    assert_eq!(
        pressed,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(6),
            code: ActivationCode::Pressed,
        }]
    );
    assert_eq!(pressed_messages.len(), 1);

    assert!(matches!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(6)
    ));

    let (released, released_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::PhysicalSoftKeyUp(0))
        .unwrap();
    assert_eq!(
        released,
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(6),
            code: ActivationCode::Released,
        }]
    );
    assert_eq!(released_messages.len(), 1);
}

#[test]
fn runtime_physical_soft_key_down_up_supports_user_layout_key_group_cells() {
    let mut runtime = VtRenderRuntime::from_pool(
        key_group_activation_pool(),
        LayoutConfig {
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 2),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyDown(2)),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Pressed,
        }]
    );
    assert!(matches!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(31)
    ));
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyUp(2)),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Released,
        }]
    );
}

#[test]
fn runtime_physical_key_group_soft_key_ignores_stray_pointer_events_until_physical_release() {
    let mut runtime = VtRenderRuntime::from_pool(
        key_group_activation_pool(),
        LayoutConfig {
            physical_soft_key_count: 6,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 2),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let key_group_rect = runtime.scene().find(ObjectID::new(30)).unwrap().rect;

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyDown(2)),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Pressed,
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PointerMove(1, 1)),
        vec![VtEvent::Ignored {
            reason: "pointer move ignored for physical key-group press",
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PointerUp(
            key_group_rect.x + 1,
            key_group_rect.y + 1,
        )),
        vec![VtEvent::Ignored {
            reason: "pointer release ignored for physical key-group press",
        }]
    );
    assert!(matches!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(31)
    ));
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyUp(2)),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(31),
            code: ActivationCode::Released,
        }]
    );
}

#[test]
fn runtime_physical_navigation_soft_key_waits_for_release_before_changing_page() {
    let mut runtime = VtRenderRuntime::from_pool(
        paged_soft_key_navigation_pool(),
        LayoutConfig {
            physical_soft_key_count: 4,
            navigation_soft_key_count: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let next_cell = runtime
        .scene()
        .soft_keys
        .iter()
        .find(|key| key.kind == SoftKeyKind::NavigationNext && key.enabled)
        .map(|key| key.cell_index)
        .expect("next navigation cell is rendered");

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyDown(next_cell)),
        vec![VtEvent::Ignored {
            reason: "soft-key activation waits for release"
        }]
    );
    assert_eq!(runtime.soft_key_page(), 0);
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyUp(next_cell)),
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
    assert_eq!(runtime.soft_key_page(), 1);
}

#[test]
fn runtime_pending_navigation_soft_key_blocks_other_pointer_press_until_release() {
    let mut runtime = VtRenderRuntime::from_pool(
        paged_soft_key_navigation_pool(),
        LayoutConfig {
            physical_soft_key_count: 4,
            navigation_soft_key_count: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let next_cell = runtime
        .scene()
        .soft_keys
        .iter()
        .find(|key| key.kind == SoftKeyKind::NavigationNext && key.enabled)
        .map(|key| key.cell_index)
        .expect("next navigation cell is rendered");
    let app_rect = runtime
        .scene()
        .soft_keys
        .iter()
        .find(|key| key.kind == SoftKeyKind::Application && key.enabled)
        .map(|key| key.rect)
        .expect("application soft-key cell is rendered");

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyDown(next_cell)),
        vec![VtEvent::Ignored {
            reason: "soft-key activation waits for release"
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKey(0)),
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::SoftKeyNavigation(
            SoftKeyKind::NavigationNext
        )),
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PointerDown(
            app_rect.x + 1,
            app_rect.y + 1
        )),
        vec![VtEvent::Ignored {
            reason: "simultaneous soft-key/button activation is not supported"
        }]
    );
    assert_eq!(runtime.soft_key_page(), 0);
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyUp(next_cell)),
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
    assert_eq!(runtime.soft_key_page(), 1);
}

#[test]
fn runtime_physical_navigation_soft_key_ignores_stray_pointer_release_until_physical_release() {
    let mut runtime = VtRenderRuntime::from_pool(
        paged_soft_key_navigation_pool(),
        LayoutConfig {
            physical_soft_key_count: 4,
            navigation_soft_key_count: 2,
            ..Default::default()
        },
    )
    .unwrap();
    let next_cell = runtime
        .scene()
        .soft_keys
        .iter()
        .find(|key| key.kind == SoftKeyKind::NavigationNext && key.enabled)
        .map(|key| key.cell_index)
        .expect("next navigation cell is rendered");
    let app_rect = runtime
        .scene()
        .soft_keys
        .iter()
        .find(|key| key.kind == SoftKeyKind::Application && key.enabled)
        .map(|key| key.rect)
        .expect("application soft-key cell is rendered");

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyDown(next_cell)),
        vec![VtEvent::Ignored {
            reason: "soft-key activation waits for release"
        }]
    );
    assert!(
        runtime
            .handle_operator_event(OperatorEvent::PointerUp(app_rect.x + 1, app_rect.y + 1))
            .is_empty()
    );
    assert_eq!(runtime.soft_key_page(), 0);
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyUp(next_cell)),
        vec![VtEvent::SoftKeyPageChanged {
            page: 1,
            page_count: 3,
        }]
    );
}

#[test]
fn runtime_physical_application_soft_key_ignores_stray_pointer_release_until_physical_release() {
    let mut runtime =
        VtRenderRuntime::from_pool(activation_exclusivity_pool(), LayoutConfig::default()).unwrap();
    let second_cell_rect = runtime
        .scene()
        .soft_keys
        .iter()
        .find(|key| key.id == ObjectID::new(7))
        .map(|key| key.rect)
        .expect("second application soft-key is rendered");

    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyDown(0)),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(6),
            code: ActivationCode::Pressed,
        }]
    );
    assert!(
        runtime
            .handle_operator_event(OperatorEvent::PointerUp(
                second_cell_rect.x + 1,
                second_cell_rect.y + 1
            ))
            .is_empty()
    );
    assert!(matches!(
        runtime
            .advance_activation_hold_time(ActivationHoldTiming::DEFAULT_INITIAL_DELAY_MS, ActivationHoldTiming::default())
            .as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(6)
    ));
    assert_eq!(
        runtime.handle_operator_event(OperatorEvent::PhysicalSoftKeyUp(0)),
        vec![VtEvent::SoftKeyActivation {
            id: ObjectID::new(6),
            code: ActivationCode::Released,
        }]
    );
}
