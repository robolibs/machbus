impl ObjectPool {
    pub fn set_version_label(&mut self, label: impl Into<String>) {
        self.version_label = label.into();
    }

    #[must_use]
    pub fn version_label(&self) -> &str {
        &self.version_label
    }

    pub fn add(&mut self, obj: VTObject) -> Result<()> {
        if obj.id == ObjectID::NULL {
            return Err(Error::invalid_state(
                "NULL object ID is reserved for references and cannot identify an object",
            ));
        }
        if self.objects.iter().any(|o| o.id == obj.id) {
            return Err(Error::invalid_state("duplicate object ID"));
        }
        self.objects.push(obj);
        Ok(())
    }

    #[must_use]
    pub fn find(&self, id: impl Into<ObjectID>) -> Option<&VTObject> {
        let id = id.into();
        self.objects.iter().find(|o| o.id == id)
    }

    #[must_use]
    pub fn find_mut(&mut self, id: impl Into<ObjectID>) -> Option<&mut VTObject> {
        let id = id.into();
        self.objects.iter_mut().find(|o| o.id == id)
    }

    #[must_use]
    pub fn objects(&self) -> &[VTObject] {
        &self.objects
    }

    /// Write a new value into a Number Variable object's body. Returns
    /// `false` (no mutation) if the id is absent or is not a Number
    /// Variable. Used by the macro runtime to apply Change Numeric Value.
    pub fn set_number_variable_value(&mut self, id: impl Into<ObjectID>, value: u32) -> bool {
        let id = id.into();
        match self.find_mut(id) {
            Some(obj) if obj.r#type == ObjectType::NumberVariable => {
                let body = NumberVariableBody { value }.encode();
                if obj.body == body {
                    false
                } else {
                    obj.body = body;
                    true
                }
            }
            _ => false,
        }
    }

    /// Write a new value into a String Variable object's body. Returns
    /// `false` (no mutation) if the id is absent or is not a String
    /// Variable, or if the new value would exceed the variable's fixed
    /// maximum length. Used by the macro/runtime paths to apply Change String
    /// Value without changing the object size.
    pub fn set_string_variable_value(&mut self, id: impl Into<ObjectID>, value: Vec<u8>) -> bool {
        let id = id.into();
        match self.find_mut(id) {
            Some(obj) if obj.r#type == ObjectType::StringVariable => {
                let Ok(body) = obj.get_string_variable_body() else {
                    return false;
                };
                let max_len = body.length as usize;
                if value.len() > max_len {
                    return false;
                }
                let mut value = value;
                value.resize(max_len, b' ');
                let body = StringVariableBody {
                    length: body.length,
                    value,
                }
                .encode();
                if obj.body == body {
                    false
                } else {
                    obj.body = body;
                    true
                }
            }
            _ => false,
        }
    }

    #[must_use]
    pub fn size(&self) -> usize {
        self.objects.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    /// Serialize the entire pool. Always succeeds — return type is
    /// kept as [`Result`] for symmetry with the C++ surface.
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        for obj in &self.objects {
            data.extend(obj.serialize()?);
        }
        Ok(data)
    }

    /// Deserialize a pool from raw ISO 11783-6 object-pool bytes.
    ///
    /// Objects carry no per-object length prefix; each object's extent is
    /// recovered by `object_body_total_len`, a parse-by-type walker. The
    /// same path consumes machbus-produced pools, real `.iop` files, and
    /// pools uploaded by third-party VT clients.
    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let mut pool = Self::default();
        let mut offset = 0usize;
        while offset < data.len() {
            if data.len() - offset < 3 {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "object header extends past pool data",
                ));
            }
            let id: ObjectID = u16_le_as(&data[offset..]);
            let raw_type = data[offset + 2];
            let Some(r#type) = ObjectType::try_from_u8(raw_type) else {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    format!("unknown VT object type 0x{raw_type:02X}"),
                ));
            };
            let body_off = offset + 3;
            let body_len = object_body_total_len(r#type, data, body_off)?;
            let Some(body_end) = body_off.checked_add(body_len) else {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "object body length overflows pool data",
                ));
            };
            if body_end > data.len() {
                return Err(Error::with_message(
                    ErrorCode::PoolValidation,
                    "object body extends past pool data",
                ));
            }
            let (body, children_pos, macros) =
                split_body_and_children(r#type, &data[body_off..body_end])?;
            offset = body_end;
            let children: Vec<ObjectID> = children_pos.iter().map(|r| r.id).collect();
            pool.add(VTObject {
                id,
                r#type,
                body,
                children,
                children_pos,
                macros,
            })?;
        }
        Ok(pool)
    }

    /// Validate (§4.6.8): exactly one Working Set; every WS child is a
    /// DataMask or AlarmMask; no orphan child references.
    pub fn validate(&self) -> Result<()> {
        let ws_count = self
            .objects
            .iter()
            .filter(|o| o.r#type == ObjectType::WorkingSet)
            .count();
        if ws_count == 0 {
            return Err(Error::invalid_state(
                "pool must contain exactly one Working Set object",
            ));
        }
        if ws_count > 1 {
            return Err(Error::invalid_state(format!(
                "pool must contain exactly one Working Set object, found {ws_count}"
            )));
        }
        if let Some(ws) = self
            .objects
            .iter()
            .find(|o| o.r#type == ObjectType::WorkingSet)
        {
            for &cid in &ws.children {
                if let Some(other) = self.find(cid).map(|c| c.r#type) {
                    // A Working Set's child is its *designator* — the icon/label
                    // shown in the VT's working-set selector — which is an
                    // output/drawable object (ISO 11783-6 / AgIsoStack). We also
                    // accept Data/Alarm Masks: some pools (and machbus's own
                    // fixtures) list the initial mask as a child rather than only
                    // via the `active_mask` field, and a lenient validator should
                    // not reject those.
                    let child_ok = matches!(
                        other,
                        ObjectType::OutputList
                            | ObjectType::Container
                            | ObjectType::OutputString
                            | ObjectType::OutputNumber
                            | ObjectType::Line
                            | ObjectType::Rectangle
                            | ObjectType::Ellipse
                            | ObjectType::Polygon
                            | ObjectType::Meter
                            | ObjectType::LinearBarGraph
                            | ObjectType::ArchedBarGraph
                            | ObjectType::GraphicContext
                            | ObjectType::GraphicsContext
                            | ObjectType::PictureGraphic
                            | ObjectType::ObjectPointer
                            | ObjectType::DataMask
                            | ObjectType::AlarmMask
                    );
                    if !child_ok {
                        return Err(Error::invalid_state(format!(
                            "Working Set child {cid} has type {other:?}, which is not a valid Working Set designator object"
                        )));
                    }
                }
            }
        }
        let label_ref_count = self
            .objects
            .iter()
            .filter(|o| o.r#type == ObjectType::ObjectLabelRef)
            .count();
        if label_ref_count > 1 {
            return Err(Error::invalid_state(format!(
                "pool must contain at most one Object Label Reference List object, found {label_ref_count}"
            )));
        }
        let special_controls_count = self
            .objects
            .iter()
            .filter(|o| o.r#type == ObjectType::WorkingSetSpecialControls)
            .count();
        if special_controls_count > 1 {
            return Err(Error::invalid_state(format!(
                "pool must contain at most one Working Set Special Controls object, found {special_controls_count}"
            )));
        }
        for obj in &self.objects {
            obj.validate_body()?;
            self.validate_body_references(obj)?;
            for &cid in &obj.children {
                if self.find(cid).is_none() {
                    return Err(Error::invalid_state(format!(
                        "object {} references non-existent child {cid}",
                        obj.id
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_body_references(&self, obj: &VTObject) -> Result<()> {
        match obj.r#type {
            ObjectType::WorkingSet => {
                let body = obj.get_working_set_body()?;
                for language in body.languages {
                    if !is_ascii_letter_pair(language) {
                        return Err(Error::invalid_state(format!(
                            "object {} Working Set language code must be two ASCII letters",
                            obj.id
                        )));
                    }
                }
            }
            ObjectType::DataMask => {
                let body = obj.get_data_mask_body()?;
                self.require_object_type(
                    obj.id,
                    body.soft_key_mask,
                    ObjectType::SoftKeyMask,
                    "data-mask soft-key mask",
                )?;
            }
            ObjectType::AlarmMask => {
                let body = obj.get_alarm_mask_body()?;
                self.require_object_type(
                    obj.id,
                    body.soft_key_mask,
                    ObjectType::SoftKeyMask,
                    "alarm-mask soft-key mask",
                )?;
            }
            ObjectType::WindowMask => {
                let body = obj.get_window_mask_body()?;
                if !window_mask_text_reference_is_valid(self, body.name) {
                    return Err(Error::invalid_state(format!(
                        "object {} window-mask name must reference OutputString or ObjectPointer to OutputString",
                        obj.id
                    )));
                }
                if !window_mask_text_reference_is_valid(self, body.window_title) {
                    return Err(Error::invalid_state(format!(
                        "object {} window-mask title must reference OutputString or ObjectPointer to OutputString",
                        obj.id
                    )));
                }
                if !window_mask_icon_reference_is_valid(self, body.window_icon) {
                    return Err(Error::invalid_state(format!(
                        "object {} window-mask icon must reference an Object Label graphic representation object",
                        obj.id
                    )));
                }
                self.validate_window_mask_required_objects(obj.id, &body)?;
            }
            ObjectType::SoftKeyMask => {
                if obj.children.len() > 64 {
                    return Err(Error::invalid_state(format!(
                        "SoftKeyMask {} has {} key children, maximum is 64",
                        obj.id,
                        obj.children.len()
                    )));
                }
                for &child in &obj.children {
                    self.validate_soft_key_child(obj.id, child)?;
                }
            }
            ObjectType::KeyGroup => {
                let body = obj.get_key_group_body()?;
                if !key_group_name_reference_is_valid(self, body.name) {
                    return Err(Error::invalid_state(format!(
                        "object {} key-group name must reference OutputString or ObjectPointer to OutputString",
                        obj.id
                    )));
                }
                if !key_group_icon_reference_is_valid(self, body.key_group_icon) {
                    return Err(Error::invalid_state(format!(
                        "object {} key-group icon must reference an Object Label graphic representation object",
                        obj.id
                    )));
                }
                if !(1..=4).contains(&obj.children.len()) {
                    return Err(Error::invalid_state(format!(
                        "KeyGroup {} has {} key children, expected 1..=4",
                        obj.id,
                        obj.children.len()
                    )));
                }
                for &child in &obj.children {
                    self.validate_key_group_child(obj.id, child)?;
                }
            }
            ObjectType::InputString => {
                let body = obj.get_input_string_body()?;
                self.require_object_type(
                    obj.id,
                    body.font_attributes,
                    ObjectType::FontAttributes,
                    "input-string font attributes",
                )?;
                self.require_any_object_type(
                    obj.id,
                    body.input_attributes,
                    &[
                        ObjectType::InputAttributes,
                        ObjectType::ExtendedInputAttributes,
                    ],
                    "input-string input attributes",
                )?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::StringVariable,
                    "input-string variable reference",
                )?;
            }
            ObjectType::InputNumber => {
                let body = obj.get_input_number_body()?;
                self.require_object_type(
                    obj.id,
                    body.font_attributes,
                    ObjectType::FontAttributes,
                    "input-number font attributes",
                )?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "input-number variable reference",
                )?;
            }
            ObjectType::InputBoolean => {
                let body = obj.get_input_boolean_body()?;
                self.require_object_type(
                    obj.id,
                    body.foreground,
                    ObjectType::FontAttributes,
                    "input-boolean foreground",
                )?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "input-boolean variable reference",
                )?;
            }
            ObjectType::InputList => {
                let body = obj.get_input_list_body()?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "input-list variable reference",
                )?;
                for &item in &body.items {
                    if item != ObjectID::NULL {
                        self.require_existing_object(obj.id, item, "input-list item")?;
                    }
                }
            }
            ObjectType::OutputString => {
                let body = obj.get_output_string_body()?;
                self.require_object_type(
                    obj.id,
                    body.font_attributes,
                    ObjectType::FontAttributes,
                    "output-string font attributes",
                )?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::StringVariable,
                    "output-string variable reference",
                )?;
            }
            ObjectType::OutputNumber => {
                let body = obj.get_output_number_body()?;
                self.require_object_type(
                    obj.id,
                    body.font_attributes,
                    ObjectType::FontAttributes,
                    "output-number font attributes",
                )?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "output-number variable reference",
                )?;
            }
            ObjectType::OutputList => {
                let body = obj.get_output_list_body()?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "output-list variable reference",
                )?;
                for &item in &body.items {
                    if !output_list_item_reference_is_valid(self, item) {
                        return Err(Error::invalid_state(format!(
                            "object {} output-list item must reference a displayable object or standard no-display placeholder",
                            obj.id
                        )));
                    }
                }
            }
            ObjectType::Meter => {
                let body = obj.get_meter_body()?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "meter variable reference",
                )?;
            }
            ObjectType::LinearBarGraph => {
                let body = obj.get_linear_bar_graph_body()?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "linear-bar-graph variable reference",
                )?;
                self.require_object_type(
                    obj.id,
                    body.target_value_variable_reference,
                    ObjectType::NumberVariable,
                    "linear-bar-graph target-value variable reference",
                )?;
            }
            ObjectType::ArchedBarGraph => {
                let body = obj.get_arched_bar_graph_body()?;
                self.require_object_type(
                    obj.id,
                    body.variable_reference,
                    ObjectType::NumberVariable,
                    "arched-bar-graph variable reference",
                )?;
                self.require_object_type(
                    obj.id,
                    body.target_value_variable_reference,
                    ObjectType::NumberVariable,
                    "arched-bar-graph target-value variable reference",
                )?;
            }
            ObjectType::FillAttributes => {
                let body = obj.get_fill_attributes_body()?;
                self.require_object_type(
                    obj.id,
                    body.fill_pattern,
                    ObjectType::PictureGraphic,
                    "fill-attributes pattern",
                )?;
                self.validate_fill_pattern_buffer(obj.id, &body)?;
            }
            ObjectType::ObjectPointer => {
                let body = obj.get_object_pointer_body()?;
                self.require_existing_object(obj.id, body.value, "object-pointer value")?;
            }
            ObjectType::AuxFunction => {
                let body = obj.get_aux_function_body()?;
                self.require_object_type(
                    obj.id,
                    body.designator,
                    ObjectType::AuxControlDesig,
                    "aux-function designator",
                )?;
            }
            ObjectType::AuxFunction2 => {
                let body = obj.get_aux_function2_body()?;
                self.require_existing_object(obj.id, body.name, "aux-function2 name")?;
                self.require_existing_object(obj.id, body.icon, "aux-function2 icon")?;
            }
            ObjectType::AuxInput2 => {
                let body = obj.get_aux_input2_body()?;
                self.require_existing_object(obj.id, body.name, "aux-input2 name")?;
            }
            ObjectType::AuxControlDesig => {
                let body = obj.get_aux_control_designator_body()?;
                self.require_any_object_type(
                    obj.id,
                    body.aux_object,
                    &[
                        ObjectType::AuxFunction,
                        ObjectType::AuxInput,
                        ObjectType::AuxFunction2,
                        ObjectType::AuxInput2,
                    ],
                    "aux-control-designator object",
                )?;
            }
            ObjectType::ScaledGraphic => {
                let body = obj.get_scaled_graphic_body()?;
                if !scaled_graphic_value_source_is_valid(self, body.value) {
                    return Err(Error::with_message(
                        ErrorCode::PoolValidation,
                        format!(
                            "object {} scaled-graphic value reference must resolve to GraphicData, PictureGraphic, ObjectPointer, or NULL",
                            obj.id.raw()
                        ),
                    ));
                }
            }
            ObjectType::Animation => {
                let body = obj.get_animation_body()?;
                validate_animation_child_indices(obj.id, &body, obj.children_pos.len())?;
            }
            ObjectType::GraphicContext => {
                let body = obj.get_graphic_context_body()?;
                self.require_object_type(
                    obj.id,
                    body.font_attributes,
                    ObjectType::FontAttributes,
                    "graphic-context font attributes",
                )?;
                self.require_object_type(
                    obj.id,
                    body.line_attributes,
                    ObjectType::LineAttributes,
                    "graphic-context line attributes",
                )?;
                self.require_object_type(
                    obj.id,
                    body.fill_attributes,
                    ObjectType::FillAttributes,
                    "graphic-context fill attributes",
                )?;
            }
            ObjectType::WorkingSetSpecialControls => {
                let body = obj.get_working_set_special_controls_body()?;
                self.require_object_type(
                    obj.id,
                    body.colour_map,
                    ObjectType::ColourMap,
                    "working-set-special-controls colour map",
                )?;
                self.require_object_type(
                    obj.id,
                    body.colour_palette,
                    ObjectType::ColourPalette,
                    "working-set-special-controls colour palette",
                )?;
                for pair in body.languages {
                    if !is_ascii_letter_pair(pair.language) {
                        return Err(Error::invalid_state(format!(
                            "object {} Working Set Special Controls language code must be two ASCII letters",
                            obj.id
                        )));
                    }
                    if !is_ascii_letter_pair(pair.country)
                        && !is_not_applicable_country_pair(pair.country)
                    {
                        return Err(Error::invalid_state(format!(
                            "object {} Working Set Special Controls country code must be two ASCII letters or two spaces when not applicable",
                            obj.id
                        )));
                    }
                }
            }
            ObjectType::ExternalObjectDefinition => {
                let body = obj.get_external_object_definition_body()?;
                for referenced in body.object_ids {
                    self.require_existing_object(
                        obj.id,
                        referenced,
                        "external-object-definition listed object",
                    )?;
                }
            }
            ObjectType::ExternalObjectPointer => {
                let body = obj.get_external_object_pointer_body()?;
                self.require_existing_object(
                    obj.id,
                    body.default_object_id,
                    "external-object-pointer default object",
                )?;
                self.require_object_type(
                    obj.id,
                    body.external_reference_name,
                    ObjectType::ExternalReferenceName,
                    "external-object-pointer reference NAME",
                )?;
            }
            ObjectType::ObjectLabelRef => {
                let body = obj.get_object_label_ref_body()?;
                let mut labelled = Vec::with_capacity(body.labels.len());
                for label in body.labels {
                    if label.labelled_object == ObjectID::NULL {
                        return Err(Error::invalid_state(format!(
                            "object {} object-label entry targets NULL object id",
                            obj.id
                        )));
                    }
                    if labelled.contains(&label.labelled_object) {
                        return Err(Error::invalid_state(format!(
                            "object {} contains duplicate label for object {}",
                            obj.id, label.labelled_object
                        )));
                    }
                    labelled.push(label.labelled_object);
                    self.require_existing_object(
                        obj.id,
                        label.labelled_object,
                        "object-label target",
                    )?;
                    self.require_object_type(
                        obj.id,
                        label.string_variable,
                        ObjectType::StringVariable,
                        "object-label string variable",
                    )?;
                    if label.string_variable != ObjectID::NULL
                        && !is_standard_font_type(label.font_type)
                    {
                        return Err(Error::invalid_state(format!(
                            "object {} object-label entry uses reserved font type {}",
                            obj.id, label.font_type
                        )));
                    }
                    self.require_object_label_graphic_designator(
                        obj.id,
                        label.graphic_designator,
                        "object-label graphic designator",
                    )?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn validate_window_mask_required_objects(
        &self,
        owner: ObjectID,
        body: &WindowMaskBody,
    ) -> Result<()> {
        let Some(expected) = window_mask_required_object_types(body.window_type) else {
            return Ok(());
        };

        if body.window_type == 0 && !body.required_objects.is_empty() {
            return Err(Error::invalid_state(format!(
                "WindowMask {owner} is free-form but has {} required objects",
                body.required_objects.len()
            )));
        }
        if body.window_type == 0 {
            return Ok(());
        }

        if body.required_objects.len() != expected.len() {
            return Err(Error::invalid_state(format!(
                "WindowMask {owner} type {} has {} required objects, expected {}",
                body.window_type,
                body.required_objects.len(),
                expected.len()
            )));
        }

        for (index, (&reference, &expected_type)) in body
            .required_objects
            .iter()
            .zip(expected.iter())
            .enumerate()
        {
            if reference == ObjectID::NULL {
                return Err(Error::invalid_state(format!(
                    "WindowMask {owner} required object slot {index} is NULL"
                )));
            }
            let role = format!("window-mask required object slot {index}");
            self.require_object_type(owner, reference, expected_type, &role)?;
        }

        Ok(())
    }

    fn require_object_type(
        &self,
        owner: ObjectID,
        reference: ObjectID,
        expected: ObjectType,
        role: &str,
    ) -> Result<()> {
        if reference == ObjectID::NULL {
            return Ok(());
        }
        let Some(target) = self.find(reference) else {
            return Err(Error::invalid_state(format!(
                "object {owner} {role} references missing object {reference}"
            )));
        };
        if target.r#type != expected {
            return Err(Error::invalid_state(format!(
                "object {owner} {role} references object {reference} of type {:?}, expected {expected:?}",
                target.r#type
            )));
        }
        Ok(())
    }

    fn validate_fill_pattern_buffer(
        &self,
        owner: ObjectID,
        body: &FillAttributesBody,
    ) -> Result<()> {
        if body.fill_type != 3 || body.fill_pattern == ObjectID::NULL {
            return Ok(());
        }
        let Some(pattern) = self.find(body.fill_pattern) else {
            return Ok(());
        };
        if pattern.r#type != ObjectType::PictureGraphic {
            return Ok(());
        }
        let picture = pattern.get_picture_graphic_body()?;
        if picture_graphic_fill_pattern_buffer_is_valid(&picture) {
            return Ok(());
        }
        Err(Error::invalid_state(format!(
            "object {owner} fill-attributes pattern {} has unused row bits for bitmap format {} and raw width {}",
            body.fill_pattern, picture.format, picture.actual_width
        )))
    }

    fn validate_soft_key_child(&self, owner: ObjectID, reference: ObjectID) -> Result<()> {
        let Some(child) = self.find(reference) else {
            return Err(Error::invalid_state(format!(
                "object {owner} soft-key child references missing object {reference}"
            )));
        };
        match child.r#type {
            ObjectType::Key => Ok(()),
            ObjectType::ObjectPointer => {
                let body = child.get_object_pointer_body()?;
                if body.value == ObjectID::NULL {
                    return Ok(());
                }
                self.require_object_type(
                    owner,
                    body.value,
                    ObjectType::Key,
                    "soft-key pointer target",
                )
            }
            ObjectType::ExternalObjectPointer => {
                let body = child.get_external_object_pointer_body()?;
                if body.default_object_id == ObjectID::NULL {
                    return Ok(());
                }
                self.require_object_type(
                    owner,
                    body.default_object_id,
                    ObjectType::Key,
                    "soft-key external-pointer default object",
                )
            }
            other => Err(Error::invalid_state(format!(
                "object {owner} soft-key child references object {reference} of type {other:?}, expected Key, ObjectPointer, or ExternalObjectPointer"
            ))),
        }
    }

    fn validate_key_group_child(&self, owner: ObjectID, reference: ObjectID) -> Result<()> {
        let Some(child) = self.find(reference) else {
            return Err(Error::invalid_state(format!(
                "object {owner} key-group child references missing object {reference}"
            )));
        };
        match child.r#type {
            ObjectType::Key => Ok(()),
            ObjectType::ObjectPointer => {
                let body = child.get_object_pointer_body()?;
                if body.value == ObjectID::NULL {
                    return Err(Error::invalid_state(format!(
                        "object {owner} key-group pointer child {reference} targets NULL"
                    )));
                }
                self.require_object_type(
                    owner,
                    body.value,
                    ObjectType::Key,
                    "key-group pointer target",
                )
            }
            ObjectType::ExternalObjectPointer => {
                let body = child.get_external_object_pointer_body()?;
                if body.default_object_id == ObjectID::NULL {
                    return Ok(());
                }
                self.require_object_type(
                    owner,
                    body.default_object_id,
                    ObjectType::Key,
                    "key-group external-pointer default object",
                )
            }
            other => Err(Error::invalid_state(format!(
                "object {owner} key-group child references object {reference} of type {other:?}, expected Key, ObjectPointer to Key, or ExternalObjectPointer"
            ))),
        }
    }

    fn require_existing_object(
        &self,
        owner: ObjectID,
        reference: ObjectID,
        role: &str,
    ) -> Result<()> {
        if reference == ObjectID::NULL || self.find(reference).is_some() {
            return Ok(());
        }
        Err(Error::invalid_state(format!(
            "object {owner} {role} references missing object {reference}"
        )))
    }

    fn require_any_object_type(
        &self,
        owner: ObjectID,
        reference: ObjectID,
        expected: &[ObjectType],
        role: &str,
    ) -> Result<()> {
        if reference == ObjectID::NULL {
            return Ok(());
        }
        let Some(target) = self.find(reference) else {
            return Err(Error::invalid_state(format!(
                "object {owner} {role} references missing object {reference}"
            )));
        };
        if expected.contains(&target.r#type) {
            return Ok(());
        }
        Err(Error::invalid_state(format!(
            "object {owner} {role} references object {reference} of type {:?}",
            target.r#type
        )))
    }

    fn require_object_label_graphic_designator(
        &self,
        owner: ObjectID,
        reference: ObjectID,
        role: &str,
    ) -> Result<()> {
        if reference == ObjectID::NULL {
            return Ok(());
        }
        let Some(target) = self.find(reference) else {
            return Err(Error::invalid_state(format!(
                "object {owner} {role} references missing object {reference}"
            )));
        };
        if is_object_label_graphic_representation_type(target.r#type) {
            return Ok(());
        }
        Err(Error::invalid_state(format!(
            "object {owner} {role} references object {reference} of type {:?}, expected Object Label graphic representation",
            target.r#type
        )))
    }

    #[must_use]
    pub fn with_object(mut self, obj: VTObject) -> Self {
        let _ = self.add(obj);
        self
    }

    #[must_use]
    pub fn with_version_label(mut self, label: impl Into<String>) -> Self {
        self.set_version_label(label);
        self
    }
}

