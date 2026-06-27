#[test]
fn runtime_persists_operator_user_layout_placements_across_rebuilds() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16, 40u16]))
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    width_cells: 1,
                    height_cells: 2,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_key_group(
                40,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([41u16]),
        )
        .with_object(create_key(
            41,
            &KeyBody {
                key_code: 41,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool.clone(), LayoutConfig::default()).unwrap();

    assert!(matches!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(30), 1, 4),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert_eq!(
        runtime.scene().find(ObjectID::new(30)).unwrap().rect,
        Rect::new(240, 160, 240, 80)
    );
    assert!(
        runtime
            .place_window_mask_in_user_layout(ObjectID::new(30), 1, 5)
            .is_err(),
        "1x2 window cannot start in the final row of the 2x6 user-layout grid"
    );
    assert_eq!(
        runtime.scene().find(ObjectID::new(30)).unwrap().rect,
        Rect::new(240, 160, 240, 80),
        "rejected mapping must not mutate the stored placement"
    );

    assert!(matches!(
        runtime.place_key_group_in_user_layout(ObjectID::new(40), 3),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert_eq!(
        runtime.scene().find(ObjectID::new(40)).unwrap().rect,
        Rect::new(480, 120, 64, 40)
    );
    let snapshot = runtime.user_layout_placements();
    assert_eq!(
        snapshot,
        vec![
            UserLayoutPlacement::WindowMask {
                id: ObjectID::new(30),
                column: 1,
                row: 4,
            },
            UserLayoutPlacement::KeyGroup {
                id: ObjectID::new(40),
                first_cell: 3,
            },
        ]
    );

    let mut recalled = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    assert!(matches!(
        recalled.restore_user_layout_placements(&snapshot),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert_eq!(recalled.user_layout_placements(), snapshot);
    assert_eq!(
        recalled.scene().find(ObjectID::new(30)).unwrap().rect,
        Rect::new(240, 160, 240, 80)
    );
    assert_eq!(
        recalled.scene().find(ObjectID::new(40)).unwrap().rect,
        Rect::new(480, 120, 64, 40)
    );

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeBackgroundColour {
            id: ObjectID::new(2),
            colour: 7,
        })
        .unwrap();
    assert_eq!(
        runtime.scene().find(ObjectID::new(30)).unwrap().rect,
        Rect::new(240, 160, 240, 80),
        "operator-selected user-layout placements survive normal scene rebuilds"
    );
    assert_eq!(
        runtime.scene().find(ObjectID::new(40)).unwrap().rect,
        Rect::new(480, 120, 64, 40)
    );

    assert!(matches!(
        runtime.clear_user_layout_placement(ObjectID::new(30)),
        RenderUpdate::SceneRebuilt { .. }
    ));
    assert_ne!(
        runtime.scene().find(ObjectID::new(30)).unwrap().rect,
        Rect::new(240, 160, 240, 80)
    );
}

#[test]
fn gtui_blanks_unavailable_transparent_key_group_user_layout_cells() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([40u16]))
        .with_object(
            create_key_group(
                40,
                &KeyGroupBody {
                    options: 0x03,
                    ..Default::default()
                },
            )
            .with_children([41u16]),
        )
        .with_object(create_key(
            41,
            &KeyBody {
                key_code: 41,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    runtime
        .place_key_group_in_user_layout(ObjectID::new(40), 0)
        .unwrap();

    let renderer = GtuiRenderer::default();
    let group_rect = Rect::new(480, 0, 64, 40);
    let available_commands = renderer.render(runtime.scene());
    assert!(
        available_commands
            .iter()
            .any(|command| matches!(command, RenderCommand::SoftKey { rect, .. } if *rect == group_rect)),
        "available Key Group renders its Key cell"
    );
    assert!(
        !available_commands
            .iter()
            .any(|command| matches!(command, RenderCommand::FillRect { rect, .. } if *rect == group_rect)),
        "transparent available Key Group preserves the underlying soft-key area"
    );

    assert_eq!(
        runtime.apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(40),
            attribute_id: 1,
            value: 0x02,
        }),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    let unavailable_commands = renderer.render(runtime.scene());
    assert!(
        unavailable_commands
            .iter()
            .any(|command| matches!(command, RenderCommand::FillRect { rect, .. } if *rect == group_rect)),
        "unavailable Key Group must blank/fill its remembered Key cell even when transparent"
    );
    assert!(
        !unavailable_commands
            .iter()
            .any(|command| matches!(command, RenderCommand::SoftKey { rect, .. } if *rect == group_rect)),
        "unavailable Key Group must not leave an activatable visible Key cell"
    );
}

#[test]
fn runtime_rejects_overlapping_user_layout_placements() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default())
                .with_children([30u16, 31u16, 40u16, 50u16]),
        )
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
                    width_cells: 2,
                    height_cells: 1,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_window_mask(
                31,
                &WindowMaskBody {
                    width_cells: 1,
                    height_cells: 1,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_key_group(
                40,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([41u16, 42u16]),
        )
        .with_object(create_key(41, &KeyBody::default()))
        .with_object(create_key(42, &KeyBody::default()))
        .with_object(
            create_key_group(
                50,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([51u16, 52u16]),
        )
        .with_object(create_key(51, &KeyBody::default()))
        .with_object(create_key(52, &KeyBody::default()));
    let mut runtime = VtRenderRuntime::from_pool(pool.clone(), LayoutConfig::default()).unwrap();

    assert!(matches!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(30), 0, 0),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert!(
        runtime
            .place_window_mask_in_user_layout(ObjectID::new(31), 1, 0)
            .is_err(),
        "Window Mask placements must not claim a cell already occupied by another Window Mask"
    );
    assert_eq!(
        runtime.user_layout_placements(),
        vec![UserLayoutPlacement::WindowMask {
            id: ObjectID::new(30),
            column: 0,
            row: 0,
        }]
    );
    assert!(matches!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(31), 0, 1),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));

    assert!(matches!(
        runtime.place_key_group_in_user_layout(ObjectID::new(40), 1),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert!(
        runtime
            .place_key_group_in_user_layout(ObjectID::new(50), 2)
            .is_err(),
        "Key Group placements must not claim a soft-key cell already occupied by another Key Group"
    );
    assert!(matches!(
        runtime.place_key_group_in_user_layout(ObjectID::new(50), 3),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));

    let mut overlapping_profile = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            soft_key_area: Rect::new(0, 0, 64, 240),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        overlapping_profile.place_window_mask_in_user_layout(ObjectID::new(30), 0, 0),
        Ok(RenderUpdate::SceneRebuilt { .. })
    ));
    assert!(
        overlapping_profile
            .place_key_group_in_user_layout(ObjectID::new(40), 0)
            .is_err(),
        "Window Mask and Key Group placements must not overlap when a VT profile maps soft-key cells into the same physical area as user-layout cells"
    );

    assert!(
        runtime
            .restore_user_layout_placements(&[
                UserLayoutPlacement::WindowMask {
                    id: ObjectID::new(30),
                    column: 0,
                    row: 0,
                },
                UserLayoutPlacement::WindowMask {
                    id: ObjectID::new(31),
                    column: 1,
                    row: 0,
                },
            ])
            .is_err()
    );
}

#[test]
fn runtime_rejects_key_group_placement_on_reserved_navigation_soft_key_cells() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(20),
                    ..Default::default()
                },
            )
            .with_children([30u16]),
        )
        .with_object(
            create_soft_key_mask(20, &SoftKeyMaskBody::default()).with_children([
                101u16, 102, 103, 104, 105, 106, 107, 108,
            ]),
        )
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([31u16, 32u16]),
        );
    for id in [31u16, 32, 101, 102, 103, 104, 105, 106, 107, 108] {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 6,
            navigation_soft_key_count: 2,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(
        runtime
            .place_key_group_in_user_layout(ObjectID::new(30), 2)
            .is_err(),
        "Key Groups must not overlap active Soft Key Mask application cells"
    );
    assert!(
        runtime
            .place_key_group_in_user_layout(ObjectID::new(30), 3)
            .is_err(),
        "Key Groups must not straddle the first VT-reserved navigation soft-key cell"
    );
    assert!(
        runtime
            .place_key_group_in_user_layout(ObjectID::new(30), 4)
            .is_err(),
        "Key Groups must not start inside VT-reserved navigation soft-key cells"
    );
}

