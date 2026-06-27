#[cfg(test)]
mod tests {
    use super::*;
    use crate::isobus::vt::{
        AlarmMaskBody, AnimationBody, ArchedBarGraphBody, ColourMapBody, ContainerBody,
        DataMaskBody, ExternalObjectPointerBody, ExternalReferenceNameBody, FillAttributesBody,
        FontAttributesBody, InputAttributesBody, InputBooleanBody, InputListBody, InputNumberBody,
        InputStringBody, KeyBody, KeyGroupBody, LineAttributesBody, LinearBarGraphBody, MacroBody,
        MacroCommand, MeterBody, NumberVariableBody, ObjectLabelRefBody, ObjectLabelRefEntry,
        ObjectPointerBody, OutputLineBody, OutputListBody, OutputNumberBody, OutputPolygonBody,
        OutputRectangleBody, OutputStringBody, PictureGraphicBody, PolygonPoint, ScaledGraphicBody,
        SoftKeyMaskBody, StringVariableBody, WindowMaskBody, WorkingSetBody, create_alarm_mask,
        create_animation, create_arched_bar_graph, create_colour_map, create_container,
        create_data_mask, create_external_object_pointer, create_external_reference_name,
        create_fill_attributes, create_font_attributes, create_input_attributes,
        create_input_boolean, create_input_list, create_input_number, create_input_string,
        create_key, create_key_group, create_line_attributes, create_linear_bar_graph,
        create_macro, create_meter, create_number_variable, create_object_label_ref,
        create_object_pointer, create_output_line, create_output_list, create_output_number,
        create_output_polygon, create_output_rectangle, create_output_string,
        create_picture_graphic, create_scaled_graphic, create_soft_key_mask,
        create_string_variable, create_window_mask, create_working_set,
    };

    fn pool_with_macros() -> ObjectPool {
        // A Key (id 5) that fires macro 9 on event 0x05, and macro 10 on
        // event 0x05 as well; plus a Data Mask (id 2) firing macro 9 on
        // event 0x01. Macro objects 9 and 10 exist in the pool.
        let mut key = create_key(5, &KeyBody::default());
        key.add_macro(0x05, 9);
        key.add_macro(0x05, 10);
        let mut mask = create_data_mask(2, &DataMaskBody::default());
        mask.add_macro(0x01, 9);

        ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(mask)
            .with_object(key)
            .with_object(create_macro(9, &MacroBody::default()))
            .with_object(create_macro(10, &MacroBody::default()))
    }

    #[test]
    fn index_maps_object_event_pairs_to_macros() {
        let index = MacroTriggerIndex::build(&pool_with_macros());
        assert!(!index.is_empty());
        // Two distinct (object, event) bindings: Key@0x05 and Mask@0x01.
        assert_eq!(index.binding_count(), 2);

        let key_macros = index.macros_for(ObjectID::new(5), 0x05);
        assert_eq!(key_macros, &[ObjectID::new(9), ObjectID::new(10)]);

        let mask_macros = index.macros_for(ObjectID::new(2), 0x01);
        assert_eq!(mask_macros, &[ObjectID::new(9)]);
    }

    #[test]
    fn unbound_object_or_event_returns_empty() {
        let index = MacroTriggerIndex::build(&pool_with_macros());
        // Right object, wrong event.
        assert!(index.macros_for(ObjectID::new(5), 0x06).is_empty());
        // Object with no macros.
        assert!(index.macros_for(ObjectID::new(1), 0x05).is_empty());
    }