fn validate_animation_child_indices(
    owner: ObjectID,
    body: &AnimationBody,
    child_count: usize,
) -> Result<()> {
    if child_count == 0 {
        return Ok(());
    }
    if body.value != u8::MAX && usize::from(body.value) >= child_count {
        return Err(Error::invalid_state(format!(
            "object {owner} Animation value index {} exceeds child count {child_count}",
            body.value
        )));
    }
    if usize::from(body.default_child_index) >= child_count {
        return Err(Error::invalid_state(format!(
            "object {owner} Animation default child index {} exceeds child count {child_count}",
            body.default_child_index
        )));
    }
    if usize::from(body.first_child_index) > usize::from(body.last_child_index) {
        return Err(Error::invalid_state(format!(
            "object {owner} Animation first child index {} exceeds last child index {}",
            body.first_child_index, body.last_child_index
        )));
    }
    if usize::from(body.last_child_index) >= child_count {
        return Err(Error::invalid_state(format!(
            "object {owner} Animation last child index {} exceeds child count {child_count}",
            body.last_child_index
        )));
    }
    Ok(())
}

// ─── Builder helpers ──────────────────────────────────────────────────

pub fn create_window_mask(id: impl Into<ObjectID>, body: &WindowMaskBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::WindowMask)
        .with_window_mask_body(body)
}