#[test]
fn runtime_allows_key_group_placement_in_navigation_config_when_no_paging_is_active() {
    let mut pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(20),
                    ..Default::default()
                },
            )
            .with_children([30u16]),
        )
        .with_object(
            create_soft_key_mask(20, &SoftKeyMaskBody::default())
                .with_children([101u16, 102, 103, 104]),
        )
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 0x01,
                    ..Default::default()
                },
            )
            .with_children([31u16, 32u16]),
        );
    for id in [31u16, 32, 101, 102, 103, 104] {
        pool = pool.with_object(create_key(id, &KeyBody::default()));
    }

    let mut runtime = VtRenderRuntime::from_pool(
        pool,
        LayoutConfig {
            physical_soft_key_count: 6,
            navigation_soft_key_count: 2,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(
        runtime
            .place_key_group_in_user_layout(ObjectID::new(30), 3)
            .is_err(),
        "Key Groups must not overlap active Soft Key Mask application cells"
    );
    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(30), 4),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
}

#[test]
fn runtime_rejects_null_object_ids_before_vt_to_ecu_message_emission() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()));
    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert!(
        runtime
            .bus_messages_for_events(&[VtEvent::InputEsc {
                id: ObjectID::NULL,
                error_code: 0,
                transfer_sequence_number: Some(0x0A),
            }])
            .is_err()
    );
    assert!(
        runtime
            .messages_for_events(
                &[VtEvent::NumberValueChanged {
                    id: ObjectID::NULL,
                    raw: 12,
                }],
                0x80,
                0x42,
            )
            .is_err()
    );
}

