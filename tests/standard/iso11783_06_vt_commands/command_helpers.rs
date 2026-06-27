use machbus::isobus::vt::commands::VT_STRING_VALUE_MAX_LEN;
use machbus::isobus::vt::{
    ActivationCode, AlarmMaskBody, AlarmPriority, AnimationBody, ArchedBarGraphBody,
    AuxCapabilityDiscovery, AuxChannelCapability, AuxFunction2Body, AuxFunctionBody, AuxInput2Body,
    AuxInputBody, ButtonBody, ColourMapBody, ColourPaletteBody, ContainerBody, DataMaskBody,
    ExtendedInputAttributesBody, ExtendedInputCodePlane, ExternalObjectPointerBody,
    ExternalReferenceNameBody, FillAttributesBody, FontAttributesBody, GraphicContextBody,
    InputAttributesBody, InputBooleanBody, InputListBody, InputNumberBody, InputStringBody,
    KeyBody, KeyGroupBody, LanguageCode, LineAttributesBody, LinearBarGraphBody, MacroBody,
    MeterBody, NumberVariableBody, ObjectID, ObjectPointerBody, ObjectPool, ObjectType,
    OutputEllipseBody, OutputLineBody, OutputListBody, OutputNumberBody, OutputPolygonBody,
    OutputRectangleBody, OutputStringBody, PictureGraphicBody, PolygonPoint, ScaledGraphicBody,
    ScaledBitmapBody, ServerRenderEffect, ServerWorkingSet, SoftKeyMaskBody, StringVariableBody,
    VTClient, VTClientConfig, VTClientStateTracker, VTServer, VTServerConfig, VTState,
    WideCharRange, WindowMaskBody, WorkingSetBody, WorkingSetSpecialControlsBody, cmd,
    create_alarm_mask, create_animation, create_arched_bar_graph, create_aux_function,
    create_aux_function2, create_aux_input, create_aux_input2, create_button, create_colour_map,
    create_colour_palette, create_container, create_data_mask, create_extended_input_attributes,
    create_external_object_pointer, create_external_reference_name, create_fill_attributes,
    create_font_attributes, create_graphic_context, create_input_attributes, create_input_boolean,
    create_input_list, create_input_number, create_input_string, create_key, create_key_group,
    create_line_attributes, create_linear_bar_graph, create_macro, create_meter,
    create_number_variable, create_object_label_ref, create_object_pointer, create_output_ellipse,
    create_output_line, create_output_list, create_output_number, create_output_polygon,
    create_output_rectangle, create_output_string, create_picture_graphic, create_scaled_bitmap,
    create_scaled_graphic, create_soft_key_mask, create_string_variable, create_window_mask,
    create_working_set, create_working_set_special_controls,
};
use machbus::isobus::vt::{ObjectLabelRefBody, ObjectLabelRefEntry};
use machbus::isobus::{AuxFunctionState, AuxFunctionType, AuxNFunction, AuxOFunction};
use machbus::j1939::{
    AreaUnit, DateFormat, DecimalSymbol, DistanceUnit, ForceUnit, LanguageData, MassUnit,
    PressureUnit, TemperatureUnit, TimeFormat, UnitSystem, VolumeUnit,
};
use machbus::net::pgn_defs::{
    PGN_AUX_INPUT_STATUS, PGN_AUX_INPUT_TYPE2, PGN_ECU_TO_VT, PGN_LANGUAGE_COMMAND, PGN_REQUEST,
    PGN_VT_TO_ECU,
};
use machbus::net::{BROADCAST_ADDRESS, Message, NULL_ADDRESS, Priority};

use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

fn fixed_command(command: u8) -> Vec<u8> {
    let mut data = [0xFFu8; 8];
    data[0] = command;
    data.to_vec()
}

fn numeric_value_change(command: u8, id: u16, reserved: u8, value: u32) -> Vec<u8> {
    let mut data = vec![command];
    data.extend_from_slice(&id.to_le_bytes());
    data.push(reserved);
    data.extend_from_slice(&value.to_le_bytes());
    data
}

fn string_value_change(command: u8, id: u16, bytes: &[u8]) -> Vec<u8> {
    let mut data = vec![command];
    data.extend_from_slice(&id.to_le_bytes());
    let len = u16::try_from(bytes.len()).unwrap();
    data.extend_from_slice(&len.to_le_bytes());
    data.extend_from_slice(bytes);
    data
}

fn vt_status(active_working_set: u8, version: u8) -> Vec<u8> {
    let mut data = [0xFFu8; 8];
    data[0] = cmd::VT_STATUS;
    data[1] = active_working_set;
    data[6] = version;
    data.to_vec()
}

fn classic_label_command(command: u8, label: &[u8]) -> Vec<u8> {
    let mut data = fixed_command(command);
    for (slot, byte) in data.iter_mut().skip(1).take(7).zip(label.iter()) {
        *slot = *byte;
    }
    data
}

fn minimal_object_pool() -> ObjectPool {
    let working_set = create_working_set(1, &WorkingSetBody::default()).with_children([2u16]);
    let data_mask = create_data_mask(2, &DataMaskBody::default());
    ObjectPool::default()
        .with_object(working_set)
        .with_object(data_mask)
}

fn minimal_object_pool_transfer() -> Vec<u8> {
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(minimal_object_pool().serialize().unwrap());
    transfer
}