    #[test]
    fn decode_macro_effects_models_known_commands_in_order() {
        let body = MacroBody {
            commands: vec![
                MacroCommand {
                    command_type: cmd::HIDE_SHOW,
                    parameters: vec![0x34, 0x12, 1, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::ENABLE_DISABLE,
                    parameters: vec![0x05, 0x00, 0, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::SELECT_INPUT_OBJECT_COMMAND,
                    parameters: vec![0x09, 0x00, 1, 0xFF, 0xFF, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CONTROL_AUDIO_SIGNAL,
                    parameters: vec![2, 0x34, 0x12, 0x78, 0x56, 0xBC, 0x9A],
                },
                MacroCommand {
                    command_type: cmd::SET_AUDIO_VOLUME,
                    parameters: vec![80, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_NUMERIC_VALUE,
                    parameters: vec![0x05, 0x00, 0xFF, 0x0D, 0xF0, 0xFE, 0xCA],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_ATTRIBUTE,
                    parameters: vec![0x06, 0x00, 4, 0x05, 0x00, 0x00, 0x00],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_FONT_ATTRIBUTES,
                    parameters: vec![0x16, 0x00, 3, 4, 1, 0x12, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_LINE_ATTRIBUTES,
                    parameters: vec![0x17, 0x00, 5, 6, 0xAA, 0x55, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_FILL_ATTRIBUTES,
                    parameters: vec![0x18, 0x00, 2, 7, 0xFF, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_END_POINT,
                    parameters: vec![0x07, 0x00, 30, 0, 40, 0, 1],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_SOFT_KEY_MASK,
                    parameters: vec![1, 0x08, 0x00, 0x09, 0x00, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_LIST_ITEM,
                    parameters: vec![0x0A, 0x00, 1, 0x0B, 0x00, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::DELETE_OBJECT_POOL,
                    parameters: vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_PRIORITY,
                    parameters: vec![0x0C, 0x00, 2, 0xFF, 0xFF, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::CHANGE_OBJECT_LABEL,
                    parameters: vec![0x0D, 0x00, 0x0E, 0x00, 3, 0x0F, 0x00],
                },
                MacroCommand {
                    command_type: cmd::LOCK_UNLOCK_MASK,
                    parameters: vec![1, 0x02, 0x00, 100, 0, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: cmd::EXECUTE_MACRO,
                    parameters: vec![0x21, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
                },
                MacroCommand {
                    command_type: 0xAD, // Too short for Change Active Mask.
                    parameters: vec![0, 0, 0],
                },
            ],
        };
        assert_eq!(
            decode_macro_effects(&body),
            vec![
                MacroEffect::HideShow {
                    object: ObjectID::new(0x1234),
                    show: true,
                },
                MacroEffect::EnableDisable {
                    object: ObjectID::new(5),
                    enable: false,
                },
                MacroEffect::SelectInputObject {
                    object: ObjectID::new(9),
                    option: 1,
                },
                MacroEffect::ControlAudioSignal {
                    audio: AudioSignalState {
                        activations: 2,
                        frequency_hz: 0x1234,
                        duration_ms: 0x5678,
                        off_time_ms: 0x9ABC,
                    },
                },
                MacroEffect::SetAudioVolume { percent: 80 },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(5),
                    value: 0xCAFE_F00D,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(6),
                    attribute_id: 4,
                    value: 5,
                },
                MacroEffect::ChangeFontAttributes {
                    object: ObjectID::new(0x16),
                    colour: 3,
                    size: 4,
                    font_type: 1,
                    style: 0x12,
                },
                MacroEffect::ChangeLineAttributes {
                    object: ObjectID::new(0x17),
                    colour: 5,
                    width: 6,
                    line_art: 0x55AA,
                },
                MacroEffect::ChangeFillAttributes {
                    object: ObjectID::new(0x18),
                    fill_type: 2,
                    colour: 7,
                    pattern: ObjectID::NULL,
                },
                MacroEffect::ChangeEndPoint {
                    object: ObjectID::new(7),
                    width: 30,
                    height: 40,
                    line_direction: 1,
                },
                MacroEffect::ChangeSoftKeyMask {
                    mask_type: 1,
                    data_mask: ObjectID::new(8),
                    soft_key_mask: ObjectID::new(9),
                },
                MacroEffect::ChangeListItem {
                    list: ObjectID::new(10),
                    index: 1,
                    item: ObjectID::new(11),
                },
                MacroEffect::DeleteObjectPool,
                MacroEffect::ChangePriority {
                    object: ObjectID::new(12),
                    priority: 2,
                },
                MacroEffect::ChangeObjectLabel {
                    object: ObjectID::new(13),
                    label: ObjectLabelState {
                        string_variable: ObjectID::new(14),
                        font_type: 3,
                        graphic_designator: ObjectID::new(15),
                    },
                },
                MacroEffect::LockUnlockMask {
                    object: ObjectID::new(2),
                    locked: true,
                    timeout_ms: 100,
                },
                MacroEffect::ExecuteMacro {
                    object: ObjectID::new(33),
                },
                MacroEffect::Unsupported { command_type: 0xAD },
            ]
        );
    }

    #[test]
    fn decode_macro_effects_covers_all_fixed_length_macro_commands_admitted_by_walker() {
        let commands = vec![
            MacroCommand {
                command_type: cmd::HIDE_SHOW,
                parameters: vec![1, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::ENABLE_DISABLE,
                parameters: vec![1, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::SELECT_INPUT_OBJECT_COMMAND,
                parameters: vec![3, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CONTROL_AUDIO_SIGNAL,
                parameters: vec![1, 0xB8, 0x01, 0xFA, 0x00, 0x32, 0x00],
            },
            MacroCommand {
                command_type: cmd::SET_AUDIO_VOLUME,
                parameters: vec![80, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_CHILD_LOCATION,
                parameters: vec![1, 0, 2, 0, 3, 4, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_SIZE,
                parameters: vec![1, 0, 10, 0, 20, 0, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_BACKGROUND_COLOUR,
                parameters: vec![1, 0, 7, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_NUMERIC_VALUE,
                parameters: vec![1, 0, 0xFF, 5, 0, 0, 0],
            },
            MacroCommand {
                command_type: cmd::CHANGE_END_POINT,
                parameters: vec![1, 0, 10, 0, 20, 0, 1],
            },
            MacroCommand {
                command_type: cmd::CHANGE_ACTIVE_MASK,
                parameters: vec![1, 0, 2, 0, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_SOFT_KEY_MASK,
                parameters: vec![1, 2, 0, 3, 0, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_ATTRIBUTE,
                parameters: vec![1, 0, 1, 5, 0, 0, 0],
            },
            MacroCommand {
                command_type: cmd::CHANGE_FONT_ATTRIBUTES,
                parameters: vec![1, 0, 5, 6, 1, 0, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_LINE_ATTRIBUTES,
                parameters: vec![1, 0, 5, 3, 0xC0, 0, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_FILL_ATTRIBUTES,
                parameters: vec![1, 0, 2, 7, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_PRIORITY,
                parameters: vec![1, 0, 2, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_LIST_ITEM,
                parameters: vec![1, 0, 0, 2, 0, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::DELETE_OBJECT_POOL,
                parameters: vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_CHILD_POSITION,
                parameters: vec![1, 0, 2, 0, 3, 0, 4, 0],
            },
            MacroCommand {
                command_type: cmd::CHANGE_OBJECT_LABEL,
                parameters: vec![1, 0, 2, 0, 3, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::CHANGE_POLYGON_POINT,
                parameters: vec![1, 0, 0, 5, 0, 6, 0],
            },
            MacroCommand {
                command_type: cmd::CHANGE_POLYGON_SCALE,
                parameters: vec![1, 0, 10, 0, 20, 0, 0xFF],
            },
            MacroCommand {
                command_type: cmd::SELECT_COLOUR_MAP,
                parameters: vec![1, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::LOCK_UNLOCK_MASK,
                parameters: vec![1, 1, 0, 100, 0, 0xFF, 0xFF],
            },
            MacroCommand {
                command_type: cmd::EXECUTE_MACRO,
                parameters: vec![1, 0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF],
            },
        ];
        for command in &commands {
            assert!(
                MacroCommand::get_command_length(command.command_type) > 0,
                "command 0x{:02X} must be admitted by MacroBody::decode",
                command.command_type
            );
        }

        let effects = decode_macro_effects(&MacroBody { commands });

        assert!(
            effects
                .iter()
                .all(|effect| !matches!(effect, MacroEffect::Unsupported { .. })),
            "every fixed-length command admitted by MacroBody::decode should have an explicit macro effect: {effects:?}"
        );
    }

    #[test]
    fn apply_macro_effects_writes_variables_and_reports_runtime_changes() {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_output_number(7, &OutputNumberBody::default()).unwrap())
            .with_object(create_number_variable(9, &NumberVariableBody { value: 0 }))
            .with_object(create_string_variable(
                11,
                &StringVariableBody {
                    length: 4,
                    value: b"Name".to_vec(),
                },
            ))
            .with_object(create_input_string(8, &InputStringBody::default()).unwrap())
            .with_object(create_output_string(10, &OutputStringBody::default()).unwrap())
            .with_object(create_macro(12, &MacroBody::default()))
            .with_object(create_object_label_ref(
                13,
                &ObjectLabelRefBody {
                    labels: vec![ObjectLabelRefEntry {
                        labelled_object: ObjectID::new(10),
                        string_variable: ObjectID::new(11),
                        font_type: 2,
                        graphic_designator: ObjectID::NULL,
                    }],
                },
            ))
            .with_object(
                create_output_number(
                    4,
                    &OutputNumberBody {
                        variable_reference: 9.into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(create_container(5, &ContainerBody::default()))
            .with_object(create_input_boolean(6, &InputBooleanBody::default()).unwrap());

        let effects = vec![
            MacroEffect::ChangeNumericValue {
                object: ObjectID::new(9),
                value: 111,
            },
            // Targets the Output Number, which routes to variable 9.
            MacroEffect::ChangeNumericValue {
                object: ObjectID::new(4),
                value: 222,
            },
            MacroEffect::HideShow {
                object: ObjectID::new(5),
                show: true,
            },
            MacroEffect::EnableDisable {
                object: ObjectID::new(6),
                enable: false,
            },
            MacroEffect::SelectInputObject {
                object: ObjectID::new(8),
                option: 0xFF,
            },
            MacroEffect::ControlAudioSignal {
                audio: AudioSignalState {
                    activations: 1,
                    frequency_hz: 440,
                    duration_ms: 250,
                    off_time_ms: 50,
                },
            },
            MacroEffect::SetAudioVolume { percent: 60 },
            MacroEffect::ChangeGenericAttribute {
                object: ObjectID::new(7),
                attribute_id: 5,
                value: 5,
            },
            MacroEffect::ChangeObjectLabel {
                object: ObjectID::new(10),
                label: ObjectLabelState {
                    string_variable: ObjectID::new(11),
                    font_type: 2,
                    graphic_designator: ObjectID::NULL,
                },
            },
            MacroEffect::ExecuteMacro {
                object: ObjectID::new(12),
            },
            MacroEffect::ExecuteMacro {
                object: ObjectID::new(10),
            },
            MacroEffect::ExecuteMacro {
                object: ObjectID::new(77),
            },
            MacroEffect::Unsupported { command_type: 0xAD },
        ];

        let report = apply_macro_effects(&mut pool, &effects);
        assert_eq!(report.numeric_applied, 2);
        assert_eq!(report.visibility_changes, vec![(ObjectID::new(5), true)]);
        assert_eq!(report.enable_changes, vec![(ObjectID::new(6), false)]);
        assert_eq!(report.selected_input_change, Some(ObjectID::new(8)));
        assert_eq!(
            report.audio_signal,
            Some(AudioSignalState {
                activations: 1,
                frequency_hz: 440,
                duration_ms: 250,
                off_time_ms: 50,
            })
        );
        assert_eq!(report.audio_volume_percent, Some(60));
        assert_eq!(
            report.generic_attribute_changes,
            vec![(ObjectID::new(7), 5, 5)]
        );
        assert_eq!(
            report.object_label_changes,
            vec![(
                ObjectID::new(10),
                ObjectLabelState {
                    string_variable: ObjectID::new(11),
                    font_type: 2,
                    graphic_designator: ObjectID::NULL,
                },
            )]
        );
        assert_eq!(report.macro_executions, vec![ObjectID::new(12)]);
        assert_eq!(
            report.skipped, 3,
            "macro Execute Macro reports must reject wrong-type and unknown targets before hosted runtime replay"
        );
        // Both numeric writes land in variable 9; last write wins.
        assert_eq!(
            pool.find(9)
                .unwrap()
                .get_number_variable_body()
                .unwrap()
                .value,
            222
        );
    }

    #[test]
    fn apply_macro_effects_rejects_reserved_object_label_font_type() {
        let mut pool = ObjectPool::default()
            .with_object(create_output_string(10, &OutputStringBody::default()).unwrap())
            .with_object(create_object_label_ref(
                13,
                &ObjectLabelRefBody {
                    labels: vec![ObjectLabelRefEntry {
                        labelled_object: ObjectID::new(10),
                        string_variable: ObjectID::NULL,
                        font_type: 0,
                        graphic_designator: ObjectID::NULL,
                    }],
                },
            ));
        let report = apply_macro_effects(
            &mut pool,
            &[MacroEffect::ChangeObjectLabel {
                object: ObjectID::new(10),
                label: ObjectLabelState {
                    string_variable: ObjectID::NULL,
                    font_type: 3,
                    graphic_designator: ObjectID::NULL,
                },
            }],
        );

        assert!(report.object_label_changes.is_empty());
        assert_eq!(
            report.skipped, 1,
            "macro helper must reject reserved Annex K font-type metadata even when the string reference is NULL"
        );
    }

    #[test]
    fn apply_macro_effects_rejects_undeclared_object_label_targets() {
        let mut pool = ObjectPool::default()
            .with_object(create_output_string(10, &OutputStringBody::default()).unwrap())
            .with_object(create_output_string(11, &OutputStringBody::default()).unwrap())
            .with_object(create_object_label_ref(
                13,
                &ObjectLabelRefBody {
                    labels: vec![ObjectLabelRefEntry {
                        labelled_object: ObjectID::new(10),
                        string_variable: ObjectID::NULL,
                        font_type: 0,
                        graphic_designator: ObjectID::NULL,
                    }],
                },
            ));
        let report = apply_macro_effects(
            &mut pool,
            &[MacroEffect::ChangeObjectLabel {
                object: ObjectID::new(11),
                label: ObjectLabelState {
                    string_variable: ObjectID::NULL,
                    font_type: 0,
                    graphic_designator: ObjectID::NULL,
                },
            }],
        );

        assert!(report.object_label_changes.is_empty());
        assert_eq!(
            report.skipped, 1,
            "macro helper must reject Change Object Label targets that are not declared by Object Label Reference List"
        );
    }

    #[test]
    fn apply_macro_effects_gates_visibility_and_mask_lock_targets() {
        let mut pool = ObjectPool::default()
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_container(5, &ContainerBody::default()))
            .with_object(create_output_string(10, &OutputStringBody::default()).unwrap());

        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::HideShow {
                    object: ObjectID::new(10),
                    show: false,
                },
                MacroEffect::HideShow {
                    object: ObjectID::new(5),
                    show: false,
                },
                MacroEffect::LockUnlockMask {
                    object: ObjectID::new(10),
                    locked: true,
                    timeout_ms: 100,
                },
                MacroEffect::LockUnlockMask {
                    object: ObjectID::new(2),
                    locked: true,
                    timeout_ms: 100,
                },
            ],
        );

        assert_eq!(
            report.visibility_changes,
            vec![(ObjectID::new(5), false)],
            "macro Hide/Show effects must remain Container-only like server and direct runtime replay"
        );
        assert_eq!(
            report.mask_lock_changes,
            vec![(ObjectID::new(2), true, 100)],
            "macro Lock/Unlock Mask effects must target Data Mask or Window Mask objects"
        );
        assert_eq!(report.skipped, 2);
    }

    #[test]
    fn apply_macro_effects_gates_select_input_targets() {
        let mut pool = ObjectPool::default()
            .with_object(create_input_string(8, &InputStringBody::default()).unwrap())
            .with_object(create_key(9, &KeyBody::default()))
            .with_object(create_output_string(10, &OutputStringBody::default()).unwrap());

        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::SelectInputObject {
                    object: ObjectID::new(77),
                    option: 0xFF,
                },
                MacroEffect::SelectInputObject {
                    object: ObjectID::new(10),
                    option: 0xFF,
                },
                MacroEffect::SelectInputObject {
                    object: ObjectID::new(9),
                    option: 0x00,
                },
                MacroEffect::SelectInputObject {
                    object: ObjectID::new(8),
                    option: 0x00,
                },
            ],
        );

        assert_eq!(report.selected_input_change, Some(ObjectID::new(8)));
        assert_eq!(
            report.skipped, 3,
            "macro Select Input Object reports must reject missing targets, non-selectable display objects, and open requests for Key/Button focus targets"
        );
    }

    #[test]
    fn apply_macro_effects_allows_null_soft_key_mask_attribute() {
        let mut pool = ObjectPool::default()
            .with_object(create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID::new(3),
                    ..Default::default()
                },
            ))
            .with_object(create_soft_key_mask(3, &SoftKeyMaskBody::default()));

        let report = apply_macro_effects(
            &mut pool,
            &[MacroEffect::ChangeGenericAttribute {
                object: ObjectID::new(2),
                attribute_id: 2,
                value: u32::from(ObjectID::NULL.raw()),
            }],
        );

        assert_eq!(
            report.generic_attribute_changes,
            vec![(ObjectID::new(2), 2, u32::from(ObjectID::NULL.raw()))],
            "Data/Alarm Mask Soft Key Mask AID 2 must accept the standard NULL clear"
        );
        assert_eq!(report.skipped, 0);
    }

    #[test]
    fn apply_macro_effects_rejects_null_required_font_attribute_references() {
        let mut pool = ObjectPool::default()
            .with_object(create_font_attributes(4, &FontAttributesBody::default()))
            .with_object(
                create_output_string(
                    3,
                    &OutputStringBody {
                        font_attributes: ObjectID::new(4),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_boolean(
                    5,
                    &InputBooleanBody {
                        foreground: ObjectID::new(4),
                        ..Default::default()
                    },
                )
                .unwrap(),
            );

        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(3),
                    attribute_id: 4,
                    value: u32::from(ObjectID::NULL.raw()),
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(5),
                    attribute_id: 3,
                    value: u32::from(ObjectID::NULL.raw()),
                },
            ],
        );

        assert!(report.generic_attribute_changes.is_empty());
        assert_eq!(
            report.skipped, 2,
            "text/number field and Input Boolean foreground Font Attributes references are mandatory 0..=65534 fields, not nullable style selectors"
        );
        assert_eq!(
            pool.find(ObjectID::new(5))
                .unwrap()
                .get_input_boolean_body()
                .unwrap()
                .foreground,
            ObjectID::new(4)
        );
    }

    #[test]
    fn apply_macro_effects_rejects_null_required_polygon_line_attribute_reference() {
        let mut pool = ObjectPool::default()
            .with_object(create_line_attributes(4, &LineAttributesBody::default()))
            .with_object(
                create_output_polygon(
                    3,
                    &OutputPolygonBody {
                        line_attributes: ObjectID::new(4),
                        points: vec![
                            PolygonPoint { x: 0, y: 0 },
                            PolygonPoint { x: 20, y: 0 },
                            PolygonPoint { x: 0, y: 10 },
                        ],
                        ..Default::default()
                    },
                )
                .unwrap(),
            );

        let report = apply_macro_effects(
            &mut pool,
            &[MacroEffect::ChangeGenericAttribute {
                object: ObjectID::new(3),
                attribute_id: 3,
                value: u32::from(ObjectID::NULL.raw()),
            }],
        );

        assert!(report.generic_attribute_changes.is_empty());
        assert_eq!(
            report.skipped, 1,
            "Output Polygon Line Attributes AID 3 is a mandatory 0..=65534 reference, not a nullable style selector"
        );
        assert_eq!(
            pool.find(ObjectID::new(3))
                .unwrap()
                .get_output_polygon_body()
                .unwrap()
                .line_attributes,
            ObjectID::new(4)
        );
    }

    #[test]
    fn apply_macro_effects_skips_unsupported_generic_attribute_aids() {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_key(7, &KeyBody::default()))
            .with_object(create_font_attributes(20, &FontAttributesBody::default()))
            .with_object(
                create_output_string(
                    41,
                    &OutputStringBody {
                        font_attributes: 20.into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_boolean(
                    42,
                    &InputBooleanBody {
                        foreground: 20.into(),
                        enabled: 1,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(create_output_number(43, &OutputNumberBody::default()).unwrap())
            .with_object(
                create_input_number(
                    44,
                    &InputNumberBody {
                        min_value: 0,
                        max_value: 100,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_window_mask(
                    40,
                    &WindowMaskBody {
                        width_cells: 1,
                        height_cells: 1,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_alarm_mask(
                    45,
                    &AlarmMaskBody {
                        acoustic_signal: 3,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_key_group(
                    46,
                    &KeyGroupBody {
                        options: 1,
                        name: ObjectID::new(41),
                        key_group_icon: ObjectID::NULL,
                    },
                )
                .with_children([7u16]),
            )
            .with_object(
                create_input_list(
                    47,
                    &InputListBody {
                        value: 0,
                        items: vec![ObjectID::new(41)],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_list(
                    48,
                    &OutputListBody {
                        value: 0,
                        items: vec![ObjectID::new(41)],
                        ..Default::default()
                    },
                )
                .unwrap(),
            );

        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(7),
                    attribute_id: 1,
                    value: 4,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(7),
                    attribute_id: 3,
                    value: 1,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(77),
                    attribute_id: 1,
                    value: 1,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(40),
                    attribute_id: 1,
                    value: 2,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(40),
                    attribute_id: 3,
                    value: 1,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(40),
                    attribute_id: 5,
                    value: 0x04,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(40),
                    attribute_id: 6,
                    value: 20,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(40),
                    attribute_id: 7,
                    value: 20,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(40),
                    attribute_id: 8,
                    value: 20,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(41),
                    attribute_id: 4,
                    value: 20,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(41),
                    attribute_id: 4,
                    value: 2,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(41),
                    attribute_id: 5,
                    value: 0x04,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(42),
                    attribute_id: 5,
                    value: 2,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(42),
                    attribute_id: 6,
                    value: 2,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(43),
                    attribute_id: 9,
                    value: 8,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(43),
                    attribute_id: 10,
                    value: 1,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(43),
                    attribute_id: 10,
                    value: 2,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(44),
                    attribute_id: 11,
                    value: 8,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(44),
                    attribute_id: 12,
                    value: 2,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(45),
                    attribute_id: 4,
                    value: 4,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(46),
                    attribute_id: 2,
                    value: 43,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(46),
                    attribute_id: 3,
                    value: 20,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(47),
                    attribute_id: 4,
                    value: 1,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(47),
                    attribute_id: 4,
                    value: 0x0100,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(48),
                    attribute_id: 4,
                    value: 1,
                },
                MacroEffect::ChangeGenericAttribute {
                    object: ObjectID::new(48),
                    attribute_id: 4,
                    value: 0x0100,
                },
            ],
        );

        assert_eq!(
            report.generic_attribute_changes,
            vec![
                (ObjectID::new(7), 1, 4),
                (ObjectID::new(40), 1, 2),
                (ObjectID::new(41), 4, 20),
                (ObjectID::new(42), 5, 2),
                (ObjectID::new(42), 6, 2),
                (ObjectID::new(43), 10, 1),
                (ObjectID::new(47), 4, 1),
                (ObjectID::new(47), 4, 256),
                (ObjectID::new(48), 4, 1),
                (ObjectID::new(48), 4, 256),
            ]
        );
        assert_eq!(
            report.skipped, 16,
            "unsupported/read-only AIDs, missing targets, invalid Window Mask values/designators, invalid typed references, invalid Key Group designators, invalid scalar flags, reserved numeric formats/decimal counts, list-value Change Attribute attempts, and reserved Alarm Mask acoustic signals must not leak into generic replay"
        );
        let window_mask = pool
            .find(ObjectID::new(40))
            .unwrap()
            .get_window_mask_body()
            .unwrap();
        assert_eq!(window_mask.name, ObjectID::NULL);
        assert_eq!(window_mask.window_title, ObjectID::NULL);
        assert_eq!(window_mask.window_icon, ObjectID::NULL);
        assert_eq!(
            pool.find(ObjectID::new(45))
                .unwrap()
                .get_alarm_mask_body()
                .unwrap()
                .acoustic_signal,
            3,
            "reserved Alarm Mask acoustic signal values must not mutate macro replay state"
        );
        let key_group = pool
            .find(ObjectID::new(46))
            .unwrap()
            .get_key_group_body()
            .unwrap();
        assert_eq!(key_group.name, ObjectID::new(41));
        assert_eq!(key_group.key_group_icon, ObjectID::NULL);
    }

    #[test]
    fn apply_macro_effects_mutates_attribute_objects() {
        let mut pool = ObjectPool::default()
            .with_object(create_font_attributes(6, &FontAttributesBody::default()))
            .with_object(create_line_attributes(7, &LineAttributesBody::default()))
            .with_object(create_fill_attributes(8, &FillAttributesBody::default()).unwrap());

        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::ChangeFontAttributes {
                    object: ObjectID::new(6),
                    colour: 3,
                    size: 4,
                    font_type: 1,
                    style: 0x12,
                },
                MacroEffect::ChangeLineAttributes {
                    object: ObjectID::new(7),
                    colour: 5,
                    width: 6,
                    line_art: 0x55AA,
                },
                MacroEffect::ChangeFillAttributes {
                    object: ObjectID::new(8),
                    fill_type: 2,
                    colour: 7,
                    pattern: ObjectID::NULL,
                },
            ],
        );

        assert_eq!(report.pool_applied, 3);
        assert_eq!(
            pool.find(6).unwrap().get_font_attributes_body().unwrap(),
            FontAttributesBody {
                font_color: 3,
                font_size: 4,
                font_type: 1,
                font_style: 0x12,
            }
        );
        assert_eq!(
            pool.find(7).unwrap().get_line_attributes_body().unwrap(),
            LineAttributesBody {
                line_color: 5,
                line_width: 6,
                line_art: 0x55AA,
            }
        );
        assert_eq!(
            pool.find(8).unwrap().get_fill_attributes_body().unwrap(),
            FillAttributesBody {
                fill_type: 2,
                fill_color: 7,
                fill_pattern: ObjectID::NULL,
            }
        );

        let skipped = apply_macro_effects(
            &mut pool,
            &[MacroEffect::ChangeFillAttributes {
                object: ObjectID::new(8),
                fill_type: 0,
                colour: 9,
                pattern: ObjectID::new(6),
            }],
        );
        assert_eq!(skipped.pool_applied, 0);
        assert_eq!(
            skipped.skipped, 1,
            "macro Change Fill Attributes must reject non-PictureGraphic pattern references even when fill type is not pattern-fill"
        );
        assert_eq!(
            pool.find(8).unwrap().get_fill_attributes_body().unwrap(),
            FillAttributesBody {
                fill_type: 2,
                fill_color: 7,
                fill_pattern: ObjectID::NULL,
            }
        );
    }

    #[test]
    fn apply_macro_effects_can_report_delete_object_pool_lifecycle_effect() {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()));

        let report = apply_macro_effects(&mut pool, &[MacroEffect::DeleteObjectPool]);

        assert!(report.delete_object_pool);
        assert_eq!(report.pool_applied, 1);
        assert!(pool.is_empty());
    }

    #[test]
    fn decode_and_apply_change_string_value() {
        // Macro Change String Value: [id:u16][len:u16][value bytes].
        let body = MacroBody {
            commands: vec![MacroCommand {
                command_type: cmd::CHANGE_STRING_VALUE,
                parameters: vec![0x09, 0x00, 0x02, 0x00, b'h', b'i'],
            }],
        };
        assert_eq!(
            decode_macro_effects(&body),
            vec![MacroEffect::ChangeStringValue {
                object: ObjectID::new(9),
                value: b"hi".to_vec(),
            }]
        );

        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_string_variable(
                9,
                &StringVariableBody {
                    length: 2,
                    value: b"  ".to_vec(),
                },
            ))
            .with_object(
                create_output_string(
                    4,
                    &OutputStringBody {
                        variable_reference: 9.into(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_string(
                    10,
                    &OutputStringBody {
                        value: b"old".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_attributes(
                    11,
                    &InputAttributesBody {
                        validation_type: 0,
                        validation_string: b"ABC".to_vec(),
                    },
                )
                .unwrap(),
            );

        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::ChangeStringValue {
                    object: ObjectID::new(9),
                    value: b"hi".to_vec(),
                },
                // Output String with a variable reference is not a fixed-length
                // Change String Value target; the variable must be targeted
                // directly.
                MacroEffect::ChangeStringValue {
                    object: ObjectID::new(4),
                    value: b"yo".to_vec(),
                },
                MacroEffect::ChangeStringValue {
                    object: ObjectID::new(10),
                    value: b"go".to_vec(),
                },
                MacroEffect::ChangeStringValue {
                    object: ObjectID::new(11),
                    value: b"Z".to_vec(),
                },
            ],
        );
        assert_eq!(report.string_applied, 3);
        assert_eq!(report.skipped, 1);
        assert_eq!(
            pool.find(9)
                .unwrap()
                .get_string_variable_body()
                .unwrap()
                .value,
            b"hi"
        );
        assert_eq!(
            pool.find(10)
                .unwrap()
                .get_output_string_body()
                .unwrap()
                .value,
            b"go "
        );
        assert_eq!(
            pool.find(11)
                .unwrap()
                .get_input_attributes_body()
                .unwrap()
                .validation_string,
            b"Z  "
        );
    }

    #[test]
    fn apply_macro_effects_rejects_invalid_utf8_string_value() {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_string_variable(
                9,
                &StringVariableBody {
                    length: 2,
                    value: b"OK".to_vec(),
                },
            ));

        let report = apply_macro_effects(
            &mut pool,
            &[MacroEffect::ChangeStringValue {
                object: ObjectID::new(9),
                value: vec![0xFF],
            }],
        );

        assert_eq!(report.string_applied, 0);
        assert_eq!(report.skipped, 1);
        assert_eq!(
            pool.find(9)
                .unwrap()
                .get_string_variable_body()
                .unwrap()
                .value,
            b"OK"
        );
    }

    #[test]
    fn decode_and_apply_change_active_mask() {
        // F.34 Change Active Mask: [working_set:u16][mask:u16][FF×3].
        let body = MacroBody {
            commands: vec![MacroCommand {
                command_type: cmd::CHANGE_ACTIVE_MASK,
                parameters: vec![0x01, 0x00, 0x07, 0x00, 0xFF, 0xFF, 0xFF],
            }],
        };
        assert_eq!(
            decode_macro_effects(&body),
            vec![MacroEffect::ChangeActiveMask {
                working_set: ObjectID::new(1),
                mask: ObjectID::new(7),
            }]
        );

        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_data_mask(7, &DataMaskBody::default()))
            .with_object(
                create_colour_map(
                    8,
                    &ColourMapBody {
                        entries: vec![0, 1],
                    },
                )
                .unwrap(),
            )
            .with_object(create_output_string(9, &OutputStringBody::default()).unwrap());
        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::SelectColourMap {
                    object: ObjectID::new(8),
                },
                MacroEffect::SelectColourMap {
                    object: ObjectID::new(9),
                },
                MacroEffect::ChangeActiveMask {
                    working_set: ObjectID::new(1),
                    mask: ObjectID::new(7),
                },
                MacroEffect::ChangeActiveMask {
                    working_set: ObjectID::new(99),
                    mask: ObjectID::new(7),
                },
                MacroEffect::ChangeActiveMask {
                    working_set: ObjectID::new(1),
                    mask: ObjectID::new(9),
                },
            ],
        );
        assert_eq!(report.colour_selection_change, Some(ObjectID::new(8)));
        assert_eq!(report.active_mask_change, Some(ObjectID::new(7)));
        assert_eq!(
            report.skipped, 3,
            "macro colour selection and active-mask reports must reject wrong object families and missing working sets before hosted runtime replay"
        );
    }

    #[test]
    fn apply_macro_effects_writes_geometry_background_and_polygon_pool_state() {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(
                create_data_mask(2, &DataMaskBody::default()).with_children([3u16, 6u16, 8u16]),
            )
            .with_object(create_container(
                3,
                &ContainerBody {
                    width: 4,
                    height: 5,
                    ..Default::default()
                },
            ))
            .with_object(
                create_output_rectangle(
                    4,
                    &OutputRectangleBody {
                        width: 8,
                        height: 9,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_polygon(
                    5,
                    &OutputPolygonBody {
                        width: 10,
                        height: 20,
                        points: vec![
                            PolygonPoint { x: 0, y: 0 },
                            PolygonPoint { x: 5, y: 10 },
                            PolygonPoint { x: 10, y: 20 },
                        ],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_line(
                    6,
                    &OutputLineBody {
                        width: 2,
                        height: 3,
                        line_direction: 0,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_list(
                    8,
                    &OutputListBody {
                        width: 20,
                        height: 10,
                        value: 1,
                        items: vec![ObjectID::NULL, ObjectID::NULL],
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(create_soft_key_mask(9, &SoftKeyMaskBody::default()))
            .with_object(
                create_output_string(
                    10,
                    &OutputStringBody {
                        width: 20,
                        height: 10,
                        value: b"ok".to_vec(),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_alarm_mask(
                    11,
                    &AlarmMaskBody {
                        priority: 0,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(create_font_attributes(12, &FontAttributesBody::default()));
        pool = pool.with_object(
            create_animation(
                13,
                &AnimationBody {
                    width: 20,
                    height: 10,
                    value: 1,
                    last_child_index: 1,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_children_pos([
                ChildRef::at_origin(ObjectID::new(10)),
                ChildRef::at_origin(ObjectID::NULL),
            ]),
        );

        let report = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::ChangeChildLocation {
                    parent: ObjectID::new(2),
                    child: ObjectID::new(3),
                    x: 6,
                    y: 7,
                },
                MacroEffect::ChangeChildPosition {
                    parent: ObjectID::new(2),
                    child: ObjectID::new(3),
                    x: -8,
                    y: 9,
                },
                MacroEffect::ChangeSize {
                    object: ObjectID::new(4),
                    width: 30,
                    height: 40,
                },
                MacroEffect::ChangeBackgroundColour {
                    object: ObjectID::new(2),
                    colour: 7,
                },
                MacroEffect::ChangeEndPoint {
                    object: ObjectID::new(6),
                    width: 13,
                    height: 14,
                    line_direction: 1,
                },
                MacroEffect::ChangeSoftKeyMask {
                    mask_type: 1,
                    data_mask: ObjectID::new(2),
                    soft_key_mask: ObjectID::new(9),
                },
                MacroEffect::ChangeListItem {
                    list: ObjectID::new(8),
                    index: 1,
                    item: ObjectID::new(10),
                },
                MacroEffect::ChangeListItem {
                    list: ObjectID::new(8),
                    index: 0,
                    item: ObjectID::new(12),
                },
                MacroEffect::ChangeListItem {
                    list: ObjectID::new(13),
                    index: 1,
                    item: ObjectID::new(10),
                },
                MacroEffect::ChangePriority {
                    object: ObjectID::new(11),
                    priority: 2,
                },
                MacroEffect::LockUnlockMask {
                    object: ObjectID::new(2),
                    locked: true,
                    timeout_ms: 250,
                },
                MacroEffect::ChangePolygonScale {
                    object: ObjectID::new(5),
                    width: 30,
                    height: 40,
                },
                MacroEffect::ChangePolygonPoint {
                    object: ObjectID::new(5),
                    index: 1,
                    x: 11,
                    y: 12,
                },
            ],
        );

        assert_eq!(report.pool_applied, 11);
        assert_eq!(report.skipped, 1);
        assert_eq!(
            report.mask_lock_changes,
            vec![(ObjectID::new(2), true, 250)]
        );

        let data_mask = pool.find(2).unwrap();
        assert_eq!(
            data_mask.children_pos,
            vec![
                ChildRef {
                    id: ObjectID::new(3),
                    x: -8,
                    y: 9,
                },
                ChildRef {
                    id: ObjectID::new(6),
                    x: 0,
                    y: 0,
                },
                ChildRef {
                    id: ObjectID::new(8),
                    x: 0,
                    y: 0,
                },
            ]
        );
        assert_eq!(data_mask.get_data_mask_body().unwrap().background_color, 7);
        assert_eq!(
            data_mask.get_data_mask_body().unwrap().soft_key_mask,
            ObjectID::new(9)
        );

        let rect = pool.find(4).unwrap().get_output_rectangle_body().unwrap();
        assert_eq!((rect.width, rect.height), (30, 40));

        let line = pool.find(6).unwrap().get_output_line_body().unwrap();
        assert_eq!((line.width, line.height, line.line_direction), (13, 14, 1));

        let output_list = pool.find(8).unwrap().get_output_list_body().unwrap();
        assert_eq!(output_list.items[0], ObjectID::NULL);
        assert_eq!(output_list.items[1], ObjectID::new(10));
        assert_eq!(
            pool.find(13).unwrap().children_pos[1].id,
            ObjectID::new(10)
        );

        let alarm = pool.find(11).unwrap().get_alarm_mask_body().unwrap();
        assert_eq!(alarm.priority, 2);

        let polygon = pool.find(5).unwrap().get_output_polygon_body().unwrap();
        assert_eq!((polygon.width, polygon.height), (30, 40));
        assert_eq!(polygon.points[0], PolygonPoint { x: 0, y: 0 });
        assert_eq!(polygon.points[1], PolygonPoint { x: 11, y: 12 });
        assert_eq!(polygon.points[2], PolygonPoint { x: 30, y: 40 });
    }

    #[test]
    fn decode_change_string_value_rejects_truncated_payload() {
        // Declares 5 value bytes but only supplies 1.
        let body = MacroBody {
            commands: vec![MacroCommand {
                command_type: cmd::CHANGE_STRING_VALUE,
                parameters: vec![0x09, 0x00, 0x05, 0x00, b'x'],
            }],
        };
        assert_eq!(
            decode_macro_effects(&body),
            vec![MacroEffect::Unsupported {
                command_type: cmd::CHANGE_STRING_VALUE,
            }]
        );
    }

    #[test]
    fn apply_macro_effects_skips_numeric_writes_to_invalid_targets() {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()));
        // Target id 99 does not exist.
        let report = apply_macro_effects(
            &mut pool,
            &[MacroEffect::ChangeNumericValue {
                object: ObjectID::new(99),
                value: 5,
            }],
        );
        assert_eq!(report.numeric_applied, 0);
        assert_eq!(report.skipped, 1);
    }

    #[test]
    fn apply_macro_effects_uses_standard_numeric_value_targets_and_context() {
        let mut pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()))
            .with_object(create_output_string(5, &OutputStringBody::default()).unwrap())
            .with_object(create_number_variable(9, &NumberVariableBody { value: 0 }))
            .with_object(create_output_number(21, &OutputNumberBody::default()).unwrap())
            .with_object(
                create_input_number(
                    25,
                    &InputNumberBody {
                        max_value: i32::MAX,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_boolean(
                    39,
                    &InputBooleanBody {
                        value: 0,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_input_list(
                    4,
                    &InputListBody {
                        items: vec![ObjectID::new(5)],
                        value: 0,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_output_list(
                    20,
                    &OutputListBody {
                        items: vec![ObjectID::new(5)],
                        value: 0,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_picture_graphic(
                    23,
                    &PictureGraphicBody {
                        width: 1,
                        actual_width: 1,
                        actual_height: 1,
                        format: 2,
                        options: 0,
                        transparency: 0xFF,
                        data: vec![1],
                    },
                )
                .unwrap(),
            )
            .with_object(create_object_pointer(
                19,
                &ObjectPointerBody {
                    value: ObjectID::new(5),
                },
            ))
            .with_object(create_object_pointer(
                44,
                &ObjectPointerBody {
                    value: ObjectID::new(23),
                },
            ))
            .with_object(
                create_scaled_graphic(
                    38,
                    &ScaledGraphicBody {
                        value: ObjectID::new(23),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_scaled_graphic(
                    45,
                    &ScaledGraphicBody {
                        value: ObjectID::new(44),
                        ..Default::default()
                    },
                )
                .unwrap(),
            )
            .with_object(
                create_animation(32, &AnimationBody::default())
                    .unwrap()
                    .with_children([5u16, 23u16]),
            )
            .with_object(create_meter(29, &MeterBody::default()).unwrap())
            .with_object(create_linear_bar_graph(30, &LinearBarGraphBody::default()).unwrap())
            .with_object(create_arched_bar_graph(31, &ArchedBarGraphBody::default()).unwrap())
            .with_object(create_external_reference_name(
                36,
                &ExternalReferenceNameBody {
                    options: 1,
                    name0: 0x1122_3344,
                    name1: 0x5566_7788,
                },
            ))
            .with_object(create_external_object_pointer(
                37,
                &ExternalObjectPointerBody {
                    default_object_id: ObjectID::new(5),
                    external_reference_name: ObjectID::NULL,
                    external_object_id: ObjectID::NULL,
                },
            ));

        let valid = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(39),
                    value: 1,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(4),
                    value: 0,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(20),
                    value: 0,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(21),
                    value: 0x1122_3344,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(25),
                    value: 0x5566_7788,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(19),
                    value: 23,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(38),
                    value: 23,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(37),
                    value: (77_u32 << 16) | u32::from(36_u16),
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(32),
                    value: 1,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(29),
                    value: 100,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(30),
                    value: 101,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(31),
                    value: 102,
                },
            ],
        );
        assert_eq!(valid.numeric_applied, 12);
        assert_eq!(valid.skipped, 0);
        assert_eq!(
            pool.find(ObjectID::new(39))
                .unwrap()
                .get_input_boolean_body()
                .unwrap()
                .value,
            1
        );
        assert_eq!(
            pool.find(ObjectID::new(19))
                .unwrap()
                .get_object_pointer_body()
                .unwrap()
                .value,
            ObjectID::new(23)
        );
        assert_eq!(
            pool.find(ObjectID::new(37))
                .unwrap()
                .get_external_object_pointer_body()
                .unwrap()
                .external_object_id,
            ObjectID::new(77)
        );
        assert_eq!(
            pool.find(ObjectID::new(32))
                .unwrap()
                .get_animation_body()
                .unwrap()
                .value,
            1
        );

        let invalid = apply_macro_effects(
            &mut pool,
            &[
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(39),
                    value: 2,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(44),
                    value: 5,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(38),
                    value: 5,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(37),
                    value: (88_u32 << 16) | u32::from(5_u16),
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(32),
                    value: 2,
                },
                MacroEffect::ChangeNumericValue {
                    object: ObjectID::new(29),
                    value: 0x1_0000,
                },
            ],
        );
        assert_eq!(invalid.numeric_applied, 0);
        assert_eq!(invalid.skipped, 6);
        assert_eq!(
            pool.find(ObjectID::new(44))
                .unwrap()
                .get_object_pointer_body()
                .unwrap()
                .value,
            ObjectID::new(23),
            "ObjectPointer values reached by ScaledGraphic stay graphic sources"
        );
        assert_eq!(
            pool.find(ObjectID::new(38))
                .unwrap()
                .get_scaled_graphic_body()
                .unwrap()
                .value,
            ObjectID::new(23)
        );
    }

    #[test]
    fn decode_macro_effects_rejects_short_parameters() {
        let body = MacroBody {
            commands: vec![MacroCommand {
                command_type: cmd::HIDE_SHOW,
                parameters: vec![0x01], // too short for an object id + flag
            }],
        };
        assert_eq!(
            decode_macro_effects(&body),
            vec![MacroEffect::Unsupported {
                command_type: cmd::HIDE_SHOW,
            }]
        );
    }

    #[test]
    fn pool_without_macros_yields_empty_index() {
        let pool = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()));
        let index = MacroTriggerIndex::build(&pool);
        assert!(index.is_empty());
        assert_eq!(index.binding_count(), 0);
    }
}