#[test]
fn runtime_wraps_vt_events_in_full_vt_to_ecu_messages() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    assert!(
        runtime
            .handle_operator_event_with_messages(OperatorEvent::FocusNext, NULL_ADDRESS, 0x42)
            .is_err()
    );
    assert_eq!(runtime.selected_input(), None);
    assert_eq!(runtime.open_input(), None);

    runtime
        .handle_operator_event_with_messages(OperatorEvent::FocusNext, 0x80, 0x42)
        .unwrap();
    assert!(
        runtime
            .handle_operator_event_with_messages(OperatorEvent::Char('X'), NULL_ADDRESS, 0x42)
            .is_err()
    );
    assert_eq!(runtime.selected_input(), Some(ObjectID::new(3)));
    assert_eq!(runtime.open_input(), None);
    runtime
        .handle_operator_event_with_messages(OperatorEvent::Char('A'), 0x80, 0x42)
        .unwrap();

    let (events, messages) = runtime
        .handle_operator_event_with_messages(OperatorEvent::Cancel, 0x80, 0x42)
        .unwrap();

    assert!(matches!(
        events.as_slice(),
        [VtEvent::InputEsc {
            id,
            error_code: 0,
            transfer_sequence_number: None,
        }] if *id == ObjectID::new(3)
    ));
    assert_eq!(
        messages
            .iter()
            .map(|message| (
                message.pgn,
                message.source,
                message.destination,
                message.data.clone()
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                PGN_VT_TO_ECU,
                0x80,
                0x42,
                vec![cmd::VT_ESC, 3, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
            ),
            (
                PGN_VT_TO_ECU,
                0x80,
                0x42,
                vec![cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 0, 0xFF, 0xFF, 0xFF],
            ),
        ]
    );

    let direct = runtime
        .messages_for_events(
            &[VtEvent::InputEsc {
                id: ObjectID::new(3),
                error_code: 0x10,
                transfer_sequence_number: Some(0x0A),
            }],
            0x80,
            0x42,
        )
        .unwrap();
    assert_eq!(direct[0].pgn, PGN_VT_TO_ECU);
    assert_eq!(direct[0].source, 0x80);
    assert_eq!(direct[0].destination, 0x42);
    assert_eq!(
        direct[0].data,
        vec![cmd::VT_ESC, 3, 0, 0x10, 0xFF, 0xFF, 0xFF, 0xAF]
    );

    let ordered = runtime
        .messages_for_events(
            &[
                VtEvent::FocusChanged {
                    id: ObjectID::new(3),
                },
                VtEvent::InputEsc {
                    id: ObjectID::new(3),
                    error_code: 0x10,
                    transfer_sequence_number: Some(0x0B),
                },
                VtEvent::StringValueChanged {
                    id: ObjectID::new(3),
                    text: "OK".to_string(),
                },
            ],
            0x80,
            0x42,
        )
        .unwrap();
    assert_eq!(
        ordered
            .iter()
            .map(|message| message.data.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 0, 0xFF, 0xFF, 0xFF],
            vec![cmd::VT_ESC, 3, 0, 0x10, 0xFF, 0xFF, 0xFF, 0xBF],
            vec![cmd::STRING_VALUE_CHANGE, 3, 0, 2, 0, b'O', b'K'],
        ],
        "direct full-message lowering must preserve caller event order around VT ESC TAN payloads"
    );
    assert!(ordered.iter().all(|message| message.pgn == PGN_VT_TO_ECU
        && message.source == 0x80
        && message.destination == 0x42));
    assert!(
        runtime
            .messages_for_events(
                &[
                    VtEvent::FocusChanged {
                        id: ObjectID::new(3),
                    },
                    VtEvent::InputEsc {
                        id: ObjectID::new(3),
                        error_code: 0,
                        transfer_sequence_number: Some(0x10),
                    },
                ],
                0x80,
                0x42,
            )
            .is_err(),
        "ordered full-message emission must reject malformed VT ESC TAN instead of returning a partial prefix"
    );

    assert!(
        runtime
            .messages_for_events(
                &[VtEvent::InputEsc {
                    id: ObjectID::new(3),
                    error_code: 0,
                    transfer_sequence_number: None,
                }],
                NULL_ADDRESS,
                0x42,
            )
            .is_err()
    );
    assert!(
        runtime
            .messages_for_events(
                &[VtEvent::InputEsc {
                    id: ObjectID::new(3),
                    error_code: 0,
                    transfer_sequence_number: None,
                }],
                0x80,
                BROADCAST_ADDRESS,
            )
            .is_err()
    );
}

#[test]
fn runtime_generates_held_activation_repeats_from_pressed_state() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .with_children([8u16]),
        )
        .with_object(create_soft_key_mask(5, &SoftKeyMaskBody::default()).with_children([6u16]))
        .with_object(create_key(
            6,
            &KeyBody {
                key_code: 42,
                ..Default::default()
            },
        ))
        .with_object(create_button(
            8,
            &ButtonBody {
                width: 40,
                height: 20,
                key_code: 9,
                ..Default::default()
            },
        ));
    let timing = ActivationHoldTiming::new(500, 200);
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    let pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(490, 10));
    assert!(matches!(
        pressed.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(6)
    ));
    assert!(
        runtime
            .advance_activation_hold_time_with_messages(500, timing, NULL_ADDRESS, 0x42)
            .is_err()
    );
    assert!(runtime.advance_activation_hold_time(499, timing).is_empty());
    let first_held = runtime
        .advance_activation_hold_time_with_messages(1, timing, 0x80, 0x42)
        .unwrap();
    assert!(matches!(
        first_held.0.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(6)
    ));
    assert_eq!(first_held.1[0].pgn, PGN_VT_TO_ECU);
    assert_eq!(first_held.1[0].source, 0x80);
    assert_eq!(first_held.1[0].destination, 0x42);
    assert_eq!(
        first_held.1[0].data,
        vec![
            cmd::SOFT_KEY_ACTIVATION,
            ActivationCode::Held.as_u8(),
            6,
            0,
            5,
            0,
            42,
            0xFF
        ]
    );
    assert!(runtime.advance_activation_hold_time(199, timing).is_empty());
    assert!(matches!(
        runtime.advance_activation_hold_time(1, timing).as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(6)
    ));
    let released = runtime.handle_operator_event(OperatorEvent::PointerUp(490, 10));
    assert!(matches!(
        released.as_slice(),
        [VtEvent::SoftKeyActivation {
            id,
            code: ActivationCode::Released,
        }] if *id == ObjectID::new(6)
    ));
    assert!(
        runtime
            .advance_activation_hold_time(1000, timing)
            .is_empty()
    );

    let button_pressed = runtime.handle_operator_event(OperatorEvent::PointerDown(5, 5));
    assert!(matches!(
        button_pressed.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Pressed,
        }] if *id == ObjectID::new(8)
    ));
    let button_held = runtime.advance_activation_hold_time(500, timing);
    assert!(matches!(
        button_held.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Held,
        }] if *id == ObjectID::new(8)
    ));
    let aborted = runtime.handle_operator_event(OperatorEvent::PointerUp(80, 80));
    assert!(matches!(
        aborted.as_slice(),
        [VtEvent::ButtonActivation {
            id,
            code: ActivationCode::Aborted,
        }] if *id == ObjectID::new(8)
    ));
    assert!(runtime.advance_activation_hold_time(500, timing).is_empty());
}

