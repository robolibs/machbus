#[test]
fn runtime_input_number_rejects_non_decimal_keys_without_mutating_edit_state() {
    let pool = ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(create_data_mask(2, &DataMaskBody::default()).with_children([3u16]))
        .with_object(
            create_input_number(
                3,
                &InputNumberBody {
                    options2: 0x01,
                    width: 100,
                    height: 20,
                    min_value: 0,
                    max_value: 999,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut runtime = InputRuntime::new();
    runtime.bind(&scene);

    let _focus = runtime.handle(&scene, &OperatorEvent::FocusNext);
    let rejected_first = runtime.handle(&scene, &OperatorEvent::HardwareKey(b'A'));

    assert!(matches!(
        rejected_first[0],
        VtEvent::Ignored {
            reason: "input number accepts only decimal digits"
        }
    ));
    assert_eq!(runtime.open_input(), None);
    assert!(matches!(runtime.edit_state(), EditState::Idle));

    let first_digit = runtime.handle(&scene, &OperatorEvent::HardwareKey(b'1'));
    let rejected_dash = runtime.handle(&scene, &OperatorEvent::Char('-'));
    let second_digit = runtime.handle(&scene, &OperatorEvent::HardwareKey(b'2'));
    let committed = runtime.handle(&scene, &OperatorEvent::Commit);

    assert!(matches!(
        first_digit[0],
        VtEvent::NumberEditPreview {
            id,
            raw: 1
        } if id == ObjectID::new(3)
    ));
    assert!(matches!(
        rejected_dash[0],
        VtEvent::Ignored {
            reason: "input number accepts only decimal digits"
        }
    ));
    assert!(matches!(
        second_digit[0],
        VtEvent::NumberEditPreview {
            id,
            raw: 12
        } if id == ObjectID::new(3)
    ));
    assert!(matches!(
        committed[0],
        VtEvent::NumberValueChanged {
            id,
            raw: 12
        } if id == ObjectID::new(3)
    ));
}

#[test]
fn runtime_real_time_input_number_rejects_non_decimal_keys_without_value_event() {
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
                    max_value: 99,
                    ..Default::default()
                },
            )
            .unwrap(),
        );
    let scene = render(&pool, ObjectID::NULL);
    let mut runtime = InputRuntime::new();
    runtime.bind(&scene);

    let _focus = runtime.handle(&scene, &OperatorEvent::FocusNext);
    let digit = runtime.handle(&scene, &OperatorEvent::HardwareKey(b'7'));
    let rejected = runtime.handle(&scene, &OperatorEvent::HardwareKey(b'.'));
    let next_digit = runtime.handle(&scene, &OperatorEvent::HardwareKey(b'8'));

    assert!(matches!(
        digit[0],
        VtEvent::NumberValueChanged {
            id,
            raw: 7
        } if id == ObjectID::new(3)
    ));
    assert!(matches!(
        rejected[0],
        VtEvent::Ignored {
            reason: "input number accepts only decimal digits"
        }
    ));
    assert!(matches!(
        next_digit[0],
        VtEvent::NumberValueChanged {
            id,
            raw: 78
        } if id == ObjectID::new(3)
    ));
}