#[must_use]
pub fn create_key_group(id: impl Into<ObjectID>, body: &KeyGroupBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::KeyGroup)
        .with_key_group_body(body)
}

#[must_use]
pub fn create_key(id: impl Into<ObjectID>, body: &KeyBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Key)
        .with_key_body(body)
}

#[must_use]
pub fn create_macro(id: impl Into<ObjectID>, body: &MacroBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Macro)
        .with_macro_body(body)
}

pub fn create_alarm_mask(id: impl Into<ObjectID>, body: &AlarmMaskBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::AlarmMask)
        .with_alarm_mask_body(body)
}

#[must_use]
pub fn create_data_mask(id: impl Into<ObjectID>, body: &DataMaskBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::DataMask)
        .with_data_mask_body(body)
}

#[must_use]
pub fn create_container(id: impl Into<ObjectID>, body: &ContainerBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Container)
        .with_container_body(body)
}

#[must_use]
pub fn create_soft_key_mask(id: impl Into<ObjectID>, body: &SoftKeyMaskBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::SoftKeyMask)
        .with_soft_key_mask_body(body)
}

#[must_use]
pub fn create_button(id: impl Into<ObjectID>, body: &ButtonBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Button)
        .with_button_body(body)
}

#[must_use]
pub fn create_number_variable(id: impl Into<ObjectID>, body: &NumberVariableBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::NumberVariable)
        .with_number_variable_body(body)
}