#[test]
fn runtime_event_bus_bridge_sequences_input_open_and_close() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_number(
                4,
                &InputNumberBody {
                    options: 0x01,
                    options2: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    let (focus_events, focus_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::FocusNext)
        .unwrap();
    assert_eq!(
        focus_events,
        vec![VtEvent::FocusChanged {
            id: ObjectID::new(3)
        }]
    );
    assert_eq!(
        focus_messages[0].as_bytes(),
        &[cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 0, 0xFF, 0xFF, 0xFF]
    );

    let (type_events, type_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::Char('A'))
        .unwrap();
    assert!(matches!(
        type_events.as_slice(),
        [VtEvent::StringEditPreview { id, text }] if *id == ObjectID::new(3) && text == "A"
    ));
    assert_eq!(
        type_messages[0].as_bytes(),
        &[cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 1, 0xFF, 0xFF, 0xFF]
    );

    let (commit_events, commit_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::Commit)
        .unwrap();
    assert!(matches!(
        commit_events.as_slice(),
        [VtEvent::StringValueChanged { id, text }] if *id == ObjectID::new(3) && text == "A"
    ));
    assert_eq!(
        commit_messages
            .iter()
            .map(|message| message.as_bytes().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![cmd::STRING_VALUE_CHANGE, 3, 0, 1, 0, b'A'],
            vec![cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 0, 0xFF, 0xFF, 0xFF],
        ]
    );

    let (_, reopen_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::Char('B'))
        .unwrap();
    assert_eq!(
        reopen_messages[0].as_bytes(),
        &[cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 1, 0xFF, 0xFF, 0xFF]
    );
    let (cancel_events, cancel_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::Cancel)
        .unwrap();
    assert!(matches!(
        cancel_events.as_slice(),
        [VtEvent::InputEsc {
            id,
            error_code: 0,
            transfer_sequence_number: None,
        }] if *id == ObjectID::new(3)
    ));
    assert_eq!(
        cancel_messages
            .iter()
            .map(|message| message.as_bytes().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![cmd::VT_ESC, 3, 0, 0, 0xFF, 0xFF, 0xFF, 0xFF],
            vec![cmd::SELECT_INPUT_OBJECT, 3, 0, 1, 0, 0xFF, 0xFF, 0xFF],
        ]
    );

    let (next_events, next_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::FocusNext)
        .unwrap();
    assert_eq!(
        next_events,
        vec![VtEvent::FocusChanged {
            id: ObjectID::new(4)
        }]
    );
    assert_eq!(
        next_messages
            .iter()
            .map(|message| message.as_bytes().to_vec())
            .collect::<Vec<_>>(),
        vec![
            vec![cmd::SELECT_INPUT_OBJECT, 3, 0, 0, 0, 0xFF, 0xFF, 0xFF],
            vec![cmd::SELECT_INPUT_OBJECT, 4, 0, 1, 0, 0xFF, 0xFF, 0xFF],
        ]
    );
}

#[test]
fn runtime_applies_ecu_select_input_and_esc_to_input_state() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16]),
        )
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_string(
                4,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_button(
            5,
            &ButtonBody {
                options: 0x01,
                width: 100,
                height: 20,
                ..Default::default()
            },
        ));
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
            id: ObjectID::new(4),
            open_for_input: false,
        })
        .unwrap();
    assert!(matches!(update, RenderUpdate::NotRenderAffecting { .. }));
    assert_eq!(runtime.input().selected_input(), Some(ObjectID::new(4)));
    assert_eq!(runtime.input().open_input(), None);

    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
            id: ObjectID::new(3),
            open_for_input: true,
        })
        .unwrap();
    assert!(matches!(update, RenderUpdate::NotRenderAffecting { .. }));
    assert_eq!(runtime.input().selected_input(), Some(ObjectID::new(3)));
    assert_eq!(
        runtime.input().open_input(),
        Some(ObjectID::new(3)),
        "Select Input Object option 0 must open input fields for data input"
    );

    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
            id: ObjectID::new(5),
            open_for_input: false,
        })
        .unwrap();
    assert!(matches!(update, RenderUpdate::NotRenderAffecting { .. }));
    assert_eq!(
        runtime.input().selected_input(),
        Some(ObjectID::new(5)),
        "Select Input Object option FF must admit Button focus targets"
    );
    assert_eq!(runtime.input().open_input(), None);

    let (typed_events, typed_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::Char('Q'))
        .unwrap();
    assert!(matches!(typed_events.as_slice(), [VtEvent::Ignored { .. }]));
    assert!(typed_messages.is_empty());

    runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
            id: ObjectID::new(4),
            open_for_input: false,
        })
        .unwrap();
    let (typed_events, typed_messages) = runtime
        .handle_operator_event_with_bus_messages(OperatorEvent::Char('Q'))
        .unwrap();
    assert!(matches!(
        typed_events.as_slice(),
        [VtEvent::StringEditPreview { id, text }] if *id == ObjectID::new(4) && text == "Q"
    ));
    assert_eq!(
        typed_messages[0].as_bytes(),
        &[cmd::SELECT_INPUT_OBJECT, 4, 0, 1, 1, 0xFF, 0xFF, 0xFF]
    );

    let update = runtime.apply_ecu_command(&VtRuntimeCommand::Esc).unwrap();
    assert!(matches!(update, RenderUpdate::NotRenderAffecting { .. }));
    assert_eq!(runtime.input().selected_input(), Some(ObjectID::new(4)));
    assert_eq!(runtime.input().open_input(), None);
    assert_eq!(runtime.input().edit_state(), &EditState::Idle);

    let commit_events = runtime.handle_operator_event(OperatorEvent::Commit);
    assert!(matches!(
        commit_events.as_slice(),
        [VtEvent::Ignored { .. }]
    ));
}