fn vt_standard_temp_dir(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("machbus_standard_{name}_{nanos}"));
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn object_reference_pool() -> ObjectPool {
    let working_set = create_working_set(1, &WorkingSetBody::default()).with_children([2u16]);
    let input_list = create_input_list(
        4,
        &InputListBody {
            items: vec![ObjectID(5)],
            value: 0,
            options: 0x03,
            ..Default::default()
        },
    )
    .unwrap();
    ObjectPool::default()
        .with_object(working_set)
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID(3),
                    ..Default::default()
                },
            )
            .with_children([4u16, 5u16, 26u16, 33u16]),
        )
        .with_object(create_soft_key_mask(3, &SoftKeyMaskBody::default()).with_children([34u16]))
        .with_object(input_list)
        .with_object(
            create_output_string(
                5,
                &OutputStringBody {
                    value: b"old".to_vec(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_font_attributes(6, &FontAttributesBody::default()))
        .with_object(create_line_attributes(7, &LineAttributesBody::default()))
        .with_object(create_fill_attributes(8, &FillAttributesBody::default()).unwrap())
        .with_object(
            create_output_polygon(
                9,
                &OutputPolygonBody {
                    points: vec![
                        PolygonPoint { x: 0, y: 0 },
                        PolygonPoint { x: 10, y: 0 },
                        PolygonPoint { x: 0, y: 10 },
                    ],
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_colour_map(
                10,
                &ColourMapBody {
                    entries: vec![0, 1],
                },
            )
            .unwrap(),
        )
        .with_object(create_graphic_context(11, &GraphicContextBody::default()).unwrap())
        .with_object(create_macro(12, &MacroBody::default()))
        .with_object(
            create_colour_palette(
                13,
                &ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_11_22_33],
                },
            )
            .unwrap(),
        )
        .with_object(create_string_variable(
            14,
            &StringVariableBody {
                length: 2,
                value: b"OK".to_vec(),
            },
        ))
        .with_object(create_number_variable(18, &NumberVariableBody::default()))
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
        .with_object(
            create_input_string(
                24,
                &InputStringBody {
                    font_attributes: ObjectID(6),
                    input_attributes: ObjectID(17),
                    variable_reference: ObjectID(14),
                    max_length: 2,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_number(
                25,
                &InputNumberBody {
                    font_attributes: ObjectID(6),
                    variable_reference: ObjectID(18),
                    min_value: 0,
                    max_value: 100,
                    options2: 0x03,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            19,
            &ObjectPointerBody { value: ObjectID(5) },
        ))
        .with_object(
            create_output_list(
                20,
                &OutputListBody {
                    items: vec![ObjectID(5)],
                    value: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_output_number(43, &OutputNumberBody::default()).unwrap())
        .with_object(
            create_output_line(
                21,
                &OutputLineBody {
                    width: 10,
                    height: 10,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_attributes(
                17,
                &InputAttributesBody {
                    validation_type: 0,
                    validation_string: b"AB".to_vec(),
                },
            )
            .unwrap(),
        )
        .with_object(create_object_label_ref(
            15,
            &ObjectLabelRefBody {
                labels: vec![ObjectLabelRefEntry {
                    labelled_object: ObjectID(5),
                    string_variable: ObjectID(14),
                    font_type: 0,
                    graphic_designator: ObjectID::NULL,
                }],
            },
        ))
        .with_object(
            create_working_set_special_controls(
                16,
                &WorkingSetSpecialControlsBody {
                    colour_map: ObjectID::NULL,
                    colour_palette: ObjectID::new(13),
                    languages: Vec::new(),
                    extra_bytes: Vec::new(),
                },
            )
            .unwrap(),
        )
        .with_object(
            create_alarm_mask(
                22,
                &AlarmMaskBody {
                    priority: 2,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_container(26, &ContainerBody::default()).with_children([25u16]))
        .with_object(create_output_rectangle(27, &OutputRectangleBody::default()).unwrap())
        .with_object(create_output_ellipse(28, &OutputEllipseBody::default()).unwrap())
        .with_object(create_button(
            33,
            &ButtonBody {
                options: 0x01,
                ..Default::default()
            },
        ))
        .with_object(
            create_meter(
                29,
                &MeterBody {
                    min_value: 5,
                    max_value: 100,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_linear_bar_graph(
                30,
                &LinearBarGraphBody {
                    min_value: 5,
                    max_value: 100,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_arched_bar_graph(
                31,
                &ArchedBarGraphBody {
                    min_value: 5,
                    max_value: 100,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_animation(32, &AnimationBody::default())
                .unwrap()
                .with_children([5u16, 27u16]),
        )
        .with_object(create_key(
            34,
            &KeyBody {
                background_color: 7,
                key_code: 9,
            },
        ))
        .with_object(
            create_key_group(
                44,
                &KeyGroupBody {
                    options: 1,
                    name: ObjectID(5),
                    key_group_icon: ObjectID(27),
                },
            )
            .with_children([34u16]),
        )
        .with_object(
            create_extended_input_attributes(
                35,
                &ExtendedInputAttributesBody {
                    validation_type: 0,
                    code_planes: vec![ExtendedInputCodePlane {
                        plane: 0,
                        ranges: vec![WideCharRange {
                            first: 0x0041,
                            last: 0x005A,
                        }],
                    }],
                },
            )
            .unwrap(),
        )
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
                default_object_id: ObjectID(5),
                external_reference_name: ObjectID::NULL,
                external_object_id: ObjectID::NULL,
            },
        ))
        .with_object(
            create_scaled_graphic(
                38,
                &ScaledGraphicBody {
                    value: ObjectID(23),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_input_boolean(
                39,
                &InputBooleanBody {
                    foreground: ObjectID(6),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_window_mask(
                40,
                &WindowMaskBody {
                    width_cells: 2,
                    height_cells: 6,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(create_object_pointer(
            41,
            &ObjectPointerBody {
                value: ObjectID(23),
            },
        ))
        .with_object(
            create_scaled_graphic(
                42,
                &ScaledGraphicBody {
                    value: ObjectID(41),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_scaled_bitmap(
                45,
                &ScaledBitmapBody {
                    bitmap_data: ObjectID::NULL,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
}

fn object_reference_pool_transfer() -> Vec<u8> {
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(object_reference_pool().serialize().unwrap());
    transfer
}

fn key_pointer_context_pool() -> ObjectPool {
    ObjectPool::default()
        .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
        .with_object(
            create_data_mask(
                2,
                &DataMaskBody {
                    soft_key_mask: ObjectID(3),
                    ..Default::default()
                },
            )
            .with_children([30u16]),
        )
        .with_object(
            create_soft_key_mask(3, &SoftKeyMaskBody::default()).with_children([41u16, 42u16]),
        )
        .with_object(
            create_key_group(
                30,
                &KeyGroupBody {
                    options: 1,
                    ..Default::default()
                },
            )
            .with_children([40u16]),
        )
        .with_object(create_object_pointer(
            40,
            &ObjectPointerBody {
                value: ObjectID(31),
            },
        ))
        .with_object(create_object_pointer(
            41,
            &ObjectPointerBody {
                value: ObjectID(31),
            },
        ))
        .with_object(create_external_object_pointer(
            42,
            &ExternalObjectPointerBody {
                default_object_id: ObjectID(31),
                external_reference_name: ObjectID::NULL,
                external_object_id: ObjectID::NULL,
            },
        ))
        .with_object(create_key(31, &KeyBody::default()))
        .with_object(create_key(
            32,
            &KeyBody {
                key_code: 32,
                ..Default::default()
            },
        ))
        .with_object(create_output_string(50, &OutputStringBody::default()).unwrap())
}

fn key_pointer_context_pool_transfer() -> Vec<u8> {
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(key_pointer_context_pool().serialize().unwrap());
    transfer
}

fn aux_assignment_pool() -> ObjectPool {
    let working_set = create_working_set(1, &WorkingSetBody::default()).with_children([2u16]);
    ObjectPool::default()
        .with_object(working_set)
        .with_object(create_data_mask(2, &DataMaskBody::default()))
        .with_object(
            create_aux_function(
                20,
                &AuxFunctionBody {
                    function_type: AuxFunctionType::Type1.as_u8(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_aux_input(
                21,
                &AuxInputBody {
                    input_type: AuxFunctionType::Type1.as_u8(),
                    input_id: 7,
                    options: 1,
                },
            )
            .unwrap(),
        )
        .with_object(
            create_aux_function2(
                22,
                &AuxFunction2Body {
                    function_type: AuxFunctionType::Type2.as_u8(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_aux_input2(
                23,
                &AuxInput2Body {
                    input_type: AuxFunctionType::Type2.as_u8(),
                    input_id: 9,
                    input_status: AuxFunctionState::Variable.as_u8(),
                    input_value: 0,
                    ..Default::default()
                },
            )
            .unwrap(),
        )
        .with_object(
            create_aux_function(
                24,
                &AuxFunctionBody {
                    function_type: AuxFunctionType::Type2.as_u8(),
                    ..Default::default()
                },
            )
            .unwrap(),
        )
}

fn aux_assignment_pool_transfer() -> Vec<u8> {
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(aux_assignment_pool().serialize().unwrap());
    transfer
}

fn alternate_object_pool_transfer() -> Vec<u8> {
    let working_set = create_working_set(10, &WorkingSetBody::default()).with_children([11u16]);
    let data_mask = create_data_mask(11, &DataMaskBody::default());
    let pool = ObjectPool::default()
        .with_object(working_set)
        .with_object(data_mask);
    let mut transfer = vec![cmd::OBJECT_POOL_TRANSFER];
    transfer.extend(pool.serialize().unwrap());
    transfer
}

fn activate_standard_pool(server: &mut VTServer, source: u8) {
    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                source,
            ))
            .len(),
        1
    );
    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                minimal_object_pool_transfer(),
                source,
            ))
            .is_empty()
    );
    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].data[1], 0x00);
    assert!(server.clients()[0].pool_activated);
}

fn activate_reference_pool(server: &mut VTServer, source: u8) {
    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                source,
            ))
            .len(),
        1
    );
    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                object_reference_pool_transfer(),
                source,
            ))
            .is_empty()
    );
    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].data[1], 0x00);
    assert!(server.clients()[0].pool_activated);
}

fn activate_key_pointer_context_pool(server: &mut VTServer, source: u8) {
    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                source,
            ))
            .len(),
        1
    );
    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                key_pointer_context_pool_transfer(),
                source,
            ))
            .is_empty()
    );
    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].data[1], 0x00);
    assert!(server.clients()[0].pool_activated);
}

fn activate_aux_assignment_pool(server: &mut VTServer, source: u8) {
    assert_eq!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                fixed_command(cmd::GET_MEMORY),
                source,
            ))
            .len(),
        1
    );
    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                aux_assignment_pool_transfer(),
                source,
            ))
            .is_empty()
    );
    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::END_OF_POOL),
        source,
    ));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].data[1], 0x00);
    assert!(server.clients()[0].pool_activated);
}