#[must_use]
pub fn create_string_variable(id: impl Into<ObjectID>, body: &StringVariableBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::StringVariable)
        .with_string_variable_body(body)
}

#[must_use]
pub fn create_font_attributes(id: impl Into<ObjectID>, body: &FontAttributesBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::FontAttributes)
        .with_font_attributes_body(body)
}

#[must_use]
pub fn create_line_attributes(id: impl Into<ObjectID>, body: &LineAttributesBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::LineAttributes)
        .with_line_attributes_body(body)
}

pub fn create_fill_attributes(
    id: impl Into<ObjectID>,
    body: &FillAttributesBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::FillAttributes)
        .with_fill_attributes_body(body)
}

pub fn create_input_attributes(
    id: impl Into<ObjectID>,
    body: &InputAttributesBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::InputAttributes)
        .with_input_attributes_body(body)
}

pub fn create_extended_input_attributes(
    id: impl Into<ObjectID>,
    body: &ExtendedInputAttributesBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ExtendedInputAttributes)
        .with_extended_input_attributes_body(body)
}

#[must_use]
pub fn create_object_pointer(id: impl Into<ObjectID>, body: &ObjectPointerBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ObjectPointer)
        .with_object_pointer_body(body)
}

pub fn create_output_line(id: impl Into<ObjectID>, body: &OutputLineBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Line)
        .with_output_line_body(body)
}