#[test]
fn render_runtime_select_input_object_can_focus_key_group_keys() {
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

    let focus_update = runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
            id: ObjectID::new(31),
            open_for_input: false,
        })
        .unwrap();
    assert!(matches!(
        focus_update,
        RenderUpdate::NotRenderAffecting { .. }
    ));
    assert_eq!(runtime.input().selected_input(), Some(ObjectID::new(31)));
    assert_eq!(runtime.input().open_input(), None);

    let open_update = runtime
        .apply_ecu_command(&VtRuntimeCommand::SelectInputObject {
            id: ObjectID::new(31),
            open_for_input: true,
        })
        .unwrap();
    assert!(matches!(open_update, RenderUpdate::Unchanged));
    assert_eq!(
        runtime.input().selected_input(),
        Some(ObjectID::new(31)),
        "Key Group Key focus must remain focus-only; Keys cannot be opened for text/number editing"
    );
    assert_eq!(runtime.input().open_input(), None);
}

#[test]
fn runtime_focus_and_typing_route_to_vt_events() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_number(
                4,
                &InputNumberBody {
                    options: 0x01,
                    options2: 0x01,
                    width: 100,
                    height: 20,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);

    let mut rt = InputRuntime::new();
    rt.bind(&scene);

    // Focus the first field, type into it.
    let _focus = rt.handle(&scene, &OperatorEvent::FocusNext);
    assert!(rt.focused().is_some());
    let typed = rt.handle(&scene, &OperatorEvent::Char('Z'));
    assert!(matches!(
        typed[0],
        VtEvent::StringEditPreview { id, ref text } if id == ObjectID::new(3) && text == "Z"
    ));
    assert_eq!(rt.selected_input(), Some(ObjectID::new(3)));
    assert_eq!(rt.open_input(), Some(ObjectID::new(3)));
    assert!(matches!(rt.edit_state(), EditState::String { .. }));
    let committed = rt.handle(&scene, &OperatorEvent::Commit);
    assert!(matches!(
        committed[0],
        VtEvent::StringValueChanged { id, ref text } if id == ObjectID::new(3) && text == "Z"
    ));

    // Move to the number field and type digits.
    let _ = rt.handle(&scene, &OperatorEvent::FocusNext);
    let _n1 = rt.handle(&scene, &OperatorEvent::Char('7'));
    let n2 = rt.handle(&scene, &OperatorEvent::Char('3'));
    assert!(matches!(
        n2[0],
        VtEvent::NumberEditPreview { id, raw: 73 } if id == ObjectID::new(4)
    ));
    let committed = rt.handle(&scene, &OperatorEvent::Commit);
    assert!(matches!(
        committed[0],
        VtEvent::NumberValueChanged { id, raw: 73 } if id == ObjectID::new(4)
    ));
}