fn connect_standard_client(client: &mut VTClient) {
    client.set_object_pool(minimal_object_pool());
    client.set_self_address(0x44);
    client.connect().unwrap();

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vt_status(0x44, 5), 0x80));
    assert_eq!(client.vt_address(), 0x80);

    let working_set_master = client.update(0);
    assert_eq!(working_set_master.len(), 1);
    assert_eq!(
        working_set_master[0].pgn,
        machbus::net::pgn_defs::PGN_WORKING_SET_MASTER
    );

    let get_memory = client.update(0);
    assert_eq!(get_memory.len(), 1);
    assert_eq!(get_memory[0].data[0], cmd::GET_MEMORY);

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![
            cmd::GET_MEMORY_RESPONSE,
            0x00,
            0x00,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x80,
    ));

    let transfer = client.update(0);
    assert_eq!(transfer.len(), 1);
    assert_eq!(transfer[0].data[0], cmd::OBJECT_POOL_TRANSFER);

    let end_of_pool = client.update(1000);
    assert_eq!(end_of_pool.len(), 1);
    assert_eq!(end_of_pool[0].data[0], cmd::END_OF_POOL);

    let mut response = [0xFFu8; 8];
    response[0] = cmd::END_OF_POOL;
    response[1] = 0x00;
    response[6] = 0x00;
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, response.to_vec(), 0x80));
    assert_eq!(client.state(), VTState::Connected);
}

#[test]
fn vt_activation_codes_accept_only_defined_wire_values() {
    for value in 0..=3 {
        let code = ActivationCode::try_from_u8(value).expect("defined activation code");
        assert_eq!(code.as_u8(), value);
        assert_eq!(ActivationCode::from_u8(value), code);
    }

    assert_eq!(ActivationCode::try_from_u8(4), None);
    assert_eq!(ActivationCode::try_from_u8(0xFF), None);
    assert_eq!(ActivationCode::from_u8(0xFF), ActivationCode::Released);
}

#[test]
fn vt_activation_code_public_decoder_rejects_noncanonical_bytes() {
    for (raw, code) in [
        (0, ActivationCode::Released),
        (1, ActivationCode::Pressed),
        (2, ActivationCode::Held),
        (3, ActivationCode::Aborted),
    ] {
        assert_eq!(ActivationCode::try_from_u8(raw), Some(code));
        assert_eq!(ActivationCode::from_u8(raw), code);
    }
    for raw in [4, 5, 0x10, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(ActivationCode::try_from_u8(raw), None);
    }
}

