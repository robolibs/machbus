#[test]
fn vt_server_visual_graphics_and_macro_commands_require_uploaded_objects() {
    let mut server = VTServer::new(VTServerConfig::default());
    server.start().unwrap();
    activate_reference_pool(&mut server, 0x42);

    let change_size = |object_id: u16, width: u16, height: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_SIZE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3..5].copy_from_slice(&width.to_le_bytes());
        data[5..7].copy_from_slice(&height.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_size(5, 64, 32), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_size(20, 70, 24), 0x42));
    assert_eq!(
        server.clients()[0].object_state.sizes.get(&ObjectID(5)),
        Some(&(64, 32))
    );
    assert_eq!(
        server.clients()[0].object_state.sizes.get(&ObjectID(20)),
        Some(&(70, 24)),
        "Output List has width/height fields and accepts Change Size"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_size(0x7777, 1, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, change_size(6, 1, 1), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .sizes
            .contains_key(&ObjectID(0x7777))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .sizes
            .contains_key(&ObjectID(6)),
        "Change Size must reject style/reference objects without size fields"
    );

    let change_end_point = |object_id: u16, width: u16, height: u16, direction: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_END_POINT;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3..5].copy_from_slice(&width.to_le_bytes());
        data[5..7].copy_from_slice(&height.to_le_bytes());
        data[7] = direction;
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_end_point(21, 44, 12, 1),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .endpoints
            .get(&ObjectID(21)),
        Some(&(44, 12, 1))
    );
    assert!(server.clients()[0].object_state.accepted_effects.contains(
        &ServerRenderEffect::ChangeEndPoint {
            id: ObjectID(21),
            width: 44,
            height: 12,
            line_direction: 1,
        }
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_end_point(21, 45, 13, 2),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_end_point(5, 46, 14, 0),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .endpoints
            .get(&ObjectID(21)),
        Some(&(44, 12, 1)),
        "Change End Point must reject reserved line directions and wrong target object types"
    );

    let background = |object_id: u16, colour: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_BACKGROUND_COLOUR;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = colour;
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, background(5, 7), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .background_colours
            .get(&ObjectID(5)),
        Some(&7)
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, background(0x7777, 9), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, background(21, 9), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .background_colours
            .contains_key(&ObjectID(0x7777))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .background_colours
            .contains_key(&ObjectID(21)),
        "Change Background Colour must reject objects without a background field"
    );

    let label = |object_id: u16, label_string: u16, font_type: u8, graphic: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_OBJECT_LABEL;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3..5].copy_from_slice(&label_string.to_le_bytes());
        data[5] = font_type;
        data[6..8].copy_from_slice(&graphic.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, label(5, 14, 1, 0xFFFF), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .object_labels
            .get(&ObjectID(5))
            .copied(),
        Some(machbus::isobus::vt::ObjectLabelState {
            string_variable: ObjectID(14),
            font_type: 1,
            graphic_designator: ObjectID::NULL
        })
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, label(5, 14, 2, 5), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .object_labels
            .get(&ObjectID(5))
            .copied(),
        Some(machbus::isobus::vt::ObjectLabelState {
            string_variable: ObjectID(14),
            font_type: 2,
            graphic_designator: ObjectID(5)
        }),
        "Change Object Label accepts output objects as graphic designators"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, label(5, 14, 3, 0xFFFF), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .object_labels
            .get(&ObjectID(5))
            .copied(),
        Some(machbus::isobus::vt::ObjectLabelState {
            string_variable: ObjectID(14),
            font_type: 2,
            graphic_designator: ObjectID(5)
        }),
        "Change Object Label must reject reserved Annex K font-type values before retaining label metadata"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        label(5, ObjectID::NULL.raw(), 3, ObjectID::NULL.raw()),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .object_labels
            .get(&ObjectID(5))
            .copied(),
        Some(machbus::isobus::vt::ObjectLabelState {
            string_variable: ObjectID(14),
            font_type: 2,
            graphic_designator: ObjectID(5)
        }),
        "Change Object Label must reject reserved Annex K font-type values even when the string reference is NULL"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, label(5, 14, 3, 14), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .object_labels
            .get(&ObjectID(5))
            .copied(),
        Some(machbus::isobus::vt::ObjectLabelState {
            string_variable: ObjectID(14),
            font_type: 2,
            graphic_designator: ObjectID(5)
        }),
        "String Variables are not valid Object Label graphic designators and must not replace the accepted label"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        label(0x7777, 14, 1, 0xFFFF),
        0x42,
    ));
    assert!(
        !server.clients()[0]
            .object_state
            .object_labels
            .contains_key(&ObjectID(0x7777))
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, label(4, 14, 1, 0xFFFF), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .object_labels
            .contains_key(&ObjectID(4)),
        "Change Object Label must target an object admitted by the Object Label Reference List"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        label(5, 0x7777, 1, 0xFFFF),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .object_labels
            .get(&ObjectID(5))
            .map(|label| label.string_variable),
        Some(ObjectID(14)),
        "invalid label-string references must not replace the accepted label"
    );

    let polygon_point = |object_id: u16, index: u8| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_POLYGON_POINT;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = index;
        data[4..6].copy_from_slice(&0x0011u16.to_le_bytes());
        data[6..8].copy_from_slice(&0x0022u16.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, polygon_point(9, 1), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .polygon_points
            .get(&(ObjectID(9), 1)),
        Some(&(0x0011, 0x0022))
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, polygon_point(5, 2), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, polygon_point(0x7777, 3), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, polygon_point(9, 3), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .polygon_points
            .contains_key(&(ObjectID(5), 2))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .polygon_points
            .contains_key(&(ObjectID(0x7777), 3))
    );
    assert!(
        !server.clients()[0]
            .object_state
            .polygon_points
            .contains_key(&(ObjectID(9), 3)),
        "polygon point mutation must reject indexes outside the uploaded point list"
    );

    let colour_map = |object_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::SELECT_COLOUR_MAP;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, colour_map(10), 0x42));
    assert_eq!(
        server.clients()[0].object_state.selected_colour_map,
        ObjectID(10)
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, colour_map(13), 0x42));
    assert_eq!(
        server.clients()[0].object_state.selected_colour_palette,
        Some(ObjectID(13))
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        colour_map(ObjectID::NULL.0),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.selected_colour_map,
        ObjectID::NULL
    );
    assert_eq!(
        server.clients()[0].object_state.selected_colour_palette,
        Some(ObjectID::NULL)
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, colour_map(10), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, colour_map(11), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, colour_map(0x7777), 0x42));
    assert_eq!(
        server.clients()[0].object_state.selected_colour_map,
        ObjectID(10),
        "colour-map selection must ignore wrong-type and unknown references"
    );

    let change_attribute = |object_id: u16, attribute_id: u8, value: u32| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::CHANGE_ATTRIBUTE;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data[3] = attribute_id;
        data[4..8].copy_from_slice(&value.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(16, 2, 10),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.selected_colour_map,
        ObjectID(10),
        "Working Set Special Controls colour-map attribute drives retained render state"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(16, 3, ObjectID::NULL.raw() as u32),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.selected_colour_palette,
        Some(ObjectID::NULL),
        "Working Set Special Controls colour-palette attribute drives retained render state"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(13, 1, 0),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(6, 3, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(6, 2, 8),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(6, 4, 0x80),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(6, 2, 32),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(13), 1)),
        Some(&0),
        "Colour Palette options AID is standard but reserved to zero"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 3)),
        Some(&7),
        "Font Attributes font type AID must accept standard font type values"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 2)),
        Some(&32),
        "Font Attributes proportional font size AID must admit standard height values once proportional style is selected"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 4)),
        Some(&0x80),
        "Font Attributes font style AID must retain the proportional style bit when the current size is valid"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(30, 10, 18),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(30, 11, 77),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(31, 12, 18),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(31, 13, 88),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(9, 1, 80),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 13, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 13, ObjectID::NULL.raw() as u32),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(23, 2, 0x07),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(8, 3, 23),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 1, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 2, 5),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(7, 3, 0x55AA),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(30), 10)),
        Some(&18),
        "Linear Bar Graph target-value variable reference AID must be accepted"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(30), 11)),
        Some(&77),
        "Linear Bar Graph target value AID must be accepted"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(31), 12)),
        Some(&18),
        "Arched Bar Graph target-value variable reference AID must be accepted"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(31), 13)),
        Some(&88),
        "Arched Bar Graph target value AID must be accepted"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(9), 1)),
        Some(&80),
        "Polygon AID 1 is width; line-attribute reference validation belongs to AID 3"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(11), 13)),
        Some(&(ObjectID::NULL.raw() as u32)),
        "Graphic Context style references admit typed objects and the standard NULL selector"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(23), 2)),
        Some(&0x03),
        "Picture Graphic Options Change Attribute must ignore the static raw/RLE bit while retaining transparency/flashing bits"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(8), 3)),
        Some(&23),
        "Fill Attributes pattern AID must accept PictureGraphic references"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(40), 1)),
        Some(&1),
        "Window Mask width AID must accept standard 1..=2 user-layout column values"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(40), 2)),
        Some(&5),
        "Window Mask height AID must accept standard 1..=6 user-layout row values"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(7), 3)),
        Some(&0x55AA),
        "Line Attributes line-art AID is a two-byte pattern and must not be treated as a one-byte scalar"
    );
    assert!(server.clients()[0].object_state.accepted_effects.contains(
        &ServerRenderEffect::ChangeGenericAttribute {
            id: ObjectID(30),
            attribute_id: 10,
            value: 18,
        }
    ));
    assert!(server.clients()[0].object_state.accepted_effects.contains(
        &ServerRenderEffect::ChangeGenericAttribute {
            id: ObjectID(31),
            attribute_id: 12,
            value: 18,
        }
    ));
    assert!(server.clients()[0].object_state.accepted_effects.contains(
        &ServerRenderEffect::ChangeGenericAttribute {
            id: ObjectID(23),
            attribute_id: 2,
            value: 0x03,
        }
    ));
    assert!(server.clients()[0].object_state.accepted_effects.contains(
        &ServerRenderEffect::ChangeGenericAttribute {
            id: ObjectID(7),
            attribute_id: 3,
            value: 0x55AA,
        }
    ));
    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(16, 1, 10),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(18, 1, 1234),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(19, 1, 5),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(34, 3, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(17, 1, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(6, 3, 3),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(6, 2, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(6, 4, 0),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(35, 1, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 1, 32768),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 3, 0x0001_0000),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 5, 20),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 6, 20),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 8, 0x0001_0000),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 7, f32::NAN.to_bits()),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(11, 14, 5),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(8, 3, 5),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(39, 5, 2),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(39, 6, 2),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(39, 3, 5),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 3, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 6, 18),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 7, 18),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 8, 18),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(44, 2, 18),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(44, 3, 18),
        0x42,
    ));
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(16), 1)),
        "Change Attribute must reject unsupported AIDs instead of recording inert render state"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(18), 1)),
        "Number Variable Value is changed through Change Numeric Value, not Change Attribute"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(19), 1)),
        "Object Pointer target value is changed through Change Numeric Value, not Change Attribute"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(34), 3)),
        "Key objects only expose standard Change Attribute AIDs 1 and 2; AID 3 belongs to Key Group options, not Key"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(17), 1)),
        "Input Attributes validation type is readable but not Change Attribute mutable; Change String Value updates only the validation string"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(35), 1)),
        "Extended Input Attributes only exposes Get Attribute Value for validation type, not Change Attribute mutation"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(44), 2)),
        "Key Group Name AID must reference OutputString or ObjectPointer to OutputString"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(44), 3)),
        "Key Group Icon AID must reference an Object Label graphic representation object"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 3)),
        Some(&7),
        "Font Attributes font type AID must reject reserved values without overwriting retained state"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 4)),
        Some(&0x80),
        "Font Attributes font style AID must reject changes that would make the retained proportional size invalid"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(6), 2)),
        Some(&32),
        "Font Attributes font size AID must reject proportional sizes below 8 without overwriting retained state"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(11), 1)),
        "Graphic Context viewport/canvas dimensions must stay inside the signed 15-bit range"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(11), 3)),
        "Graphic Context viewport position payload must fit the standard two-byte signed field"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(11), 5)),
        "Graphic Context canvas width is read-only and cannot be changed with Change Attribute"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(11), 6)),
        "Graphic Context canvas height is read-only and cannot be changed with Change Attribute"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(11), 8)),
        "Graphic Context cursor position payload must fit the standard two-byte signed field"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(11), 7)),
        "Graphic Context zoom must be finite and inside the supported standard range"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(11), 14)),
        "Graphic Context fill selector must reject non-FillAttributes references"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(8), 3)),
        Some(&23),
        "Fill Attributes pattern AID must reject non-PictureGraphic references without overwriting the retained pattern"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(39), 5)),
        Some(&2),
        "Input Boolean value (AID 5) is settable via Change Attribute, matching the reference VT stack"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(39), 6)),
        Some(&2),
        "Input Boolean enabled (AID 6) is settable via Change Attribute, matching the reference VT stack"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(39), 3)),
        "Input Boolean foreground AID must reference FontAttributes, not arbitrary objects"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(40), 3)),
        "Window Mask type AID must reject values whose required-object list does not match the retained body"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(40), 6)),
        "Window Mask Name AID must reference OutputString or ObjectPointer to OutputString"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(40), 7)),
        "Window Mask Title AID must reference OutputString or ObjectPointer to OutputString"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(40), 8)),
        "Window Mask Icon AID must reference an Object Label graphic representation object"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count + 2,
        "the two accepted Input Boolean value/enabled Change Attributes append render replay effects; the rejected commands do not"
    );
    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(5, 4, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(5, 6, 18),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(20, 3, 14),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(30, 10, 14),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(31, 12, 14),
        0x42,
    ));
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(5), 4)),
        "Output String font-attribute AID must reject non-FontAttributes references"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(5), 6)),
        "Output String variable-reference AID must reject non-StringVariable references"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(20), 3)),
        "Output List variable-reference AID must reject non-NumberVariable references"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(30), 10)),
        Some(&18),
        "Linear Bar Graph target-value reference AID must reject non-NumberVariable references"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(31), 12)),
        Some(&18),
        "Arched Bar Graph target-value reference AID must reject non-NumberVariable references"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count,
        "rejected reference-valued Change Attribute commands must not append replay effects"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(24, 6, 0x04),
        0x42,
    ));
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(24), 6)),
        Some(&0x04),
        "Input String Options is a scalar bitfield AID, not a StringVariable reference"
    );
    let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(5, 5, 0x80),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(5, 7, 3),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(24, 6, 0x08),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(24, 8, 3),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(25, 7, 101),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(25, 8, u32::MAX),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(25, 10, f32::NAN.to_bits()),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(25, 11, 8),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(25, 12, 2),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(43, 9, 8),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(25, 15, 0x04),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(13, 1, 1),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(23, 2, 0x08),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 1, 0),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(40, 2, 7),
        0x42,
    ));
    let additional_rejected_scalar_attributes = [
        (2, 1, 0x0100),      // Data Mask background colour is one byte, not a truncating u32.
        (22, 4, 4),          // Alarm Mask acoustic signal admits only high/medium/low/none.
        (5, 4, 0x0001_0006), // Output String font reference is two bytes, not a truncating u32.
        (26, 3, 2),          // Container hidden state is boolean.
        (40, 3, 19),         // Window Mask type admits the standard range 0..=18.
        (40, 5, 0x04),       // Window Mask options reserves bits 2..=7.
        (8, 1, 4),           // Fill type admits 0..=3.
        (21, 4, 2),          // Output Line direction is 0 or 1.
        (27, 4, 0x10),       // Rectangle line-suppression has four bits.
        (28, 4, 4),          // Ellipse type admits 0..=3.
        (28, 5, 181),        // Ellipse start angle is a half-degree angle.
        (28, 6, 181),        // Ellipse end angle is a half-degree angle.
        (9, 5, 4),           // Polygon type admits 0..=3.
        (29, 5, 2),          // Meter options has only bit 0.
        (29, 7, 181),        // Meter start angle is a half-degree angle.
        (29, 8, 181),        // Meter end angle is a half-degree angle.
        (30, 5, 0x40),       // Linear Bar Graph options reserves bits 6..=7.
        (31, 5, 0x20),       // Arched Bar Graph options reserves bits 5..=7.
        (31, 6, 181),        // Arched Bar Graph start angle is a half-degree angle.
        (31, 7, 181),        // Arched Bar Graph end angle is a half-degree angle.
        (32, 4, 2),          // Animation selected child index must exist or be 255.
        (32, 5, 2),          // Animation enabled state is boolean.
        (32, 6, 2),          // Animation first child index must exist and not exceed current last.
        (32, 7, 2),          // Animation default child index must exist.
        (32, 8, 2),          // Animation last child index must exist and follow current first.
        (32, 9, 8),          // Animation options reserves bits 3..=7.
        (11, 16, 4),         // Graphic Context options reserves bits 2..=7.
    ];
    for (object_id, attribute_id, value) in additional_rejected_scalar_attributes {
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            change_attribute(object_id, attribute_id, value),
            0x42,
        ));
    }
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        change_attribute(7, 3, 0x0001_55AA),
        0x42,
    ));
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(5), 5)),
        "Output String option reserved bits must be rejected before retained state mutation"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(5), 7)),
        "Output String justification must reject reserved values"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(24), 6)),
        Some(&0x04),
        "rejected Input String options must not replace the accepted value"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(24), 8)),
        "Input String justification must reject reserved values"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(25), 7)),
        "Input Number minimum must not be raised above the uploaded maximum"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(25), 8)),
        "Input Number maximum must not be lowered below the uploaded minimum"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(25), 11)),
        "Input Number number of decimals must reject reserved values"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(25), 12)),
        "Input Number format must reject reserved values"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(43), 9)),
        "Output Number number of decimals must reject reserved values"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(25), 10)),
        "Input Number scale must reject non-finite values"
    );
    assert!(
        !server.clients()[0]
            .object_state
            .attributes
            .contains_key(&(ObjectID(25), 15)),
        "Input Number Options 2 must reject reserved bits"
    );
    for (object_id, attribute_id, _) in additional_rejected_scalar_attributes {
        assert!(
            !server.clients()[0]
                .object_state
                .attributes
                .contains_key(&(ObjectID(object_id), attribute_id)),
            "Change Attribute must reject reserved scalar value for object {object_id} AID {attribute_id}"
        );
    }
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(13), 1)),
        Some(&0),
        "rejected Colour Palette options must not replace the accepted zero value"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(23), 2)),
        Some(&0x03),
        "rejected Picture Graphic options must not replace the accepted transparent/flashing bits"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(40), 1)),
        Some(&1),
        "rejected Window Mask width must not replace the accepted user-layout column count"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(40), 2)),
        Some(&5),
        "rejected Window Mask height must not replace the accepted user-layout row count"
    );
    assert_eq!(
        server.clients()[0]
            .object_state
            .attributes
            .get(&(ObjectID(7), 3)),
        Some(&0x55AA),
        "Line Attributes line-art AID must reject non-zero upper Change Attribute bytes without overwriting the retained two-byte pattern"
    );
    assert_eq!(
        server.clients()[0].object_state.accepted_effects.len(),
        accepted_effect_count,
        "rejected scalar Change Attribute values must not append replay effects"
    );

    {
        let (object_id, min_attribute, max_attribute) = (25, 7, 8);
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            change_attribute(object_id, min_attribute, 80),
            0x42,
        ));
        assert_eq!(
            server.clients()[0]
                .object_state
                .attributes
                .get(&(ObjectID(object_id), min_attribute)),
            Some(&80),
            "valid min update for object {object_id} should be retained before the follow-up max check"
        );
        let accepted_effect_count = server.clients()[0].object_state.accepted_effects.len();
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            change_attribute(object_id, max_attribute, 70),
            0x42,
        ));
        assert!(
            !server.clients()[0]
                .object_state
                .attributes
                .contains_key(&(ObjectID(object_id), max_attribute)),
            "max update for object {object_id} must compare against the already-retained min attribute"
        );
        assert_eq!(
            server.clients()[0].object_state.accepted_effects.len(),
            accepted_effect_count,
            "rejected state-relative max update for object {object_id} must not append replay effects"
        );
    }

    for (object_id, min_attribute, max_attribute) in [(29, 9, 10), (30, 7, 8), (31, 9, 10)] {
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            change_attribute(object_id, min_attribute, 80),
            0x42,
        ));
        server.handle_ecu_message(&Message::new(
            PGN_ECU_TO_VT,
            change_attribute(object_id, max_attribute, 70),
            0x42,
        ));
        assert_eq!(
            server.clients()[0]
                .object_state
                .attributes
                .get(&(ObjectID(object_id), min_attribute)),
            Some(&80),
            "Output Meter/Bar Graph min update for object {object_id} should be retained even when later not less than max"
        );
        assert_eq!(
            server.clients()[0]
                .object_state
                .attributes
                .get(&(ObjectID(object_id), max_attribute)),
            Some(&70),
            "Output Meter/Bar Graph max update for object {object_id} should be retained; render-time clamping handles min>=max"
        );
    }

    let graphics_draw_text = |object_id: u16| {
        let mut data = vec![cmd::GRAPHICS_CONTEXT];
        data.extend(object_id.to_le_bytes());
        data.extend([0x0D, 1, 2, b'o', b'k']);
        data
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, graphics_draw_text(11), 0x42));
    assert_eq!(server.clients()[0].object_state.graphics_contexts.len(), 1);
    assert_eq!(
        server.clients()[0].object_state.graphics_contexts[0].object_id,
        ObjectID(11)
    );
    assert_eq!(
        server.clients()[0].object_state.graphics_contexts[0].payload,
        vec![1, 2, b'o', b'k']
    );
    assert!(matches!(
        server.clients()[0].object_state.accepted_effects.last(),
        Some(ServerRenderEffect::GraphicsContext {
            id,
            subcommand: 0x0D,
            payload,
        }) if *id == ObjectID(11) && payload == &vec![1, 2, b'o', b'k']
    ));
    let graphics_ref = |object_id: u16, subcommand: u8, reference: u16| {
        let mut data = vec![cmd::GRAPHICS_CONTEXT];
        data.extend(object_id.to_le_bytes());
        data.push(subcommand);
        data.extend(reference.to_le_bytes());
        data
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x04, 7),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.graphics_contexts.len(),
        2,
        "valid Graphics Context line-attribute references are retained"
    );
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x12, 5),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x13, 23),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.graphics_contexts.len(),
        4,
        "valid Graphics Context Draw VT Object and Copy Canvas references are retained"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, graphics_draw_text(5), 0x42));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_draw_text(0x7777),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x00, 1, 0],
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        vec![cmd::GRAPHICS_CONTEXT, 11, 0, 0x15],
        0x42,
    ));
    let invalid_zoom = |object_id: u16, zoom: f32| {
        let mut data = vec![cmd::GRAPHICS_CONTEXT];
        data.extend(object_id.to_le_bytes());
        data.push(0x0F);
        data.extend(zoom.to_bits().to_le_bytes());
        data
    };
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        invalid_zoom(11, f32::NAN),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, invalid_zoom(11, 33.0), 0x42));
    let mut invalid_pan_zoom = vec![cmd::GRAPHICS_CONTEXT];
    invalid_pan_zoom.extend(11u16.to_le_bytes());
    invalid_pan_zoom.push(0x10);
    invalid_pan_zoom.extend([1, 0, 2, 0]);
    invalid_pan_zoom.extend(0.0f32.to_bits().to_le_bytes());
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, invalid_pan_zoom, 0x42));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x04, 8),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x05, 7),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x06, 8),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x12, 11),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x12, 14),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x12, 45),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x13, ObjectID::NULL.raw()),
        0x42,
    ));
    server.handle_ecu_message(&Message::new(
        PGN_ECU_TO_VT,
        graphics_ref(11, 0x13, 5),
        0x42,
    ));
    assert_eq!(
        server.clients()[0].object_state.graphics_contexts.len(),
        4,
        "graphics-context commands must ignore wrong-type target objects, unknown subcommands, malformed known payloads, non-canonical zooms, NULL/non-drawable Draw VT Object references, ScaledBitmap compatibility targets, and wrong-type Picture Graphic references"
    );

    let lock_mask = |object_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::LOCK_UNLOCK_MASK;
        data[1] = 1;
        data[2..4].copy_from_slice(&object_id.to_le_bytes());
        data[4..6].copy_from_slice(&100u16.to_le_bytes());
        data.to_vec()
    };
    let unlock_mask = |object_id: u16| {
        let mut data = lock_mask(object_id);
        data[1] = 0;
        data
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, lock_mask(2), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .mask_locks
            .get(&ObjectID(2)),
        Some(&machbus::isobus::vt::MaskLockState {
            locked: true,
            timeout_ms: 100,
        })
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, unlock_mask(2), 0x42));
    assert_eq!(
        server.clients()[0]
            .object_state
            .mask_locks
            .get(&ObjectID(2)),
        Some(&machbus::isobus::vt::MaskLockState {
            locked: false,
            timeout_ms: 100,
        })
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, lock_mask(3), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .mask_locks
            .contains_key(&ObjectID(3)),
        "Lock/Unlock Mask does not apply to Soft Key Masks"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, lock_mask(5), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .mask_locks
            .contains_key(&ObjectID(5)),
        "Lock/Unlock Mask must target a data/user-layout mask object"
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, lock_mask(0x7777), 0x42));
    assert!(
        !server.clients()[0]
            .object_state
            .mask_locks
            .contains_key(&ObjectID(0x7777))
    );

    let execute_macro = |object_id: u16| {
        let mut data = [0xFFu8; 8];
        data[0] = cmd::EXECUTE_MACRO;
        data[1..3].copy_from_slice(&object_id.to_le_bytes());
        data.to_vec()
    };
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, execute_macro(12), 0x42));
    assert_eq!(
        server.clients()[0].object_state.executed_macros,
        vec![ObjectID(12)]
    );
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, execute_macro(5), 0x42));
    server.handle_ecu_message(&Message::new(PGN_ECU_TO_VT, execute_macro(0x7777), 0x42));
    assert_eq!(
        server.clients()[0].object_state.executed_macros,
        vec![ObjectID(12)],
        "macro execution must ignore wrong-type and unknown references"
    );
}
