#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn object_id_checked_constructors_reject_unencodable_values() {
        assert_eq!(ObjectID::try_new_i32(0), Some(ObjectID(0)));
        assert_eq!(ObjectID::try_new_i32(u16::MAX as i32), Some(ObjectID::NULL));
        assert_eq!(ObjectID::try_new_i32(-1), None);
        assert_eq!(ObjectID::try_new_i32(u16::MAX as i32 + 1), None);
        assert_eq!(
            ObjectID::try_new_usize(u16::MAX as usize),
            Some(ObjectID::NULL)
        );
        assert_eq!(ObjectID::try_new_usize(u16::MAX as usize + 1), None);
    }

    #[test]
    fn object_type_round_trip() {
        for v in 0..=47u8 {
            let t = ObjectType::from_u8(v);
            assert_eq!(t.as_u8(), v);
        }
        // Out-of-range falls back to WorkingSet.
        assert_eq!(ObjectType::from_u8(0xFF), ObjectType::WorkingSet);
    }

    #[test]
    fn window_mask_round_trip() {
        let wm = WindowMaskBody {
            width_cells: 2,
            height_cells: 3,
            window_type: 1,
            background_color: 7,
            options: 0x03,
            name: ObjectID(100),
            window_title: ObjectID(200),
            window_icon: ObjectID(300),
            required_objects: vec![ObjectID(400)],
        };
        let bytes = wm.encode().unwrap();
        assert_eq!(bytes.len(), 14);
        assert_eq!(WindowMaskBody::decode(&bytes).unwrap(), wm);
        assert!(WindowMaskBody::decode(&bytes[..11]).is_err());
    }

    #[test]
    fn key_group_round_trip() {
        let kg = KeyGroupBody {
            options: 0x02,
            name: ObjectID(50),
            key_group_icon: ObjectID(60),
        };
        assert_eq!(KeyGroupBody::decode(&kg.encode()).unwrap(), kg);
    }

    #[test]
    fn key_round_trip() {
        let k = KeyBody {
            background_color: 10,
            key_code: 0xAB,
        };
        assert_eq!(KeyBody::decode(&k.encode()).unwrap(), k);
    }

    #[test]
    fn alarm_mask_round_trip() {
        let am = AlarmMaskBody {
            background_color: 5,
            soft_key_mask: ObjectID(9999),
            priority: 1,
            acoustic_signal: 2,
        };
        assert_eq!(AlarmMaskBody::decode(&am.encode().unwrap()).unwrap(), am);
        // Standard Alarm Mask fixed body is exactly 5 bytes (no options byte).
        assert_eq!(am.encode().unwrap().len(), 5);
        assert!(
            AlarmMaskBody { priority: 3, ..am }.encode().is_err(),
            "standard Alarm Mask priority accepts only high/medium/low"
        );
        assert!(
            AlarmMaskBody {
                acoustic_signal: 4,
                ..am
            }
            .encode()
            .is_err(),
            "standard Alarm Mask acoustic signal accepts only high/medium/low/none"
        );
        let mut invalid_priority = am.encode().unwrap();
        invalid_priority[3] = 3;
        assert!(AlarmMaskBody::decode(&invalid_priority).is_err());
        let mut invalid_acoustic = am.encode().unwrap();
        invalid_acoustic[4] = 4;
        assert!(AlarmMaskBody::decode(&invalid_acoustic).is_err());
    }

    #[test]
    fn additional_modeled_vt_bodies_round_trip_and_validate_reserved_fields() {
        let data_mask = DataMaskBody {
            background_color: 2,
            soft_key_mask: ObjectID(0x1234),
        };
        assert_eq!(
            DataMaskBody::decode(&data_mask.encode()).unwrap(),
            data_mask
        );

        let container = ContainerBody {
            width: 320,
            height: 240,
            hidden: true,
        };
        assert_eq!(
            ContainerBody::decode(&container.encode()).unwrap(),
            container
        );
        assert!(ContainerBody::decode(&[0, 0, 0, 0, 2]).is_err());

        let soft_key_mask = SoftKeyMaskBody {
            background_color: 9,
        };
        assert_eq!(
            SoftKeyMaskBody::decode(&soft_key_mask.encode()).unwrap(),
            soft_key_mask
        );

        let button = ButtonBody {
            width: 100,
            height: 50,
            background_color: 1,
            border_color: 2,
            key_code: 3,
            options: 0x3F,
        };
        assert_eq!(ButtonBody::decode(&button.encode()).unwrap(), button);
        let mut bad_button = button.encode();
        bad_button[7] = 0x80;
        assert!(ButtonBody::decode(&bad_button).is_err());

        let number = NumberVariableBody { value: 0x4433_2211 };
        assert_eq!(
            NumberVariableBody::decode(&number.encode()).unwrap(),
            number
        );

        let string = StringVariableBody {
            length: 3,
            value: b"abc".to_vec(),
        };
        assert_eq!(StringVariableBody::decode(&string.encode()), string);

        let font = FontAttributesBody {
            font_color: 4,
            font_size: 14,
            font_type: 1,
            font_style: 0x05,
        };
        assert_eq!(FontAttributesBody::decode(&font.encode()).unwrap(), font);
        assert!(FontAttributesBody::decode(&[0, 15, 0, 0]).is_err());

        let line = LineAttributesBody {
            line_color: 7,
            line_width: 2,
            line_art: 0xA55A,
        };
        assert_eq!(LineAttributesBody::decode(&line.encode()).unwrap(), line);

        let fill = FillAttributesBody {
            fill_type: 3,
            fill_color: 8,
            fill_pattern: ObjectID(0xCAFE),
        };
        assert_eq!(
            FillAttributesBody::decode(&fill.encode().unwrap()).unwrap(),
            fill
        );
        assert!(
            FillAttributesBody {
                fill_type: 4,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let input = InputAttributesBody {
            validation_type: 1,
            validation_string: b"0123456789".to_vec(),
        };
        assert_eq!(
            InputAttributesBody::decode(&input.encode().unwrap()).unwrap(),
            input
        );
        assert!(
            InputAttributesBody {
                validation_type: 2,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let pointer = ObjectPointerBody {
            value: ObjectID(44),
        };
        assert_eq!(
            ObjectPointerBody::decode(&pointer.encode()).unwrap(),
            pointer
        );
    }

    #[test]
    fn modeled_output_shape_bodies_round_trip_and_validate_reserved_fields() {
        let line = OutputLineBody {
            width: 100,
            height: 50,
            line_attributes: ObjectID(24),
            line_direction: 1,
        };
        assert_eq!(
            OutputLineBody::decode(&line.encode().unwrap()).unwrap(),
            line
        );
        assert!(
            OutputLineBody {
                line_direction: 2,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let rectangle = OutputRectangleBody {
            width: 200,
            height: 80,
            line_attributes: ObjectID(24),
            line_suppression: 0x0F,
            fill_attributes: ObjectID(25),
        };
        assert_eq!(
            OutputRectangleBody::decode(&rectangle.encode().unwrap()).unwrap(),
            rectangle
        );
        assert!(
            OutputRectangleBody {
                line_suppression: 0x10,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let ellipse = OutputEllipseBody {
            width: 90,
            height: 90,
            line_attributes: ObjectID(24),
            ellipse_type: 3,
            start_angle: 0,
            end_angle: 180,
            fill_attributes: ObjectID(25),
        };
        assert_eq!(
            OutputEllipseBody::decode(&ellipse.encode().unwrap()).unwrap(),
            ellipse
        );
        assert!(
            OutputEllipseBody {
                ellipse_type: 4,
                ..Default::default()
            }
            .encode()
            .is_err()
        );
        assert!(
            OutputEllipseBody {
                start_angle: 181,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let polygon = OutputPolygonBody {
            width: 100,
            height: 100,
            line_attributes: ObjectID(24),
            fill_attributes: ObjectID(25),
            polygon_type: 0,
            points: vec![
                PolygonPoint { x: 0, y: 0 },
                PolygonPoint { x: 100, y: 0 },
                PolygonPoint { x: 50, y: 100 },
            ],
        };
        assert_eq!(
            OutputPolygonBody::decode(&polygon.encode().unwrap()).unwrap(),
            polygon
        );
        assert!(
            OutputPolygonBody {
                polygon_type: 4,
                points: polygon.points.clone(),
                ..Default::default()
            }
            .encode()
            .is_err()
        );
        assert!(
            OutputPolygonBody {
                points: polygon.points[..2].to_vec(),
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let mut mismatched = polygon.encode().unwrap();
        mismatched[9] = 4;
        assert!(OutputPolygonBody::decode(&mismatched).is_err());
    }

    #[test]
    fn modeled_input_output_value_and_gauge_bodies_round_trip_and_validate_reserved_fields() {
        let boolean = InputBooleanBody {
            width: 30,
            background_color: 1,
            foreground: ObjectID(23),
            variable_reference: ObjectID(21),
            value: 1,
            enabled: 1,
        };
        assert_eq!(
            InputBooleanBody::decode(&boolean.encode().unwrap()).unwrap(),
            boolean
        );
        // Standard Input Boolean fixed body is exactly 9 bytes.
        assert_eq!(boolean.encode().unwrap().len(), 9);
        assert!(
            InputBooleanBody {
                value: 2,
                ..boolean
            }
            .encode()
            .is_err(),
            "Input Boolean value is a standard FALSE/TRUE field"
        );
        assert!(
            InputBooleanBody {
                enabled: 2,
                ..boolean
            }
            .encode()
            .is_err(),
            "Input Boolean enabled is a standard FALSE/TRUE field"
        );
        let mut bad_boolean = boolean.encode().unwrap();
        bad_boolean[7] = 2;
        assert!(InputBooleanBody::decode(&bad_boolean).is_err());
        bad_boolean[7] = 1;
        bad_boolean[8] = 2;
        assert!(InputBooleanBody::decode(&bad_boolean).is_err());

        let input_string = InputStringBody {
            width: 120,
            height: 20,
            background_color: 3,
            font_attributes: ObjectID(23),
            input_attributes: ObjectID(26),
            options: 0x03,
            variable_reference: ObjectID(22),
            max_length: 32,
            justification: 0x0A,
        };
        assert_eq!(
            InputStringBody::decode(&input_string.encode().unwrap()).unwrap(),
            input_string
        );
        assert!(
            InputStringBody {
                justification: 3,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let input_number = InputNumberBody {
            width: 90,
            height: 18,
            background_color: 4,
            font_attributes: ObjectID(23),
            options: 0x07,
            variable_reference: ObjectID(21),
            value: 42,
            min_value: -100,
            max_value: 100,
            offset: 10,
            scale: 2.0,
            number_of_decimals: 1,
            format: 1,
            justification: 0x05,
            options2: 0x03,
        };
        assert_eq!(
            InputNumberBody::decode(&input_number.encode().unwrap()).unwrap(),
            input_number
        );
        assert!(
            InputNumberBody {
                format: 2,
                ..Default::default()
            }
            .encode()
            .is_err()
        );
        assert!(
            InputNumberBody {
                min_value: 10,
                max_value: 0,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let input_list = InputListBody {
            width: 80,
            height: 100,
            variable_reference: ObjectID(21),
            value: 2,
            options: 1,
            items: vec![ObjectID(100), ObjectID(101), ObjectID(102)],
        };
        assert_eq!(
            InputListBody::decode(&input_list.encode().unwrap()).unwrap(),
            input_list
        );
        let mut bad_list = input_list.encode().unwrap();
        bad_list[7] = 4;
        assert!(InputListBody::decode(&bad_list).is_err());

        let output_string = OutputStringBody {
            width: 130,
            height: 20,
            background_color: 5,
            font_attributes: ObjectID(23),
            options: 0x03,
            variable_reference: ObjectID(22),
            justification: 0x04,
            value: b"speed".to_vec(),
        };
        assert_eq!(
            OutputStringBody::decode(&output_string.encode().unwrap()).unwrap(),
            output_string
        );
        let mut bad_string = output_string.encode().unwrap();
        bad_string[11] = 9;
        assert!(OutputStringBody::decode(&bad_string).is_err());

        let output_number = OutputNumberBody {
            width: 90,
            height: 18,
            background_color: 6,
            font_attributes: ObjectID(23),
            options: 0x07,
            variable_reference: ObjectID(21),
            value: 7,
            offset: -5,
            scale: 10.0,
            number_of_decimals: 2,
            format: 0,
            justification: 0x0A,
        };
        assert_eq!(
            OutputNumberBody::decode(&output_number.encode().unwrap()).unwrap(),
            output_number
        );
        assert!(
            OutputNumberBody {
                format: 2,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let meter = MeterBody {
            width: 70,
            needle_color: 1,
            border_color: 2,
            arc_and_tick_color: 3,
            options: 1,
            number_of_ticks: 10,
            start_angle: 20,
            end_angle: 160,
            min_value: 0,
            max_value: 250,
            variable_reference: ObjectID(21),
            value: 125,
        };
        assert_eq!(MeterBody::decode(&meter.encode().unwrap()).unwrap(), meter);
        assert!(
            MeterBody {
                start_angle: 181,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let linear = LinearBarGraphBody {
            width: 100,
            height: 12,
            color: 4,
            target_line_color: 5,
            options: 0x03,
            number_of_ticks: 5,
            min_value: 20,
            max_value: 80,
            variable_reference: ObjectID(21),
            value: 50,
            target_value_variable_reference: ObjectID(22),
            target_value: 60,
        };
        assert_eq!(
            LinearBarGraphBody::decode(&linear.encode().unwrap()).unwrap(),
            linear
        );
        assert!(
            LinearBarGraphBody {
                options: 0x40,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let arched = ArchedBarGraphBody {
            width: 120,
            height: 80,
            color: 6,
            target_line_color: 7,
            options: 0x03,
            start_angle: 10,
            end_angle: 170,
            bar_width: 24,
            min_value: 0,
            max_value: 100,
            variable_reference: ObjectID(21),
            value: 40,
            target_value_variable_reference: ObjectID(22),
            target_value: 60,
        };
        assert_eq!(
            ArchedBarGraphBody::decode(&arched.encode().unwrap()).unwrap(),
            arched
        );
        assert!(
            ArchedBarGraphBody {
                end_angle: 181,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let picture = PictureGraphicBody {
            width: 16,
            actual_width: 16,
            actual_height: 8,
            format: 2,
            options: 1,
            transparency: 0,
            data: vec![0xAA, 0xBB, 0xCC],
        };
        assert_eq!(
            PictureGraphicBody::decode(&picture.encode().unwrap()).unwrap(),
            picture
        );
        assert!(
            PictureGraphicBody {
                format: 3,
                ..Default::default()
            }
            .encode()
            .is_err()
        );
        assert!(
            PictureGraphicBody {
                options: 0x08,
                ..Default::default()
            }
            .encode()
            .is_err()
        );
    }

    #[test]
    fn remaining_vt_object_bodies_round_trip_and_validate_reserved_fields() {
        let ws = WorkingSetBody {
            background_colour: 7,
            selectable: 1,
            active_mask: ObjectID(2),
            languages: vec![[b'e', b'n']],
        };
        assert_eq!(WorkingSetBody::decode(&ws.encode()).unwrap(), ws);
        // Internal body keeps fixed fields plus modelled language codes.
        assert_eq!(ws.encode().len(), 6);
        assert!(WorkingSetBody::decode(&[0]).is_err());

        let aux_function = AuxFunctionBody {
            function_type: 2,
            options: 3,
            designator: ObjectID(33),
        };
        assert_eq!(
            AuxFunctionBody::decode(&aux_function.encode().unwrap()).unwrap(),
            aux_function
        );
        assert!(
            AuxFunctionBody {
                function_type: 3,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let aux_input = AuxInputBody {
            input_type: 1,
            input_id: 7,
            options: 1,
        };
        assert_eq!(
            AuxInputBody::decode(&aux_input.encode().unwrap()).unwrap(),
            aux_input
        );
        assert!(
            AuxInputBody {
                options: 2,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let aux_function2 = AuxFunction2Body {
            function_type: 1,
            function_attributes: 0xAA,
            name: ObjectID(50),
            icon: ObjectID(51),
        };
        assert_eq!(
            AuxFunction2Body::decode(&aux_function2.encode().unwrap()).unwrap(),
            aux_function2
        );

        let aux_input2 = AuxInput2Body {
            input_type: 2,
            input_id: 8,
            input_status: 3,
            input_value: 500,
            name: ObjectID(60),
        };
        assert_eq!(
            AuxInput2Body::decode(&aux_input2.encode().unwrap()).unwrap(),
            aux_input2
        );
        assert!(
            AuxInput2Body {
                input_status: 4,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let aux_designator = AuxControlDesignatorBody {
            aux_object: ObjectID(31),
            designator: b"boom".to_vec(),
        };
        assert_eq!(
            AuxControlDesignatorBody::decode(&aux_designator.encode().unwrap()).unwrap(),
            aux_designator
        );

        let graphic_data = GraphicDataBody {
            format: 0,
            options: 0,
            data: b"\x89PNG\r\n\x1a\n".to_vec(),
        };
        assert_eq!(
            GraphicDataBody::decode(&graphic_data.encode().unwrap()).unwrap(),
            graphic_data
        );
        assert_eq!(graphic_data.encode().unwrap().len(), 13);
        assert!(
            GraphicDataBody {
                format: 1,
                ..Default::default()
            }
            .encode()
            .is_err()
        );
        assert!(
            GraphicDataBody {
                options: 1,
                ..Default::default()
            }
            .encode()
            .is_err()
        );

        let scaled_graphic = ScaledGraphicBody {
            width: 64,
            height: 32,
            scale_type: 3,
            options: 1,
            value: ObjectID(36),
        };
        assert_eq!(
            ScaledGraphicBody::decode(&scaled_graphic.encode().unwrap()).unwrap(),
            scaled_graphic
        );

        let animation = AnimationBody {
            width: 80,
            height: 40,
            refresh_interval_ms: 100,
            value: 1,
            enabled: 1,
            first_child_index: 0,
            default_child_index: 0,
            last_child_index: 1,
            options: 1,
        };
        assert_eq!(
            AnimationBody::decode(&animation.encode().unwrap()).unwrap(),
            animation
        );
        let mut bad_animation = animation.encode().unwrap();
        bad_animation[11] = 0x80;
        assert!(AnimationBody::decode(&bad_animation).is_err());

        let colour_map = ColourMapBody {
            entries: vec![0, 1],
        };
        assert_eq!(
            ColourMapBody::decode(&colour_map.encode().unwrap()).unwrap(),
            colour_map
        );

        let graphic_context = GraphicContextBody {
            viewport_width: 100,
            viewport_height: 80,
            viewport_x: -5,
            viewport_y: 7,
            canvas_width: 200,
            canvas_height: 160,
            viewport_zoom_raw: 1.5f32.to_bits(),
            cursor_x: -3,
            cursor_y: 4,
            foreground_colour: 9,
            background_colour: 2,
            font_attributes: ObjectID::new(23),
            line_attributes: ObjectID::new(24),
            fill_attributes: ObjectID::new(25),
            format: 2,
            options: 3,
            transparency_colour: 0,
        };
        assert_eq!(graphic_context.encode().unwrap().len(), 31);
        assert_eq!(
            GraphicContextBody::decode(&graphic_context.encode().unwrap()).unwrap(),
            graphic_context
        );
        let mut bad_graphic_context = graphic_context.encode().unwrap();
        bad_graphic_context[29] = 0x04;
        assert!(GraphicContextBody::decode(&bad_graphic_context).is_err());

        let external_ref = ExternalReferenceNameBody {
            options: 1,
            name0: 0x1122_3344,
            name1: 0x5566_7788,
        };
        assert_eq!(
            ExternalReferenceNameBody::decode(&external_ref.encode()).unwrap(),
            external_ref
        );

        let external_pointer = ExternalObjectPointerBody {
            default_object_id: ObjectID(1),
            external_reference_name: ObjectID(42),
            external_object_id: ObjectID(99),
        };
        assert_eq!(
            ExternalObjectPointerBody::decode(&external_pointer.encode()).unwrap(),
            external_pointer
        );

        let palette = ColourPaletteBody {
            options: 0,
            entries_argb: vec![0xFF_00_00_00, 0x80_12_34_56, 0xFF_FF_FF_FF],
        };
        assert_eq!(
            palette.encode().unwrap(),
            vec![
                0x00, // options
                0x03, 0x00, // ARGB count
                0x00, 0x00, 0x00, 0xFF, // B, G, R, A
                0x56, 0x34, 0x12, 0x80, 0xFF, 0xFF, 0xFF, 0xFF,
            ]
        );
        assert_eq!(
            ColourPaletteBody::decode(&palette.encode().unwrap()).unwrap(),
            palette
        );
        assert!(
            ColourPaletteBody {
                options: 1,
                entries_argb: Vec::new(),
            }
            .encode()
            .is_err()
        );
        assert!(ColourPaletteBody::decode(&[0x01, 0x00, 0x00]).is_err());
        assert!(ColourPaletteBody::decode(&[0x00, 0x01, 0x01]).is_err());
        assert!(
            ColourPaletteBody::decode(&[
                0x00, // options
                0x01, 0x00, // one ARGB entry declared
                0x00, 0x00, 0x00, // but only three bytes supplied
            ])
            .is_err()
        );

        let graphics_context = GraphicsContextBody {
            context: GraphicsContextV6 {
                transparency: 200,
                line_style: 1,
                line_width: 3,
                fill_color_rgb: 0x12_34_56,
                line_color_rgb: 0x65_43_21,
                anti_aliasing: true,
                blend_mode: 2,
            },
        };
        assert_eq!(
            GraphicsContextBody::decode(&graphics_context.encode()).unwrap(),
            graphics_context
        );

        let label_ref = ObjectLabelRefBody {
            labels: vec![ObjectLabelRefEntry {
                labelled_object: ObjectID(46),
                string_variable: ObjectID(47),
                font_type: 2,
                graphic_designator: ObjectID::NULL,
            }],
        };
        assert_eq!(
            ObjectLabelRefBody::decode(&label_ref.encode()).unwrap(),
            label_ref
        );

        assert_eq!(
            create_graphics_context(45, &graphics_context)
                .get_graphics_context_body()
                .unwrap(),
            graphics_context
        );
    }

    #[test]
    fn modeled_child_bodies_split_at_known_offsets() {
        let data_mask = create_data_mask(
            1,
            &DataMaskBody {
                background_color: 3,
                soft_key_mask: ObjectID(100),
            },
        )
        .with_children([2u16]);
        let container = create_container(
            2,
            &ContainerBody {
                width: 20,
                height: 10,
                hidden: false,
            },
        )
        .with_children([3u16]);
        let soft_key_mask = create_soft_key_mask(
            3,
            &SoftKeyMaskBody {
                background_color: 4,
            },
        )
        .with_children([4u16]);
        let button = create_button(
            4,
            &ButtonBody {
                width: 10,
                height: 5,
                background_color: 1,
                border_color: 2,
                key_code: 7,
                options: 0,
            },
        );

        for obj in [data_mask, container, soft_key_mask, button] {
            let round = ObjectPool::deserialize(&obj.serialize().unwrap())
                .unwrap()
                .objects()
                .first()
                .unwrap()
                .clone();
            assert_eq!(round.r#type, obj.r#type);
            assert_eq!(round.body, obj.body);
            assert_eq!(round.children, obj.children);
        }
    }

    #[test]
    fn macro_round_trip_with_string_value() {
        // One Hide/Show command (fixed 8 bytes total) and one
        // Change String Value (variable).
        let str_payload = b"hi";
        let mut change_string = vec![0xB3, 0x10, 0x00];
        change_string.push(str_payload.len() as u8);
        change_string.push(0);
        change_string.extend_from_slice(str_payload);
        // Hide/Show is a fixed 8-byte command (7 parameter bytes), then a
        // variable Change String Value command.
        let commands: Vec<u8> = vec![0xA0, 1, 2, 3, 4, 5, 6, 7]
            .into_iter()
            .chain(change_string)
            .collect();
        // ISO 11783-6 Macro body is `[num_bytes:u16][commands…]`.
        let mut macro_body_bytes = (commands.len() as u16).to_le_bytes().to_vec();
        macro_body_bytes.extend_from_slice(&commands);

        let mb = MacroBody::decode(&macro_body_bytes).unwrap();
        assert_eq!(mb.commands.len(), 2);
        assert_eq!(mb.commands[0].command_type, 0xA0);
        assert_eq!(mb.commands[0].parameters, vec![1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(mb.commands[1].command_type, 0xB3);

        let round = mb.encode();
        assert_eq!(round, macro_body_bytes);
    }

    #[test]
    fn macro_change_child_position_uses_nine_byte_command_length() {
        let commands: Vec<u8> = vec![
            0xB4, // Change Child Position
            2, 0, // parent
            3, 0, // child
            0xFE, 0xFF, // x = -2
            9, 0, // y = 9
        ];
        let mut macro_body_bytes = (commands.len() as u16).to_le_bytes().to_vec();
        macro_body_bytes.extend_from_slice(&commands);

        let mb = MacroBody::decode(&macro_body_bytes).unwrap();
        assert_eq!(MacroCommand::get_command_length(0xB4), 9);
        assert_eq!(mb.commands.len(), 1);
        assert_eq!(mb.commands[0].command_type, 0xB4);
        assert_eq!(mb.commands[0].parameters, commands[1..].to_vec());
        assert_eq!(mb.encode(), macro_body_bytes);
    }

    #[test]
    fn macro_render_effect_command_lengths_include_polygon_and_colour_map() {
        assert_eq!(MacroCommand::get_command_length(0xB6), 8);
        assert_eq!(MacroCommand::get_command_length(0xB7), 8);
        assert_eq!(MacroCommand::get_command_length(0xBA), 8);
    }

    #[test]
    fn macro_decode_unknown_command_errors() {
        let bytes = vec![0x12, 0x00, 0x00];
        assert!(MacroBody::decode(&bytes).is_err());
    }

    #[test]
    fn vt_object_serialize_layout() {
        let obj = VTObject::default()
            .with_id(0x1234)
            .with_type(ObjectType::Key)
            .with_body(vec![1, 2, 3])
            .with_children(vec![10, 20]);
        let bytes = obj.serialize().unwrap();
        // [id_lo id_hi][type][body...][num_obj:u8][num_macros:u8][child records...]
        // (ISO 11783-6: both counts precede both lists; no length prefix).
        assert_eq!(bytes[0..2], [0x34, 0x12]);
        assert_eq!(bytes[2], ObjectType::Key.as_u8());
        assert_eq!(&bytes[3..6], &[1, 2, 3]);
        assert_eq!(bytes[6], 2); // num_objects (u8)
        assert_eq!(bytes[7], 0); // num_macros (u8)
        // First child record: oid=10 at (0,0) → [0x0A,0x00, 0,0, 0,0].
        assert_eq!(&bytes[8..14], &[0x0A, 0x00, 0, 0, 0, 0]);
        // Second child record: oid=20 at (0,0).
        assert_eq!(&bytes[14..20], &[0x14, 0x00, 0, 0, 0, 0]);
        assert_eq!(bytes.len(), 20);
    }

    #[test]
    fn working_set_serializes_standard_counts_before_children_macros_and_languages() {
        let mut obj = create_working_set(
            1,
            &WorkingSetBody {
                background_colour: 7,
                selectable: 1,
                active_mask: ObjectID(2),
                languages: vec![[b'e', b'n'], [b'd', b'e']],
            },
        )
        .with_children_pos([ChildRef::new(ObjectID(2), 3, 4)]);
        obj.macros.push(MacroRef::new(5, 6));

        let bytes = obj.serialize().unwrap();
        assert_eq!(bytes[0..3], [1, 0, ObjectType::WorkingSet.as_u8()]);
        assert_eq!(&bytes[3..7], &[7, 1, 2, 0]);
        assert_eq!(bytes[7], 1, "object count");
        assert_eq!(bytes[8], 1, "macro count");
        assert_eq!(bytes[9], 2, "language count");
        assert_eq!(&bytes[10..16], &[2, 0, 3, 0, 4, 0]);
        assert_eq!(&bytes[16..18], &[5, 6]);
        assert_eq!(&bytes[18..22], b"ende");

        let decoded = ObjectPool::deserialize(&bytes)
            .unwrap()
            .objects()
            .first()
            .unwrap()
            .clone();
        assert_eq!(decoded.children_pos, vec![ChildRef::new(ObjectID(2), 3, 4)]);
        assert_eq!(decoded.macros, vec![MacroRef::new(5, 6)]);
        assert_eq!(
            decoded.get_working_set_body().unwrap().languages,
            vec![[b'e', b'n'], [b'd', b'e']]
        );
        assert_eq!(decoded.serialize().unwrap(), bytes);
    }

    #[test]
    fn every_object_type_round_trips_without_length_prefix() {
        // One object of every ObjectType, each with a representative body
        // (and, for parents, a child + macro tail) so the parse-by-type
        // walker is exercised for all 48 types. The pool must survive a
        // serialize → deserialize cycle byte-for-byte, which proves
        // `object_body_total_len` agrees with every `encode()`.
        let leaf = |id: u16, t: ObjectType, body: Vec<u8>| {
            VTObject::default().with_id(id).with_type(t).with_body(body)
        };
        let parent = |id: u16, t: ObjectType, body: Vec<u8>, rec6: bool| {
            let child = if rec6 {
                ChildRef::new(ObjectID(900), 3, -4)
            } else {
                ChildRef::at_origin(ObjectID(900))
            };
            let mut o = VTObject::default()
                .with_id(id)
                .with_type(t)
                .with_body(body)
                .with_children_pos(vec![child]);
            o.add_macro(1, 7);
            o
        };

        let objects = vec![
            // Parents (6-byte child records).
            parent(
                1,
                ObjectType::WorkingSet,
                WorkingSetBody::default().encode(),
                true,
            ),
            parent(
                2,
                ObjectType::DataMask,
                DataMaskBody::default().encode(),
                true,
            ),
            parent(
                3,
                ObjectType::AlarmMask,
                AlarmMaskBody::default().encode().unwrap(),
                true,
            ),
            parent(
                4,
                ObjectType::Container,
                ContainerBody::default().encode(),
                true,
            ),
            parent(5, ObjectType::Key, KeyBody::default().encode(), true),
            parent(6, ObjectType::Button, ButtonBody::default().encode(), true),
            parent(
                7,
                ObjectType::WindowMask,
                WindowMaskBody::default().encode().unwrap(),
                true,
            ),
            // Parents (OID-only child records).
            parent(
                8,
                ObjectType::SoftKeyMask,
                SoftKeyMaskBody::default().encode().to_vec(),
                false,
            ),
            parent(
                9,
                ObjectType::KeyGroup,
                KeyGroupBody::default().encode(),
                false,
            ),
            // Fixed leaves.
            leaf(
                10,
                ObjectType::InputBoolean,
                InputBooleanBody::default().encode().unwrap(),
            ),
            leaf(
                11,
                ObjectType::InputString,
                InputStringBody::default().encode().unwrap(),
            ),
            leaf(
                12,
                ObjectType::InputNumber,
                InputNumberBody::default().encode().unwrap(),
            ),
            leaf(
                13,
                ObjectType::OutputNumber,
                OutputNumberBody::default().encode().unwrap(),
            ),
            leaf(
                14,
                ObjectType::Line,
                OutputLineBody::default().encode().unwrap(),
            ),
            leaf(
                15,
                ObjectType::Rectangle,
                OutputRectangleBody::default().encode().unwrap(),
            ),
            leaf(
                16,
                ObjectType::Ellipse,
                OutputEllipseBody::default().encode().unwrap(),
            ),
            leaf(
                17,
                ObjectType::Meter,
                MeterBody::default().encode().unwrap(),
            ),
            leaf(
                18,
                ObjectType::LinearBarGraph,
                LinearBarGraphBody::default().encode().unwrap(),
            ),
            leaf(
                19,
                ObjectType::ArchedBarGraph,
                ArchedBarGraphBody::default().encode().unwrap(),
            ),
            leaf(
                20,
                ObjectType::NumberVariable,
                NumberVariableBody::default().encode(),
            ),
            leaf(
                21,
                ObjectType::FontAttributes,
                FontAttributesBody::default().encode(),
            ),
            leaf(
                22,
                ObjectType::LineAttributes,
                LineAttributesBody::default().encode(),
            ),
            leaf(
                23,
                ObjectType::FillAttributes,
                FillAttributesBody::default().encode().unwrap(),
            ),
            leaf(
                24,
                ObjectType::ObjectPointer,
                ObjectPointerBody::default().encode(),
            ),
            leaf(
                25,
                ObjectType::AuxFunction,
                AuxFunctionBody::default().encode().unwrap(),
            ),
            leaf(
                26,
                ObjectType::AuxInput,
                AuxInputBody::default().encode().unwrap(),
            ),
            leaf(
                27,
                ObjectType::AuxFunction2,
                AuxFunction2Body::default().encode().unwrap(),
            ),
            leaf(
                28,
                ObjectType::AuxInput2,
                AuxInput2Body::default().encode().unwrap(),
            ),
            leaf(
                29,
                ObjectType::ScaledGraphic,
                ScaledGraphicBody::default().encode().unwrap(),
            ),
            leaf(
                30,
                ObjectType::GraphicContext,
                GraphicContextBody::default().encode().unwrap(),
            ),
            leaf(
                31,
                ObjectType::ExternalObjectDefinition,
                ExternalObjectDefinitionBody::default().encode().unwrap(),
            ),
            leaf(
                32,
                ObjectType::ExternalObjectPointer,
                ExternalObjectPointerBody::default().encode(),
            ),
            leaf(
                33,
                ObjectType::GraphicsContext,
                GraphicsContextBody::default().encode(),
            ),
            leaf(
                34,
                ObjectType::ObjectLabelRef,
                ObjectLabelRefBody::default().encode(),
            ),
            leaf(
                35,
                ObjectType::ScaledBitmap,
                ScaledBitmapBody::default().encode().unwrap(),
            ),
            // Variable leaves.
            leaf(
                36,
                ObjectType::InputList,
                InputListBody {
                    items: vec![ObjectID(7), ObjectID(8)],
                    ..Default::default()
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                37,
                ObjectType::OutputString,
                OutputStringBody {
                    value: b"hi".to_vec(),
                    ..Default::default()
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                38,
                ObjectType::Polygon,
                OutputPolygonBody {
                    points: vec![
                        PolygonPoint { x: 1, y: 2 },
                        PolygonPoint { x: 3, y: 4 },
                        PolygonPoint { x: 5, y: 6 },
                    ],
                    ..Default::default()
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                39,
                ObjectType::PictureGraphic,
                PictureGraphicBody {
                    data: vec![1, 2, 3],
                    ..Default::default()
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                40,
                ObjectType::StringVariable,
                StringVariableBody {
                    length: 3,
                    value: b"abc".to_vec(),
                }
                .encode(),
            ),
            leaf(
                41,
                ObjectType::AuxControlDesig,
                AuxControlDesignatorBody {
                    aux_object: ObjectID(5),
                    designator: b"x".to_vec(),
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                42,
                ObjectType::GraphicData,
                GraphicDataBody {
                    data: vec![9, 9],
                    ..Default::default()
                }
                .encode()
                .unwrap(),
            ),
            create_animation(
                43,
                &AnimationBody {
                    enabled: 1,
                    last_child_index: 1,
                    ..Default::default()
                },
            )
            .unwrap()
            .with_children_pos(vec![
                ChildRef::at_origin(ObjectID(1)),
                ChildRef::new(ObjectID(2), 3, 4),
            ]),
            leaf(
                44,
                ObjectType::ColourMap,
                ColourMapBody {
                    entries: vec![0, 1],
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                45,
                ObjectType::ExternalReferenceName,
                ExternalReferenceNameBody {
                    options: 1,
                    name0: 0x1122_3344,
                    name1: 0x5566_7788,
                }
                .encode(),
            ),
            leaf(
                46,
                ObjectType::ColourPalette,
                ColourPaletteBody {
                    options: 0,
                    entries_argb: vec![0xFF_10_20_30, 0x80_40_50_60],
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                47,
                ObjectType::InputAttributes,
                InputAttributesBody {
                    validation_type: 0,
                    validation_string: b"AB".to_vec(),
                }
                .encode()
                .unwrap(),
            ),
            leaf(
                48,
                ObjectType::Macro,
                MacroBody {
                    commands: vec![MacroCommand {
                        command_type: 0xA0,
                        parameters: vec![1, 2, 3, 4, 5],
                    }],
                }
                .encode(),
            ),
        ];

        let mut pool = ObjectPool::default();
        for o in objects {
            pool.add(o).unwrap();
        }
        let bytes = pool.serialize().unwrap();
        let decoded = ObjectPool::deserialize(&bytes).unwrap();
        assert_eq!(decoded, pool, "every object type must survive a round trip");
        assert_eq!(
            decoded.serialize().unwrap(),
            bytes,
            "round-trip must be byte-identical"
        );
        // 48 distinct object-type variants exercised.
        assert_eq!(pool.size(), 48);
    }

    #[test]
    fn vt_object_macro_list_round_trip() {
        // ISO 11783-6 parent tail carries a macro reference list after
        // the child list: `[num_macros:u8][num × (event:u8, macro:u8)]`.
        let mut obj = VTObject::default()
            .with_id(1)
            .with_type(ObjectType::DataMask)
            .with_body(vec![0, 0xFF, 0xFF])
            .with_children_pos(vec![ChildRef::at_origin(ObjectID(10))]);
        obj.add_macro(1, 42);
        let bytes = obj.serialize().unwrap();
        let pool = ObjectPool::deserialize(&bytes).unwrap();
        let again = pool.find(1).unwrap();
        assert_eq!(again.macros.len(), 1);
        assert_eq!(again.macros[0], MacroRef::new(1, 42));
    }

    #[test]
    fn vt_object_softkeymask_uses_oid_only_child_records() {
        // Soft Key Mask child records are 2 bytes (OID only), not 6.
        let obj = VTObject::default()
            .with_id(1)
            .with_type(ObjectType::SoftKeyMask)
            .with_body(vec![0]) // background
            .with_children_pos(vec![ChildRef::at_origin(ObjectID(5))]);
        let bytes = obj.serialize().unwrap();
        let pool = ObjectPool::deserialize(&bytes).unwrap();
        let again = pool.find(1).unwrap();
        assert_eq!(again.children_pos.len(), 1);
        assert_eq!(again.children_pos[0].id, ObjectID(5));
    }

    #[test]
    fn vt_object_serialize_child_positions_round_trip() {
        // ISO 11783-6 child locations are signed 16-bit; verify they
        // survive a serialize → deserialize cycle, including negatives.
        let obj = VTObject::default()
            .with_id(1)
            .with_type(ObjectType::DataMask)
            .with_body(vec![0, 0xFF, 0xFF]) // bg=0, soft_key_mask=NULL
            .with_children_pos(vec![
                ChildRef::new(ObjectID(10), 100, 200),
                ChildRef::new(ObjectID(11), -5, -10),
            ]);
        let bytes = obj.serialize().unwrap();
        let pool = ObjectPool::deserialize(&bytes).unwrap();
        let again = pool.find(1).unwrap();
        assert_eq!(again.children_pos.len(), 2);
        assert_eq!(again.children_pos[0], ChildRef::new(ObjectID(10), 100, 200));
        assert_eq!(again.children_pos[1], ChildRef::new(ObjectID(11), -5, -10));
    }

    #[test]
    fn vt_object_serialize_rejects_unencodable_child_counts() {
        // The on-wire format has no per-object length prefix, but parent
        // child/macro counts are still u8 fields: too many children must
        // be rejected rather than silently truncated.
        let too_many_children = create_working_set(2, &WorkingSetBody::default())
            .with_children((0..=u16::MAX).map(ObjectID).collect::<Vec<_>>());
        assert!(too_many_children.serialize().is_err());
    }

    #[test]
    fn pool_add_dup_id_errors() {
        let mut p = ObjectPool::default();
        p.add(VTObject::default().with_id(1)).unwrap();
        assert!(p.add(VTObject::default().with_id(1)).is_err());
    }

    #[test]
    fn pool_validate_requires_working_set() {
        let p = ObjectPool::default();
        assert!(p.validate().is_err());

        let p = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]));
        // Missing the referenced child.
        assert!(p.validate().is_err());
    }

    #[test]
    fn pool_validate_happy_path() {
        let p = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()));
        p.validate().unwrap();
    }

    #[test]
    fn pool_deserialize_restores_children_from_child_list_tail() {
        let p = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(2, &DataMaskBody::default()));
        let bytes = p.serialize().unwrap();
        let restored = ObjectPool::deserialize(&bytes).unwrap();
        assert_eq!(restored, p);
        restored.validate().unwrap();
    }

    #[test]
    fn pool_deserialize_uses_known_body_lengths_before_child_tail() {
        let obj = create_key(
            7,
            &KeyBody {
                background_color: 0xAA,
                key_code: 2,
            },
        )
        .with_children(vec![8]);
        let bytes = obj.serialize().unwrap();
        let restored = ObjectPool::deserialize(&bytes).unwrap();
        let restored_key = restored.find(7).unwrap();
        assert_eq!(restored_key.body, vec![0xAA, 2]);
        assert_eq!(restored_key.children, vec![ObjectID(8)]);
        assert_eq!(restored.serialize().unwrap(), bytes);
    }

    #[test]
    fn pool_serialize_deserialize_round_trip() {
        let p = ObjectPool::default()
            .with_object(create_working_set(1, &WorkingSetBody::default()).with_children([2u16]))
            .with_object(create_data_mask(
                2,
                &DataMaskBody {
                    background_color: 0xAA,
                    soft_key_mask: ObjectID::NULL,
                },
            ));
        let bytes = p.serialize().unwrap();
        let restored = ObjectPool::deserialize(&bytes).unwrap();
        assert_eq!(restored.size(), 2);
        assert_eq!(restored.find(2).unwrap().body, vec![0xAA, 0xFF, 0xFF]);
    }

    #[test]
    fn pool_deserialize_trailing_partial_header_errors() {
        assert!(ObjectPool::deserialize(&[0x01, 0x00, ObjectType::WorkingSet.as_u8()]).is_err());

        let mut bytes = ObjectPool::default()
            .with_object(create_data_mask(1, &DataMaskBody::default()))
            .serialize()
            .unwrap();
        bytes.extend_from_slice(&[0x02, 0x00]);
        assert!(ObjectPool::deserialize(&bytes).is_err());
    }

    #[test]
    fn pool_deserialize_body_len_past_end_errors() {
        let bytes = [
            0x01,
            0x00,
            ObjectType::WorkingSet.as_u8(),
            0x04,
            0x00,
            0xAA,
            0xBB,
        ];
        assert!(ObjectPool::deserialize(&bytes).is_err());
    }

    #[test]
    fn pool_deserialize_unknown_object_type_errors() {
        let bytes = [0x01, 0x00, 0xFE, 0x00, 0x00];
        let err = ObjectPool::deserialize(&bytes).unwrap_err();
        assert_eq!(err.code, ErrorCode::PoolValidation);
    }

    #[test]
    fn pool_deserialize_malformed_known_child_tail_errors() {
        let bytes = [
            0x07,
            0x00,
            ObjectType::Key.as_u8(),
            0x05,
            0x00,
            0xAA,
            0x02,
            0x00,
            0x01,
            0x00,
        ];
        let err = ObjectPool::deserialize(&bytes).unwrap_err();
        assert_eq!(err.code, ErrorCode::PoolValidation);
    }

    #[test]
    fn touch_gesture_round_trip() {
        let g = TouchGesture {
            r#type: GestureType::Tap,
            x: -100,
            y: 200,
            duration_ms: 0,
            distance: 0,
            scale: 1.0,
            rotation_deg: 0.0,
            touch_count: 1,
            target_object: ObjectID(0xCAFE),
        };
        let bytes = g.encode();
        let decoded = TouchGesture::decode(&bytes).unwrap();
        assert_eq!(decoded.r#type, GestureType::Tap);
        assert_eq!(decoded.x, -100);
        assert_eq!(decoded.y, 200);
        assert_eq!(decoded.target_object, ObjectID(0xCAFE));
        assert!(TouchGesture::decode(&bytes[..11]).is_none());
        let mut bad_type = bytes;
        bad_type[0] = 0xFF;
        assert!(TouchGesture::decode(&bad_type).is_none());
    }

    #[test]
    fn graphics_context_v6_round_trip() {
        let g = GraphicsContextV6 {
            transparency: 200,
            line_style: 1,
            line_width: 5,
            fill_color_rgb: 0x12_34_56,
            line_color_rgb: 0x78_9A_BC,
            anti_aliasing: true,
            blend_mode: 2,
        };
        let bytes = g.encode();
        let d = GraphicsContextV6::decode(&bytes).unwrap();
        assert_eq!(d.transparency, 200);
        assert_eq!(d.line_width, 5);
        assert_eq!(d.fill_color_rgb, 0x12_34_56);
        assert_eq!(d.line_color_rgb, 0x78_9A_BC);
        assert!(d.anti_aliasing);
        assert_eq!(d.blend_mode, 2);
        assert!(GraphicsContextV6::decode(&bytes[..11]).is_none());
        let mut bad_line = bytes.clone();
        bad_line[1] = 3;
        assert!(GraphicsContextV6::decode(&bad_line).is_none());
        let mut bad_aa = bytes.clone();
        bad_aa[10] = 2;
        assert!(GraphicsContextV6::decode(&bad_aa).is_none());
        let mut bad_blend = bytes;
        bad_blend[11] = 4;
        assert!(GraphicsContextV6::decode(&bad_blend).is_none());
    }

    #[test]
    fn external_object_definition_round_trip() {
        let e = ExternalObjectDefinitionBody {
            options: 0x01,
            name0: 0x1122_3344,
            name1: 0x5566_7788,
            object_ids: vec![ObjectID(100), ObjectID::NULL],
        };
        assert_eq!(
            ExternalObjectDefinitionBody::decode(&e.encode().unwrap()).unwrap(),
            e
        );
    }

    #[test]
    fn scaled_bitmap_round_trip() {
        let s = ScaledBitmapBody {
            width: 640,
            height: 480,
            scale_x: 1.5,
            scale_y: 2.0,
            offset_x: -10,
            offset_y: 20,
            format: 3,
            options: 1,
            bitmap_data: ObjectID(5),
        };
        let bytes = s.encode().unwrap();
        let d = ScaledBitmapBody::decode(&bytes).unwrap();
        assert_eq!(d.width, 640);
        assert_eq!(d.height, 480);
        assert!((d.scale_x - 1.5).abs() < 1e-3);
        assert!((d.scale_y - 2.0).abs() < 1e-3);
        assert_eq!(d.offset_x, -10);
        assert_eq!(d.offset_y, 20);
        assert_eq!(d.format, 3);
        assert_eq!(d.bitmap_data, ObjectID(5));
    }

    #[test]
    fn scaled_bitmap_rejects_unencodable_scale_and_reserved_fields() {
        let mut s = ScaledBitmapBody {
            scale_x: 255.0,
            scale_y: 1.0,
            format: 3,
            options: 1,
            ..Default::default()
        };
        assert_eq!(
            ScaledBitmapBody::decode(&s.encode().unwrap())
                .unwrap()
                .scale_x,
            255.0
        );

        s.scale_x = f32::NAN;
        assert!(s.encode().is_err());
        s.scale_x = -0.1;
        assert!(s.encode().is_err());
        s.scale_x = 256.0;
        assert!(s.encode().is_err());

        s.scale_x = 1.0;
        s.format = 4;
        assert!(s.encode().is_err());
        s.format = 3;
        s.options = 0x02;
        assert!(s.encode().is_err());

        let mut bad = ScaledBitmapBody {
            format: 3,
            ..Default::default()
        }
        .encode()
        .unwrap();
        bad[12] = 4;
        assert!(ScaledBitmapBody::decode(&bad).is_err());
        bad[12] = 3;
        bad[13] = 0x02;
        assert!(ScaledBitmapBody::decode(&bad).is_err());
    }

    #[test]
    fn colour_palette_standard_has_16_entries() {
        let p = ColourPalette::create_standard_v6();
        assert_eq!(p.entries.len(), 16);
        assert_eq!(p.entries[0].rgb, 0x00_00_00);
        assert_eq!(p.entries[1].rgb, 0xFF_FF_FF);
    }

    #[test]
    fn create_helpers_set_type() {
        let k = create_key(7, &KeyBody::default());
        assert_eq!(k.id, 7);
        assert_eq!(k.r#type, ObjectType::Key);
    }

    use proptest::prelude::*;

    proptest! {
        #[test]
        fn proptest_object_pool_deserialize_arbitrary_bytes_is_bounded(
            data in proptest::collection::vec(any::<u8>(), 0..=512),
        ) {
            if let Ok(pool) = ObjectPool::deserialize(&data) {
                let serialized = pool.serialize().unwrap();
                prop_assert_eq!(serialized.len(), data.len());
                prop_assert!(pool.size() <= data.len() / 5);
            }
        }

        #[test]
        fn proptest_macro_body_decode_arbitrary_bytes_does_not_panic(
            data in proptest::collection::vec(any::<u8>(), 0..=512),
        ) {
            if let Ok(macro_body) = MacroBody::decode(&data) {
                prop_assert_eq!(macro_body.encode(), data);
            }
        }
    }
}