#[test]
fn vt_command_code_groups_keep_client_server_direction_boundaries() {
    let notification_codes = [cmd::SOFT_KEY_ACTIVATION, cmd::END_OF_POOL];
    let ecu_to_vt_codes = [cmd::SELECT_ACTIVE_WORKING_SET, cmd::CHANGE_STRING_VALUE];
    let pool_transfer_codes = [cmd::GET_MEMORY, cmd::UNSUPPORTED_VT_FUNCTION];

    assert!(notification_codes.iter().all(|code| *code < 0x20));
    assert!(ecu_to_vt_codes.iter().all(|code| *code >= 0x90));
    assert!(pool_transfer_codes.iter().all(|code| *code >= 0xC0));

    assert_eq!(VT_STRING_VALUE_MAX_LEN, u16::MAX as usize);
    assert_eq!(cmd::CLASSIC_VERSION_LABEL_SIZE, 7);
    assert_eq!(cmd::EXTENDED_VERSION_LABEL_SIZE, 32);
}

#[test]
fn vt_client_rejects_invalid_vt_sources_before_state_or_event_mutation() {
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let mut client = VTClient::new(VTClientConfig::default());
        let soft_keys: Rc<RefCell<Vec<ObjectID>>> = Rc::new(RefCell::new(Vec::new()));
        let soft_key_log = soft_keys.clone();
        client
            .on_soft_key
            .subscribe(move |(id, _)| soft_key_log.borrow_mut().push(*id));

        let mut vt_status = [0xFFu8; 8];
        vt_status[0] = cmd::VT_STATUS;
        vt_status[1] = 0x44;
        vt_status[6] = 5;
        client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vt_status.to_vec(), source));
        assert_eq!(client.state(), VTState::Disconnected);
        assert_eq!(client.vt_address(), NULL_ADDRESS);
        assert_eq!(client.vt_version_value(), 0);

        client.handle_vt_message(&Message::new(
            PGN_VT_TO_ECU,
            vec![cmd::SOFT_KEY_ACTIVATION, 1, 0x34, 0x12, 0, 0, 0, 0xFF],
            source,
        ));
        assert!(soft_keys.borrow().is_empty());

        client.handle_language_command(&Message::new(
            PGN_LANGUAGE_COMMAND,
            b"de\0\0\0\0\0\0".to_vec(),
            source,
        ));
        assert_eq!(client.vt_language().code, *b"en");
    }

    let mut client = VTClient::new(VTClientConfig::default());
    let mut vt_status = [0xFFu8; 8];
    vt_status[0] = cmd::VT_STATUS;
    vt_status[1] = 0x44;
    vt_status[6] = 5;
    client.handle_vt_message(&Message::with_addressing(
        PGN_VT_TO_ECU,
        vt_status.to_vec(),
        0x80,
        NULL_ADDRESS,
        Priority::Default,
    ));
    assert_eq!(client.state(), VTState::Disconnected);
    assert_eq!(client.vt_address(), NULL_ADDRESS);

    client.handle_language_command(&Message::with_addressing(
        PGN_LANGUAGE_COMMAND,
        b"de\0\0\0\0\0\0".to_vec(),
        0x80,
        0x42,
        Priority::Default,
    ));
    assert_eq!(
        client.vt_language().code,
        *b"en",
        "Language Command is a PDU2 group-extension PGN and must not accept destination-specific metadata"
    );
}

#[test]
fn vt_server_rejects_invalid_ecu_sources_before_client_or_reply_state() {
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        let mut server = VTServer::new(VTServerConfig::default());
        server.start().unwrap();

        let mut get_memory = [0xFFu8; 8];
        get_memory[0] = cmd::GET_MEMORY;
        assert!(
            server
                .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, get_memory.to_vec(), source))
                .is_empty()
        );
        assert!(server.clients().is_empty());

        assert!(
            server
                .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, vec![0xEF], source))
                .is_empty(),
            "unsupported-function replies must not be addressed to unusable source addresses"
        );
        assert!(server.clients().is_empty());
    }

    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    let mut get_memory = [0xFFu8; 8];
    get_memory[0] = cmd::GET_MEMORY;
    assert!(
        server
            .handle_ecu_message(&Message::with_addressing(
                PGN_ECU_TO_VT,
                get_memory.to_vec(),
                0x42,
                NULL_ADDRESS,
                Priority::Default,
            ))
            .is_empty()
    );
    assert!(
        server.clients().is_empty(),
        "null-destination ECU-to-VT frames must not create VT server clients"
    );
}

#[test]
fn vt_state_tracker_rejects_wrong_pgn_and_invalid_sources_before_cache_update() {
    let mut tracker = VTClientStateTracker::new();
    let mut status = [0xFFu8; 8];
    status[0] = cmd::VT_STATUS;
    status[2..4].copy_from_slice(&0x1234u16.to_le_bytes());
    status[4..6].copy_from_slice(&0x5678u16.to_le_bytes());
    status[6] = 0x09;
    status[7] = 0x0A;

    tracker.handle_vt_message(&Message::new(PGN_REQUEST, status.to_vec(), 0x80));
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, status.to_vec(), source));
    }
    tracker.handle_vt_message(&Message::with_addressing(
        PGN_VT_TO_ECU,
        status.to_vec(),
        0x80,
        NULL_ADDRESS,
        Priority::Default,
    ));
    assert_eq!(tracker.active_data_mask(), ObjectID::NULL);
    assert_eq!(tracker.active_soft_key_mask(), ObjectID::NULL);
    assert_eq!(tracker.vt_busy_code(), 0);
    assert_eq!(tracker.vt_function_code(), 0xFF);
    assert_eq!(tracker.vt_address(), NULL_ADDRESS);

    tracker.handle_vt_message(&Message::new(PGN_VT_TO_ECU, status.to_vec(), 0x80));
    assert_eq!(tracker.active_data_mask(), ObjectID(0x1234));
    assert_eq!(tracker.active_soft_key_mask(), ObjectID(0x5678));
    assert_eq!(tracker.vt_busy_code(), 0x09);
    assert_eq!(tracker.vt_function_code(), 0x0A);
    assert_eq!(tracker.vt_address(), 0x80);
}

