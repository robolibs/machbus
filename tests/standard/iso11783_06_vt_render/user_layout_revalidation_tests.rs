#[test]
fn runtime_removes_changed_window_placement_before_moving_other_windows() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16, 31u16]))
        .with_object(
            create_window_mask(
                30,
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
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(30), 0, 0),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(31), 1, 0),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(30),
                attribute_id: 1,
                value: 2,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(
        runtime.user_layout_placements(),
        vec![UserLayoutPlacement::WindowMask {
            id: ObjectID::new(31),
            column: 1,
            row: 0,
        }],
        "a Window Mask that grows into another mapped cell must be removed without moving the unchanged Window Mask"
    );
}

#[test]
fn runtime_user_layout_snapshot_is_deterministic_by_object_id() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16, 40u16]))
        .with_object(
            create_window_mask(
                30,
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

    assert_eq!(
        runtime.place_key_group_in_user_layout(ObjectID::new(40), 4),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(30), 0, 0),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );

    assert_eq!(
        runtime.user_layout_placements(),
        vec![
            UserLayoutPlacement::WindowMask {
                id: ObjectID::new(30),
                column: 0,
                row: 0,
            },
            UserLayoutPlacement::KeyGroup {
                id: ObjectID::new(40),
                first_cell: 4,
            },
        ],
        "host-persisted placement snapshots must not inherit HashMap iteration order"
    );
}

#[test]
fn runtime_user_layout_hide_show_records_are_deterministic_by_object_id() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([40u16, 30u16]))
        .with_object(
            create_window_mask(
                30,
                &WindowMaskBody {
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
    let runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.user_layout_hide_show_events(Some(0x0A)),
        vec![VtEvent::UserLayoutHideShow {
            first: (ObjectID::new(30), true),
            second: Some((ObjectID::new(40), true)),
            transfer_sequence_number: Some(0x0A),
        }],
        "hide/show messages must not inherit scene paint order or HashMap order"
    );
}

#[test]
fn runtime_remembers_changed_window_while_mask_lock_defers_revalidation() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([30u16, 31u16]))
        .with_object(
            create_window_mask(
                30,
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
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(30), 0, 0),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(31), 1, 0),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
                id: ObjectID::new(2),
                locked: true,
                timeout_ms: 0,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting {
            reason: "mask lock freezes visible data-mask refreshes",
        }
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(30),
                attribute_id: 1,
                value: 2,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting {
            reason: "active mask is locked; visible refresh deferred until unlock",
        }
    );
    assert_eq!(
        runtime.user_layout_placements(),
        vec![
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
        ],
        "deferred refresh must not mutate the exposed operator mapping while the mask is locked"
    );

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
                id: ObjectID::new(2),
                locked: false,
                timeout_ms: 0,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(
        runtime.user_layout_placements(),
        vec![UserLayoutPlacement::WindowMask {
            id: ObjectID::new(31),
            column: 1,
            row: 0,
        }],
        "unlock must apply the deferred changed-object priority before dropping any stable mapping"
    );
}

#[test]
fn runtime_revalidates_multiple_changed_user_layout_windows_by_object_id() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([40u16, 30u16]))
        .with_object(
            create_window_mask(
                40,
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
            create_window_mask(
                30,
                &WindowMaskBody {
                    width_cells: 1,
                    height_cells: 1,
                    options: 0x01,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let mut runtime = VtRenderRuntime::from_pool(pool, LayoutConfig::default()).unwrap();

    assert_eq!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(40), 0, 0),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime.place_window_mask_in_user_layout(ObjectID::new(30), 0, 1),
        Ok(RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        })
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
                id: ObjectID::new(2),
                locked: true,
                timeout_ms: 0,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting {
            reason: "mask lock freezes visible data-mask refreshes",
        }
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(40),
                attribute_id: 2,
                value: 2,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting {
            reason: "active mask is locked; visible refresh deferred until unlock",
        }
    );
    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::ChangeGenericAttribute {
                id: ObjectID::new(30),
                attribute_id: 2,
                value: 2,
            })
            .unwrap(),
        RenderUpdate::NotRenderAffecting {
            reason: "active mask is locked; visible refresh deferred until unlock",
        }
    );

    assert_eq!(
        runtime
            .apply_ecu_command(&VtRuntimeCommand::LockUnlockMask {
                id: ObjectID::new(2),
                locked: false,
                timeout_ms: 0,
            })
            .unwrap(),
        RenderUpdate::SceneRebuilt {
            active_mask: ObjectID::new(2)
        }
    );
    assert_eq!(
        runtime.user_layout_placements(),
        vec![UserLayoutPlacement::WindowMask {
            id: ObjectID::new(30),
            column: 0,
            row: 1,
        }],
        "when multiple deferred changes conflict, user-layout revalidation must pick a deterministic ObjectID order instead of HashMap or command order"
    );
}