pub fn create_output_rectangle(
    id: impl Into<ObjectID>,
    body: &OutputRectangleBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Rectangle)
        .with_output_rectangle_body(body)
}

pub fn create_output_ellipse(
    id: impl Into<ObjectID>,
    body: &OutputEllipseBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Ellipse)
        .with_output_ellipse_body(body)
}

pub fn create_output_polygon(
    id: impl Into<ObjectID>,
    body: &OutputPolygonBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Polygon)
        .with_output_polygon_body(body)
}

pub fn create_input_boolean(id: impl Into<ObjectID>, body: &InputBooleanBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::InputBoolean)
        .with_input_boolean_body(body)
}

pub fn create_input_string(id: impl Into<ObjectID>, body: &InputStringBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::InputString)
        .with_input_string_body(body)
}

pub fn create_input_number(id: impl Into<ObjectID>, body: &InputNumberBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::InputNumber)
        .with_input_number_body(body)
}

pub fn create_input_list(id: impl Into<ObjectID>, body: &InputListBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::InputList)
        .with_input_list_body(body)
}

pub fn create_output_list(id: impl Into<ObjectID>, body: &OutputListBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::OutputList)
        .with_output_list_body(body)
}

pub fn create_output_string(id: impl Into<ObjectID>, body: &OutputStringBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::OutputString)
        .with_output_string_body(body)
}