#[test]
fn vt_auxiliary_public_function_decoders_reject_noncanonical_bytes() {
    for (raw, ty) in [
        (0, AuxFunctionType::Type0),
        (1, AuxFunctionType::Type1),
        (2, AuxFunctionType::Type2),
    ] {
        assert_eq!(AuxFunctionType::try_from_u8(raw), Some(ty));
        assert_eq!(AuxFunctionType::from_u8(raw), ty);
    }

    for (raw, state) in [
        (0, AuxFunctionState::Off),
        (1, AuxFunctionState::On),
        (2, AuxFunctionState::Variable),
    ] {
        assert_eq!(AuxFunctionState::try_from_u8(raw), Some(state));
        assert_eq!(AuxFunctionState::from_u8(raw), state);
    }

    for reserved in [3, 4, 0x08, 0x10, 0x40, 0xFE, 0xFF] {
        assert_eq!(AuxFunctionType::try_from_u8(reserved), None);
        assert_eq!(AuxFunctionState::try_from_u8(reserved), None);
    }

    let aux_o = AuxOFunction::with_setpoint(5, AuxFunctionType::Type1, 1_234);
    assert_eq!(
        AuxOFunction::decode(&Message::new(
            PGN_AUX_INPUT_STATUS,
            aux_o.encode().to_vec(),
            0x70,
        )),
        Some(aux_o)
    );

    let aux_n = AuxNFunction::with_setpoint(6, AuxFunctionType::Type2, 0xBEEF);
    assert_eq!(
        AuxNFunction::decode(&Message::new(
            PGN_AUX_INPUT_TYPE2,
            aux_n.encode().to_vec(),
            0x70,
        )),
        Some(aux_n)
    );
}

#[test]
fn vt_auxiliary_public_function_decoders_reject_wrong_pgn_and_invalid_sources() {
    let aux_o = AuxOFunction::with_setpoint(5, AuxFunctionType::Type1, 1_234);
    let aux_o_bytes = aux_o.encode();
    assert_eq!(
        AuxOFunction::decode(&Message::new(
            PGN_AUX_INPUT_STATUS,
            aux_o_bytes.to_vec(),
            0x70,
        )),
        Some(aux_o)
    );

    assert_eq!(
        AuxOFunction::decode(&Message::new(
            PGN_AUX_INPUT_TYPE2,
            aux_o_bytes.to_vec(),
            0x70,
        )),
        None,
        "AUX-O decoder must stay bound to the AUX-O status PGN"
    );
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert_eq!(
            AuxOFunction::decode(&Message::new(
                PGN_AUX_INPUT_STATUS,
                aux_o_bytes.to_vec(),
                source,
            )),
            None,
            "AUX-O status from invalid source address {source:#04X} must be rejected"
        );
    }

    let aux_n = AuxNFunction::with_setpoint(6, AuxFunctionType::Type2, 0xBEEF);
    let aux_n_bytes = aux_n.encode();
    assert_eq!(
        AuxNFunction::decode(&Message::new(
            PGN_AUX_INPUT_TYPE2,
            aux_n_bytes.to_vec(),
            0x70,
        )),
        Some(aux_n)
    );

    assert_eq!(
        AuxNFunction::decode(&Message::new(
            PGN_AUX_INPUT_STATUS,
            aux_n_bytes.to_vec(),
            0x70,
        )),
        None,
        "AUX-N decoder must stay bound to the AUX-N status PGN"
    );
    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert_eq!(
            AuxNFunction::decode(&Message::new(
                PGN_AUX_INPUT_TYPE2,
                aux_n_bytes.to_vec(),
                source,
            )),
            None,
            "AUX-N status from invalid source address {source:#04X} must be rejected"
        );
    }
}

#[test]
fn vt_alarm_priority_public_decoder_rejects_noncanonical_bytes() {
    assert_eq!(AlarmPriority::try_from_u8(0), Some(AlarmPriority::Critical));
    assert_eq!(AlarmPriority::try_from_u8(1), Some(AlarmPriority::Warning));
    assert_eq!(
        AlarmPriority::try_from_u8(2),
        Some(AlarmPriority::Information)
    );
    for raw in [3, 4, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(AlarmPriority::try_from_u8(raw), None);
    }
}

