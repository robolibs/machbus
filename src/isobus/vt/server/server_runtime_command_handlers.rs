impl VTServer {
    fn handle_hide_show(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data)
            || !is_canonical_bool(msg.data[3])
            || !has_ff_tail(&msg.data, 4)
        {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let object_type = self.client_pool_object_type(msg.source, id);
        if object_type != Some(ObjectType::Container) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            let visible = msg.data[3] != 0;
            state.visibility.insert(id, visible);
            // ISO 11783-6 F.2 says Hide/Show updates both visibility and the
            // remembered hidden state of a Container. Keep the retained
            // attribute cache in the same shape returned by Get Attribute
            // Value: AID 3 is TRUE when hidden.
            state.attributes.insert((id, 3), u32::from(!visible));
            state
                .accepted_effects
                .push(ServerRenderEffect::HideShow { id, visible });
        }
    }

    fn handle_enable_disable(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data)
            || !is_canonical_bool(msg.data[3])
            || !has_ff_tail(&msg.data, 4)
        {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let Some(object_type) = self.client_pool_object_type(msg.source, id) else {
            return;
        };
        if !is_enable_disable_object_type(object_type) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            let enabled = msg.data[3] != 0;
            state.enable_state.insert(id, enabled);
            state
                .accepted_effects
                .push(ServerRenderEffect::EnableDisable { id, enabled });
        }
    }

    fn handle_select_input_object_command(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 4) {
            return Vec::new();
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let option = msg.data[3];
        let mut error_bits = 0u8;
        let mut response_code = 0u8;
        let mut accepted = false;
        let mut open_for_input = false;

        if id == ObjectID::NULL {
            if option != 0xFF {
                error_bits |= SELECT_INPUT_ERROR_INVALID_OPTION;
            } else if self
                .find_client(msg.source)
                .is_some_and(|client| client.pool_activated)
            {
                accepted = true;
            } else {
                error_bits |= SELECT_INPUT_ERROR_INVALID_OBJECT_ID;
            }
        } else if !matches!(option, 0x00 | 0xFF) {
            error_bits |= SELECT_INPUT_ERROR_INVALID_OPTION;
        } else {
            open_for_input = option == 0x00;
            match self.client_select_input_target_state(msg.source, id) {
                None => error_bits |= SELECT_INPUT_ERROR_INVALID_OBJECT_ID,
                Some((object_type, enabled, visible_on_active_mask)) => {
                    if open_for_input && !is_select_input_open_target_type(object_type) {
                        error_bits |= SELECT_INPUT_ERROR_INVALID_OPTION;
                    } else if !enabled {
                        error_bits |= SELECT_INPUT_ERROR_DISABLED;
                    } else if !visible_on_active_mask {
                        error_bits |= SELECT_INPUT_ERROR_NOT_ON_ACTIVE_OR_HIDDEN;
                    } else if self.client_select_input_is_busy(msg.source, id) {
                        error_bits |= SELECT_INPUT_ERROR_COULD_NOT_COMPLETE;
                    } else {
                        response_code = if open_for_input { 2 } else { 1 };
                        accepted = true;
                    }
                }
            }
        }

        if accepted && let Some(state) = self.client_object_state_mut(msg.source) {
            if id == ObjectID::NULL {
                state.selected_input_object = ObjectID::NULL;
                state.open_input_object = ObjectID::NULL;
            } else {
                state.selected_input_object = id;
                state.open_input_object = if open_for_input { id } else { ObjectID::NULL };
            }
            state
                .accepted_effects
                .push(ServerRenderEffect::SelectInputObject { id, open_for_input });
        }

        vec![OutboundFrame::to(
            build_select_input_object_response(id, response_code, error_bits).to_vec(),
            msg.source,
        )]
    }

    fn handle_control_audio_signal(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) {
            return;
        }
        let audio = AudioSignalState {
            activations: msg.data[1],
            frequency_hz: u16_le(&msg.data[2..]),
            duration_ms: u16_le(&msg.data[4..]),
            off_time_ms: u16_le(&msg.data[6..]),
        };
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.audio_signal = Some(audio);
            state.accepted_effects.push(ServerRenderEffect::AudioSignal);
        }
    }

    fn handle_set_audio_volume(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || msg.data[1] > 100 || !has_ff_tail(&msg.data, 2) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            let percent = msg.data[1];
            state.audio_volume_percent = Some(percent);
            state
                .accepted_effects
                .push(ServerRenderEffect::SetAudioVolume { percent });
        }
    }

    fn handle_change_child_location(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 7) {
            return;
        }
        let parent = ObjectID(u16_le(&msg.data[1..]));
        let child = ObjectID(u16_le(&msg.data[3..]));
        if !self.client_pool_object_is_child_of(msg.source, parent, child) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state
                .child_locations
                .insert((parent, child), (msg.data[5], msg.data[6]));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeChildLocation {
                    parent,
                    child,
                    x: msg.data[5],
                    y: msg.data[6],
                });
        }
    }

    fn handle_change_size(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 7) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let Some(object_type) = self.client_pool_object_type(msg.source, id) else {
            return;
        };
        if !change_size_target_is_valid(object_type) {
            return;
        }
        let width = u16_le(&msg.data[3..]);
        let height = u16_le(&msg.data[5..]);
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.sizes.insert(id, (width, height));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeSize { id, width, height });
        }
    }

    fn handle_change_background_colour(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 4) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let Some(object_type) = self.client_pool_object_type(msg.source, id) else {
            return;
        };
        if !change_background_colour_target_is_valid(object_type) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            let colour = msg.data[3];
            state.background_colours.insert(id, colour);
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeBackgroundColour { id, colour });
        }
    }

    fn handle_change_end_point(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let width = u16_le(&msg.data[3..]);
        let height = u16_le(&msg.data[5..]);
        let line_direction = msg.data[7];
        if line_direction > 1 || !self.client_pool_has_object_type(msg.source, id, ObjectType::Line)
        {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.endpoints.insert(id, (width, height, line_direction));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeEndPoint {
                    id,
                    width,
                    height,
                    line_direction,
                });
        }
    }

    fn handle_change_font_attributes(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 7) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let colour = msg.data[3];
        let size = msg.data[4];
        let font_type = msg.data[5];
        let style = msg.data[6];
        if !is_standard_font_size_for_style(size, style)
            || !is_standard_font_type(font_type)
            || !self.client_pool_has_object_type(msg.source, id, ObjectType::FontAttributes)
        {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.attributes.insert((id, 1), u32::from(colour));
            state.attributes.insert((id, 2), u32::from(size));
            state.attributes.insert((id, 3), u32::from(font_type));
            state.attributes.insert((id, 4), u32::from(style));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeFontAttributeValues {
                    id,
                    colour,
                    size,
                    font_type,
                    style,
                });
        }
    }

    fn handle_change_line_attributes(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 7) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let colour = msg.data[3];
        let width = msg.data[4];
        let line_art = u16_le(&msg.data[5..]);
        if !self.client_pool_has_object_type(msg.source, id, ObjectType::LineAttributes) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.attributes.insert((id, 1), u32::from(colour));
            state.attributes.insert((id, 2), u32::from(width));
            state.attributes.insert((id, 3), u32::from(line_art));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeLineAttributeValues {
                    id,
                    colour,
                    width,
                    line_art,
                });
        }
    }

    fn handle_change_fill_attributes(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 7) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let fill_type = msg.data[3];
        let colour = msg.data[4];
        let pattern = ObjectID(u16_le(&msg.data[5..]));
        if fill_type > 3
            || !self.client_pool_has_object_type(msg.source, id, ObjectType::FillAttributes)
        {
            return;
        }
        if pattern != ObjectID::NULL
            && !self.client_pool_has_object_type(msg.source, pattern, ObjectType::PictureGraphic)
        {
            return;
        }
        if fill_type == 3
            && pattern != ObjectID::NULL
            && !self.client_pool_fill_pattern_buffer_is_valid(msg.source, pattern)
        {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.attributes.insert((id, 1), u32::from(fill_type));
            state.attributes.insert((id, 2), u32::from(colour));
            state.attributes.insert((id, 3), u32::from(pattern.raw()));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeFillAttributeValues {
                    id,
                    fill_type,
                    colour,
                    pattern,
                });
        }
    }

    fn handle_change_active_mask(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 5) {
            return;
        }
        let working_set = ObjectID(u16_le(&msg.data[1..]));
        if !self.client_pool_has_object_type(msg.source, working_set, ObjectType::WorkingSet) {
            return;
        }
        let mask = ObjectID(u16_le(&msg.data[3..]));
        if !self.client_pool_has_any_object_type(
            msg.source,
            mask,
            &[ObjectType::DataMask, ObjectType::AlarmMask],
        ) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.active_data_mask = mask;
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeActiveMask { mask });
        }
    }

    fn handle_change_soft_key_mask(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 6) {
            return;
        }
        let mask_type = msg.data[1];
        let data_mask = ObjectID(u16_le(&msg.data[2..]));
        let soft_key_mask = ObjectID(u16_le(&msg.data[4..]));
        let Some(object_type) = self.client_pool_object_type(msg.source, data_mask) else {
            return;
        };
        if !change_soft_key_mask_type_matches(mask_type, object_type) {
            return;
        }
        if soft_key_mask != ObjectID::NULL
            && !self.client_pool_has_object_type(msg.source, soft_key_mask, ObjectType::SoftKeyMask)
        {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.soft_key_masks.insert(data_mask, soft_key_mask);
            state.active_soft_key_mask = soft_key_mask;
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeSoftKeyMask {
                    data_mask,
                    soft_key_mask,
                });
        }
    }

    fn handle_change_attribute(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let attribute_id = msg.data[3];
        let value = u32_le(&msg.data[4..]);
        let Some(object_type) = self.client_pool_object_type(msg.source, id) else {
            return;
        };
        if !self.client_pool_change_attribute_is_valid(msg.source, id, attribute_id, value) {
            return;
        }
        let value = self
            .client_pool_retained_change_attribute_value(msg.source, id, attribute_id, value)
            .unwrap_or(value);
        if let Some(state) = self.client_object_state_mut(msg.source) {
            if matches!(
                (object_type, attribute_id),
                (ObjectType::WorkingSetSpecialControls, 2 | 3)
            ) {
                state.attributes.remove(&(id, attribute_id));
            } else {
                state.attributes.insert((id, attribute_id), value);
            }
            if object_type == ObjectType::Container && attribute_id == 3 {
                state.visibility.insert(id, value == 0);
            }
            if matches!(
                (object_type, attribute_id),
                (ObjectType::DataMask | ObjectType::AlarmMask, 2)
            ) {
                let soft_key_mask = ObjectID(value as u16);
                state.soft_key_masks.insert(id, soft_key_mask);
                state.active_soft_key_mask = soft_key_mask;
            }
            if object_type == ObjectType::WorkingSetSpecialControls {
                match attribute_id {
                    2 => state.selected_colour_map = ObjectID(value as u16),
                    3 => state.selected_colour_palette = Some(ObjectID(value as u16)),
                    _ => {}
                }
            }
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeGenericAttribute {
                    id,
                    attribute_id,
                    value,
                });
        }
    }

    fn handle_get_attribute_value(&self, msg: &Message) -> Vec<OutboundFrame> {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 4) {
            return Vec::new();
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let attribute_id = msg.data[3];
        let Some(client) = self
            .find_client(msg.source)
            .filter(|client| client.pool_activated)
        else {
            return Vec::new();
        };

        let mut response = [0xFFu8; 8];
        response[0] = cmd::GET_ATTRIBUTE_VALUE;
        response[3] = attribute_id;

        let error_bits = match client.pool.find(id) {
            None => Some(0x01),
            Some(object) => match vt_get_attribute_value(
                &client.pool,
                &client.object_state,
                object,
                attribute_id,
            ) {
                Ok(Some(value)) => {
                    response[1..3].copy_from_slice(&id.to_le_bytes());
                    response[4..8].copy_from_slice(&value.to_le_bytes());
                    None
                }
                Ok(None) => Some(0x02),
                Err(_) => Some(0x10),
            },
        };

        if let Some(error_bits) = error_bits {
            response[1..3].copy_from_slice(&ObjectID::NULL.to_le_bytes());
            response[4..6].copy_from_slice(&id.to_le_bytes());
            response[6] = error_bits;
            response[7] = 0xFF;
        }

        vec![OutboundFrame::to(response.to_vec(), msg.source)]
    }

    fn handle_change_priority(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 4) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let priority = msg.data[3];
        if priority > 2 || !self.client_pool_has_object_type(msg.source, id, ObjectType::AlarmMask)
        {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.priorities.insert(id, priority);
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangePriority { id, priority });
        }
    }

    fn handle_change_list_item(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 6) {
            return;
        }
        let list = ObjectID(u16_le(&msg.data[1..]));
        let index = msg.data[3];
        let item = ObjectID(u16_le(&msg.data[4..]));
        if !self.client_pool_has_any_object_type(
            msg.source,
            list,
            &[
                ObjectType::InputList,
                ObjectType::OutputList,
                ObjectType::Animation,
                ObjectType::ExternalObjectDefinition,
            ],
        ) || !self.client_pool_list_index_is_valid(msg.source, list, index)
            || !self.client_pool_list_item_reference_is_valid(msg.source, list, item)
        {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.list_items.insert((list, index), item);
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeListItem { list, index, item });
        }
    }

    fn handle_delete_object_pool(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 1) {
            return;
        }
        if let Some(client) = self.find_client_mut(msg.source) {
            client.pool.clear();
            client.pool_uploaded = false;
            client.pool_upload_allowed = false;
            client.pool_activated = false;
            client.object_state = ServerObjectState::default();
        }
        if self.active_working_set == msg.source {
            self.set_active_working_set(NULL_ADDRESS);
        }
    }

    fn handle_change_child_position(&mut self, msg: &Message) {
        if msg.data.len() != 9 {
            return;
        }
        let parent = ObjectID(u16_le(&msg.data[1..]));
        let child = ObjectID(u16_le(&msg.data[3..]));
        let x = u16_le(&msg.data[5..]);
        let y = u16_le(&msg.data[7..]);
        if !self.client_pool_object_is_child_of(msg.source, parent, child) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.child_positions.insert((parent, child), (x, y));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeChildPosition {
                    parent,
                    child,
                    x,
                    y,
                });
        }
    }

    fn handle_change_object_label(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let label_string = ObjectID(u16_le(&msg.data[3..]));
        let font_type = msg.data[5];
        let graphic_designator = ObjectID(u16_le(&msg.data[6..]));
        if !self.client_pool_has_object_label_target(msg.source, id) {
            return;
        }
        if label_string != ObjectID::NULL
            && !self.client_pool_has_object_type(
                msg.source,
                label_string,
                ObjectType::StringVariable,
            )
        {
            return;
        }
        if !is_standard_font_type(font_type) {
            return;
        }
        if !self.client_pool_has_object_label_graphic_designator(msg.source, graphic_designator) {
            return;
        }
        let label = ObjectLabelState {
            string_variable: label_string,
            font_type,
            graphic_designator,
        };
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.object_labels.insert(id, label);
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangeObjectLabel { id, label });
        }
    }

    fn handle_change_polygon_point(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let index = msg.data[3];
        let x = u16_le(&msg.data[4..]);
        let y = u16_le(&msg.data[6..]);
        if !self.client_pool_polygon_point_index_is_valid(msg.source, id, index) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.polygon_points.insert((id, index), (x, y));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangePolygonPoint { id, index, x, y });
        }
    }

    fn handle_change_polygon_scale(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 7) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let width = u16_le(&msg.data[3..]);
        let height = u16_le(&msg.data[5..]);
        if !self.client_pool_has_object_type(msg.source, id, ObjectType::Polygon) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.polygon_scales.insert(id, (width, height));
            state
                .accepted_effects
                .push(ServerRenderEffect::ChangePolygonScale { id, width, height });
        }
    }

    fn handle_graphics_context(&mut self, msg: &Message) -> Vec<OutboundFrame> {
        if msg.data.len() < 4 {
            return Vec::new();
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let subcommand = msg.data[3];
        let mut error_bits = 0u8;

        let valid_object = self.client_pool_has_any_object_type(
            msg.source,
            id,
            &[ObjectType::GraphicContext, ObjectType::GraphicsContext],
        );
        if !valid_object {
            error_bits |= GRAPHICS_CONTEXT_ERROR_INVALID_OBJECT_ID;
        }

        let payload = if !graphics_context_subcommand_is_supported(subcommand) {
            error_bits |= GRAPHICS_CONTEXT_ERROR_INVALID_SUBCOMMAND_ID;
            None
        } else {
            match graphics_context_payload_without_padding(subcommand, &msg.data[4..]) {
                Some(payload) if graphics_context_payload_is_canonical(subcommand, payload) => {
                    Some(payload.to_vec())
                }
                _ => {
                    error_bits |= GRAPHICS_CONTEXT_ERROR_INVALID_PARAMETER;
                    None
                }
            }
        };

        if error_bits == 0
            && let Some(payload) = payload
        {
            if self.client_pool_graphics_context_reference_is_valid(
                msg.source,
                id,
                subcommand,
                &payload,
            ) {
                if let Some(state) = self.client_object_state_mut(msg.source) {
                    state.graphics_contexts.push(GraphicsContextCommand {
                        object_id: id,
                        subcommand,
                        payload: payload.clone(),
                    });
                    state
                        .accepted_effects
                        .push(ServerRenderEffect::GraphicsContext {
                            id,
                            subcommand,
                            payload,
                    });
                }
            } else {
                error_bits |= GRAPHICS_CONTEXT_ERROR_INVALID_RESULTS;
            }
        }

        let mut response = [0xFFu8; 8];
        response[0] = cmd::GRAPHICS_CONTEXT;
        response[1..3].copy_from_slice(&id.to_le_bytes());
        response[3] = subcommand;
        response[4] = error_bits;
        vec![OutboundFrame::to(
            response.to_vec(),
            msg.source,
        )]
    }

    fn handle_select_colour_map(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 3) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        let object_type = if id == ObjectID::NULL {
            None
        } else {
            let Some(object_type) = self.client_pool_object_type(msg.source, id) else {
                return;
            };
            if !matches!(
                object_type,
                ObjectType::ColourMap | ObjectType::ColourPalette
            ) {
                return;
            }
            Some(object_type)
        };
        let special_controls_id = self
            .find_client(msg.source)
            .filter(|client| client.pool_activated)
            .and_then(|client| {
                client
                    .pool
                    .objects()
                    .iter()
                    .find(|object| object.r#type == ObjectType::WorkingSetSpecialControls)
                    .map(|object| object.id)
            });
        if let Some(state) = self.client_object_state_mut(msg.source) {
            match object_type {
                Some(ObjectType::ColourMap) => state.selected_colour_map = id,
                Some(ObjectType::ColourPalette) => state.selected_colour_palette = Some(id),
                None => {
                    state.selected_colour_map = ObjectID::NULL;
                    state.selected_colour_palette = Some(ObjectID::NULL);
                }
                _ => unreachable!("validated colour-selection object type"),
            }
            if let Some(special_controls_id) = special_controls_id {
                state.attributes.remove(&(special_controls_id, 2));
                state.attributes.remove(&(special_controls_id, 3));
            }
            state
                .accepted_effects
                .push(ServerRenderEffect::SelectColourMap { id });
        }
    }

    fn handle_lock_unlock_mask(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || msg.data[1] > 1 || !has_ff_tail(&msg.data, 6) {
            return;
        }
        let locked = msg.data[1] == 1;
        let id = ObjectID(u16_le(&msg.data[2..]));
        let timeout_ms = u16_le(&msg.data[4..]);
        if !self.client_pool_has_any_object_type(
            msg.source,
            id,
            &[ObjectType::DataMask, ObjectType::WindowMask],
        ) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state
                .mask_locks
                .insert(id, MaskLockState { locked, timeout_ms });
            state
                .accepted_effects
                .push(ServerRenderEffect::LockUnlockMask {
                    id,
                    locked,
                    timeout_ms,
                });
        }
    }

    fn handle_execute_macro(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 3) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        if !self.client_pool_has_object_type(msg.source, id, ObjectType::Macro) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.executed_macros.push(id);
            state
                .accepted_effects
                .push(ServerRenderEffect::ExecuteMacro {
                    id,
                    extended: false,
                });
        }
    }

    fn handle_execute_extended_macro(&mut self, msg: &Message) {
        if !is_fixed_vt_payload(&msg.data) || !has_ff_tail(&msg.data, 3) {
            return;
        }
        let id = ObjectID(u16_le(&msg.data[1..]));
        if !self.client_pool_has_object_type(msg.source, id, ObjectType::Macro) {
            return;
        }
        if let Some(state) = self.client_object_state_mut(msg.source) {
            state.executed_extended_macros.push(id);
            state
                .accepted_effects
                .push(ServerRenderEffect::ExecuteMacro { id, extended: true });
        }
    }

    // ─── Client tracking ──────────────────────────────────────────────

}
