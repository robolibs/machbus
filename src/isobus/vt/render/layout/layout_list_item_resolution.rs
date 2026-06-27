impl LayoutEngine {
    fn selected_output_list_item(
        &self,
        pool: &ObjectPool,
        body: &OutputListBody,
        selected: usize,
    ) -> Option<ObjectID> {
        if selected == usize::from(u8::MAX) {
            return None;
        }
        let item = *body.items.get(selected)?;
        self.output_list_item_can_materialise(pool, item, &mut Vec::new())
            .then_some(item)
    }

    fn output_list_item_can_materialise(
        &self,
        pool: &ObjectPool,
        item: ObjectID,
        path: &mut Vec<ObjectID>,
    ) -> bool {
        if item == ObjectID::NULL || path.contains(&item) {
            return false;
        }
        let Some(obj) = pool.find(item) else {
            return false;
        };
        path.push(item);
        let can_materialise = match obj.r#type {
            ObjectType::ObjectPointer => obj.get_object_pointer_body().is_ok_and(|body| {
                self.output_list_item_can_materialise(pool, body.value, path)
            }),
            ObjectType::ExternalObjectPointer => {
                obj.get_external_object_pointer_body().is_ok_and(|body| {
                    if let Some((external_pool, target)) = self.resolve_external_object(pool, &body)
                    {
                        return self.output_list_item_can_materialise(
                            external_pool,
                            target,
                            &mut Vec::new(),
                        );
                    }
                    self.output_list_item_can_materialise(pool, body.default_object_id, path)
                })
            }
            ObjectType::Container => obj
                .get_container_body()
                .map(|body| !body.hidden)
                .unwrap_or(false),
            ObjectType::DataMask
            | ObjectType::AlarmMask
            | ObjectType::WindowMask
            | ObjectType::Button
            | ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
            | ObjectType::OutputString
            | ObjectType::OutputNumber
            | ObjectType::OutputList
            | ObjectType::Line
            | ObjectType::Rectangle
            | ObjectType::Ellipse
            | ObjectType::Polygon
            | ObjectType::Meter
            | ObjectType::LinearBarGraph
            | ObjectType::ArchedBarGraph
            | ObjectType::PictureGraphic
            | ObjectType::ScaledGraphic
            | ObjectType::ScaledBitmap
            | ObjectType::GraphicContext
            | ObjectType::Key
            | ObjectType::KeyGroup => true,
            _ => false,
        };
        path.pop();
        can_materialise
    }

    fn input_list_item_is_operator_selectable(
        &self,
        pool: &ObjectPool,
        item: ObjectID,
        path: &mut Vec<ObjectID>,
    ) -> bool {
        if item == ObjectID::NULL || path.contains(&item) {
            return false;
        }
        let Some(obj) = pool.find(item) else {
            return false;
        };
        path.push(item);
        let selectable = match obj.r#type {
            ObjectType::ObjectPointer => obj.get_object_pointer_body().is_ok_and(|body| {
                body.value == ObjectID::NULL
                    || self.input_list_item_is_operator_selectable(pool, body.value, path)
            }),
            ObjectType::ExternalObjectPointer => {
                obj.get_external_object_pointer_body().is_ok_and(|body| {
                    if let Some((external_pool, target)) = self.resolve_external_object(pool, &body)
                    {
                        return self.input_list_item_is_operator_selectable(
                            external_pool,
                            target,
                            &mut Vec::new(),
                        );
                    }
                    self.input_list_item_is_operator_selectable(pool, body.default_object_id, path)
                })
            }
            ObjectType::Container => true,
            ObjectType::DataMask
            | ObjectType::AlarmMask
            | ObjectType::WindowMask
            | ObjectType::Button
            | ObjectType::InputBoolean
            | ObjectType::InputString
            | ObjectType::InputNumber
            | ObjectType::InputList
            | ObjectType::OutputString
            | ObjectType::OutputNumber
            | ObjectType::OutputList
            | ObjectType::StringVariable
            | ObjectType::NumberVariable
            | ObjectType::Line
            | ObjectType::Rectangle
            | ObjectType::Ellipse
            | ObjectType::Polygon
            | ObjectType::Meter
            | ObjectType::LinearBarGraph
            | ObjectType::ArchedBarGraph
            | ObjectType::PictureGraphic
            | ObjectType::ScaledGraphic
            | ObjectType::ScaledBitmap
            | ObjectType::GraphicContext
            | ObjectType::Key
            | ObjectType::KeyGroup
            | ObjectType::Animation => true,
            _ => false,
        };
        path.pop();
        selectable
    }

    fn input_list_item_can_materialise_display_value(
        &self,
        pool: &ObjectPool,
        item: ObjectID,
        path: &mut Vec<ObjectID>,
    ) -> bool {
        if item == ObjectID::NULL || path.contains(&item) {
            return false;
        }
        let Some(obj) = pool.find(item) else {
            return false;
        };
        path.push(item);
        let can_materialise = match obj.r#type {
            ObjectType::ObjectPointer => obj.get_object_pointer_body().is_ok_and(|body| {
                self.input_list_item_can_materialise_display_value(pool, body.value, path)
            }),
            ObjectType::ExternalObjectPointer => {
                obj.get_external_object_pointer_body().is_ok_and(|body| {
                    if let Some((external_pool, target)) = self.resolve_external_object(pool, &body)
                    {
                        return self.input_list_item_can_materialise_display_value(
                            external_pool,
                            target,
                            &mut Vec::new(),
                        );
                    }
                    self.input_list_item_can_materialise_display_value(
                        pool,
                        body.default_object_id,
                        path,
                    )
                })
            }
            ObjectType::OutputString
            | ObjectType::OutputNumber
            | ObjectType::Line
            | ObjectType::Rectangle
            | ObjectType::Ellipse
            | ObjectType::Polygon
            | ObjectType::Meter
            | ObjectType::LinearBarGraph
            | ObjectType::ArchedBarGraph
            | ObjectType::PictureGraphic
            | ObjectType::ScaledGraphic
            | ObjectType::GraphicContext
            | ObjectType::Key => true,
            _ => false,
        };
        path.pop();
        can_materialise
    }

    fn resolve_output_list_item_text(&self, pool: &ObjectPool, item: ObjectID) -> Option<String> {
        self.resolve_output_list_item_text_inner(pool, item, &mut Vec::new())
    }

    fn resolve_output_list_item_text_inner(
        &self,
        pool: &ObjectPool,
        item: ObjectID,
        path: &mut Vec<ObjectID>,
    ) -> Option<String> {
        if item == ObjectID::NULL {
            return Some(String::new());
        }
        if path.contains(&item) {
            return Some(String::new());
        }
        let obj = pool.find(item)?;
        path.push(item);
        let resolved = match obj.r#type {
            ObjectType::OutputString => obj
                .get_output_string_body()
                .ok()
                .map(|body| self.resolve_string_value(pool, &body)),
            ObjectType::OutputNumber => obj.get_output_number_body().ok().map(|body| {
                let raw = self.resolve_number_value_or(pool, body.variable_reference, body.value);
                format_number(
                    raw,
                    body.offset,
                    body.scale,
                    body.number_of_decimals,
                    body.format,
                    body.options,
                    None,
                )
            }),
            ObjectType::StringVariable => Some(self.resolve_string_variable(pool, item)),
            ObjectType::NumberVariable => Some(self.resolve_number_value(pool, item).to_string()),
            ObjectType::ObjectPointer => match obj.get_object_pointer_body().ok() {
                Some(body) if body.value == ObjectID::NULL => Some(String::new()),
                Some(body) => {
                    let resolved = self.resolve_output_list_item_text_inner(pool, body.value, path);
                    path.pop();
                    return resolved;
                }
                None => None,
            },
            ObjectType::ExternalObjectPointer => match obj.get_external_object_pointer_body().ok() {
                Some(body) => {
                    let resolved = if let Some((external_pool, target)) =
                        self.resolve_external_object(pool, &body)
                    {
                        self.resolve_output_list_item_text_inner(
                            external_pool,
                            target,
                            &mut Vec::new(),
                        )
                    } else if body.default_object_id == ObjectID::NULL {
                        Some(String::new())
                    } else {
                        self.resolve_output_list_item_text_inner(pool, body.default_object_id, path)
                    };
                    path.pop();
                    return resolved;
                }
                None => None,
            },
            ObjectType::Container => match obj.get_container_body().ok() {
                Some(body) if body.hidden => Some(String::new()),
                Some(_) => {
                    for child in &obj.children {
                        if let Some(text) =
                            self.resolve_output_list_item_text_inner(pool, *child, path)
                            && !text.is_empty()
                        {
                            path.pop();
                            return Some(text);
                        }
                    }
                    None
                }
                None => None,
            },
            _ => None,
        };
        path.pop();
        resolved
    }
}