#[test]
fn vt_language_command_public_unit_decoders_reject_noncanonical_bytes() {
    for (raw, value) in [
        (0, DistanceUnit::Metric),
        (1, DistanceUnit::Imperial),
        (2, DistanceUnit::Us),
    ] {
        assert_eq!(DistanceUnit::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [
        (0, AreaUnit::Metric),
        (1, AreaUnit::Imperial),
        (2, AreaUnit::Us),
    ] {
        assert_eq!(AreaUnit::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [
        (0, VolumeUnit::Metric),
        (1, VolumeUnit::Imperial),
        (2, VolumeUnit::Us),
    ] {
        assert_eq!(VolumeUnit::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [
        (0, MassUnit::Metric),
        (1, MassUnit::Imperial),
        (2, MassUnit::Us),
    ] {
        assert_eq!(MassUnit::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [(0, TemperatureUnit::Metric), (1, TemperatureUnit::Imperial)] {
        assert_eq!(TemperatureUnit::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [(0, PressureUnit::Metric), (1, PressureUnit::Imperial)] {
        assert_eq!(PressureUnit::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [(0, ForceUnit::Metric), (1, ForceUnit::Imperial)] {
        assert_eq!(ForceUnit::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [(0, UnitSystem::Metric), (1, UnitSystem::Us)] {
        assert_eq!(UnitSystem::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [(0, TimeFormat::TwentyFourHour), (1, TimeFormat::TwelveHour)] {
        assert_eq!(TimeFormat::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [
        (0, DateFormat::DdMmYyyy),
        (1, DateFormat::MmDdYyyy),
        (4, DateFormat::YyyyMmDd),
    ] {
        assert_eq!(DateFormat::try_from_u8(raw), Some(value));
    }
    for (raw, value) in [(0, DecimalSymbol::Comma), (1, DecimalSymbol::Period)] {
        assert_eq!(DecimalSymbol::try_from_u8(raw), Some(value));
    }

    for raw in [3, 5, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(DistanceUnit::try_from_u8(raw), None);
        assert_eq!(AreaUnit::try_from_u8(raw), None);
        assert_eq!(VolumeUnit::try_from_u8(raw), None);
        assert_eq!(MassUnit::try_from_u8(raw), None);
    }
    for raw in [2, 3, 4, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(TemperatureUnit::try_from_u8(raw), None);
        assert_eq!(PressureUnit::try_from_u8(raw), None);
        assert_eq!(ForceUnit::try_from_u8(raw), None);
        assert_eq!(UnitSystem::try_from_u8(raw), None);
        assert_eq!(TimeFormat::try_from_u8(raw), None);
        assert_eq!(DecimalSymbol::try_from_u8(raw), None);
    }
    for raw in [2, 3, 5, 0x7F, 0x80, 0xFE, 0xFF] {
        assert_eq!(DateFormat::try_from_u8(raw), None);
    }
}

#[test]
fn vt_language_command_public_decoder_rejects_wrong_pgn_and_invalid_sources() {
    let language = LanguageData {
        language_code: *b"de",
        ..LanguageData::default()
    };
    let encoded = language.encode();
    assert_eq!(
        LanguageData::decode(&Message::new(PGN_LANGUAGE_COMMAND, encoded.to_vec(), 0x80,)),
        Some(language)
    );

    assert_eq!(
        LanguageData::decode(&Message::new(PGN_REQUEST, encoded.to_vec(), 0x80)),
        None,
        "Language Command decoder must stay bound to the Language Command PGN"
    );

    for source in [NULL_ADDRESS, BROADCAST_ADDRESS] {
        assert_eq!(
            LanguageData::decode(&Message::new(
                PGN_LANGUAGE_COMMAND,
                encoded.to_vec(),
                source,
            )),
            None,
            "Language Command from invalid source address {source:#04X} must be rejected"
        );
    }
}

#[test]
fn vt_handlers_reject_wrong_pgn_envelopes_before_state_or_events() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_REQUEST,
                fixed_command(cmd::GET_MEMORY),
                0x42,
            ))
            .is_empty()
    );
    assert!(
        server.clients().is_empty(),
        "ECU-to-VT handler must ignore VT commands carried under a non-VT PGN"
    );

    let mut client = VTClient::new(VTClientConfig::default());
    let soft_keys: Rc<RefCell<Vec<ObjectID>>> = Rc::new(RefCell::new(Vec::new()));
    let soft_key_log = soft_keys.clone();
    client
        .on_soft_key
        .subscribe(move |(id, _)| soft_key_log.borrow_mut().push(*id));

    let mut vt_status = [0xFFu8; 8];
    vt_status[0] = cmd::VT_STATUS;
    vt_status[1] = 0x42;
    vt_status[6] = 5;
    client.handle_vt_message(&Message::new(PGN_REQUEST, vt_status.to_vec(), 0x80));
    assert_eq!(client.vt_address(), NULL_ADDRESS);
    assert_eq!(client.vt_version_value(), 0);

    client.handle_vt_message(&Message::new(
        PGN_REQUEST,
        vec![cmd::SOFT_KEY_ACTIVATION, 1, 0x34, 0x12, 0, 0, 0, 0xFF],
        0x80,
    ));
    assert!(soft_keys.borrow().is_empty());

    client.handle_language_command(&Message::new(PGN_REQUEST, b"de\0\0\0\0\0\0".to_vec(), 0x80));
    assert_eq!(client.vt_language().code, *b"en");
}

#[test]
fn vt_client_rejects_unsolicited_memory_response_before_state_change() {
    let mut client = VTClient::new(VTClientConfig::default());
    client.set_object_pool(minimal_object_pool());
    let response = vec![
        cmd::GET_MEMORY_RESPONSE,
        0x00,
        0x00,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
        0xFF,
    ];

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, response.clone(), 0x80));
    assert_eq!(
        client.state(),
        VTState::Disconnected,
        "unsolicited memory response must not advance the upload state machine"
    );

    client.connect().unwrap();
    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        fixed_command(cmd::VT_STATUS),
        0x80,
    ));
    let outbound = client.update(0);
    assert_eq!(outbound.len(), 1);
    assert_eq!(
        outbound[0].pgn,
        machbus::net::pgn_defs::PGN_WORKING_SET_MASTER
    );
    let outbound = client.update(0);
    assert_eq!(outbound.len(), 1);
    assert_eq!(outbound[0].data[0], cmd::GET_MEMORY);
    assert_eq!(client.state(), VTState::WaitForMemory);

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, response, 0x80));
    assert_eq!(
        client.state(),
        VTState::UploadPool,
        "memory response is accepted only after the client has requested memory"
    );
}

#[test]
fn vt_client_binds_to_negotiated_vt_source_until_disconnect() {
    let mut client = VTClient::new(VTClientConfig::default());
    client.set_object_pool(minimal_object_pool());
    client.set_self_address(0x44);

    let soft_keys: Rc<RefCell<Vec<(ObjectID, ActivationCode)>>> = Rc::new(RefCell::new(Vec::new()));
    let soft_key_log = soft_keys.clone();
    client
        .on_soft_key
        .subscribe(move |event| soft_key_log.borrow_mut().push(*event));

    client.connect().unwrap();
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vt_status(0x44, 5), 0x80));
    assert_eq!(client.vt_address(), 0x80);
    assert_eq!(client.vt_version_value(), 5);
    assert!(client.is_active_ws());

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vt_status(0x00, 6), 0x81));
    assert_eq!(
        client.vt_address(),
        0x80,
        "a different VT source must not steal an established client session"
    );
    assert_eq!(client.vt_version_value(), 5);
    assert!(client.is_active_ws());

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        VTServer::build_soft_key_activation(ActivationCode::Pressed, 0x1234, 0x0002, 1).to_vec(),
        0x81,
    ));
    assert!(soft_keys.borrow().is_empty());

    client.handle_language_command(&Message::new(
        PGN_LANGUAGE_COMMAND,
        b"de\0\0\0\0\0\0".to_vec(),
        0x81,
    ));
    assert_eq!(client.vt_language().code, *b"en");

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        VTServer::build_soft_key_activation(ActivationCode::Pressed, 0x1234, 0x0002, 1).to_vec(),
        0x80,
    ));
    assert_eq!(
        *soft_keys.borrow(),
        vec![(ObjectID(0x1234), ActivationCode::Pressed)]
    );

    client.handle_language_command(&Message::new(
        PGN_LANGUAGE_COMMAND,
        b"de\0\0\0\0\0\0".to_vec(),
        0x80,
    ));
    assert_eq!(client.vt_language().code, *b"de");

    client.disconnect().unwrap();
    assert_eq!(client.state(), VTState::Disconnected);
    assert_eq!(client.vt_address(), NULL_ADDRESS);
    assert!(
        !client.is_active_ws(),
        "disconnect must clear active-working-set ownership"
    );

    client.connect().unwrap();
    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vt_status(0x44, 4), 0x81));
    assert_eq!(
        client.vt_address(),
        0x81,
        "disconnect must release the old VT source binding so a later session can bind again"
    );
    assert_eq!(client.vt_version_value(), 4);
    assert!(client.is_active_ws());

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, vt_status(0x00, 7), 0x80));
    assert_eq!(
        client.vt_address(),
        0x81,
        "old VT source traffic must not steal a newly rebound session"
    );
    assert_eq!(client.vt_version_value(), 4);
    assert!(client.is_active_ws());
}