#[test]
fn runtime_input_string_honours_extended_input_attributes_ranges() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    input_attributes: 4.into(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_extended_input_attributes(
                4,
                &ExtendedInputAttributesBody {
                    validation_type: 0,
                    code_planes: vec![
                        ExtendedInputCodePlane {
                            plane: 0,
                            ranges: vec![WideCharRange {
                                first: 0x00E0,
                                last: 0x00FF,
                            }],
                        },
                        ExtendedInputCodePlane {
                            plane: 1,
                            ranges: vec![WideCharRange {
                                first: 0xF600,
                                last: 0xF64F,
                            }],
                        },
                    ],
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);

    let _focus = rt.handle(&scene, &OperatorEvent::FocusNext);
    let accepted_accent = rt.handle(&scene, &OperatorEvent::Char('é'));
    let accepted_emoji = rt.handle(&scene, &OperatorEvent::Char('😀'));
    let rejected_ascii = rt.handle(&scene, &OperatorEvent::Char('A'));

    assert!(
        matches!(&accepted_accent[0], VtEvent::StringEditPreview { id, text } if *id == ObjectID::new(3) && text == "é")
    );
    assert!(
        matches!(&accepted_emoji[0], VtEvent::StringEditPreview { id, text } if *id == ObjectID::new(3) && text == "é😀")
    );
    assert!(matches!(rejected_ascii[0], VtEvent::Ignored { .. }));
}

#[test]
fn render_runtime_decodes_wide_strings_and_matches_validation_encoding() {
    let wide_e_acute = vec![0xFF, 0xFE, 0xE9, 0x00];
    let extended_attrs = create_extended_input_attributes(
        11,
        &ExtendedInputAttributesBody {
            validation_type: 0,
            code_planes: vec![ExtendedInputCodePlane {
                plane: 0,
                ranges: vec![WideCharRange {
                    first: 0x00E9,
                    last: 0x00E9,
                }],
            }],
        },
    )
    .unwrap();

    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 4u16, 5u16]),
        )
        .with_object(
            create_output_string(
                3,
                &OutputStringBody {
                    variable_reference: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_string(
                4,
                &InputStringBody {
                    options: 0x01,
                    input_attributes: ObjectID::new(11),
                    variable_reference: ObjectID::new(10),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_string(
                5,
                &InputStringBody {
                    options: 0x01,
                    input_attributes: ObjectID::new(11),
                    variable_reference: ObjectID::new(12),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            10,
            &StringVariableBody {
                length: wide_e_acute.len() as u16,
                value: wide_e_acute,
            },
        ))
        .with_object(create_string_variable(
            12,
            &StringVariableBody {
                length: 1,
                value: b"A".to_vec(),
            },
        ))
        .with_object(extended_attrs);

    let scene = render(&pool, ObjectID::NULL);
    assert!(matches!(
        scene.find(ObjectID::new(3)).map(|node| &node.kind),
        Some(NodeKind::OutputString { text, .. }) if text == "é"
    ));
    match scene.find(ObjectID::new(4)).map(|node| &node.kind) {
        Some(NodeKind::InputString {
            text, validation, ..
        }) => {
            assert_eq!(text, "é");
            let rule = validation
                .as_ref()
                .expect("Extended Input Attributes apply to WideStrings");
            assert!(rule.accepts('é'));
            assert!(!rule.accepts('A'));
        }
        other => panic!("expected wide input string, got {other:?}"),
    }
    assert!(
        matches!(
            scene.find(ObjectID::new(5)).map(|node| &node.kind),
            Some(NodeKind::InputString {
                validation: None,
                ..
            })
        ),
        "Extended Input Attributes must not validate an 8-bit String Variable"
    );
}

#[test]
fn render_runtime_change_string_value_updates_input_attributes_validation_string() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    input_attributes: ObjectID::new(4),
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
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    let before = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert!(matches!(
        &before.kind,
        NodeKind::InputString {
            validation: Some(validation),
            ..
        } if validation.chars == b"A"
    ));

    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeStringValue {
            id: ObjectID::new(4),
            text: "B".to_owned(),
        })
        .unwrap();

    assert!(matches!(update, RenderUpdate::SceneRebuilt { .. }));
    let after = runtime.scene().find(ObjectID::new(3)).unwrap();
    assert!(matches!(
        &after.kind,
        NodeKind::InputString {
            validation: Some(validation),
            ..
        } if validation.chars == b"B"
    ));

    let mut input = InputRuntime::new();
    input.bind(runtime.scene());
    let _focus = input.handle(runtime.scene(), &OperatorEvent::FocusNext);
    assert!(matches!(
        input.handle(runtime.scene(), &OperatorEvent::Char('A'))[0],
        VtEvent::Ignored { .. }
    ));
    assert!(matches!(
        &input.handle(runtime.scene(), &OperatorEvent::Char('B'))[0],
        VtEvent::StringEditPreview { id, text } if *id == ObjectID::new(3) && text == "B"
    ));
}

#[test]
fn render_runtime_generic_attribute_does_not_mutate_input_attributes_validation_type() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    input_attributes: ObjectID::new(4),
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
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(matches!(
        runtime.scene().find(ObjectID::new(3)).map(|node| &node.kind),
        Some(NodeKind::InputString {
            validation: Some(validation),
            ..
        }) if validation.allow_listed
    ));

    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 1,
            value: 1,
        })
        .unwrap();

    assert_eq!(update, RenderUpdate::Unchanged);
    assert!(matches!(
        runtime.scene().find(ObjectID::new(3)).map(|node| &node.kind),
        Some(NodeKind::InputString {
            validation: Some(validation),
            ..
        }) if validation.allow_listed && validation.accepts('A') && !validation.accepts('B')
    ));
}