pub fn create_output_number(id: impl Into<ObjectID>, body: &OutputNumberBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::OutputNumber)
        .with_output_number_body(body)
}

pub fn create_meter(id: impl Into<ObjectID>, body: &MeterBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Meter)
        .with_meter_body(body)
}

pub fn create_linear_bar_graph(
    id: impl Into<ObjectID>,
    body: &LinearBarGraphBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::LinearBarGraph)
        .with_linear_bar_graph_body(body)
}

pub fn create_arched_bar_graph(
    id: impl Into<ObjectID>,
    body: &ArchedBarGraphBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ArchedBarGraph)
        .with_arched_bar_graph_body(body)
}

pub fn create_picture_graphic(
    id: impl Into<ObjectID>,
    body: &PictureGraphicBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::PictureGraphic)
        .with_picture_graphic_body(body)
}

#[must_use]
pub fn create_working_set(id: impl Into<ObjectID>, body: &WorkingSetBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::WorkingSet)
        .with_working_set_body(body)
}

pub fn create_aux_function(id: impl Into<ObjectID>, body: &AuxFunctionBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::AuxFunction)
        .with_aux_function_body(body)
}

pub fn create_aux_input(id: impl Into<ObjectID>, body: &AuxInputBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::AuxInput)
        .with_aux_input_body(body)
}

pub fn create_aux_function2(id: impl Into<ObjectID>, body: &AuxFunction2Body) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::AuxFunction2)
        .with_aux_function2_body(body)
}

pub fn create_aux_input2(id: impl Into<ObjectID>, body: &AuxInput2Body) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::AuxInput2)
        .with_aux_input2_body(body)
}

pub fn create_aux_control_designator(
    id: impl Into<ObjectID>,
    body: &AuxControlDesignatorBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::AuxControlDesig)
        .with_aux_control_designator_body(body)
}

pub fn create_graphic_data(id: impl Into<ObjectID>, body: &GraphicDataBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::GraphicData)
        .with_graphic_data_body(body)
}

pub fn create_scaled_graphic(
    id: impl Into<ObjectID>,
    body: &ScaledGraphicBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ScaledGraphic)
        .with_scaled_graphic_body(body)
}

pub fn create_scaled_bitmap(id: impl Into<ObjectID>, body: &ScaledBitmapBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ScaledBitmap)
        .with_scaled_bitmap_body(body)
}

pub fn create_animation(id: impl Into<ObjectID>, body: &AnimationBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::Animation)
        .with_animation_body(body)
}

pub fn create_colour_map(id: impl Into<ObjectID>, body: &ColourMapBody) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ColourMap)
        .with_colour_map_body(body)
}

pub fn create_graphic_context(
    id: impl Into<ObjectID>,
    body: &GraphicContextBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::GraphicContext)
        .with_graphic_context_body(body)
}

pub fn create_external_object_definition(
    id: impl Into<ObjectID>,
    body: &ExternalObjectDefinitionBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ExternalObjectDefinition)
        .with_external_object_definition_body(body)
}

#[must_use]
pub fn create_external_reference_name(
    id: impl Into<ObjectID>,
    body: &ExternalReferenceNameBody,
) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ExternalReferenceName)
        .with_external_reference_name_body(body)
}

#[must_use]
pub fn create_external_object_pointer(
    id: impl Into<ObjectID>,
    body: &ExternalObjectPointerBody,
) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ExternalObjectPointer)
        .with_external_object_pointer_body(body)
}

pub fn create_colour_palette(
    id: impl Into<ObjectID>,
    body: &ColourPaletteBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ColourPalette)
        .with_colour_palette_body(body)
}

pub fn create_working_set_special_controls(
    id: impl Into<ObjectID>,
    body: &WorkingSetSpecialControlsBody,
) -> Result<VTObject> {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::WorkingSetSpecialControls)
        .with_working_set_special_controls_body(body)
}