#[test]
fn vt_unsupported_function_reports_are_canonical_and_source_bound() {
    let unsupported = VTServer::build_unsupported_function(cmd::CHANGE_CHILD_POSITION);
    assert_eq!(
        unsupported,
        [
            cmd::UNSUPPORTED_VT_FUNCTION,
            cmd::CHANGE_CHILD_POSITION,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF
        ],
        "Unsupported Function reports must use the canonical fixed-frame payload"
    );

    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    let response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, vec![0xEF], 0x42));
    assert_eq!(response.len(), 1);
    assert_eq!(response[0].dest, Some(0x42));
    assert_eq!(
        response[0].data,
        vec![
            cmd::UNSUPPORTED_VT_FUNCTION,
            0xEF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF
        ]
    );

    let mut client = VTClient::new(VTClientConfig::default());
    connect_standard_client(&mut client);
    assert_eq!(client.vt_address(), 0x80);

    let observed: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let observed_log = observed.clone();
    client
        .on_unsupported_function
        .subscribe(move |function| observed_log.borrow_mut().push(*function));

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, unsupported.to_vec(), 0x81));
    assert!(client.unsupported_functions().is_empty());
    assert!(observed.borrow().is_empty());

    client.handle_vt_message(&Message::new(
        PGN_VT_TO_ECU,
        vec![cmd::UNSUPPORTED_VT_FUNCTION, cmd::CHANGE_CHILD_POSITION],
        0x80,
    ));
    assert!(client.unsupported_functions().is_empty());
    assert!(observed.borrow().is_empty());

    client.handle_vt_message(&Message::new(PGN_VT_TO_ECU, unsupported.to_vec(), 0x80));
    assert_eq!(
        client.unsupported_functions(),
        &[cmd::CHANGE_CHILD_POSITION]
    );
    assert_eq!(*observed.borrow(), vec![cmd::CHANGE_CHILD_POSITION]);
}

#[test]
fn vt_server_get_window_mask_data_reports_user_layout_backgrounds() {
    let server_config = VTServerConfig {
        user_layout_data_mask_background_colour: 0x2A,
        user_layout_soft_key_background_colour: 0x35,
        ..Default::default()
    };
    let mut server = VTServer::new(server_config);

    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::GET_WINDOW_MASK_DATA),
        0x42,
    ));

    assert_eq!(response.len(), 1);
    assert_eq!(response[0].dest, Some(0x42));
    assert_eq!(
        response[0].data,
        vec![
            cmd::GET_WINDOW_MASK_DATA,
            0x2A,
            0x35,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF
        ],
        "Get Window Mask Data must expose VT-owned user-layout background colours, not a Working Set object colour"
    );

    assert!(
        server
            .handle_ecu_message(&Message::new(
                PGN_ECU_TO_VT,
                vec![cmd::GET_WINDOW_MASK_DATA, 0xFF, 0xFF],
                0x42,
            ))
            .is_empty(),
        "short technical-data requests are ignored instead of returning prefix-compatible responses"
    );

    let mut noncanonical = fixed_command(cmd::GET_WINDOW_MASK_DATA);
    noncanonical[3] = 0x00;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, noncanonical, 0x42))
            .is_empty(),
        "reserved request bytes must be 0xFF for parameterless technical-data requests"
    );
}

#[test]
fn vt_server_get_supported_objects_reports_sorted_standard_object_list() {
    let mut server = VTServer::new(VTServerConfig::default());

    let response = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        fixed_command(cmd::GET_SUPPORTED_OBJECTS),
        0x42,
    ));

    assert_eq!(response.len(), 1);
    assert_eq!(response[0].dest, Some(0x42));
    assert_eq!(response[0].data[0], cmd::GET_SUPPORTED_OBJECTS);
    assert_eq!(
        usize::from(response[0].data[1]),
        response[0].data.len() - 2,
        "byte 2 is the number of bytes to follow, not a fixed object count assumption"
    );

    let object_types = &response[0].data[2..];
    assert!(
        object_types.windows(2).all(|pair| pair[0] < pair[1]),
        "Get Supported Objects response must be numerically ascending"
    );
    for required in [
        ObjectType::WorkingSet.as_u8(),
        ObjectType::DataMask.as_u8(),
        ObjectType::WindowMask.as_u8(),
        ObjectType::KeyGroup.as_u8(),
        ObjectType::GraphicContext.as_u8(),
        ObjectType::OutputList.as_u8(),
        ObjectType::ExternalObjectPointer.as_u8(),
        ObjectType::WorkingSetSpecialControls.as_u8(),
        ObjectType::ScaledGraphic.as_u8(),
    ] {
        assert!(
            object_types.contains(&required),
            "supported standard object type {required} should be advertised"
        );
    }
    for not_advertised in [
        ObjectType::AuxFunction.as_u8(),
        ObjectType::AuxInput.as_u8(),
        ObjectType::ScaledBitmap.as_u8(),
        ObjectType::GraphicsContext.as_u8(),
    ] {
        assert!(
            !object_types.contains(&not_advertised),
            "type {not_advertised} must not be listed in the standard supported-object response"
        );
    }

    let malformed = server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        vec![
            cmd::GET_SUPPORTED_OBJECTS,
            0x00,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
            0xFF,
        ],
        0x42,
    ));
    assert!(
        malformed.is_empty(),
        "malformed Get Supported Objects variants must not be answered as the standard query"
    );
}