#[test]
fn render_runtime_generic_attribute_does_not_mutate_extended_input_attributes_validation_type() {
    let wide_e_acute = vec![0xFF, 0xFE, 0xE9, 0x00];
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    input_attributes: ObjectID::new(4),
                    variable_reference: ObjectID::new(5),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_extended_input_attributes(
                4,
                &ExtendedInputAttributesBody {
                    validation_type: 0,
                    code_planes: vec![ExtendedInputCodePlane {
                        plane: 0,
                        ranges: vec![WideCharRange {
                            first: 0x00E9,
                            last: 0x00E9,
                        }],
                    }],
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            5,
            &StringVariableBody {
                length: wide_e_acute.len() as u16,
                value: wide_e_acute,
            },
        ));

    let mut runtime = VtRenderRuntime::from_pool(pool, DocConfig::default()).unwrap();
    assert!(matches!(
        runtime.scene().find(ObjectID::new(3)).map(|node| &node.kind),
        Some(NodeKind::InputString {
            validation: Some(validation),
            ..
        }) if validation.allow_listed && validation.accepts('é') && !validation.accepts('A')
    ));

    let update = runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
            id: ObjectID::new(4),
            attribute_id: 1,
            value: 1,
        })
        .unwrap();

    assert_eq!(update, RenderUpdate::Unchanged);
    assert!(matches!(
        runtime.scene().find(ObjectID::new(3)).map(|node| &node.kind),
        Some(NodeKind::InputString {
            validation: Some(validation),
            ..
        }) if validation.allow_listed && validation.accepts('é') && !validation.accepts('A')
    ));
}

#[test]
fn runtime_input_string_honours_standard_max_length() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_string(
                3,
                &InputStringBody {
                    options: 0x01,
                    width: 100,
                    height: 20,
                    max_length: 2,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);

    let _focus = rt.handle(&scene, &OperatorEvent::FocusNext);
    let first = rt.handle(&scene, &OperatorEvent::Char('O'));
    let second = rt.handle(&scene, &OperatorEvent::Char('K'));
    let rejected = rt.handle(&scene, &OperatorEvent::Char('!'));
    let committed = rt.handle(&scene, &OperatorEvent::Commit);

    assert!(
        matches!(&first[0], VtEvent::StringEditPreview { id, text } if *id == ObjectID::new(3) && text == "O")
    );
    assert!(
        matches!(&second[0], VtEvent::StringEditPreview { id, text } if *id == ObjectID::new(3) && text == "OK")
    );
    assert!(matches!(
        rejected[0],
        VtEvent::Ignored {
            reason: "input string maximum length reached"
        }
    ));
    assert!(
        matches!(&committed[0], VtEvent::StringValueChanged { id, text } if *id == ObjectID::new(3) && text == "OK")
    );
}

#[test]
fn runtime_input_number_honours_standard_min_max_on_commit() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_number(
                3,
                &InputNumberBody {
                    options: 0x01,
                    options2: 0x01,
                    width: 100,
                    height: 20,
                    min_value: 0,
                    max_value: 45,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);

    let _focus = rt.handle(&scene, &OperatorEvent::FocusNext);
    let _ = rt.handle(&scene, &OperatorEvent::Char('7'));
    let _ = rt.handle(&scene, &OperatorEvent::Char('3'));
    let rejected = rt.handle(&scene, &OperatorEvent::Commit);

    assert!(matches!(
        rejected[0],
        VtEvent::Ignored {
            reason: "input number value outside range"
        }
    ));
    assert_eq!(rt.open_input(), Some(ObjectID::new(3)));
    assert!(matches!(rt.edit_state(), EditState::Number { .. }));
}

#[test]
fn runtime_real_time_input_number_emits_value_changes_immediately() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_number(
                3,
                &InputNumberBody {
                    options2: 0x03,
                    width: 100,
                    height: 20,
                    min_value: 0,
                    max_value: 45,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);

    let _focus = rt.handle(&scene, &OperatorEvent::FocusNext);
    let first = rt.handle(&scene, &OperatorEvent::Char('4'));
    assert!(matches!(
        first[0],
        VtEvent::NumberValueChanged {
            id,
            raw: 4
        } if id == ObjectID::new(3)
    ));

    let rejected = rt.handle(&scene, &OperatorEvent::Char('9'));
    assert!(matches!(
        rejected[0],
        VtEvent::Ignored {
            reason: "input number value outside range"
        }
    ));

    let close = rt.handle(&scene, &OperatorEvent::Commit);
    assert!(matches!(
        close[0],
        VtEvent::Ignored {
            reason: "real-time input value already sent"
        }
    ));
    assert_eq!(rt.open_input(), None);
}

#[test]
fn runtime_tap_on_input_list_emits_next_selection_event() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_number_variable(9, &NumberVariableBody { value: 1 }))
        .with_object(create_output_string(4, &OutputStringBody::default()).unwrap())
        .with_object(create_output_string(5, &OutputStringBody::default()).unwrap())
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(9),
                    value: 255,
                    options: 0x03,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut rt = InputRuntime::new();
    rt.bind(&scene);

    let event = rt.handle(&scene, &OperatorEvent::Tap(10, 10));
    assert!(matches!(
        event[0],
        VtEvent::ListSelectionChanged {
            id,
            index: 0
        } if id == ObjectID::new(3)
    ));
    assert_eq!(rt.selected_input(), Some(ObjectID::new(3)));
}

