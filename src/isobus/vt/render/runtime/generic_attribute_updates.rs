fn apply_generic_attribute_to_pool(
    pool: &mut ObjectPool,
    id: ObjectID,
    attribute_id: u8,
    value: u32,
) -> Result<bool> {
    if let Some(obj) = pool.find(id)
        && obj.r#type == ObjectType::FillAttributes
        && !fill_attribute_generic_change_is_valid(pool, obj, attribute_id, value)
    {
        return Ok(false);
    }
    let Some(obj) = pool.find_mut(id) else {
        return Ok(false);
    };
    match obj.r#type {
        ObjectType::DataMask => {
            let mut body = obj.get_data_mask_body()?;
            let changed = match attribute_id {
                1 => {
                    body.background_color = low_u8(value);
                    true
                }
                2 => {
                    body.soft_key_mask = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::AlarmMask => {
            let mut body = obj.get_alarm_mask_body()?;
            let changed = match attribute_id {
                1 => {
                    body.background_color = low_u8(value);
                    true
                }
                2 => {
                    body.soft_key_mask = ObjectID(low_u16(value));
                    true
                }
                3 => {
                    body.priority = low_u8(value);
                    true
                }
                4 => {
                    body.acoustic_signal = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::WindowMask => {
            let mut body = obj.get_window_mask_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width_cells = low_u8(value);
                    true
                }
                2 => {
                    body.height_cells = low_u8(value);
                    true
                }
                3 => {
                    body.window_type = low_u8(value);
                    true
                }
                4 => {
                    body.background_color = low_u8(value);
                    true
                }
                5 => {
                    body.options = low_u8(value);
                    true
                }
                6 => {
                    body.name = ObjectID(low_u16(value));
                    true
                }
                7 => {
                    body.window_title = ObjectID(low_u16(value));
                    true
                }
                8 => {
                    body.window_icon = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Container => Ok(false),
        ObjectType::SoftKeyMask => {
            let mut body = obj.get_soft_key_mask_body()?;
            let changed = match attribute_id {
                1 => {
                    body.background_color = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::Key => {
            let mut body = obj.get_key_body()?;
            let changed = match attribute_id {
                1 => {
                    body.background_color = low_u8(value);
                    true
                }
                2 => {
                    body.key_code = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::KeyGroup => {
            let mut body = obj.get_key_group_body()?;
            let changed = match attribute_id {
                1 => {
                    body.options = low_u8(value);
                    true
                }
                2 => {
                    body.name = ObjectID(low_u16(value));
                    true
                }
                3 => {
                    body.key_group_icon = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::ExternalObjectDefinition => {
            let mut body = obj.get_external_object_definition_body()?;
            let changed = match attribute_id {
                1 => {
                    body.options = low_u8(value);
                    true
                }
                2 => {
                    body.name0 = value;
                    true
                }
                3 => {
                    body.name1 = value;
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::ExternalReferenceName => {
            let mut body = obj.get_external_reference_name_body()?;
            let changed = match attribute_id {
                1 => {
                    body.options = low_u8(value);
                    true
                }
                2 => {
                    body.name0 = value;
                    true
                }
                3 => {
                    body.name1 = value;
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::ExternalObjectPointer => {
            let mut body = obj.get_external_object_pointer_body()?;
            let changed = match attribute_id {
                1 => {
                    body.default_object_id = ObjectID(low_u16(value));
                    true
                }
                2 => {
                    body.external_reference_name = ObjectID(low_u16(value));
                    true
                }
                3 => {
                    body.external_object_id = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::OutputString => {
            let mut body = obj.get_output_string_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.background_color = low_u8(value);
                    true
                }
                4 => {
                    body.font_attributes = ObjectID(low_u16(value));
                    true
                }
                5 => {
                    body.options = low_u8(value);
                    true
                }
                6 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                7 => {
                    body.justification = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::OutputNumber => {
            let mut body = obj.get_output_number_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.background_color = low_u8(value);
                    true
                }
                4 => {
                    body.font_attributes = ObjectID(low_u16(value));
                    true
                }
                5 => {
                    body.options = low_u8(value);
                    true
                }
                6 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                7 => {
                    body.offset = value as i32;
                    true
                }
                8 => {
                    body.scale = f32::from_bits(value);
                    true
                }
                9 => {
                    body.number_of_decimals = low_u8(value);
                    true
                }
                10 => {
                    body.format = low_u8(value);
                    true
                }
                11 => {
                    body.justification = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::OutputList => {
            let mut body = obj.get_output_list_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                4 => {
                    // ISO Output List value (selected index, AID 4) via Change Attribute.
                    body.value = value.min(u32::from(u8::MAX)) as u8;
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::InputBoolean => {
            let mut body = obj.get_input_boolean_body()?;
            let changed = match attribute_id {
                1 => {
                    body.background_color = low_u8(value);
                    true
                }
                2 => {
                    body.width = low_u16(value);
                    true
                }
                3 => {
                    body.foreground = ObjectID(low_u16(value));
                    true
                }
                4 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                5 => {
                    // ISO Input Boolean value (AID 5) via Change Attribute.
                    body.value = u8::from(value != 0);
                    true
                }
                6 => {
                    // ISO Input Boolean enabled (AID 6) via Change Attribute.
                    body.enabled = u8::from(value != 0);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::InputString => {
            let mut body = obj.get_input_string_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.background_color = low_u8(value);
                    true
                }
                4 => {
                    body.font_attributes = ObjectID(low_u16(value));
                    true
                }
                5 => {
                    body.input_attributes = ObjectID(low_u16(value));
                    true
                }
                6 => {
                    body.options = low_u8(value);
                    true
                }
                7 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                8 => {
                    body.justification = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::InputNumber => {
            let mut body = obj.get_input_number_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.background_color = low_u8(value);
                    true
                }
                4 => {
                    body.font_attributes = ObjectID(low_u16(value));
                    true
                }
                5 => {
                    body.options = low_u8(value);
                    true
                }
                6 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                7 => {
                    body.min_value = value as i32;
                    true
                }
                8 => {
                    body.max_value = value as i32;
                    true
                }
                9 => {
                    body.offset = value as i32;
                    true
                }
                10 => {
                    body.scale = f32::from_bits(value);
                    true
                }
                11 => {
                    body.number_of_decimals = low_u8(value);
                    true
                }
                12 => {
                    body.format = low_u8(value);
                    true
                }
                13 => {
                    body.justification = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::InputList => {
            let mut body = obj.get_input_list_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                4 => {
                    // ISO Input List value (selected index, AID 4) via Change Attribute.
                    body.value = low_u8(value);
                    true
                }
                5 => {
                    // ISO Input List options (AID 5) via Change Attribute.
                    body.options = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::FontAttributes => {
            let mut body = obj.get_font_attributes_body()?;
            let changed = match attribute_id {
                1 => {
                    body.font_color = low_u8(value);
                    true
                }
                2 => {
                    body.font_size = low_u8(value);
                    true
                }
                3 => {
                    body.font_type = low_u8(value);
                    true
                }
                4 => {
                    body.font_style = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::LineAttributes => {
            let mut body = obj.get_line_attributes_body()?;
            let changed = match attribute_id {
                1 => {
                    body.line_color = low_u8(value);
                    true
                }
                2 => {
                    body.line_width = low_u8(value);
                    true
                }
                3 => {
                    body.line_art = low_u16(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::FillAttributes => {
            let mut body = obj.get_fill_attributes_body()?;
            let changed = match attribute_id {
                1 => {
                    body.fill_type = low_u8(value);
                    true
                }
                2 => {
                    body.fill_color = low_u8(value);
                    true
                }
                3 => {
                    body.fill_pattern = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Line => {
            let mut body = obj.get_output_line_body()?;
            let changed = match attribute_id {
                1 => {
                    body.line_attributes = ObjectID(low_u16(value));
                    true
                }
                2 => {
                    body.width = low_u16(value);
                    true
                }
                3 => {
                    body.height = low_u16(value);
                    true
                }
                4 => {
                    body.line_direction = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Rectangle => {
            let mut body = obj.get_output_rectangle_body()?;
            let changed = match attribute_id {
                1 => {
                    body.line_attributes = ObjectID(low_u16(value));
                    true
                }
                2 => {
                    body.width = low_u16(value);
                    true
                }
                3 => {
                    body.height = low_u16(value);
                    true
                }
                4 => {
                    body.line_suppression = low_u8(value);
                    true
                }
                5 => {
                    body.fill_attributes = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Ellipse => {
            let mut body = obj.get_output_ellipse_body()?;
            let changed = match attribute_id {
                1 => {
                    body.line_attributes = ObjectID(low_u16(value));
                    true
                }
                2 => {
                    body.width = low_u16(value);
                    true
                }
                3 => {
                    body.height = low_u16(value);
                    true
                }
                4 => {
                    body.ellipse_type = low_u8(value);
                    true
                }
                5 => {
                    body.start_angle = low_u8(value);
                    true
                }
                6 => {
                    body.end_angle = low_u8(value);
                    true
                }
                7 => {
                    body.fill_attributes = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Polygon => {
            let mut body = obj.get_output_polygon_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.line_attributes = ObjectID(low_u16(value));
                    true
                }
                4 => {
                    body.fill_attributes = ObjectID(low_u16(value));
                    true
                }
                5 => {
                    body.polygon_type = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Meter => {
            let mut body = obj.get_meter_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.needle_color = low_u8(value);
                    true
                }
                3 => {
                    body.border_color = low_u8(value);
                    true
                }
                4 => {
                    body.arc_and_tick_color = low_u8(value);
                    true
                }
                5 => {
                    body.options = low_u8(value);
                    true
                }
                6 => {
                    body.number_of_ticks = low_u8(value);
                    true
                }
                7 => {
                    body.start_angle = low_u8(value);
                    true
                }
                8 => {
                    body.end_angle = low_u8(value);
                    true
                }
                9 => {
                    body.min_value = low_u16(value);
                    true
                }
                10 => {
                    body.max_value = low_u16(value);
                    true
                }
                11 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                12 => {
                    // ISO Output Meter value (AID 12) via Change Attribute.
                    body.value = value.min(u32::from(u16::MAX)) as u16;
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::LinearBarGraph => {
            let mut body = obj.get_linear_bar_graph_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.color = low_u8(value);
                    true
                }
                4 => {
                    body.target_line_color = low_u8(value);
                    true
                }
                5 => {
                    body.options = low_u8(value);
                    true
                }
                6 => {
                    body.number_of_ticks = low_u8(value);
                    true
                }
                7 => {
                    body.min_value = low_u16(value);
                    true
                }
                8 => {
                    body.max_value = low_u16(value);
                    true
                }
                9 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                10 => {
                    body.target_value_variable_reference = ObjectID(low_u16(value));
                    true
                }
                11 => {
                    body.target_value = low_u16(value);
                    true
                }
                12 => {
                    // ISO Output Linear Bar Graph value (AID 12) via Change Attribute.
                    body.value = value.min(u32::from(u16::MAX)) as u16;
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::ArchedBarGraph => {
            let mut body = obj.get_arched_bar_graph_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.color = low_u8(value);
                    true
                }
                4 => {
                    body.target_line_color = low_u8(value);
                    true
                }
                5 => {
                    body.options = low_u8(value);
                    true
                }
                6 => {
                    body.start_angle = low_u8(value);
                    true
                }
                7 => {
                    body.end_angle = low_u8(value);
                    true
                }
                8 => {
                    body.bar_width = low_u16(value);
                    true
                }
                9 => {
                    body.min_value = low_u16(value);
                    true
                }
                10 => {
                    body.max_value = low_u16(value);
                    true
                }
                11 => {
                    body.variable_reference = ObjectID(low_u16(value));
                    true
                }
                12 => {
                    body.target_value_variable_reference = ObjectID(low_u16(value));
                    true
                }
                13 => {
                    body.target_value = low_u16(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Button => {
            let mut body = obj.get_button_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.background_color = low_u8(value);
                    true
                }
                4 => {
                    body.border_color = low_u8(value);
                    true
                }
                5 => {
                    body.key_code = low_u8(value);
                    true
                }
                6 => {
                    // ISO 11783-6 marks the latchable bit as static: a
                    // Change Attribute attempting to alter it is ignored by
                    // the VT, while the current state / border bits remain
                    // runtime-changeable.
                    body.options = (body.options & 0x01) | (low_u8(value) & !0x01);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()));
            }
            Ok(false)
        }
        ObjectType::PictureGraphic => {
            let mut body = obj.get_picture_graphic_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    // ISO 11783-6 Table B.41: AID 2 is the mutable Options
                    // field. Preserve the uploaded raw/RLE data-shape bit
                    // because Change Attribute is allowed to affect only
                    // transparency/flashing.
                    body.options = (body.options & 0x04) | (low_u8(value) & !0x04);
                    true
                }
                3 => {
                    body.transparency = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::GraphicData => Ok(false),
        ObjectType::ScaledGraphic => {
            let mut body = obj.get_scaled_graphic_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.scale_type = low_u8(value);
                    true
                }
                4 => {
                    body.options = low_u8(value);
                    true
                }
                5 => {
                    body.value = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::Animation => {
            let mut body = obj.get_animation_body()?;
            let changed = match attribute_id {
                1 => {
                    body.width = low_u16(value);
                    true
                }
                2 => {
                    body.height = low_u16(value);
                    true
                }
                3 => {
                    body.refresh_interval_ms = low_u16(value);
                    true
                }
                4 => {
                    body.value = low_u8(value);
                    true
                }
                5 => {
                    body.enabled = low_u8(value);
                    true
                }
                6 => {
                    body.first_child_index = low_u8(value);
                    true
                }
                7 => {
                    body.default_child_index = low_u8(value);
                    true
                }
                8 => {
                    body.last_child_index = low_u8(value);
                    true
                }
                9 => {
                    body.options = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::GraphicContext => {
            let mut body = obj.get_graphic_context_body()?;
            let changed = match attribute_id {
                1 => {
                    body.viewport_width = low_u16(value);
                    true
                }
                2 => {
                    body.viewport_height = low_u16(value);
                    true
                }
                3 => {
                    body.viewport_x = low_u16(value) as i16;
                    true
                }
                4 => {
                    body.viewport_y = low_u16(value) as i16;
                    true
                }
                7 => {
                    body.viewport_zoom_raw = value;
                    true
                }
                8 => {
                    body.cursor_x = low_u16(value) as i16;
                    true
                }
                9 => {
                    body.cursor_y = low_u16(value) as i16;
                    true
                }
                10 => {
                    body.foreground_colour = low_u8(value);
                    true
                }
                11 => {
                    body.background_colour = low_u8(value);
                    true
                }
                12 => {
                    body.font_attributes = ObjectID(low_u16(value));
                    true
                }
                13 => {
                    body.line_attributes = ObjectID(low_u16(value));
                    true
                }
                14 => {
                    body.fill_attributes = ObjectID(low_u16(value));
                    true
                }
                15 => {
                    body.format = low_u8(value);
                    true
                }
                16 => {
                    body.options = low_u8(value);
                    true
                }
                17 => {
                    body.transparency_colour = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::ColourPalette => {
            let mut body = obj.get_colour_palette_body()?;
            let changed = match attribute_id {
                1 => {
                    body.options = low_u8(value);
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        ObjectType::WorkingSetSpecialControls => {
            let mut body = obj.get_working_set_special_controls_body()?;
            let changed = match attribute_id {
                2 => {
                    body.colour_map = ObjectID(low_u16(value));
                    true
                }
                3 => {
                    body.colour_palette = ObjectID(low_u16(value));
                    true
                }
                _ => false,
            };
            if changed {
                return Ok(replace_body_if_changed(obj, body.encode()?));
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn fill_attribute_generic_change_is_valid(
    pool: &ObjectPool,
    object: &VTObject,
    attribute_id: u8,
    value: u32,
) -> bool {
    let Ok(body) = object.get_fill_attributes_body() else {
        return false;
    };
    match attribute_id {
        1 => {
            value <= 3
                && (value != 3
                    || body.fill_pattern == ObjectID::NULL
                    || fill_pattern_reference_has_valid_buffer(pool, body.fill_pattern))
        }
        3 => {
            let pattern = ObjectID(low_u16(value));
            (pattern == ObjectID::NULL
                || pool
                    .find(pattern)
                    .is_some_and(|object| object.r#type == ObjectType::PictureGraphic))
                && (body.fill_type != 3
                    || pattern == ObjectID::NULL
                    || fill_pattern_reference_has_valid_buffer(pool, pattern))
        }
        _ => true,
    }
}

fn fill_pattern_reference_has_valid_buffer(pool: &ObjectPool, reference: ObjectID) -> bool {
    pool.find(reference)
        .filter(|object| object.r#type == ObjectType::PictureGraphic)
        .and_then(|object| object.get_picture_graphic_body().ok())
        .is_some_and(|body| picture_graphic_fill_pattern_buffer_is_valid(&body))
}
