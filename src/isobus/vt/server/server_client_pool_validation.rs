impl VTServer {
    fn ensure_client(&mut self, addr: Address) {
        if !self.clients.iter().any(|c| c.client_address == addr) {
            self.clients.push(ServerWorkingSet {
                client_address: addr,
                ..Default::default()
            });
        }
    }

    fn find_client(&self, addr: Address) -> Option<&ServerWorkingSet> {
        self.clients.iter().find(|c| c.client_address == addr)
    }

    fn find_client_mut(&mut self, addr: Address) -> Option<&mut ServerWorkingSet> {
        self.clients.iter_mut().find(|c| c.client_address == addr)
    }

    fn client_object_state_mut(&mut self, addr: Address) -> Option<&mut ServerObjectState> {
        if !valid_vt_peer_address(addr) {
            return None;
        }
        self.find_client_mut(addr)
            .filter(|c| c.pool_activated)
            .map(|c| &mut c.object_state)
    }

    fn activated_client_object_state_mut(
        &mut self,
        addr: Address,
    ) -> Option<&mut ServerObjectState> {
        self.client_object_state_mut(addr)
    }

    fn client_pool_has_object_type(
        &self,
        addr: Address,
        id: ObjectID,
        expected: ObjectType,
    ) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .is_some_and(|object| object.r#type == expected)
    }

    fn client_pool_fill_pattern_buffer_is_valid(&self, addr: Address, id: ObjectID) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .filter(|object| object.r#type == ObjectType::PictureGraphic)
            .and_then(|object| object.get_picture_graphic_body().ok())
            .is_some_and(|body| picture_graphic_fill_pattern_buffer_is_valid(&body))
    }

    fn client_pool_object_type(&self, addr: Address, id: ObjectID) -> Option<ObjectType> {
        if !valid_vt_peer_address(addr) {
            return None;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .map(|object| object.r#type)
    }

    fn client_pool_change_attribute_is_valid(
        &self,
        addr: Address,
        id: ObjectID,
        attribute_id: u8,
        value: u32,
    ) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| {
                client
                    .pool
                    .find(id)
                    .map(|object| (&client.pool, &client.object_state, object))
            })
            .is_some_and(|(pool, state, object)| {
                vt_change_attribute_id_is_supported(object.r#type, attribute_id)
                    && vt_change_attribute_value_is_valid(pool, state, object, attribute_id, value)
            })
    }

    fn client_pool_retained_change_attribute_value(
        &self,
        addr: Address,
        id: ObjectID,
        attribute_id: u8,
        value: u32,
    ) -> Option<u32> {
        if !valid_vt_peer_address(addr) {
            return None;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .and_then(|object| vt_retained_change_attribute_value(object, attribute_id, value))
    }

    fn client_pool_has_any_object_type(
        &self,
        addr: Address,
        id: ObjectID,
        expected: &[ObjectType],
    ) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .is_some_and(|object| expected.contains(&object.r#type))
    }

    fn client_pool_list_index_is_valid(&self, addr: Address, id: ObjectID, index: u8) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .is_some_and(|object| match object.r#type {
                ObjectType::InputList => object
                    .get_input_list_body()
                    .is_ok_and(|body| usize::from(index) < body.items.len()),
                ObjectType::OutputList => object
                    .get_output_list_body()
                    .is_ok_and(|body| usize::from(index) < body.items.len()),
                ObjectType::ExternalObjectDefinition => object
                    .get_external_object_definition_body()
                    .is_ok_and(|body| usize::from(index) < body.object_ids.len()),
                ObjectType::Animation => usize::from(index) < object.children_pos.len(),
                _ => false,
            })
    }

    fn client_pool_list_item_reference_is_valid(
        &self,
        addr: Address,
        list: ObjectID,
        item: ObjectID,
    ) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(list).map(|object| (client, object)))
            .is_some_and(|(client, object)| match object.r#type {
                ObjectType::OutputList => output_list_item_reference_is_valid(&client.pool, item),
                ObjectType::InputList
                | ObjectType::ExternalObjectDefinition
                | ObjectType::Animation => {
                    item == ObjectID::NULL || client.pool.find(item).is_some()
                }
                _ => false,
            })
    }

    fn client_pool_polygon_point_index_is_valid(
        &self,
        addr: Address,
        id: ObjectID,
        index: u8,
    ) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .filter(|object| object.r#type == ObjectType::Polygon)
            .is_some_and(|object| {
                object
                    .get_output_polygon_body()
                    .is_ok_and(|body| usize::from(index) < body.points.len())
            })
    }

    fn client_pool_graphics_context_reference_is_valid(
        &self,
        addr: Address,
        id: ObjectID,
        subcommand: u8,
        payload: &[u8],
    ) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .is_some_and(|client| {
                graphics_context_reference_is_valid(&client.pool, id, subcommand, payload)
            })
    }

    fn client_pool_object_is_child_of(
        &self,
        addr: Address,
        parent: ObjectID,
        child: ObjectID,
    ) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| {
                client
                    .pool
                    .find(parent)
                    .filter(|parent_obj| {
                        parent_obj.children.contains(&child)
                            || parent_obj
                                .children_pos
                                .iter()
                                .any(|child_ref| child_ref.id == child)
                    })
                    .and_then(|_| client.pool.find(child))
            })
            .is_some()
    }

    fn client_pool_has_object_label_target(&self, addr: Address, id: ObjectID) -> bool {
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .is_some_and(|client| {
                client
                    .pool
                    .objects()
                    .iter()
                    .filter(|object| object.r#type == ObjectType::ObjectLabelRef)
                    .any(|object| {
                        object.get_object_label_ref_body().is_ok_and(|body| {
                            body.labels.iter().any(|label| label.labelled_object == id)
                        })
                    })
            })
    }

    fn client_pool_has_object_label_graphic_designator(&self, addr: Address, id: ObjectID) -> bool {
        if id == ObjectID::NULL {
            return true;
        }
        if !valid_vt_peer_address(addr) {
            return false;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| client.pool.find(id))
            .is_some_and(|object| is_object_label_graphic_representation_type(object.r#type))
    }

    fn client_select_input_target_state(
        &self,
        addr: Address,
        id: ObjectID,
    ) -> Option<(ObjectType, bool, bool)> {
        if !valid_vt_peer_address(addr) {
            return None;
        }
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .and_then(|client| {
                let object = client.pool.find(id)?;
                if !is_select_input_object_type(object.r#type, self.vt_version) {
                    return None;
                }
                Some((
                    object.r#type,
                    select_input_object_effective_enabled(object, &client.object_state),
                    select_input_object_visible_on_active_mask(
                        &client.pool,
                        &client.object_state,
                        id,
                    ),
                ))
            })
    }

    fn client_select_input_is_busy(&self, addr: Address, id: ObjectID) -> bool {
        self.find_client(addr)
            .filter(|client| client.pool_activated)
            .is_some_and(|client| {
                client.object_state.open_input_object != ObjectID::NULL
                    && client.object_state.open_input_object != id
            })
    }

    fn validate_aux_assignment(
        &self,
        client_addr: Address,
        input_object: ObjectID,
        function_object: ObjectID,
    ) -> Result<()> {
        let client = self
            .find_client(client_addr)
            .filter(|client| client.pool_activated)
            .ok_or_else(|| {
                Error::invalid_state("VT AUX assignment requires an active object pool")
            })?;
        let input = client
            .pool
            .find(input_object)
            .ok_or_else(|| Error::invalid_state("VT AUX assignment input object is missing"))?;
        let function = client
            .pool
            .find(function_object)
            .ok_or_else(|| Error::invalid_state("VT AUX assignment function object is missing"))?;

        match (input.r#type, function.r#type) {
            (ObjectType::AuxInput, ObjectType::AuxFunction) => {
                let input_body = input.get_aux_input_body()?;
                let function_body = function.get_aux_function_body()?;
                if input_body.input_type != function_body.function_type {
                    return Err(Error::invalid_state(
                        "VT AUX-O assignment requires matching input/function types",
                    ));
                }
                Ok(())
            }
            (ObjectType::AuxInput2, ObjectType::AuxFunction2) => {
                let input_body = input.get_aux_input2_body()?;
                let function_body = function.get_aux_function2_body()?;
                if input_body.input_type != function_body.function_type {
                    return Err(Error::invalid_state(
                        "VT AUX-N assignment requires matching input/function types",
                    ));
                }
                Ok(())
            }
            _ => Err(Error::invalid_state(
                "VT AUX assignment requires matching AUX input/function object families",
            )),
        }
    }

    fn find_aux_input_object(
        &self,
        client_addr: Address,
        style: AuxRuntimeStyle,
        function_number: u8,
        status_type: AuxFunctionType,
    ) -> Result<Option<(ObjectID, ObjectID)>> {
        let Some(client) = self
            .find_client(client_addr)
            .filter(|client| client.pool_activated)
        else {
            return Err(Error::invalid_state(
                "VT AUX status requires an active object pool",
            ));
        };

        for object in client.pool.objects() {
            match (style, object.r#type) {
                (AuxRuntimeStyle::AuxO, ObjectType::AuxInput) => {
                    let body = object.get_aux_input_body()?;
                    if body.input_id != function_number {
                        continue;
                    }
                    if body.input_type != status_type.as_u8() {
                        return Err(Error::invalid_data(
                            "AUX-O status type does not match the uploaded input object",
                        ));
                    }
                }
                (AuxRuntimeStyle::AuxN, ObjectType::AuxInput2) => {
                    let body = object.get_aux_input2_body()?;
                    if body.input_id != function_number {
                        continue;
                    }
                    if body.input_type != status_type.as_u8() {
                        return Err(Error::invalid_data(
                            "AUX-N status type does not match the uploaded input object",
                        ));
                    }
                }
                _ => continue,
            }

            if let Some(function) = client.object_state.aux_assignments.get(&object.id) {
                return Ok(Some((object.id, *function)));
            }
            return Ok(None);
        }

        Ok(None)
    }

    fn transition(&mut self, new_state: VTServerState) {
        if self.state() == new_state {
            return;
        }
        self.state.transition(new_state);
        self.on_state_change.emit(&new_state);
    }
}