#[test]
fn render_input_list_resolves_selected_item_text() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(create_number_variable(9, &NumberVariableBody { value: 1 }))
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    value: b"LOW".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"HIGH".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::new(9),
                    value: 255,
                    options: 0x01,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    assert!(matches!(
        &scene.find(ObjectID::new(3)).unwrap().kind,
        NodeKind::InputList {
            selected: 1,
            item_count: 2,
            selected_text,
            ..
        } if selected_text.as_deref() == Some("HIGH")
    ));

    let commands = GtuiRenderer::default().render(&scene);
    assert!(
        commands.iter().any(
            |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "HIGH")
        )
    );
}

#[test]
fn render_input_list_uses_inline_value_when_variable_reference_is_null() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_output_string(
                4,
                &OutputStringBody {
                    value: b"LOW".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"HIGH".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_list(
                3,
                &InputListBody {
                    width: 80,
                    height: 20,
                    variable_reference: ObjectID::NULL,
                    value: 1,
                    options: 0x01,
                    items: vec![ObjectID::new(4), ObjectID::new(5)],
                },
            )
            .unwrap(),
        );

    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::InputList {
            selected: 1,
            selected_text,
            ..
        } if selected_text.as_deref() == Some("HIGH")
    ));

    runtime
        .apply_ecu_command(&VtRuntimeCommand::ChangeNumericValue {
            id: ObjectID::new(3),
            value: 0,
        })
        .unwrap();
    assert!(matches!(
        &runtime.scene().find(ObjectID::new(3)).unwrap().kind,
        NodeKind::InputList {
            selected: 0,
            selected_text,
            ..
        } if selected_text.as_deref() == Some("LOW")
    ));
}

#[test]
fn render_input_list_no_selection_or_invalid_index_displays_blank_and_restarts_selection() {
    for (raw_value, name) in [(255_u32, "no chosen item"), (7, "invalid index")] {
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
            .with_object(create_number_variable(
                9,
                &NumberVariableBody { value: raw_value },
            ))
            .with_object(
                create_output_string(
                    4,
                    &OutputStringBody {
                        value: b"LOW".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_string(
                    5,
                    &OutputStringBody {
                        value: b"HIGH".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_list(
                    3,
                    &InputListBody {
                        width: 80,
                        height: 20,
                        variable_reference: ObjectID::new(9),
                        value: 255,
                        options: 0x03,
                        items: vec![ObjectID::new(4), ObjectID::new(5)],
                    },
                )
                .unwrap(),
            );
        let scene = render(&pool, ObjectID::NULL);
        assert!(
            matches!(
                &scene.find(ObjectID::new(3)).unwrap().kind,
                NodeKind::InputList {
                    selected,
                    item_count: 2,
                    selected_text,
                    ..
                } if *selected == raw_value as usize && selected_text.is_none()
            ),
            "{name} should be retained as a blank Input List selection instead of clamping to a valid item"
        );

        let commands = GtuiRenderer::default().render(&scene);
        assert!(
            !commands.iter().any(
                |command| matches!(command, RenderCommand::DrawText { text, .. } if text == "LOW" || text == "HIGH")
            ),
            "{name} must not display a stale selected Input List item"
        );
        assert!(
            commands.iter().any(
                |command| matches!(command, RenderCommand::DrawText { text, .. } if text.is_empty())
            ),
            "{name} should render the standard blank selected-item field"
        );

        let mut input = InputRuntime::new();
        input.bind(&scene);
        let event = input.handle(&scene, &OperatorEvent::Tap(10, 10));
        assert!(
            matches!(
                event[0],
                VtEvent::ListSelectionChanged {
                    id,
                    index: 0
                } if id == ObjectID::new(3)
            ),
            "operator navigation from {name} must restart at the first selectable item, not select an invalid/255 index"
        );
    }
}

#[test]
fn render_input_list_null_item_is_blank_and_skipped_by_operator_selection() {
    for (raw_value, expected_text, expected_next) in
        [(0_u32, Some("LOW"), 2_usize), (1_u32, Some(""), 0_usize)]
    {
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
            .with_object(create_number_variable(
                9,
                &NumberVariableBody { value: raw_value },
            ))
            .with_object(
                create_output_string(
                    4,
                    &OutputStringBody {
                        value: b"LOW".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_string(
                    5,
                    &OutputStringBody {
                        value: b"HIGH".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_list(
                    3,
                    &InputListBody {
                        width: 80,
                        height: 20,
                        variable_reference: ObjectID::new(9),
                        value: 255,
                        options: 0x03,
                        items: vec![ObjectID::new(4), ObjectID::NULL, ObjectID::new(5)],
                    },
                )
                .unwrap(),
            );
        let scene = render(&pool, ObjectID::NULL);
        let NodeKind::InputList {
            selected,
            item_count,
            selectable_indices,
            selected_text,
            ..
        } = &scene.find(ObjectID::new(3)).unwrap().kind
        else {
            panic!("expected Input List scene node");
        };
        assert_eq!(*selected, raw_value as usize);
        assert_eq!(*item_count, 3);
        assert_eq!(selectable_indices.as_slice(), [0, 2]);
        assert_eq!(selected_text.as_deref(), expected_text);

        let mut input = InputRuntime::new();
        input.bind(&scene);
        let event = input.handle(&scene, &OperatorEvent::Tap(10, 10));
        assert!(
            matches!(
                event[0],
                VtEvent::ListSelectionChanged {
                    id,
                    index
                } if id == ObjectID::new(3) && index == expected_next
            ),
            "operator selection must skip NULL Input List item slots"
        );
    }
}