#[test]
fn vt_server_get_supported_objects_preserves_aux_capability_subquery() {
    let mut server = VTServer::new(VTServerConfig::default());
    server
        .set_aux_capabilities(vec![AuxChannelCapability {
            channel_id: 3,
            aux_type: 2,
            resolution: 0xA5A5,
            function_type: AuxFunctionType::Type2.as_u8(),
        }])
        .unwrap();

    let mut query = fixed_command(cmd::GET_SUPPORTED_OBJECTS);
    query[1] = 0x01;
    query[2] = ObjectType::AuxFunction2.as_u8();
    query[3] = ObjectType::AuxInput2.as_u8();
    let response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, query, 0x42));

    assert_eq!(response.len(), 1);
    assert_eq!(
        &response[0].data[0..3],
        &[cmd::GET_SUPPORTED_OBJECTS, 0x01, 0x01],
        "the AUX type-2 capability query remains a subfunction of 0xC5"
    );
    assert_eq!(response[0].data.len(), 8);
}

#[test]
fn vt_server_get_supported_widechars_reports_minimum_code_plane_zero_ranges() {
    let mut server = VTServer::new(VTServerConfig::default());

    let mut query = fixed_command(cmd::GET_SUPPORTED_WIDECHARS);
    query[1] = 0;
    query[2..4].copy_from_slice(&0x0000u16.to_le_bytes());
    query[4..6].copy_from_slice(&0x03FFu16.to_le_bytes());
    let response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, query, 0x42));

    assert_eq!(response.len(), 1);
    assert_eq!(response[0].dest, Some(0x42));
    assert_eq!(
        &response[0].data[0..8],
        &[cmd::GET_SUPPORTED_WIDECHARS, 0, 0, 0, 0xFF, 0x03, 0, 10],
        "code-plane 0 response must echo the inquiry and report no error plus the intersecting range count"
    );
    let ranges = response[0].data[8..]
        .chunks_exact(4)
        .map(|range| {
            (
                u16::from_le_bytes([range[0], range[1]]),
                u16::from_le_bytes([range[2], range[3]]),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        ranges,
        vec![
            (0x0020, 0x007E),
            (0x00A0, 0x017E),
            (0x02C6, 0x02C7),
            (0x02C9, 0x02C9),
            (0x02D8, 0x02DD),
            (0x037E, 0x037E),
            (0x0384, 0x038A),
            (0x038C, 0x038C),
            (0x038E, 0x03A1),
            (0x03A3, 0x03CE),
        ],
        "the response is clipped to the requested code-plane-0 inquiry range"
    );

    let mut clipped = fixed_command(cmd::GET_SUPPORTED_WIDECHARS);
    clipped[1] = 0;
    clipped[2..4].copy_from_slice(&0x0041u16.to_le_bytes());
    clipped[4..6].copy_from_slice(&0x0043u16.to_le_bytes());
    let response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, clipped, 0x42));
    assert_eq!(
        response[0].data,
        vec![
            cmd::GET_SUPPORTED_WIDECHARS,
            0,
            0x41,
            0,
            0x43,
            0,
            0,
            1,
            0x41,
            0,
            0x43,
            0,
        ],
        "single-range inquiries should shrink the returned WideChar range instead of returning the whole minimum set"
    );
}

#[test]
fn vt_server_get_supported_widechars_rejects_malformed_or_invalid_queries() {
    let mut server = VTServer::new(VTServerConfig::default());

    let mut bad_tail = fixed_command(cmd::GET_SUPPORTED_WIDECHARS);
    bad_tail[6] = 0;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_tail, 0x42))
            .is_empty(),
        "reserved request bytes must be canonical before responding"
    );

    let mut bad_plane = fixed_command(cmd::GET_SUPPORTED_WIDECHARS);
    bad_plane[1] = 17;
    bad_plane[2..4].copy_from_slice(&0x0000u16.to_le_bytes());
    bad_plane[4..6].copy_from_slice(&0x0001u16.to_le_bytes());
    let response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, bad_plane, 0x42));
    assert_eq!(
        response[0].data,
        vec![cmd::GET_SUPPORTED_WIDECHARS, 17, 0, 0, 1, 0, 0x02, 0],
        "invalid code planes use the standard code-plane error bit and no ranges"
    );

    let mut inverted = fixed_command(cmd::GET_SUPPORTED_WIDECHARS);
    inverted[1] = 0;
    inverted[2..4].copy_from_slice(&0x0100u16.to_le_bytes());
    inverted[4..6].copy_from_slice(&0x0001u16.to_le_bytes());
    let response = server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, inverted, 0x42));
    assert_eq!(
        response[0].data,
        vec![cmd::GET_SUPPORTED_WIDECHARS, 0, 0, 1, 1, 0, 0x10, 0],
        "First > Last is reported as the standard any-other-error response"
    );
}

#[test]
fn vt_server_requires_memory_negotiation_before_object_pool_upload_state() {
    let transfer = minimal_object_pool_transfer();

    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();

    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer.clone(), 0x42))
            .is_empty()
    );
    assert!(
        server.clients().is_empty(),
        "object-pool transfer before memory negotiation must not create upload state"
    );

    let mut malformed_get_memory = fixed_command(cmd::GET_MEMORY);
    malformed_get_memory[1..5].copy_from_slice(&16u32.to_le_bytes());
    malformed_get_memory[5] = 0x00;
    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, malformed_get_memory, 0x42,))
            .is_empty(),
        "Get Memory requests with non-canonical reserved tail bytes must not open the upload window"
    );
    assert!(
        server.clients().is_empty(),
        "malformed Get Memory must not allocate client upload state"
    );

    let mut get_memory = [0xFFu8; 8];
    get_memory[0] = cmd::GET_MEMORY;
    assert_eq!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, get_memory.to_vec(), 0x42))
            .len(),
        1
    );
    assert_eq!(server.clients().len(), 1);

    assert!(
        server
            .handle_ecu_message(&Message::new(PGN_ECU_TO_VT, transfer, 0x42))
            .is_empty()
    );
    assert!(server.clients()[0].pool_uploaded);
    assert_eq!(server.clients()[0].pool.size(), 2);
}