#[must_use]
pub fn create_graphics_context(id: impl Into<ObjectID>, body: &GraphicsContextBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::GraphicsContext)
        .with_graphics_context_body(body)
}

#[must_use]
pub fn create_object_label_ref(id: impl Into<ObjectID>, body: &ObjectLabelRefBody) -> VTObject {
    VTObject::default()
        .with_id(id)
        .with_type(ObjectType::ObjectLabelRef)
        .with_object_label_ref_body(body)
}

// ═══════════════════════════════════════════════════════════════════════
// VT Version 6 (ISO 11783-6 Edition 4, 2018)
// ═══════════════════════════════════════════════════════════════════════

// ─── Touch gestures ───────────────────────────────────────────────────

/// VT6 gesture type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum GestureType {
    #[default]
    None = 0,
    Tap = 1,
    DoubleTap = 2,
    LongPress = 3,
    SwipeLeft = 4,
    SwipeRight = 5,
    SwipeUp = 6,
    SwipeDown = 7,
    /// Zoom out.
    PinchIn = 8,
    /// Zoom in.
    PinchOut = 9,
    Rotate = 10,
    TwoFingerTap = 11,
    ThreeFingerTap = 12,
}

impl GestureType {
    #[must_use]
    pub const fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Tap,
            2 => Self::DoubleTap,
            3 => Self::LongPress,
            4 => Self::SwipeLeft,
            5 => Self::SwipeRight,
            6 => Self::SwipeUp,
            7 => Self::SwipeDown,
            8 => Self::PinchIn,
            9 => Self::PinchOut,
            10 => Self::Rotate,
            11 => Self::TwoFingerTap,
            12 => Self::ThreeFingerTap,
            _ => Self::None,
        }
    }

    #[must_use]
    pub const fn try_from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Self::None),
            1 => Some(Self::Tap),
            2 => Some(Self::DoubleTap),
            3 => Some(Self::LongPress),
            4 => Some(Self::SwipeLeft),
            5 => Some(Self::SwipeRight),
            6 => Some(Self::SwipeUp),
            7 => Some(Self::SwipeDown),
            8 => Some(Self::PinchIn),
            9 => Some(Self::PinchOut),
            10 => Some(Self::Rotate),
            11 => Some(Self::TwoFingerTap),
            12 => Some(Self::ThreeFingerTap),
            _ => None,
        }
    }

    #[inline]
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// VT6 touch gesture event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchGesture {
    pub r#type: GestureType,
    pub x: i16,
    pub y: i16,
    pub duration_ms: u16,
    pub distance: i16,
    pub scale: f32,
    pub rotation_deg: f32,
    pub touch_count: u8,
    pub target_object: ObjectID,
}

impl Default for TouchGesture {
    fn default() -> Self {
        Self {
            r#type: GestureType::None,
            x: 0,
            y: 0,
            duration_ms: 0,
            distance: 0,
            scale: 1.0,
            rotation_deg: 0.0,
            touch_count: 1,
            target_object: ObjectID::NULL,
        }
    }
}

impl TouchGesture {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(12);
        data.push(self.r#type.as_u8());
        push_u16_le(&mut data, self.x as u16);
        push_u16_le(&mut data, self.y as u16);
        push_u16_le(&mut data, self.duration_ms);
        push_u16_le(&mut data, self.distance as u16);
        data.push(self.touch_count);
        push_u16_le(&mut data, self.target_object);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 12 {
            return None;
        }
        Some(Self {
            r#type: GestureType::try_from_u8(data[0])?,
            x: u16_le(&data[1..]) as i16,
            y: u16_le(&data[3..]) as i16,
            duration_ms: u16_le(&data[5..]),
            distance: u16_le(&data[7..]) as i16,
            touch_count: data[9],
            target_object: u16_le_as(&data[10..]),
            ..Default::default()
        })
    }
}

// ─── Graphics context (VT6 24-bit) ────────────────────────────────────

/// VT6 extended graphics context.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphicsContextV6 {
    /// 0 = fully transparent, 255 = opaque.
    pub transparency: u8,
    /// 0 = solid, 1 = dashed, 2 = dotted.
    pub line_style: u8,
    pub line_width: u16,
    /// 24-bit RGB.
    pub fill_color_rgb: u32,
    /// 24-bit RGB.
    pub line_color_rgb: u32,
    pub anti_aliasing: bool,
    /// 0 = normal, 1 = multiply, 2 = screen, 3 = overlay.
    pub blend_mode: u8,
}

impl Default for GraphicsContextV6 {
    fn default() -> Self {
        Self {
            transparency: 0xFF,
            line_style: 0,
            line_width: 1,
            fill_color_rgb: 0,
            line_color_rgb: 0,
            anti_aliasing: false,
            blend_mode: 0,
        }
    }
}

impl GraphicsContextV6 {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(12);
        data.push(self.transparency);
        data.push(self.line_style);
        push_u16_le(&mut data, self.line_width);
        push_u24_le(&mut data, self.fill_color_rgb);
        push_u24_le(&mut data, self.line_color_rgb);
        data.push(u8::from(self.anti_aliasing));
        data.push(self.blend_mode);
        data
    }

    #[must_use]
    pub fn decode(data: &[u8]) -> Option<Self> {
        if data.len() != 12 || data[1] > 2 || data[10] > 1 || data[11] > 3 {
            return None;
        }
        Some(Self {
            transparency: data[0],
            line_style: data[1],
            line_width: u16_le(&data[2..]),
            fill_color_rgb: (data[4] as u32) | ((data[5] as u32) << 8) | ((data[6] as u32) << 16),
            line_color_rgb: (data[7] as u32) | ((data[8] as u32) << 8) | ((data[9] as u32) << 16),
            anti_aliasing: data[10] != 0,
            blend_mode: data[11],
        })
    }
}

// ─── External object definition/reference/pointer (VT5+) ──────────────

/// External Object Definition body (Type 41, VT version 5+).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExternalObjectDefinitionBody {
    /// bit 0 = enabled.
    pub options: u8,
    /// NAME bytes 1-4 of the WS Master allowed to reference these objects.
    pub name0: u32,
    /// NAME bytes 5-8 of the WS Master allowed to reference these objects.
    pub name1: u32,
    pub object_ids: Vec<ObjectID>,
}

impl ExternalObjectDefinitionBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.object_ids.len() > u8::MAX as usize {
            return Err(Error::invalid_data(
                "External Object Definition object count exceeds u8 count field",
            ));
        }
        let mut data = Vec::with_capacity(10 + self.object_ids.len() * 2);
        data.push(self.options);
        data.extend_from_slice(&self.name0.to_le_bytes());
        data.extend_from_slice(&self.name1.to_le_bytes());
        data.push(self.object_ids.len() as u8);
        for id in &self.object_ids {
            push_u16_le(&mut data, *id);
        }
        Ok(data)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 10 {
            return Err(Error::invalid_data(
                "External Object Definition body too short",
            ));
        }
        let count = data[9] as usize;
        if data.len() != 10 + count * 2 {
            return Err(Error::invalid_data(
                "External Object Definition object list length does not match body length",
            ));
        }
        let mut object_ids = Vec::with_capacity(count);
        let mut cursor = 10;
        for _ in 0..count {
            object_ids.push(u16_le_as(&data[cursor..]));
            cursor += 2;
        }
        Ok(Self {
            options: data[0],
            name0: u32::from_le_bytes([data[1], data[2], data[3], data[4]]),
            name1: u32::from_le_bytes([data[5], data[6], data[7], data[8]]),
            object_ids,
        })
    }
}

/// External Reference NAME body (Type 42, VT version 5+).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ExternalReferenceNameBody {
    /// bit 0 = enabled.
    pub options: u8,
    /// NAME bytes 1-4 of the referenced WS Master.
    pub name0: u32,
    /// NAME bytes 5-8 of the referenced WS Master.
    pub name1: u32,
}

impl ExternalReferenceNameBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(9);
        data.push(self.options);
        data.extend_from_slice(&self.name0.to_le_bytes());
        data.extend_from_slice(&self.name1.to_le_bytes());
        data
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 9 {
            return Err(Error::invalid_data(
                "External Reference NAME body too short",
            ));
        }
        Ok(Self {
            options: data[0],
            name0: u32::from_le_bytes([data[1], data[2], data[3], data[4]]),
            name1: u32::from_le_bytes([data[5], data[6], data[7], data[8]]),
        })
    }
}

/// External Object Pointer body (Type 43, VT version 5+).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalObjectPointerBody {
    pub default_object_id: ObjectID,
    pub external_reference_name: ObjectID,
    pub external_object_id: ObjectID,
}

impl Default for ExternalObjectPointerBody {
    fn default() -> Self {
        Self {
            default_object_id: ObjectID::NULL,
            external_reference_name: ObjectID::NULL,
            external_object_id: ObjectID::NULL,
        }
    }
}

impl ExternalObjectPointerBody {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(6);
        push_u16_le(&mut data, self.default_object_id);
        push_u16_le(&mut data, self.external_reference_name);
        push_u16_le(&mut data, self.external_object_id);
        data
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 6 {
            return Err(Error::invalid_data(
                "External Object Pointer body too short",
            ));
        }
        Ok(Self {
            default_object_id: u16_le_as(&data[0..]),
            external_reference_name: u16_le_as(&data[2..]),
            external_object_id: u16_le_as(&data[4..]),
        })
    }
}

/// Colour Palette object body (Type 45, VT6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ColourPaletteBody {
    /// Reserved options byte from ISO 11783-6 Table B.73. Must be zero.
    pub options: u8,
    /// Colour entries encoded as `0xAARRGGBB`.
    pub entries_argb: Vec<u32>,
}

impl ColourPaletteBody {
    pub fn encode(&self) -> Result<Vec<u8>> {
        if self.options != 0 {
            return Err(Error::invalid_data(
                "Colour Palette options contain reserved bits",
            ));
        }
        if self.entries_argb.len() > 256 {
            return Err(Error::invalid_data(
                "Colour Palette entry count exceeds standard 256-entry maximum",
            ));
        }
        let mut data = Vec::with_capacity(3 + self.entries_argb.len() * 4);
        data.push(self.options);
        push_u16_le(&mut data, self.entries_argb.len() as u16);
        for argb in &self.entries_argb {
            data.extend_from_slice(&argb.to_le_bytes());
        }
        Ok(data)
    }

    pub fn decode(data: &[u8]) -> Result<Self> {
        if data.len() < 3 {
            return Err(Error::invalid_data("Colour Palette body too short"));
        }
        if data[0] != 0 {
            return Err(Error::invalid_data(
                "Colour Palette body contains reserved option bits",
            ));
        }
        let count = u16_le(&data[1..]) as usize;
        if count > 256 {
            return Err(Error::invalid_data(
                "Colour Palette entry count exceeds standard 256-entry maximum",
            ));
        }
        let bytes = count
            .checked_mul(4)
            .ok_or_else(|| Error::invalid_data("Colour Palette length overflows"))?;
        if data.len() != 3 + bytes {
            return Err(Error::invalid_data(
                "Colour Palette entry count does not match body length",
            ));
        }
        let mut entries_argb = Vec::with_capacity(count);
        let mut offset = 3;
        for _ in 0..count {
            entries_argb.push(u32_le(&data[offset..]));
            offset += 4;
        }
        Ok(Self {
            options: data[0],
            entries_argb,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageCountryPair {
    pub language: [u8; 2],
    pub country: [u8; 2],
}

/// Working Set Special Controls body (Type 47, VT version 6+).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkingSetSpecialControlsBody {
    pub colour_map: ObjectID,
    pub colour_palette: ObjectID,
    pub languages: Vec<LanguageCountryPair>,
    pub extra_bytes: Vec<u8>,
}
